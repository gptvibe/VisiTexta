use crate::errors::{PipelineError, Result};
use std::io::Read;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use regex::Regex;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct LlmOcrEngine {
    runner_paths: Vec<PathBuf>,
    mmproj_path: Option<PathBuf>,
    model_path: PathBuf,
    threads: u16,
}

impl LlmOcrEngine {
    pub fn new(model_path: PathBuf, threads: u16) -> Result<Self> {
        if !model_path.exists() {
            return Err(PipelineError::Llm(format!(
                "model file not found: {}",
                model_path.to_string_lossy()
            )));
        }

        let model_name = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !is_supported_vision_model_name(&model_name) {
            return Err(PipelineError::Llm(
                "selected model is not a supported vision OCR model".into(),
            ));
        }

        // Vision OCR should use mtmd first. Keep llama-cli as a fallback for
        // runtime compatibility on some environments.
        let runner_paths = resolve_runner_exes(&[
            "llama-mtmd-cli.exe",
            "llama-cli.exe",
        ]);
        if runner_paths.is_empty() {
            return Err(PipelineError::Llm(
                "no multimodal-compatible llama CLI executable found".into(),
            ));
        }

        let needs_mmproj = (model_name.contains("qwen") && model_name.contains("-vl"))
            || model_name.contains("llava")
            || model_name.contains("glm-ocr");

        let mmproj_path = if needs_mmproj {
            Some(
                resolve_mmproj_path(&model_path)
                    .ok_or_else(|| PipelineError::Llm("mmproj model not found for selected vision model".into()))?,
            )
        } else {
            None
        };

        Ok(Self {
            runner_paths,
            mmproj_path,
            model_path,
            threads,
        })
    }

    pub fn recognize_streaming<F>(
        &self,
        image_path: &Path,
        prompt: &str,
        mut on_chunk: F,
    ) -> Result<String>
    where
        F: FnMut(&str),
    {
        let effective_prompt = format!(
            "You are an OCR engine. Read all visible text from the provided image. {} Return only markdown output and no extra explanation.",
            prompt.trim()
        );

        let mut last_error: Option<PipelineError> = None;

        for runner in &self.runner_paths {
            match self.try_recognize_with_runner(runner, image_path, &effective_prompt, &mut on_chunk) {
                Ok(output) => return Ok(output),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            PipelineError::Llm("no OCR runner was able to produce output".into())
        }))
    }

    fn try_recognize_with_runner<F>(
        &self,
        runner: &Path,
        image_path: &Path,
        effective_prompt: &str,
        on_chunk: &mut F,
    ) -> Result<String>
    where
        F: FnMut(&str),
    {
        let mut cmd = Command::new(runner);
        cmd.arg("-m")
            .arg(&self.model_path)
            .arg("--image")
            .arg(image_path)
            .arg("-p")
            .arg(effective_prompt)
            .arg("-n")
            .arg("2048")
            .arg("--temp")
            .arg("0")
            .arg("--ctx-size")
            .arg("8192")
            .arg("--no-warmup")
            .arg("--threads")
            .arg(self.threads.max(1).to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(mmproj) = &self.mmproj_path {
            cmd.arg("--mmproj").arg(mmproj);
        }

        #[cfg(target_os = "windows")]
        {
            // Prevent a console window from flashing when launching llama CLI.
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| PipelineError::Llm(format!("spawn failed for {}: {e}", runner.to_string_lossy())))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| PipelineError::Llm("failed to capture llama-cli stdout".into()))?;

        let mut collected = String::new();
        let mut buf = [0_u8; 4096];
        loop {
            let read = stdout
                .read(&mut buf)
                .map_err(|e| PipelineError::Llm(format!("read stdout failed: {e}")))?;
            if read == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&buf[..read]).into_owned();
            collected.push_str(&chunk);
        }

        let mut stderr_bytes = Vec::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = stderr.read_to_end(&mut stderr_bytes);
        }

        let status = child
            .wait()
            .map_err(|e| PipelineError::Llm(format!("wait failed: {e}")))?;

        let stderr = sanitize_stderr(&String::from_utf8_lossy(&stderr_bytes));

        if !status.success() {
            let stdout = collected.trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "no output captured".to_string()
            };
            return Err(PipelineError::Llm(format!(
                "{} exited with {}: {}",
                runner.to_string_lossy(),
                status,
                detail
            )));
        }

        let cleaned_output = sanitize_model_stdout(&collected);

        if cleaned_output.trim().is_empty() {
            let detail = if !stderr.is_empty() {
                format!(" stderr: {}", stderr)
            } else {
                String::new()
            };
            return Err(PipelineError::Llm(format!(
                "{} produced empty output. This usually means the runtime invocation is incompatible.{}",
                runner.to_string_lossy(),
                detail
            )));
        }

        on_chunk(&cleaned_output);

        Ok(cleaned_output)
    }
}

