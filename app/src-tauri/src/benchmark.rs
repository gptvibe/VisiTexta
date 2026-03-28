use crate::defaults::DEFAULT_PROMPT_TEXT;
use crate::events::{
    CompletedEvent, ErrorEvent, PreviewEvent, ProgressEvent, RunnerEvent, RunnerStage,
};
use crate::models;
use crate::pipeline::{self, PipelineObserver};
use crate::runtime;
use crate::settings::Settings;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use uuid::Uuid;

const DEFAULT_OUTPUT_SUBDIR: &str = "out";
const DEFAULT_FIXTURE_SUBDIR: &str = "fixtures";
const LATEST_REPORT_FILE: &str = "latest.json";
const MEMORY_SAMPLE_INTERVAL_MS: u64 = 75;

#[derive(Debug, Default)]
struct CliOptions {
    manifest_path: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    filter: Option<String>,
    bless: bool,
    list_only: bool,
}

#[derive(Debug, Deserialize)]
struct FixtureManifest {
    #[serde(default = "default_schema_version")]
    schema_version: u16,
    #[serde(default)]
    suite_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    fixtures: Vec<FixtureDefinition>,
}

#[derive(Debug, Deserialize, Clone)]
struct FixtureDefinition {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    input: String,
    #[serde(default)]
    expected_output: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    dpi: Option<u16>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    schema_version: u16,
    run_id: String,
    generated_at_utc: String,
    benchmark_root: String,
    fixture_manifest: String,
    output_root: String,
    suite_name: Option<String>,
    suite_description: Option<String>,
    normalization_rules: Vec<String>,
    settings: ReportSettings,
    summary: ReportSummary,
    cases: Vec<CaseReport>,
}

#[derive(Debug, Serialize)]
struct ReportSettings {
    working_directory: String,
    model_file: Option<String>,
    threads: u16,
    default_dpi: u16,
    default_prompt: String,
    debug_assertions: bool,
}

#[derive(Debug, Serialize)]
struct ReportSummary {
    fixture_count: usize,
    success_count: usize,
    failure_count: usize,
    missing_expected_count: usize,
    average_time_to_first_preview_ms: Option<f64>,
    average_time_to_first_text_ms: Option<f64>,
    average_total_time_ms: Option<f64>,
    max_peak_memory_bytes: Option<u64>,
    max_normalized_output_diff: Option<f64>,
}

#[derive(Debug, Serialize)]
struct CaseReport {
    id: String,
    name: String,
    description: Option<String>,
    kind: String,
    source_file: String,
    staged_input_file: String,
    expected_output_file: Option<String>,
    actual_output_file: Option<String>,
    prompt: Option<String>,
    dpi: u16,
    tags: Vec<String>,
    status: String,
    error: Option<String>,
    blessed_output: bool,
    comparison_basis: String,
    metrics: CaseMetrics,
}

#[derive(Debug, Serialize)]
struct CaseMetrics {
    time_to_first_preview_ms: Option<u64>,
    time_to_first_text_ms: Option<u64>,
    total_time_ms: u64,
    peak_memory_bytes: Option<u64>,
    normalized_output_diff: Option<f64>,
    actual_token_count: Option<usize>,
    expected_token_count: Option<usize>,
}

