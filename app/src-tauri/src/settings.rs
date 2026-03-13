use crate::errors::{PipelineError, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const APP_DIR: &str = "VisiTexta";
const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub threads: u16,
    pub dpi: u16,
    pub chunk_size: usize,
    pub auto_open: bool,
    pub theme: Option<String>,
    pub model_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            threads: num_cpus::get().saturating_sub(1).max(1) as u16,
            dpi: 300,
            chunk_size: 3000,
            auto_open: false,
            theme: None,
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_vec_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }
}

fn settings_path() -> Result<PathBuf> {
    let mut base = dirs::config_dir().ok_or_else(|| {
        PipelineError::InvalidInput("could not resolve config directory".into())
    })?;
    base.push(APP_DIR);
    base.push(SETTINGS_FILE);
    Ok(base)
}
