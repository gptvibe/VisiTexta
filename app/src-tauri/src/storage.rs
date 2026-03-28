use crate::errors::{PipelineError, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const APP_DIR: &str = "VisiTexta";
const SETTINGS_FILE: &str = "settings.json";
const HISTORY_FILE: &str = "history.json";
const MODELS_DIR: &str = "models";
const TEMP_DIR: &str = "temp";
const PASTED_INPUTS_DIR: &str = "pasted-inputs";
const PORTABLE_DATA_DIR: &str = "portable-data";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageMode {
    Portable,
    Installer,
}

#[derive(Debug, Clone)]
pub struct StoragePaths {
    pub mode: StorageMode,
    pub root_dir: PathBuf,
    pub settings_path: PathBuf,
    pub history_path: PathBuf,
    pub models_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub pasted_inputs_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageInfo {
    pub mode: StorageMode,
    pub root_path: String,
    pub settings_path: String,
    pub history_path: String,
    pub models_path: String,
    pub temp_path: String,
    pub pasted_inputs_path: String,
    pub outputs_description: String,
}

impl StoragePaths {
    fn new(mode: StorageMode, root_dir: PathBuf) -> Self {
        Self {
            mode,
            settings_path: root_dir.join(SETTINGS_FILE),
            history_path: root_dir.join(HISTORY_FILE),
            models_dir: root_dir.join(MODELS_DIR),
            temp_dir: root_dir.join(TEMP_DIR),
            pasted_inputs_dir: root_dir.join(PASTED_INPUTS_DIR),
            root_dir,
        }
    }

    pub fn info(&self) -> StorageInfo {
        StorageInfo {
            mode: self.mode,
            root_path: self.root_dir.to_string_lossy().into_owned(),
            settings_path: self.settings_path.to_string_lossy().into_owned(),
            history_path: self.history_path.to_string_lossy().into_owned(),
            models_path: self.models_dir.to_string_lossy().into_owned(),
            temp_path: self.temp_dir.to_string_lossy().into_owned(),
            pasted_inputs_path: self.pasted_inputs_dir.to_string_lossy().into_owned(),
            outputs_description:
                "OCR outputs are written next to the source file as name.ocr.md, then name (ocr 2).md if needed."
                    .into(),
        }
    }
}

pub fn storage_paths() -> Result<StoragePaths> {
    let exe_dir = current_exe_dir()?;
    resolve_storage_paths(&exe_dir, installer_root_dir()?)
}

pub fn storage_info() -> Result<StorageInfo> {
    Ok(storage_paths()?.info())
}

pub fn ensure_ready() -> Result<StoragePaths> {
    prepare_startup()
}

pub fn settings_path() -> Result<PathBuf> {
    Ok(storage_paths()?.settings_path)
}

pub fn history_path() -> Result<PathBuf> {
    Ok(storage_paths()?.history_path)
}

pub fn models_dir() -> Result<PathBuf> {
    Ok(storage_paths()?.models_dir)
}

pub fn temp_dir() -> Result<PathBuf> {
    Ok(storage_paths()?.temp_dir)
}

pub fn pasted_inputs_dir() -> Result<PathBuf> {
    Ok(storage_paths()?.pasted_inputs_dir)
}

pub fn prepare_startup() -> Result<StoragePaths> {
    let paths = storage_paths()?;

    ensure_dir(&paths.root_dir)?;
    ensure_dir(&paths.models_dir)?;
    ensure_dir(&paths.temp_dir)?;
    ensure_dir(&paths.pasted_inputs_dir)?;

    migrate_small_file_if_missing(&paths.settings_path, &settings_migration_candidates(&paths))?;
    migrate_small_file_if_missing(&paths.history_path, &history_migration_candidates(&paths))?;

    cleanup_temp_root(&paths)?;

    Ok(paths)
}

pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Err(PipelineError::InvalidInput(format!(
            "missing parent directory for {}",
            path.display()
        )));
    };
    ensure_dir(parent)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure_parent_dir(path)?;

    let parent = path.parent().ok_or_else(|| {
        PipelineError::InvalidInput(format!("missing parent directory for {}", path.display()))
    })?;
    let temp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("visitexta"),
        Uuid::new_v4().simple()
    ));

    fs::write(&temp_path, bytes)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temp_path, path)?;
    Ok(())
}

pub fn create_temp_work_dir(prefix: &str) -> Result<tempfile::TempDir> {
    let paths = storage_paths()?;
    ensure_dir(&paths.temp_dir)?;
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&paths.temp_dir)
        .map_err(Into::into)
}