#[derive(Debug)]
struct CaseRunArtifacts {
    staged_input_path: PathBuf,
    actual_output_path: Option<PathBuf>,
    actual_markdown: Option<String>,
    metrics: CaseMetrics,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct BenchmarkObserver {
    started_at: Option<Instant>,
    first_preview_ms: Option<u64>,
    first_text_ms: Option<u64>,
}

pub fn run_cli<I>(args: I) -> Result<()>
where
    I: IntoIterator<Item = String>,
{
    let cli = parse_cli(args.into_iter().skip(1))?;
    let benchmark_root = default_benchmark_root();
    let fixtures_root = benchmark_root.join(DEFAULT_FIXTURE_SUBDIR);
    let manifest_path = cli
        .manifest_path
        .unwrap_or_else(|| fixtures_root.join("manifest.json"));
    let output_root = cli
        .output_dir
        .unwrap_or_else(|| benchmark_root.join(DEFAULT_OUTPUT_SUBDIR));

    runtime::hydrate_path_for_binaries();

    let settings = Settings::load();
    ensure_runtime_ready(&settings)?;

    let manifest = load_manifest(&manifest_path)?;
    let suite_name = manifest.suite_name.clone();
    let suite_description = manifest.description.clone();
    let fixtures = filter_fixtures(manifest.fixtures, cli.filter.as_deref());

    if fixtures.is_empty() {
        bail!(
            "no fixtures matched. Check {} or adjust --filter.",
            manifest_path.display()
        );
    }

    if cli.list_only {
        for fixture in &fixtures {
            println!(
                "{}\t{}\t{}",
                fixture.id,
                fixture_kind(Path::new(&fixture.input)),
                fixture.name.clone().unwrap_or_else(|| fixture.id.clone())
            );
        }
        return Ok(());
    }

    fs::create_dir_all(&output_root)
        .with_context(|| format!("failed to create {}", output_root.display()))?;

    let run_id = Uuid::new_v4().simple().to_string();
    let timestamp = Utc::now();
    let run_dir_name = format!("{}-{}", timestamp.format("%Y%m%dT%H%M%SZ"), &run_id[..8]);
    let run_output_dir = output_root.join(run_dir_name);
    fs::create_dir_all(&run_output_dir)
        .with_context(|| format!("failed to create {}", run_output_dir.display()))?;

    let mut case_reports = Vec::with_capacity(fixtures.len());
    for fixture in fixtures {
        let case_report = run_case(
            &settings,
            &fixtures_root,
            &run_output_dir,
            &fixture,
            cli.bless,
        )?;
        print_case_summary(&case_report);
        case_reports.push(case_report);
    }

    let summary = build_summary(&case_reports);
    let report = BenchmarkReport {
        schema_version: default_schema_version(),
        run_id,
        generated_at_utc: timestamp.to_rfc3339(),
        benchmark_root: benchmark_root.to_string_lossy().into_owned(),
        fixture_manifest: manifest_path.to_string_lossy().into_owned(),
        output_root: output_root.to_string_lossy().into_owned(),
        suite_name,
        suite_description,
        normalization_rules: vec![
            "Collapse all whitespace runs to a single space.".into(),
            "Strip injected page headers like `## Page N` or `Page N of M`.".into(),
            "Ignore blank-line-only formatting changes when diffing token sequences.".into(),
        ],
        settings: ReportSettings {
            working_directory: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .to_string_lossy()
                .into_owned(),
            model_file: settings.model_file.clone(),
            threads: settings.threads,
            default_dpi: settings.dpi,
            default_prompt: DEFAULT_PROMPT_TEXT.into(),
            debug_assertions: cfg!(debug_assertions),
        },
        summary,
        cases: case_reports,
    };

    let report_path = run_output_dir.join("report.json");
    let latest_path = output_root.join(LATEST_REPORT_FILE);
    let report_json = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, &report_json)
        .with_context(|| format!("failed to write {}", report_path.display()))?;
    fs::write(&latest_path, &report_json)
        .with_context(|| format!("failed to write {}", latest_path.display()))?;

    println!();
    println!("Benchmark suite complete.");
    println!("Report: {}", report_path.display());
    println!("Latest: {}", latest_path.display());

    Ok(())
}

fn parse_cli<I>(args: I) -> Result<CliOptions>
where
    I: IntoIterator<Item = String>,
{
    let mut cli = CliOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--manifest requires a path"))?;
                cli.manifest_path = Some(PathBuf::from(value));
            }
            "--output-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--output-dir requires a path"))?;
                cli.output_dir = Some(PathBuf::from(value));
            }
            "--filter" => {
                let value = iter
                    .next()
                    .ok_or_else(|| anyhow!("--filter requires a value"))?;
                cli.filter = Some(value);
            }
            "--bless" => cli.bless = true,
            "--list" => cli.list_only = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unrecognized argument: {other}"),
        }
    }

    Ok(cli)
}

fn print_help() {
    println!("VisiTexta OCR benchmark runner");
    println!();
    println!("Usage:");
    println!("  cargo run --manifest-path src-tauri/Cargo.toml --bin ocr_bench -- [options]");
    println!();
    println!("Options:");
    println!("  --manifest <path>   Override the fixture manifest path.");
    println!("  --output-dir <path> Override the benchmark output directory.");
    println!("  --filter <value>    Run only fixtures whose id or name contains the value.");
    println!("  --bless             Update expected outputs with the current OCR markdown.");
    println!("  --list              List available fixtures without running OCR.");
}

