use crate::errors::{PipelineError, Result};
use crate::settings::Settings;
use crate::storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub const DEFAULT_MODEL_PROFILE_ID: &str = "glm-ocr";

const HF_API_BASE: &str = "https://huggingface.co/api/models";
const HF_RESOLVE_BASE: &str = "https://huggingface.co";
const MODEL_MANIFEST_FILE: &str = ".visitexta-models.json";
const PART_EXTENSION: &str = "part";

const CURATED_RUNNER_COMPATIBILITY: RunnerCompatibility = RunnerCompatibility {
    transient_cli: true,
    persistent_server: true,
    notes: "Curated support path with VisiTexta's bundled multimodal llama.cpp runners.",
};

const LEGACY_RUNNER_COMPATIBILITY: RunnerCompatibility = RunnerCompatibility {
    transient_cli: true,
    persistent_server: true,
    notes: "Legacy compatibility path. Curated profiles are preferred.",
};

const EXPERIMENTAL_RUNNER_COMPATIBILITY: RunnerCompatibility = RunnerCompatibility {
    transient_cli: true,
    persistent_server: true,
    notes: "Experimental best-effort path. Compatibility still depends on the selected GGUF and bundled runner build.",
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelSupportTier {
    Recommended,
    Tested,
    Legacy,
    Experimental,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelInstallSource {
    Registry,
    Custom,
    Heuristic,
    Legacy,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RunnerCompatibility {
    pub transient_cli: bool,
    pub persistent_server: bool,
    pub notes: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub family: &'static str,
    pub repo: &'static str,
    pub default_file: &'static str,
    pub requires_mmproj: bool,
    pub tested: bool,
    pub recommended: bool,
    pub notes: &'static str,
    pub runner_compatibility: RunnerCompatibility,
    pub installed: bool,
    pub runtime_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalModelInfo {
    pub file_name: String,
    pub label: String,
    pub family: String,
    pub repo: Option<String>,
    pub profile_id: Option<String>,
    pub requires_mmproj: bool,
    pub runtime_ready: bool,
    pub tested: bool,
    pub recommended: bool,
    pub experimental: bool,
    pub notes: Option<String>,
    pub support_tier: ModelSupportTier,
    pub source: ModelInstallSource,
    pub auto_selectable: bool,
    pub runner_compatibility: RunnerCompatibility,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelCatalog {
    pub default_profile_id: &'static str,
    pub profiles: Vec<ModelProfile>,
    pub local_models: Vec<LocalModelInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ModelDownloadEvent {
    pub repo: String,
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub progress: f32,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DownloadResult {
    pub repo: String,
    pub file_name: String,
    pub file_path: String,
    pub profile_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeModelRequirements {
    pub mmproj_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
struct ModelProfileDefinition {
    id: &'static str,
    label: &'static str,
    family: &'static str,
    repo: &'static str,
    default_file: &'static str,
    requires_mmproj: bool,
    tested: bool,
    recommended: bool,
    notes: &'static str,
    runner_compatibility: RunnerCompatibility,
    file_markers: &'static [&'static str],
}

const CURATED_MODEL_PROFILES: [ModelProfileDefinition; 3] = [
    ModelProfileDefinition {
        id: "glm-ocr",
        label: "GLM-OCR",
        family: "GLM-OCR",
        repo: "mradermacher/GLM-OCR-GGUF",
        default_file: "GLM-OCR.Q4_K_M.gguf",
        requires_mmproj: true,
        tested: true,
        recommended: true,
        notes: "OCR-first default profile. This remains the simple recommended setup path.",
        runner_compatibility: CURATED_RUNNER_COMPATIBILITY,
        file_markers: &["glm-ocr"],
    },
    ModelProfileDefinition {
        id: "qwen2-vl-ocr-2b",
        label: "Qwen2-VL OCR 2B",
        family: "Qwen2-VL OCR",
        repo: "mradermacher/Qwen2-VL-OCR-2B-Instruct-GGUF",
        default_file: "Qwen2-VL-OCR-2B-Instruct.Q4_K_M.gguf",
        requires_mmproj: true,
        tested: true,
        recommended: false,
        notes: "Tested OCR-focused alternative. Supported explicitly, but GLM-OCR stays the default.",
        runner_compatibility: CURATED_RUNNER_COMPATIBILITY,
        file_markers: &["qwen2-vl-ocr-2b-instruct"],
    },
    ModelProfileDefinition {
        id: "qwen2.5-vl-3b",
        label: "Qwen2.5-VL 3B",
        family: "Qwen2.5-VL",
        repo: "mradermacher/Qwen2.5-VL-3B-Instruct-GGUF",
        default_file: "Qwen2.5-VL-3B-Instruct.Q4_K_M.gguf",
        requires_mmproj: true,
        tested: true,
        recommended: false,
        notes: "Tested general-purpose vision-language alternative. Heavier than the OCR-specific curated defaults.",
        runner_compatibility: CURATED_RUNNER_COMPATIBILITY,
        file_markers: &["qwen2.5-vl-3b-instruct"],
    },
];

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct InstallManifest {
    version: u8,
    installs: Vec<InstalledModelRecord>,
}

impl Default for InstallManifest {
    fn default() -> Self {
        Self {
            version: 1,
            installs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledModelRecord {
    file_name: String,
    repo: String,
    profile_id: Option<String>,
    family: Option<String>,
    label: Option<String>,
    requires_mmproj: bool,
    notes: Option<String>,
    support_tier: ModelSupportTier,
    source: ModelInstallSource,
    mmproj_file: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct HfModelInfo {
    files: Vec<HfSibling>,
}

#[derive(Debug, Deserialize, Clone)]
struct HfSibling {
    #[serde(alias = "path", alias = "rfilename")]
    rfilename: String,
    size: Option<u64>,
    lfs: Option<HfLfsInfo>,
}

#[derive(Debug, Deserialize, Clone)]
struct HfLfsInfo {
    oid: String,
    size: u64,
}

#[derive(Debug, Clone)]
struct DiscoveredLocalModel {
    path: PathBuf,
    info: LocalModelInfo,
}

#[derive(Debug, Clone)]
struct InspectedModel {
    profile_id: Option<String>,
    repo: Option<String>,
    label: String,
    family: String,
    requires_mmproj: bool,
    notes: Option<String>,
    support_tier: ModelSupportTier,
    source: ModelInstallSource,
    auto_selectable: bool,
    tested: bool,
    recommended: bool,
    runner_compatibility: RunnerCompatibility,
}

#[derive(Debug, Clone)]
struct DownloadPlan {
    repo: String,
    file_name: String,
    profile_id: Option<String>,
    inspected: InspectedModel,
    info: HfModelInfo,
}

#[derive(Debug, Clone)]
struct RemoteFileMetadata {
    total_bytes: Option<u64>,
    checksum_sha256: Option<String>,
}

impl ModelProfileDefinition {
    fn export(self, installed: bool, runtime_ready: bool) -> ModelProfile {
        ModelProfile {
            id: self.id,
            label: self.label,
            family: self.family,
            repo: self.repo,
            default_file: self.default_file,
            requires_mmproj: self.requires_mmproj,
            tested: self.tested,
            recommended: self.recommended,
            notes: self.notes,
            runner_compatibility: self.runner_compatibility,
            installed,
            runtime_ready,
        }
    }
}

pub fn recommended_profile_id() -> &'static str {
    DEFAULT_MODEL_PROFILE_ID
}

pub fn recommended_model_file() -> &'static str {
    recommended_profile().default_file
}

pub fn recommended_model_repo() -> &'static str {
    recommended_profile().repo
}

pub fn recommended_model_profile_id() -> &'static str {
    DEFAULT_MODEL_PROFILE_ID
}

pub fn recommended_model_label() -> &'static str {
    recommended_profile().label
}

pub fn recommended_model_file_name() -> &'static str {
    recommended_model_file()
}

pub fn model_catalog() -> Result<ModelCatalog> {
    let mut local_models = discover_local_models()?;
    local_models.sort_by_key(|model| catalog_sort_key(model));

    Ok(ModelCatalog {
        default_profile_id: recommended_profile_id(),
        profiles: CURATED_MODEL_PROFILES
            .iter()
            .copied()
            .map(|profile| {
                let installed_for_profile: Vec<&DiscoveredLocalModel> = local_models
                    .iter()
                    .filter(|model| model.info.profile_id.as_deref() == Some(profile.id))
                    .collect();

                let installed = !installed_for_profile.is_empty();
                let runtime_ready = installed_for_profile
                    .iter()
                    .any(|model| model.info.runtime_ready);

                profile.export(installed, runtime_ready)
            })
            .collect(),
        local_models: local_models.into_iter().map(|model| model.info).collect(),
    })
}

pub fn get_model_catalog() -> Result<ModelCatalog> {
    model_catalog()
}

pub fn list_models() -> Result<Vec<String>> {
    Ok(model_catalog()?
        .local_models
        .into_iter()
        .map(|model| model.file_name)
        .collect())
}

pub fn model_exists(settings: &Settings) -> bool {
    if has_explicit_model_selection(settings) {
        return resolve_selected_model(settings).is_ok();
    }

    resolve_active_auto_model().is_ok()
}

pub fn has_vision_model(settings: &Settings) -> bool {
    if has_explicit_model_selection(settings) {
        return resolve_selected_model(settings)
            .map(|model| model.info.runtime_ready)
            .unwrap_or(false);
    }

    resolve_active_auto_model()
        .map(|model| model.info.runtime_ready)
        .unwrap_or(false)
}

pub fn resolve_active_model_path(settings: &Settings) -> Result<PathBuf> {
    resolve_active_vision_model_path(settings)
}

pub fn resolve_active_vision_model_path(settings: &Settings) -> Result<PathBuf> {
    if let Ok(selected) = resolve_selected_model(settings) {
        if !selected.info.runtime_ready {
            return Err(PipelineError::InvalidInput(format!(
                "{} is installed, but its mmproj companion is missing",
                selected.info.label
            )));
        }
        return Ok(selected.path);
    }

    resolve_active_auto_model().map(|model| model.path)
}

pub fn resolve_runtime_model_requirements(model_path: &Path) -> Result<RuntimeModelRequirements> {
    let inspected = inspect_model_path(model_path).ok_or_else(|| {
        PipelineError::InvalidInput("selected model file could not be inspected".into())
    })?;

    let mmproj_path = if inspected.requires_mmproj {
        Some(resolve_model_mmproj_path(model_path).ok_or_else(|| {
            PipelineError::InvalidInput("mmproj model not found for the selected OCR model".into())
        })?)
    } else {
        None
    };

    Ok(RuntimeModelRequirements { mmproj_path })
}

pub fn active_model_file(settings: &Settings) -> String {
    if let Some(model_file) = settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        return model_file;
    }

    if let Some(profile_id) = settings
        .model_profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(profile) = profile_by_id(profile_id) {
            return profile.default_file.to_string();
        }
    }

    recommended_model_file().to_string()
}

fn recommended_profile() -> &'static ModelProfileDefinition {
    profile_by_id(DEFAULT_MODEL_PROFILE_ID).expect("default model profile must exist")
}

fn has_explicit_model_selection(settings: &Settings) -> bool {
    settings
        .model_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
        || settings
            .model_profile_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
}

fn profile_by_id(id: &str) -> Option<&'static ModelProfileDefinition> {
    CURATED_MODEL_PROFILES
        .iter()
        .find(|profile| profile.id.eq_ignore_ascii_case(id.trim()))
}

fn profile_by_repo(repo: &str) -> Option<&'static ModelProfileDefinition> {
    CURATED_MODEL_PROFILES
        .iter()
        .find(|profile| profile.repo.eq_ignore_ascii_case(repo.trim()))
}

fn profile_matching_file_name(file_name: &str) -> Option<&'static ModelProfileDefinition> {
    let lowered = file_name.to_ascii_lowercase();
    CURATED_MODEL_PROFILES.iter().find(|profile| {
        profile
            .file_markers
            .iter()
            .any(|marker| lowered.contains(marker))
    })
}

