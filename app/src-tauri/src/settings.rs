use crate::defaults;
use crate::errors::Result;
use crate::runtime::{self, RuntimeProfile};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub threads: u16,
    pub dpi: u16,
    pub chunk_size: usize,
    pub auto_open: bool,
    pub runtime_profile: RuntimeProfile,
    pub max_ocr_dimension: u32,
    pub lazy_preview_thumbnails: bool,
    pub disable_rich_preview_for_large_jobs: bool,
    pub large_job_page_threshold: usize,
    pub theme: Option<String>,
    pub model_profile_id: Option<String>,
    pub model_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            threads: num_cpus::get().saturating_sub(1).max(1) as u16,
            dpi: 300,
            chunk_size: 3000,
            auto_open: false,
            runtime_profile: runtime::default_runtime_profile(),
            max_ocr_dimension: 1600,
            lazy_preview_thumbnails: false,
            disable_rich_preview_for_large_jobs: false,
            large_job_page_threshold: 24,
            theme: Some(defaults::default_theme_id().to_string()),
            model_profile_id: None,
            model_file: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = match settings_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };

        if let Ok(bytes) = fs::read(&path) {
            if let Ok(existing) = serde_json::from_slice::<Settings>(&bytes) {
                return existing;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        let serialized = serde_json::to_vec_pretty(self)?;
        storage::atomic_write(&path, &serialized)?;
        Ok(())
    }
}

fn settings_path() -> Result<PathBuf> {
    Ok(storage::storage_paths()?.settings_path)
}