fn default_benchmark_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app root")
        .join("benchmarks")
}

fn load_manifest(path: &Path) -> Result<FixtureManifest> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let manifest: FixtureManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    if manifest.schema_version != default_schema_version() {
        bail!(
            "fixture manifest {} has unsupported schema_version {}",
            path.display(),
            manifest.schema_version
        );
    }

    if manifest.fixtures.is_empty() {
        bail!(
            "fixture manifest {} does not define any fixtures",
            path.display()
        );
    }

    Ok(manifest)
}

fn filter_fixtures(
    fixtures: Vec<FixtureDefinition>,
    filter: Option<&str>,
) -> Vec<FixtureDefinition> {
    let Some(filter) = filter.map(|value| value.to_ascii_lowercase()) else {
        return fixtures;
    };

    fixtures
        .into_iter()
        .filter(|fixture| {
            fixture.id.to_ascii_lowercase().contains(&filter)
                || fixture
                    .name
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(&filter)
        })
        .collect()
}

fn run_case(
    settings: &Settings,
    fixtures_root: &Path,
    run_output_dir: &Path,
    fixture: &FixtureDefinition,
    bless: bool,
) -> Result<CaseReport> {
    let input_path = fixtures_root.join(&fixture.input);
    if !input_path.exists() {
        bail!(
            "fixture {} input does not exist: {}",
            fixture.id,
            input_path.display()
        );
    }

    let case_dir = run_output_dir.join(sanitize_case_id(&fixture.id));
    fs::create_dir_all(&case_dir)
        .with_context(|| format!("failed to create {}", case_dir.display()))?;

    let staged_input_path = stage_fixture_input(&input_path, &case_dir)?;
    let expected_output_path = fixture
        .expected_output
        .as_ref()
        .map(|relative| fixtures_root.join(relative));
    let dpi = fixture.dpi.unwrap_or(settings.dpi);

    let observer_started = Instant::now();
    let mut observer = BenchmarkObserver::new(observer_started);
    let memory_sampler = MemorySampler::start(std::process::id());
    let pipeline_result = pipeline::process_batch_with_observer(
        vec![staged_input_path.to_string_lossy().into_owned()],
        settings,
        dpi,
        fixture.prompt.clone(),
        None,
        &mut observer,
    );
    let peak_memory_bytes = memory_sampler.finish();
    let total_time_ms = observer_started.elapsed().as_millis() as u64;

    let artifacts = build_case_artifacts(
        pipeline_result,
        staged_input_path,
        peak_memory_bytes,
        total_time_ms,
        &observer,
    )?;

    let (normalized_output_diff, expected_token_count, comparison_basis, blessed_output) =
        evaluate_output_diff(
            fixture,
            &artifacts.actual_markdown,
            expected_output_path.as_deref(),
            bless,
        )?;

    let source_kind = fixture_kind(&input_path);
    let case_name = fixture.name.clone().unwrap_or_else(|| fixture.id.clone());

    Ok(CaseReport {
        id: fixture.id.clone(),
        name: case_name,
        description: fixture.description.clone(),
        kind: source_kind.into(),
        source_file: input_path.to_string_lossy().into_owned(),
        staged_input_file: artifacts.staged_input_path.to_string_lossy().into_owned(),
        expected_output_file: expected_output_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        actual_output_file: artifacts
            .actual_output_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        prompt: fixture.prompt.clone(),
        dpi,
        tags: fixture.tags.clone(),
        status: artifacts.status,
        error: artifacts.error,
        blessed_output,
        comparison_basis,
        metrics: CaseMetrics {
            time_to_first_preview_ms: artifacts.metrics.time_to_first_preview_ms,
            time_to_first_text_ms: artifacts.metrics.time_to_first_text_ms,
            total_time_ms: artifacts.metrics.total_time_ms,
            peak_memory_bytes: artifacts.metrics.peak_memory_bytes,
            normalized_output_diff,
            actual_token_count: artifacts.metrics.actual_token_count,
            expected_token_count,
        },
    })
}