fn support_tier_rank(tier: ModelSupportTier) -> u8 {
    match tier {
        ModelSupportTier::Recommended => 0,
        ModelSupportTier::Tested => 1,
        ModelSupportTier::Legacy => 2,
        ModelSupportTier::Experimental => 3,
    }
}

fn profile_order(profile_id: Option<&str>) -> usize {
    profile_id
        .and_then(|id| {
            CURATED_MODEL_PROFILES
                .iter()
                .position(|profile| profile.id == id)
        })
        .unwrap_or(CURATED_MODEL_PROFILES.len())
}

fn catalog_sort_key(model: &DiscoveredLocalModel) -> (u8, u8, usize, u8, String) {
    let info = &model.info;
    let preferred_file = info
        .profile_id
        .as_deref()
        .and_then(profile_by_id)
        .map(|profile| profile.default_file.eq_ignore_ascii_case(&info.file_name))
        .unwrap_or(false);

    (
        support_tier_rank(info.support_tier),
        if info.runtime_ready { 0 } else { 1 },
        profile_order(info.profile_id.as_deref()),
        if preferred_file { 0 } else { 1 },
        info.label.to_ascii_lowercase(),
    )
}

fn auto_model_sort_key(model: &DiscoveredLocalModel) -> (u8, usize, u8, String) {
    let info = &model.info;
    let preferred_file = info
        .profile_id
        .as_deref()
        .and_then(profile_by_id)
        .map(|profile| profile.default_file.eq_ignore_ascii_case(&info.file_name))
        .unwrap_or(false);

    (
        support_tier_rank(info.support_tier),
        profile_order(info.profile_id.as_deref()),
        if preferred_file { 0 } else { 1 },
        info.label.to_ascii_lowercase(),
    )
}