pub fn next_output_markdown_path(source_path: &Path) -> Result<PathBuf> {
    let parent = source_path
        .parent()
        .ok_or_else(|| PipelineError::InvalidInput("missing parent directory".into()))?;
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| PipelineError::InvalidInput("invalid file name".into()))?;

    let first_choice = parent.join(format!("{stem}.ocr.md"));
    if !first_choice.exists() {
        return Ok(first_choice);
    }

    for attempt in 2..10_000 {
        let candidate = parent.join(format!("{stem} (ocr {attempt}).md"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(PipelineError::InvalidInput(
        "could not find a free output file name".into(),
    ))
}

fn cleanup_temp_root(paths: &StoragePaths) -> Result<()> {
    if !paths.temp_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&paths.temp_dir)? {
        let path = entry?.path();
        remove_path(&path)?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn settings_migration_candidates(paths: &StoragePaths) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if matches!(paths.mode, StorageMode::Portable) {
        if let Some(exe_dir) = current_exe_dir_fallible() {
            let legacy_portable = exe_dir.join(SETTINGS_FILE);
            if legacy_portable != paths.settings_path {
                candidates.push(legacy_portable);
            }
        }
    }

    if let Some(legacy_config) = legacy_config_settings_path() {
        if legacy_config != paths.settings_path {
            candidates.push(legacy_config);
        }
    }

    dedupe_paths(candidates)
}

fn history_migration_candidates(paths: &StoragePaths) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if matches!(paths.mode, StorageMode::Portable) {
        if let Some(exe_dir) = current_exe_dir_fallible() {
            let legacy_portable = exe_dir.join(HISTORY_FILE);
            if legacy_portable != paths.history_path {
                candidates.push(legacy_portable);
            }
        }
    }

    dedupe_paths(candidates)
}

fn migrate_small_file_if_missing(target: &Path, candidates: &[PathBuf]) -> Result<()> {
    if target.exists() {
        return Ok(());
    }

    for candidate in candidates {
        if !candidate.exists() || !candidate.is_file() {
            continue;
        }

        let bytes = fs::read(candidate)?;
        atomic_write(target, &bytes)?;
        return Ok(());
    }

    Ok(())
}

fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

fn current_exe_dir() -> Result<PathBuf> {
    current_exe_dir_fallible()
        .ok_or_else(|| PipelineError::InvalidInput("could not resolve executable directory".into()))
}

fn current_exe_dir_fallible() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn resolve_storage_paths(exe_dir: &Path, installer_root: PathBuf) -> Result<StoragePaths> {
    if exe_dir.join(PORTABLE_DATA_DIR).is_dir() {
        return Ok(StoragePaths::new(
            StorageMode::Portable,
            exe_dir.join(PORTABLE_DATA_DIR),
        ));
    }

    if has_legacy_portable_layout(exe_dir) {
        return Ok(StoragePaths::new(
            StorageMode::Portable,
            exe_dir.to_path_buf(),
        ));
    }

    Ok(StoragePaths::new(StorageMode::Installer, installer_root))
}

fn has_legacy_portable_layout(exe_dir: &Path) -> bool {
    exe_dir.join(SETTINGS_FILE).is_file()
        || exe_dir.join(HISTORY_FILE).is_file()
        || exe_dir.join(MODELS_DIR).is_dir()
        || exe_dir.join(TEMP_DIR).is_dir()
        || exe_dir.join(PASTED_INPUTS_DIR).is_dir()
}

fn installer_root_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            PipelineError::InvalidInput("could not resolve %LOCALAPPDATA%".into())
        })?;
        return Ok(PathBuf::from(base).join(APP_DIR));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut path = dirs::data_local_dir().ok_or_else(|| {
            PipelineError::InvalidInput("could not resolve local app data directory".into())
        })?;
        path.push(APP_DIR);
        Ok(path)
    }
}

fn legacy_config_settings_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push(APP_DIR);
    path.push(SETTINGS_FILE);
    Some(path)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for path in paths {
        if seen.insert(path.clone()) {
            deduped.push(path);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_for_test(exe_dir: &Path, installer_root: &Path) -> StoragePaths {
        resolve_storage_paths(exe_dir, installer_root.to_path_buf()).unwrap()
    }

    #[test]
    fn dedicated_portable_folder_wins() {
        let sandbox = tempfile::tempdir().unwrap();
        let exe_dir = sandbox.path().join("portable");
        let installer_root = sandbox.path().join("local");
        fs::create_dir_all(exe_dir.join(PORTABLE_DATA_DIR)).unwrap();

        let paths = resolve_for_test(&exe_dir, &installer_root);
        assert_eq!(paths.mode, StorageMode::Portable);
        assert_eq!(paths.root_dir, exe_dir.join(PORTABLE_DATA_DIR));
        assert_eq!(
            paths.models_dir,
            exe_dir.join(PORTABLE_DATA_DIR).join(MODELS_DIR)
        );
    }

    #[test]
    fn legacy_sidecar_models_trigger_portable_mode() {
        let sandbox = tempfile::tempdir().unwrap();
        let exe_dir = sandbox.path().join("legacy");
        let installer_root = sandbox.path().join("local");
        fs::create_dir_all(exe_dir.join(MODELS_DIR)).unwrap();

        let paths = resolve_for_test(&exe_dir, &installer_root);
        assert_eq!(paths.mode, StorageMode::Portable);
        assert_eq!(paths.root_dir, exe_dir);
    }

    #[test]
    fn installer_mode_uses_local_root_without_portable_signals() {
        let sandbox = tempfile::tempdir().unwrap();
        let exe_dir = sandbox.path().join("installer");
        let installer_root = sandbox.path().join("local");
        fs::create_dir_all(&exe_dir).unwrap();

        let paths = resolve_for_test(&exe_dir, &installer_root);
        assert_eq!(paths.mode, StorageMode::Installer);
        assert_eq!(paths.root_dir, installer_root);
    }

    #[test]
    fn output_names_are_non_destructive() {
        let sandbox = tempfile::tempdir().unwrap();
        let source = sandbox.path().join("scan.pdf");
        fs::write(&source, b"pdf").unwrap();

        let first = next_output_markdown_path(&source).unwrap();
        assert_eq!(first.file_name().unwrap().to_string_lossy(), "scan.ocr.md");

        fs::write(&first, b"one").unwrap();
        let second = next_output_markdown_path(&source).unwrap();
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            "scan (ocr 2).md"
        );
    }
}