fn build_case_artifacts(
    pipeline_result: crate::errors::Result<Vec<crate::pipeline::JobResult>>,
    staged_input_path: PathBuf,
    peak_memory_bytes: Option<u64>,
    total_time_ms: u64,
    observer: &BenchmarkObserver,
) -> Result<CaseRunArtifacts> {
    match pipeline_result {
        Ok(results) => {
            let result = results
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("benchmark pipeline returned no job results"))?;
            let actual_output_path = result.output_path.as_ref().map(PathBuf::from);
            let actual_markdown = actual_output_path
                .as_ref()
                .map(|path| fs::read_to_string(path))
                .transpose()
                .with_context(|| {
                    actual_output_path
                        .as_ref()
                        .map(|path| format!("failed to read {}", path.display()))
                        .unwrap_or_else(|| {
                            format!(
                                "failed to read benchmark output for {}",
                                staged_input_path.display()
                            )
                        })
                })?;

            Ok(CaseRunArtifacts {
                staged_input_path,
                actual_output_path,
                actual_markdown: actual_markdown.clone(),
                metrics: CaseMetrics {
                    time_to_first_preview_ms: observer.first_preview_ms,
                    time_to_first_text_ms: observer.first_text_ms,
                    total_time_ms,
                    peak_memory_bytes,
                    normalized_output_diff: None,
                    actual_token_count: actual_markdown.as_deref().map(normalized_token_count),
                    expected_token_count: None,
                },
                status: match result.status {
                    crate::events::JobStatus::Done => "ok".into(),
                    _ => "failed".into(),
                },
                error: result.error,
            })
        }
        Err(err) => Ok(CaseRunArtifacts {
            staged_input_path,
            actual_output_path: None,
            actual_markdown: None,
            metrics: CaseMetrics {
                time_to_first_preview_ms: observer.first_preview_ms,
                time_to_first_text_ms: observer.first_text_ms,
                total_time_ms,
                peak_memory_bytes,
                normalized_output_diff: None,
                actual_token_count: None,
                expected_token_count: None,
            },
            status: "failed".into(),
            error: Some(err.to_string()),
        }),
    }
}

fn evaluate_output_diff(
    fixture: &FixtureDefinition,
    actual_markdown: &Option<String>,
    expected_output_path: Option<&Path>,
    bless: bool,
) -> Result<(Option<f64>, Option<usize>, String, bool)> {
    let Some(actual_markdown) = actual_markdown.as_deref() else {
        return Ok((None, None, "none".into(), false));
    };

    let Some(expected_output_path) = expected_output_path else {
        return Ok((None, None, "none".into(), false));
    };

    let mut blessed_output = false;
    let comparison_basis;
    let expected_markdown = if expected_output_path.exists() {
        comparison_basis = "fixture_expected".into();
        Some(
            fs::read_to_string(expected_output_path)
                .with_context(|| format!("failed to read {}", expected_output_path.display()))?,
        )
    } else {
        comparison_basis = "missing_expected".into();
        None
    };

    if bless {
        if let Some(parent) = expected_output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(expected_output_path, actual_markdown)
            .with_context(|| format!("failed to write {}", expected_output_path.display()))?;
        blessed_output = true;
    }

    let Some(expected_markdown) = expected_markdown else {
        return Ok((None, None, comparison_basis, blessed_output));
    };

    let actual_tokens = normalize_for_diff(actual_markdown);
    let expected_tokens = normalize_for_diff(&expected_markdown);
    let diff = normalized_token_levenshtein(&actual_tokens, &expected_tokens);

    if fixture.expected_output.is_none() {
        return Ok((None, None, "none".into(), blessed_output));
    }

    Ok((
        Some(diff),
        Some(expected_tokens.len()),
        comparison_basis,
        blessed_output,
    ))
}