fn build_local_model_info(path: &Path, inspected: InspectedModel) -> LocalModelInfo {
    LocalModelInfo {
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        label: inspected.label,
        family: inspected.family,
        repo: inspected.repo,
        profile_id: inspected.profile_id,
        requires_mmproj: inspected.requires_mmproj,
        runtime_ready: if inspected.requires_mmproj {
            resolve_model_mmproj_path(path).is_some()
        } else {
            true
        },
        tested: inspected.tested,
        recommended: inspected.recommended,
        experimental: !matches!(
            inspected.support_tier,
            ModelSupportTier::Recommended | ModelSupportTier::Tested
        ),
        notes: inspected.notes,
        support_tier: inspected.support_tier,
        source: inspected.source,
        auto_selectable: inspected.auto_selectable,
        runner_compatibility: inspected.runner_compatibility,
    }
}

fn profile_inspection(
    profile: &ModelProfileDefinition,
    repo: Option<String>,
    notes_override: Option<String>,
    source: ModelInstallSource,
) -> InspectedModel {
    InspectedModel {
        profile_id: Some(profile.id.to_string()),
        repo: repo.or_else(|| Some(profile.repo.to_string())),
        label: profile.label.to_string(),
        family: profile.family.to_string(),
        requires_mmproj: profile.requires_mmproj,
        notes: notes_override.or_else(|| Some(profile.notes.to_string())),
        support_tier: if profile.recommended {
            ModelSupportTier::Recommended
        } else {
            ModelSupportTier::Tested
        },
        source,
        auto_selectable: true,
        tested: profile.tested,
        recommended: profile.recommended,
        runner_compatibility: profile.runner_compatibility,
    }
}

fn experimental_inspection(repo: Option<&str>, file_name: &str) -> InspectedModel {
    let family = infer_family_label(repo, file_name)
        .unwrap_or("Custom")
        .to_string();

    InspectedModel {
        profile_id: None,
        repo: repo.map(|value| value.to_string()),
        label: file_name.to_string(),
        family,
        requires_mmproj: infer_requires_mmproj(repo, file_name),
        notes: Some(
            "Experimental custom model. VisiTexta keeps this path for power users, but the curated registry is the supported default."
                .into(),
        ),
        support_tier: ModelSupportTier::Experimental,
        source: ModelInstallSource::Custom,
        auto_selectable: false,
        tested: false,
        recommended: false,
        runner_compatibility: EXPERIMENTAL_RUNNER_COMPATIBILITY,
    }
}

fn legacy_inspection_from_name(file_name: &str) -> Option<InspectedModel> {
    let lowered = file_name.to_ascii_lowercase();
    let (family, requires_mmproj) = if lowered.contains("llava") {
        ("LLaVA", true)
    } else if lowered.contains("-vl") || lowered.contains("qwen-vl") {
        ("Vision-Language GGUF", true)
    } else if lowered.contains("vision") {
        ("Legacy vision GGUF", true)
    } else {
        return None;
    };

    Some(InspectedModel {
        profile_id: None,
        repo: None,
        label: file_name.to_string(),
        family: family.to_string(),
        requires_mmproj,
        notes: Some(
            "Legacy filename-based compatibility is still available, but curated profiles are preferred."
                .into(),
        ),
        support_tier: ModelSupportTier::Legacy,
        source: ModelInstallSource::Legacy,
        auto_selectable: true,
        tested: false,
        recommended: false,
        runner_compatibility: LEGACY_RUNNER_COMPATIBILITY,
    })
}