pub fn runtime_has_llama_cli() -> bool {
    resolve_runner_exe(&[
        "llama-mtmd-cli.exe",
        "llama-cli.exe",
    ])
    .is_some()
}

fn sanitize_stderr(raw: &str) -> String {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn sanitize_model_stdout(raw: &str) -> String {
    let ansi = Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").unwrap();
    let stripped = ansi.replace_all(raw, "");

    let mut lines: Vec<String> = stripped
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| line.to_string())
        .collect();

    // Remove known non-content boilerplate lines emitted by llama CLIs.
    lines.retain(|line| {
        let t = line.trim();
        if t.is_empty() {
            return true;
        }
        let lower = t.to_ascii_lowercase();

        if lower.starts_with("build ")
            || lower.starts_with("model ")
            || lower.starts_with("modalities ")
            || lower.starts_with("available commands")
            || lower.starts_with("loaded media from")
            || lower.starts_with("you are an ocr engine")
            || lower.starts_with("warn:")
            || lower.starts_with("main: loading model")
            || lower.starts_with("encoding image")
            || lower.starts_with("decoding image")
            || lower.starts_with("llama_perf_context_print")
        {
            return false;
        }

        if t.starts_with('/') {
            return false;
        }

        // Drop pure block-art lines.
        if t.chars().all(|c| matches!(c, '▄' | '▀' | '█' | ' ')) {
            return false;
        }

        true
    });

    // Trim leading/trailing empty lines left after filtering.
    while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }

    lines.join("\n")
}

fn is_supported_vision_model_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered.ends_with(".gguf")
        && !lowered.starts_with("mmproj")
        && (lowered.contains("-vl")
            || lowered.contains("vision")
            || lowered.contains("llava")
            || lowered.contains("glm-ocr"))
}

fn resolve_runner_exe(names: &[&str]) -> Option<PathBuf> {
    resolve_runner_exes(names).into_iter().next()
}

fn resolve_runner_exes(names: &[&str]) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for name in names {
        let candidates = [
            exe_dir.as_ref().map(|p| p.join("bin").join(name)),
            exe_dir
                .as_ref()
                .map(|p| p.join("resources").join("bin").join(name)),
            exe_dir.as_ref().map(|p| p.join(name)),
            Some(PathBuf::from("src-tauri").join("bin").join(name)),
            Some(PathBuf::from(name)),
        ];

        for candidate in candidates.iter().flatten() {
            if candidate.exists() && seen.insert(candidate.clone()) {
                found.push(candidate.to_path_buf());
            }
        }
    }

    found
}

fn resolve_mmproj_path(model_path: &Path) -> Option<PathBuf> {
    let parent = model_path.parent()?;
    let model_name = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut search_dirs = vec![parent.to_path_buf()];

    if let Ok(cwd) = std::env::current_dir() {
        search_dirs.push(cwd.join("models"));
        search_dirs.push(cwd.join("resources").join("models"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            search_dirs.push(dir.join("models"));
            search_dirs.push(dir.join("resources").join("models"));
            if let Some(parent_dir) = dir.parent() {
                search_dirs.push(parent_dir.join("models"));
                search_dirs.push(parent_dir.join("resources").join("models"));
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    search_dirs.retain(|d| seen.insert(d.clone()));

    let mut found = Vec::new();
    for dir in search_dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_ascii_lowercase(),
                None => continue,
            };
            if name.contains("mmproj") && name.ends_with(".gguf") {
                found.push(path);
            }
        }
    }

    if found.is_empty() {
        return None;
    }

    found.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let family_score = if model_name.contains("glm-ocr") {
            if name.contains("glm-ocr") { 0 } else { 1 }
        } else if model_name.contains("qwen") && model_name.contains("-vl") {
            if name.contains("qwen") { 0 } else { 1 }
        } else if model_name.contains("llava") {
            if name.contains("llava") { 0 } else { 1 }
        } else {
            0
        };

        let precision_score = if name.contains("f16") {
            0
        } else if name.contains("bf16") {
            1
        } else if name.contains("f32") {
            2
        } else {
            3
        };

        (family_score, precision_score)
    });

    found.into_iter().next()
}