fn build_summary(cases: &[CaseReport]) -> ReportSummary {
    let success_count = cases.iter().filter(|case| case.status == "ok").count();
    let failure_count = cases.len().saturating_sub(success_count);
    let missing_expected_count = cases
        .iter()
        .filter(|case| {
            case.comparison_basis == "missing_expected" || case.comparison_basis == "none"
        })
        .count();

    ReportSummary {
        fixture_count: cases.len(),
        success_count,
        failure_count,
        missing_expected_count,
        average_time_to_first_preview_ms: average_optional_u64(
            cases
                .iter()
                .map(|case| case.metrics.time_to_first_preview_ms)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        average_time_to_first_text_ms: average_optional_u64(
            cases
                .iter()
                .map(|case| case.metrics.time_to_first_text_ms)
                .collect::<Vec<_>>()
                .as_slice(),
        ),
        average_total_time_ms: if cases.is_empty() {
            None
        } else {
            Some(
                cases
                    .iter()
                    .map(|case| case.metrics.total_time_ms as f64)
                    .sum::<f64>()
                    / cases.len() as f64,
            )
        },
        max_peak_memory_bytes: cases
            .iter()
            .filter_map(|case| case.metrics.peak_memory_bytes)
            .max(),
        max_normalized_output_diff: cases
            .iter()
            .filter_map(|case| case.metrics.normalized_output_diff)
            .max_by(|left, right| left.total_cmp(right)),
    }
}

fn average_optional_u64(values: &[Option<u64>]) -> Option<f64> {
    let present: Vec<u64> = values.iter().copied().flatten().collect();
    if present.is_empty() {
        None
    } else {
        Some(present.iter().sum::<u64>() as f64 / present.len() as f64)
    }
}

fn print_case_summary(case: &CaseReport) {
    println!(
        "{:<18} {:<6} total={} preview={} text={} diff={} peak={}",
        case.id,
        case.status,
        format_duration(case.metrics.total_time_ms),
        format_optional_duration(case.metrics.time_to_first_preview_ms),
        format_optional_duration(case.metrics.time_to_first_text_ms),
        format_optional_diff(case.metrics.normalized_output_diff),
        format_optional_bytes(case.metrics.peak_memory_bytes),
    );

    if let Some(error) = &case.error {
        println!("  error: {error}");
    }
}

fn format_duration(value: u64) -> String {
    format!("{value}ms")
}

fn format_optional_duration(value: Option<u64>) -> String {
    value.map(format_duration).unwrap_or_else(|| "n/a".into())
}

fn format_optional_diff(value: Option<f64>) -> String {
    value
        .map(|diff| format!("{diff:.4}"))
        .unwrap_or_else(|| "n/a".into())
}

fn format_optional_bytes(value: Option<u64>) -> String {
    value.map(format_bytes).unwrap_or_else(|| "n/a".into())
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let bytes_f = bytes as f64;
    if bytes_f >= GIB {
        format!("{:.2} GiB", bytes_f / GIB)
    } else {
        format!("{:.1} MiB", bytes_f / MIB)
    }
}

fn ensure_runtime_ready(settings: &Settings) -> Result<()> {
    if !runtime::runtime_has_ocr_runner(settings.runtime_profile) {
        bail!("benchmark runtime not ready: no llama OCR runner was found in src-tauri/bin or runtime resources");
    }

    if !models::has_vision_model(settings) {
        bail!("benchmark runtime not ready: no supported vision model is configured or fully available");
    }

    Ok(())
}

fn stage_fixture_input(input_path: &Path, case_dir: &Path) -> Result<PathBuf> {
    let file_name = input_path
        .file_name()
        .ok_or_else(|| anyhow!("fixture input has no file name: {}", input_path.display()))?;
    let staged_input_path = case_dir.join(file_name);
    fs::copy(input_path, &staged_input_path).with_context(|| {
        format!(
            "failed to stage fixture {} into {}",
            input_path.display(),
            staged_input_path.display()
        )
    })?;
    Ok(staged_input_path)
}

fn fixture_kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => "pdf",
        "png" | "jpg" | "jpeg" => "image",
        _ => "unknown",
    }
}

fn sanitize_case_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn default_schema_version() -> u16 {
    1
}

impl BenchmarkObserver {
    fn new(started_at: Instant) -> Self {
        Self {
            started_at: Some(started_at),
            first_preview_ms: None,
            first_text_ms: None,
        }
    }

    fn elapsed_ms(&self) -> Option<u64> {
        self.started_at
            .map(|started_at| started_at.elapsed().as_millis() as u64)
    }
}