fn inspect_model_candidate(
    path: &Path,
    manifest_record: Option<&InstalledModelRecord>,
) -> InspectedModel {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    if let Some(record) = manifest_record {
        if let Some(profile_id) = record.profile_id.as_deref() {
            if let Some(profile) = profile_by_id(profile_id) {
                return profile_inspection(
                    profile,
                    Some(record.repo.clone()),
                    record.notes.clone(),
                    ModelInstallSource::Registry,
                );
            }
        }

        return InspectedModel {
            profile_id: None,
            repo: Some(record.repo.clone()),
            label: record
                .label
                .clone()
                .unwrap_or_else(|| file_name.to_string()),
            family: record.family.clone().unwrap_or_else(|| "Custom".into()),
            requires_mmproj: record.requires_mmproj,
            notes: record.notes.clone(),
            support_tier: record.support_tier,
            source: record.source,
            auto_selectable: false,
            tested: false,
            recommended: false,
            runner_compatibility: EXPERIMENTAL_RUNNER_COMPATIBILITY,
        };
    }

    if let Some(profile) = profile_matching_file_name(file_name) {
        return profile_inspection(profile, None, None, ModelInstallSource::Heuristic);
    }

    if let Some(legacy) = legacy_inspection_from_name(file_name) {
        return legacy;
    }

    experimental_inspection(None, file_name)
}

fn infer_family_label(repo: Option<&str>, file_name: &str) -> Option<&'static str> {
    let combined = format!(
        "{} {}",
        repo.unwrap_or_default().to_ascii_lowercase(),
        file_name.to_ascii_lowercase()
    );

    if combined.contains("glm-ocr") {
        Some("GLM-OCR")
    } else if combined.contains("qwen2-vl-ocr") {
        Some("Qwen2-VL OCR")
    } else if combined.contains("qwen2.5-vl")
        || combined.contains("qwen2-vl")
        || (combined.contains("qwen") && combined.contains("-vl"))
    {
        Some("Qwen-VL")
    } else if combined.contains("llava") {
        Some("LLaVA")
    } else if combined.contains("vision") {
        Some("Vision GGUF")
    } else {
        None
    }
}

fn infer_requires_mmproj(repo: Option<&str>, file_name: &str) -> bool {
    let combined = format!(
        "{} {}",
        repo.unwrap_or_default().to_ascii_lowercase(),
        file_name.to_ascii_lowercase()
    );

    combined.contains("glm-ocr")
        || combined.contains("qwen2-vl-ocr")
        || combined.contains("qwen2.5-vl")
        || combined.contains("qwen2-vl")
        || (combined.contains("qwen") && combined.contains("-vl"))
        || combined.contains("llava")
        || combined.contains("vision")
}

fn discover_local_models() -> Result<Vec<DiscoveredLocalModel>> {
    let mut discovered = Vec::new();
    let mut seen = HashSet::new();

    for dir in model_dir_candidates() {
        if !dir.exists() {
            continue;
        }

        let manifest = read_install_manifest(&dir);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            let lowered = file_name.to_ascii_lowercase();
            if !lowered.ends_with(".gguf") || lowered.contains("mmproj") {
                continue;
            }

            if !seen.insert(file_name.to_ascii_lowercase()) {
                continue;
            }

            let inspected =
                inspect_model_candidate(&path, find_manifest_record(&manifest, file_name));
            discovered.push(DiscoveredLocalModel {
                path: path.clone(),
                info: build_local_model_info(&path, inspected),
            });
        }
    }

    Ok(discovered)
}

fn resolve_selected_model(settings: &Settings) -> Result<DiscoveredLocalModel> {
    if let Some(model_file) = settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        let discovered = discover_local_models()?;
        if let Some(model) = discovered
            .into_iter()
            .find(|candidate| candidate.info.file_name.eq_ignore_ascii_case(&model_file))
        {
            return Ok(model);
        }

        for dir in model_dir_candidates() {
            let candidate = dir.join(&model_file);
            if !candidate.exists() {
                continue;
            }

            let manifest = read_install_manifest(candidate.parent().unwrap_or(&dir));
            let inspected =
                inspect_model_candidate(&candidate, find_manifest_record(&manifest, &model_file));
            return Ok(DiscoveredLocalModel {
                path: candidate.clone(),
                info: build_local_model_info(&candidate, inspected),
            });
        }

        return Err(PipelineError::InvalidInput(format!(
            "configured model not found: {}",
            model_file
        )));
    }

    let Some(profile_id) = settings
        .model_profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Err(PipelineError::InvalidInput(
            "no explicit model selected".into(),
        ));
    };

    let profile = profile_by_id(profile_id).ok_or_else(|| {
        PipelineError::InvalidInput(format!("unknown model profile: {}", profile_id))
    })?;

    let mut discovered = discover_local_models()?;
    discovered.sort_by_key(|candidate| auto_model_sort_key(candidate));

    if let Some(model) = discovered.iter().find(|candidate| {
        candidate.info.profile_id.as_deref() == Some(profile.id) && candidate.info.runtime_ready
    }) {
        return Ok(model.clone());
    }

    if let Some(model) = discovered
        .iter()
        .find(|candidate| candidate.info.profile_id.as_deref() == Some(profile.id))
    {
        return Err(PipelineError::InvalidInput(format!(
            "{} is installed, but its mmproj companion is missing",
            model.info.label
        )));
    }

    Err(PipelineError::InvalidInput(format!(
        "{} is not downloaded yet",
        profile.label
    )))
}

fn resolve_active_auto_model() -> Result<DiscoveredLocalModel> {
    let discovered = discover_local_models()?;
    let mut auto_selectable: Vec<DiscoveredLocalModel> = discovered
        .into_iter()
        .filter(|candidate| candidate.info.auto_selectable && candidate.info.runtime_ready)
        .collect();

    auto_selectable.sort_by_key(|candidate| auto_model_sort_key(candidate));
    auto_selectable.into_iter().next().ok_or_else(|| {
        PipelineError::InvalidInput("no supported OCR model found in the models directory".into())
    })
}

pub fn inspect_model_path(model_path: &Path) -> Option<LocalModelInfo> {
    let parent = model_path.parent()?;
    let file_name = model_path.file_name()?.to_string_lossy().into_owned();
    let manifest = read_install_manifest(parent);
    let inspected =
        inspect_model_candidate(model_path, find_manifest_record(&manifest, &file_name));
    Some(build_local_model_info(model_path, inspected))
}

pub fn resolve_model_mmproj_path(model_path: &Path) -> Option<PathBuf> {
    let model_name = model_path.file_name()?.to_string_lossy().into_owned();
    let parent = model_path.parent()?;
    let manifest = read_install_manifest(parent);
    let manifest_record = find_manifest_record(&manifest, &model_name);

    if let Some(record) = manifest_record {
        if let Some(mmproj_file) = &record.mmproj_file {
            for dir in mmproj_search_dirs(model_path) {
                let candidate = dir.join(mmproj_file);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    let inspected = inspect_model_candidate(model_path, manifest_record);
    if !inspected.requires_mmproj {
        return None;
    }

    let mut found = Vec::new();
    for dir in mmproj_search_dirs(model_path) {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let lowered = name.to_ascii_lowercase();
            if lowered.ends_with(".gguf") && lowered.contains("mmproj") {
                found.push(path);
            }
        }
    }

    if found.is_empty() {
        return None;
    }

    found.sort_by_key(|path| mmproj_sort_key(path, &inspected));
    found.into_iter().next()
}

pub async fn download_model(app: &tauri::AppHandle, input: &str) -> Result<DownloadResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PipelineError::InvalidInput("model name is required".into()));
    }

    let client = reqwest::Client::builder()
        .user_agent("VisiTexta/1.0")
        .build()
        .map_err(|e| PipelineError::Other(e.into()))?;

    let plan = resolve_download_plan(&client, trimmed).await?;
    let models_dir = resolve_models_dir(true)?;
    let target_path = models_dir.join(&plan.file_name);
    let require_checksum = plan.profile_id.is_some();
    let main_metadata = remote_file_metadata_for(&plan.info, &plan.file_name)?;

    download_file_with_progress(
        app,
        &client,
        &plan.repo,
        &plan.file_name,
        &target_path,
        &main_metadata,
        require_checksum,
    )
    .await?;

    let mut downloaded_mmproj = None;
    if plan.inspected.requires_mmproj {
        let mmproj_file = select_mmproj_file_from_info(&plan.info).ok_or_else(|| {
            PipelineError::InvalidInput(
                "this model requires a mmproj companion file, but none was found in the repo"
                    .into(),
            )
        })?;

        let mmproj_target = models_dir.join(&mmproj_file);
        let mmproj_metadata = remote_file_metadata_for(&plan.info, &mmproj_file)?;
        download_file_with_progress(
            app,
            &client,
            &plan.repo,
            &mmproj_file,
            &mmproj_target,
            &mmproj_metadata,
            require_checksum,
        )
        .await?;
        downloaded_mmproj = Some(mmproj_file);
    }

    if let Err(err) = upsert_install_record(
        &models_dir,
        InstalledModelRecord {
            file_name: plan.file_name.clone(),
            repo: plan.repo.clone(),
            profile_id: plan.profile_id.clone(),
            family: Some(plan.inspected.family.clone()),
            label: Some(plan.inspected.label.clone()),
            requires_mmproj: plan.inspected.requires_mmproj,
            notes: plan.inspected.notes.clone(),
            support_tier: plan.inspected.support_tier,
            source: plan.inspected.source,
            mmproj_file: downloaded_mmproj,
        },
    ) {
        log::warn!("failed to update model manifest: {err}");
    }

    Ok(DownloadResult {
        repo: plan.repo,
        file_name: plan.file_name,
        file_path: target_path.to_string_lossy().into_owned(),
        profile_id: plan.profile_id,
    })
}

fn emit_download(
    app: &tauri::AppHandle,
    repo: &str,
    file_name: &str,
    downloaded: u64,
    total: Option<u64>,
    progress: f32,
    status: &str,
    message: Option<String>,
) {
    let payload = ModelDownloadEvent {
        repo: repo.to_string(),
        file_name: file_name.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total,
        progress,
        status: status.to_string(),
        message,
    };
    let _ = app.emit("model-download-progress", payload);
}

async fn resolve_download_plan(client: &reqwest::Client, input: &str) -> Result<DownloadPlan> {
    if let Some(profile) = profile_by_id(input) {
        let info = fetch_model_info(client, profile.repo).await?;
        validate_main_file(&info, profile.default_file)?;
        return Ok(DownloadPlan {
            repo: profile.repo.to_string(),
            file_name: profile.default_file.to_string(),
            profile_id: Some(profile.id.to_string()),
            inspected: profile_inspection(
                profile,
                Some(profile.repo.to_string()),
                None,
                ModelInstallSource::Registry,
            ),
            info,
        });
    }

    let (repo, file_hint) = parse_model_input(input)?;
    if let Some(profile) = profile_by_repo(&repo) {
        let info = fetch_model_info(client, &repo).await?;
        let file_name = file_hint.unwrap_or_else(|| profile.default_file.to_string());
        if !profile.file_markers.is_empty()
            && !profile
                .file_markers
                .iter()
                .any(|marker| file_name.to_ascii_lowercase().contains(marker))
        {
            return Err(PipelineError::InvalidInput(format!(
                "{file_name} does not belong to the curated {} profile",
                profile.label
            )));
        }
        validate_main_file(&info, &file_name)?;
        return Ok(DownloadPlan {
            repo: repo.clone(),
            file_name,
            profile_id: Some(profile.id.to_string()),
            inspected: profile_inspection(
                profile,
                Some(repo.clone()),
                None,
                ModelInstallSource::Registry,
            ),
            info,
        });
    }

    let file_name = file_hint.ok_or_else(|| {
        PipelineError::InvalidInput(
            "experimental custom downloads must use owner/repo/file.gguf. Only curated profiles can omit the file name."
                .into(),
        )
    })?;
    let info = fetch_model_info(client, &repo).await?;
    validate_main_file(&info, &file_name)?;

    Ok(DownloadPlan {
        repo: repo.clone(),
        file_name: file_name.clone(),
        profile_id: None,
        inspected: experimental_inspection(Some(&repo), &file_name),
        info,
    })
}