impl PipelineObserver for BenchmarkObserver {
    fn on_progress(&mut self, _event: ProgressEvent) {}

    fn on_preview(&mut self, _event: PreviewEvent) {
        if self.first_preview_ms.is_none() {
            self.first_preview_ms = self.elapsed_ms();
        }
    }

    fn on_runner(&mut self, event: RunnerEvent) {
        if self.first_text_ms.is_some() {
            return;
        }

        match event.stage {
            RunnerStage::FirstToken => {
                self.first_text_ms = self.elapsed_ms();
            }
            RunnerStage::Chunk => {
                if let Some(chunk) = event.chunk {
                    if has_substantive_text(&chunk) {
                        self.first_text_ms = self.elapsed_ms();
                    }
                }
            }
            _ => {}
        }
    }

    fn on_completed(&mut self, _event: CompletedEvent) {}

    fn on_error(&mut self, _event: ErrorEvent) {}
}

fn has_substantive_text(text: &str) -> bool {
    let tokens = normalize_for_diff(text);
    tokens
        .iter()
        .flat_map(|token| token.chars())
        .any(|ch| ch.is_alphanumeric())
}

fn normalize_for_diff(input: &str) -> Vec<String> {
    let normalized_newlines = input.replace("\r\n", "\n").replace('\r', "\n");
    let without_markdown_page_headers =
        markdown_page_header_regex().replace_all(&normalized_newlines, " ");
    let without_isolated_page_headers =
        isolated_page_header_regex().replace_all(&without_markdown_page_headers, "\n\n");
    whitespace_regex()
        .replace_all(&without_isolated_page_headers, " ")
        .trim()
        .split_whitespace()
        .map(|token| token.to_string())
        .collect()
}

fn normalized_token_count(input: &str) -> usize {
    normalize_for_diff(input).len()
}

fn normalized_token_levenshtein(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 0.0;
    }

    let right_len = right.len();
    let mut previous: Vec<usize> = (0..=right_len).collect();
    let mut current = vec![0; right_len + 1];

    for (left_index, left_token) in left.iter().enumerate() {
        current[0] = left_index + 1;

        for (right_index, right_token) in right.iter().enumerate() {
            let substitution_cost = if left_token == right_token { 0 } else { 1 };
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(previous[right_index] + substitution_cost);
        }

        previous.copy_from_slice(&current);
    }

    let distance = previous[right_len] as f64;
    let normalizer = left.len().max(right.len()).max(1) as f64;
    distance / normalizer
}

fn markdown_page_header_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?im)^\s{0,3}#{1,6}\s*page\s+\d+(?:\s*(?:/|of)\s*\d+)?\s*:?\s*$")
            .expect("markdown page header regex")
    })
}

fn whitespace_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\s+").expect("whitespace regex"))
}

fn isolated_page_header_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?im)(?:^|\n\s*\n)\s*page\s+\d+(?:\s*(?:/|of)\s*\d+)?\s*:?\s*(?=\n\s*\n|$)")
            .expect("isolated page header regex")
    })
}

struct MemorySampler {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<Option<u64>>,
}

impl MemorySampler {
    fn start(root_pid: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_signal = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            let root_pid = Pid::from_u32(root_pid);
            let mut system = System::new_all();
            let mut peak = 0_u64;

            loop {
                system.refresh_all();
                peak = peak.max(process_tree_memory_bytes(&system, root_pid));

                if stop_signal.load(Ordering::Relaxed) {
                    break;
                }

                thread::sleep(Duration::from_millis(MEMORY_SAMPLE_INTERVAL_MS));
            }

            if peak == 0 {
                None
            } else {
                Some(peak)
            }
        });

        Self { stop, handle }
    }

    fn finish(self) -> Option<u64> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.join().unwrap_or(None)
    }
}

fn process_tree_memory_bytes(system: &System, root_pid: Pid) -> u64 {
    let mut total = 0_u64;
    let mut seen = HashSet::new();
    let mut stack = vec![root_pid];

    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }

        if let Some(process) = system.process(pid) {
            total = total.saturating_add(process.memory());
        }

        for (candidate_pid, candidate) in system.processes() {
            if candidate.parent() == Some(pid) {
                stack.push(*candidate_pid);
            }
        }
    }

    total
}