fn parse_model_input(input: &str) -> Result<(String, Option<String>)> {
    let normalized = normalize_model_locator(input)?;
    if normalized.ends_with(".gguf") {
        if let Some((repo, file)) = normalized.rsplit_once('/') {
            if repo.is_empty() || file.is_empty() {
                return Err(PipelineError::InvalidInput("invalid model input".into()));
            }
            return Ok((repo.to_string(), Some(file.to_string())));
        }
        return Err(PipelineError::InvalidInput(
            "model input must include repo and file".into(),
        ));
    }

    Ok((normalized, None))
}

fn normalize_model_locator(input: &str) -> Result<String> {
    let mut value = input
        .trim()
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('/')
        .to_string();

    if value.is_empty() {
        return Err(PipelineError::InvalidInput("model name is required".into()));
    }

    if let Some(rest) = value.strip_prefix("https://") {
        value = rest.to_string();
    } else if let Some(rest) = value.strip_prefix("http://") {
        value = rest.to_string();
    }

    if let Some(path) = extract_hf_path(&value) {
        value = path;
    }

    if let Some(path) = parse_repo_or_file_path(&value) {
        return Ok(path);
    }

    Err(PipelineError::InvalidInput(
        "model input must look like a curated profile id, owner/repo, or owner/repo/file.gguf"
            .into(),
    ))
}

fn extract_hf_path(value: &str) -> Option<String> {
    let mut parts = value.splitn(2, '/');
    let host = parts.next()?.to_ascii_lowercase();
    let path = parts.next()?.trim_matches('/');

    if host == "huggingface.co" || host == "www.huggingface.co" || host == "hf.co" {
        return Some(path.to_string());
    }

    None
}

fn parse_repo_or_file_path(path: &str) -> Option<String> {
    let mut normalized = path.trim_matches('/');
    if let Some(rest) = normalized.strip_prefix("models/") {
        normalized = rest;
    }

    let segments: Vec<&str> = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.len() < 2 {
        return None;
    }

    let repo = format!("{}/{}", segments[0], segments[1]);

    if segments.len() >= 5 && (segments[2] == "blob" || segments[2] == "resolve") {
        let file = segments.last()?.to_string();
        return Some(format!("{repo}/{file}"));
    }

    if segments.len() == 3 && segments[2].to_ascii_lowercase().ends_with(".gguf") {
        return Some(format!("{repo}/{}", segments[2]));
    }

    Some(repo)
}

fn sanitize_file_name(file_name: &str) -> Result<String> {
    let base = Path::new(file_name)
        .file_name()
        .ok_or_else(|| PipelineError::InvalidInput("invalid file name".into()))?;
    let base = base.to_string_lossy().into_owned();
    if base.is_empty() {
        return Err(PipelineError::InvalidInput("invalid file name".into()));
    }
    Ok(base)
}

async fn fetch_model_info(client: &reqwest::Client, repo: &str) -> Result<HfModelInfo> {
    let url = format!("{}/{}/tree/main", HF_API_BASE, repo);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    if !response.status().is_success() {
        return Err(PipelineError::InvalidInput(format!(
            "unable to read model files: {}",
            response.status()
        )));
    }

    let files = response
        .json::<Vec<HfSibling>>()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    Ok(HfModelInfo { files })
}

fn validate_main_file(info: &HfModelInfo, file_name: &str) -> Result<()> {
    let sanitized = sanitize_file_name(file_name)?;
    let lowered = sanitized.to_ascii_lowercase();

    if !lowered.ends_with(".gguf") || lowered.contains("mmproj") {
        return Err(PipelineError::InvalidInput(
            "model download must target a main .gguf file, not a companion mmproj file".into(),
        ));
    }

    let exists = info
        .files
        .iter()
        .any(|entry| entry.rfilename.eq_ignore_ascii_case(&sanitized));

    if exists {
        return Ok(());
    }

    Err(PipelineError::InvalidInput(format!(
        "the requested model file was not found in the repo: {}",
        sanitized
    )))
}

fn select_mmproj_file_from_info(info: &HfModelInfo) -> Option<String> {
    let mut mmproj: Vec<HfSibling> = info
        .files
        .clone()
        .into_iter()
        .filter(|entry| {
            let lowered = entry.rfilename.to_ascii_lowercase();
            lowered.ends_with(".gguf") && lowered.contains("mmproj")
        })
        .collect();

    mmproj.sort_by_key(|entry| {
        let lowered = entry.rfilename.to_ascii_lowercase();
        let precision_score = if lowered.contains("f16") {
            0
        } else if lowered.contains("bf16") {
            1
        } else if lowered.contains("f32") {
            2
        } else {
            3
        };
        let size = entry.size.unwrap_or(u64::MAX);
        (precision_score, size)
    });

    mmproj.first().map(|entry| entry.rfilename.clone())
}

fn remote_file_metadata_for(info: &HfModelInfo, file_name: &str) -> Result<RemoteFileMetadata> {
    let sanitized = sanitize_file_name(file_name)?;
    let entry = info
        .files
        .iter()
        .find(|candidate| candidate.rfilename.eq_ignore_ascii_case(&sanitized))
        .ok_or_else(|| {
            PipelineError::InvalidInput(format!(
                "the requested model file was not found in the repo: {}",
                sanitized
            ))
        })?;

    Ok(RemoteFileMetadata {
        total_bytes: entry
            .size
            .or_else(|| entry.lfs.as_ref().map(|value| value.size)),
        checksum_sha256: entry.lfs.as_ref().map(|value| value.oid.clone()),
    })
}

async fn download_file_with_progress(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    repo: &str,
    file_name: &str,
    target_path: &Path,
    metadata: &RemoteFileMetadata,
    checksum_required: bool,
) -> Result<()> {
    let url = format!("{}/{}/resolve/main/{}", HF_RESOLVE_BASE, repo, file_name);

    emit_download(
        app,
        repo,
        file_name,
        0,
        metadata.total_bytes,
        0.0,
        "starting",
        Some("checking existing downloads".into()),
    );

    if target_path.exists() {
        match verify_file_checksum(target_path, &metadata, checksum_required).await {
            Ok(checksum_verified) => {
                emit_download(
                    app,
                    repo,
                    file_name,
                    metadata.total_bytes.unwrap_or_else(|| {
                        fs::metadata(target_path)
                            .map(|meta| meta.len())
                            .unwrap_or(0)
                    }),
                    metadata.total_bytes,
                    1.0,
                    "done",
                    Some(if checksum_verified {
                        "using existing verified download".into()
                    } else {
                        "using existing download".into()
                    }),
                );
                return Ok(());
            }
            Err(err) => {
                log::warn!(
                    "existing download at {} did not verify: {err}",
                    target_path.to_string_lossy()
                );
                let failed_target = recovery_file_path(target_path, "corrupt");
                let _ = tokio::fs::rename(target_path, &failed_target).await;
            }
        }
    }

    let temp_path = part_file_path(target_path);
    let mut resume_from = match tokio::fs::metadata(&temp_path).await {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };

    if let Some(total_bytes) = metadata.total_bytes {
        if resume_from > total_bytes {
            let _ = tokio::fs::remove_file(&temp_path).await;
            resume_from = 0;
        } else if resume_from == total_bytes && total_bytes > 0 {
            match verify_file_checksum(&temp_path, &metadata, checksum_required).await {
                Ok(checksum_verified) => {
                    if target_path.exists() {
                        let _ = tokio::fs::remove_file(target_path).await;
                    }
                    tokio::fs::rename(&temp_path, target_path).await?;
                    emit_download(
                        app,
                        repo,
                        file_name,
                        total_bytes,
                        metadata.total_bytes,
                        1.0,
                        "done",
                        Some(if checksum_verified {
                            "resumed partial download and verified it".into()
                        } else {
                            "resumed partial download".into()
                        }),
                    );
                    return Ok(());
                }
                Err(err) => {
                    log::warn!(
                        "stale partial download at {} did not verify: {err}",
                        temp_path.to_string_lossy()
                    );
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    resume_from = 0;
                }
            }
        }
    }

    let mut request = client.get(&url);
    if resume_from > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
    }

    let response = request
        .send()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    let response = if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
        let _ = tokio::fs::remove_file(&temp_path).await;
        resume_from = 0;
        client
            .get(&url)
            .send()
            .await
            .map_err(|e| PipelineError::Other(e.into()))?
    } else {
        response
    };

    if !(response.status().is_success()
        || (resume_from > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT))
    {
        emit_download(
            app,
            repo,
            file_name,
            resume_from,
            metadata.total_bytes,
            0.0,
            "error",
            Some(format!("download failed: {}", response.status())),
        );
        return Err(PipelineError::InvalidInput(format!(
            "download failed: {}",
            response.status()
        )));
    }

    let total = metadata
        .total_bytes
        .or_else(|| response.content_length().map(|value| value + resume_from));
    let mut file = if resume_from > 0 {
        OpenOptions::new().append(true).open(&temp_path).await?
    } else {
        tokio::fs::File::create(&temp_path).await?
    };
    let mut downloaded = resume_from;
    let mut last_emit_bytes = resume_from;
    let mut last_emit_progress = total
        .map(|len| (resume_from as f64 / len.max(1) as f64) as f32)
        .unwrap_or(0.0);
    let mut stream = response;

    emit_download(
        app,
        repo,
        file_name,
        downloaded,
        total,
        last_emit_progress.min(1.0),
        "downloading",
        Some(if resume_from > 0 {
            "resuming previous partial download".into()
        } else {
            "downloading model".into()
        }),
    );

    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?
    {
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        let progress = total
            .map(|len| (downloaded as f64 / len.max(1) as f64) as f32)
            .unwrap_or(0.0);

        let should_emit = total
            .map(|_| (progress - last_emit_progress) >= 0.02)
            .unwrap_or(downloaded.saturating_sub(last_emit_bytes) >= 1_000_000);

        if should_emit {
            last_emit_bytes = downloaded;
            last_emit_progress = progress;
            emit_download(
                app,
                repo,
                file_name,
                downloaded,
                total,
                progress.min(1.0),
                "downloading",
                Some(if resume_from > 0 {
                    "resuming previous partial download".into()
                } else {
                    "downloading model".into()
                }),
            );
        }
    }

    file.flush().await?;
    emit_download(
        app,
        repo,
        file_name,
        downloaded,
        total,
        1.0,
        "verifying",
        Some("verifying download checksum".into()),
    );

    let checksum_verified =
        match verify_file_checksum(&temp_path, &metadata, checksum_required).await {
            Ok(value) => value,
            Err(err) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                emit_download(
                    app,
                    repo,
                    file_name,
                    downloaded,
                    total,
                    0.0,
                    "error",
                    Some(err.to_string()),
                );
                return Err(err);
            }
        };

    if target_path.exists() {
        let _ = tokio::fs::remove_file(target_path).await;
    }
    tokio::fs::rename(&temp_path, target_path).await?;

    emit_download(
        app,
        repo,
        file_name,
        downloaded,
        total,
        1.0,
        "done",
        Some(if checksum_verified {
            "download complete and verified".into()
        } else {
            "download complete".into()
        }),
    );

    Ok(())
}

fn recovery_file_path(target_path: &Path, suffix: &str) -> PathBuf {
    let stem = target_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = target_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");

    let file_name = if extension.is_empty() {
        format!("{stem}.{suffix}-{timestamp}")
    } else {
        format!("{stem}.{suffix}-{timestamp}.{extension}")
    };

    target_path.with_file_name(file_name)
}

async fn verify_file_checksum(
    path: &Path,
    metadata: &RemoteFileMetadata,
    checksum_required: bool,
) -> Result<bool> {
    if let Some(expected_size) = metadata.total_bytes {
        let actual_size = tokio::fs::metadata(path).await?.len();
        if actual_size != expected_size {
            return Err(PipelineError::InvalidInput(format!(
                "downloaded file size mismatch (expected {expected_size} bytes, got {actual_size})"
            )));
        }
    }

    let Some(expected) = metadata.checksum_sha256.as_deref() else {
        if checksum_required {
            return Err(PipelineError::InvalidInput(
                "checksum verification is required for curated downloads, but no remote checksum was provided"
                    .into(),
            ));
        }
        return Ok(false);
    };

    let path = path.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || compute_sha256(&path))
        .await
        .map_err(|err| PipelineError::InvalidInput(format!("checksum task failed: {err}")))??;

    if actual.eq_ignore_ascii_case(expected) {
        return Ok(true);
    }

    Err(PipelineError::InvalidInput(
        "download checksum verification failed; the file will be downloaded again".into(),
    ))
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn part_file_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    target_path.with_file_name(format!("{file_name}.{PART_EXTENSION}"))
}

fn model_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(primary) = storage::models_dir() {
        candidates.push(primary);
    }
    candidates.extend(legacy_model_dir_candidates());

    let mut seen = HashSet::new();
    candidates.retain(|dir| seen.insert(dir.clone()));
    candidates
}

fn legacy_model_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models"));
        candidates.push(cwd.join("resources").join("models"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("models"));
            candidates.push(dir.join("resources").join("models"));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("models"));
                candidates.push(parent.join("resources").join("models"));
            }
        }
    }

    let mut seen = HashSet::new();
    candidates.retain(|dir| seen.insert(dir.clone()));
    candidates
}

pub fn ensure_models_dir() -> Result<PathBuf> {
    resolve_models_dir(true)
}

fn resolve_models_dir(create: bool) -> Result<PathBuf> {
    let target = storage::models_dir()?;

    if create {
        fs::create_dir_all(&target)?;
    }

    Ok(target)
}

fn mmproj_search_dirs(model_path: &Path) -> Vec<PathBuf> {
    let mut search_dirs = Vec::new();
    if let Some(parent) = model_path.parent() {
        search_dirs.push(parent.to_path_buf());
    }
    search_dirs.extend(model_dir_candidates());

    let mut seen = HashSet::new();
    search_dirs.retain(|dir| seen.insert(dir.clone()));
    search_dirs
}

fn mmproj_sort_key(path: &Path, inspected: &InspectedModel) -> (u8, u8, String) {
    let lowered = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let family_score = if lowered.contains(&inspected.family.to_ascii_lowercase())
        || inspected
            .profile_id
            .as_deref()
            .and_then(profile_by_id)
            .map(|profile| {
                profile
                    .file_markers
                    .iter()
                    .any(|marker| lowered.contains(marker))
            })
            .unwrap_or(false)
    {
        0
    } else {
        1
    };

    let precision_score = if lowered.contains("f16") {
        0
    } else if lowered.contains("bf16") {
        1
    } else if lowered.contains("f32") {
        2
    } else {
        3
    };

    (family_score, precision_score, lowered)
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join(MODEL_MANIFEST_FILE)
}

fn read_install_manifest(dir: &Path) -> InstallManifest {
    let path = manifest_path(dir);
    let Ok(bytes) = fs::read(path) else {
        return InstallManifest::default();
    };

    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn write_install_manifest(dir: &Path, manifest: &InstallManifest) -> Result<()> {
    let path = manifest_path(dir);
    let bytes = serde_json::to_vec_pretty(manifest)?;
    storage::atomic_write(&path, &bytes)?;
    Ok(())
}

fn upsert_install_record(dir: &Path, record: InstalledModelRecord) -> Result<()> {
    let mut manifest = read_install_manifest(dir);
    if let Some(existing) = manifest
        .installs
        .iter_mut()
        .find(|item| item.file_name.eq_ignore_ascii_case(&record.file_name))
    {
        *existing = record;
    } else {
        manifest.installs.push(record);
    }
    write_install_manifest(dir, &manifest)
}

fn find_manifest_record<'a>(
    manifest: &'a InstallManifest,
    file_name: &str,
) -> Option<&'a InstalledModelRecord> {
    manifest
        .installs
        .iter()
        .find(|record| record.file_name.eq_ignore_ascii_case(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_glm() {
        assert_eq!(recommended_profile_id(), "glm-ocr");
        assert_eq!(recommended_model_file(), "GLM-OCR.Q4_K_M.gguf");
    }

    #[test]
    fn parse_curated_blob_url() {
        let parsed = parse_model_input(
            "https://huggingface.co/mradermacher/GLM-OCR-GGUF/blob/main/GLM-OCR.Q4_K_M.gguf",
        )
        .unwrap();
        assert_eq!(
            parsed,
            (
                "mradermacher/GLM-OCR-GGUF".to_string(),
                Some("GLM-OCR.Q4_K_M.gguf".to_string())
            )
        );
    }

    #[test]
    fn profile_match_uses_curated_file_markers() {
        let profile = profile_matching_file_name("Qwen2-VL-OCR-2B-Instruct.Q5_K_M.gguf").unwrap();
        assert_eq!(profile.id, "qwen2-vl-ocr-2b");
    }

    #[test]
    fn qwen25_profile_match_uses_curated_file_markers() {
        let profile = profile_matching_file_name("Qwen2.5-VL-3B-Instruct.Q4_K_S.gguf").unwrap();
        assert_eq!(profile.id, "qwen2.5-vl-3b");
    }

    #[test]
    fn parse_unknown_repo_without_file_keeps_repo_only() {
        let parsed = parse_model_input("someone/custom-repo").unwrap();
        assert_eq!(parsed, ("someone/custom-repo".to_string(), None));
    }

    #[test]
    fn experimental_inspection_stays_non_auto_selectable() {
        let inspected = experimental_inspection(Some("someone/custom-repo"), "custom.gguf");
        assert_eq!(inspected.support_tier, ModelSupportTier::Experimental);
        assert!(!inspected.auto_selectable);
    }
}
