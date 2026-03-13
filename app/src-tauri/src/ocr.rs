use crate::errors::{PipelineError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct OcrEngine {
    tesseract_path: PathBuf,
    tessdata_path: PathBuf,
    lang: String,
}

impl OcrEngine {
    pub fn new(lang: &str) -> Result<Self> {
        let tess = resolve_tesseract_exe()
            .ok_or_else(|| PipelineError::Ocr("tesseract executable not found".into()))?;
        let tessdata = resolve_tessdata_path()
            .ok_or_else(|| PipelineError::Ocr("tessdata folder not found".into()))?;
        Ok(Self {
            tesseract_path: tess,
            tessdata_path: tessdata,
            lang: lang.to_string(),
        })
    }

    pub fn recognize(&self, image_path: &Path) -> Result<String> {
        let prefix = self
            .tessdata_path
            .parent()
            .unwrap_or(&self.tessdata_path);

        // Tesseract expects options before input/output args.
        let output = Command::new(&self.tesseract_path)
            .arg("--tessdata-dir")
            .arg(&self.tessdata_path)
            .arg("-l")
            .arg(&self.lang)
            .arg(image_path)
            .arg("stdout")
            .env("TESSDATA_PREFIX", prefix)
            .output()
            .map_err(|e| PipelineError::Ocr(format!("spawn failed: {e}")))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| PipelineError::Ocr(format!("utf8 decode failed: {e}")))
        } else {
            Err(PipelineError::Ocr(format!(
                "tesseract exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }
}

fn resolve_tesseract_exe() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let candidates = [
        exe_dir.as_ref().map(|p| p.join("bin").join("tesseract.exe")),
        exe_dir.as_ref().map(|p| p.join("tesseract.exe")),
        Some(PathBuf::from("src-tauri").join("bin").join("tesseract.exe")),
        Some(PathBuf::from("tesseract.exe")),
        Some(PathBuf::from("C:\\Program Files\\Tesseract-OCR\\tesseract.exe")),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn resolve_tessdata_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let candidates = [
        exe_dir
            .as_ref()
            .map(|p| p.join("resources").join("tessdata")),
        exe_dir.as_ref().map(|p| p.join("tessdata")),
        exe_dir.as_ref().map(|p| p.join("bin").join("tessdata")),
        Some(PathBuf::from("src-tauri").join("resources").join("tessdata")),
        Some(PathBuf::from("models").join("tessdata")),
        Some(PathBuf::from("C:\\Program Files\\Tesseract-OCR\\tessdata")),
    ];

    for candidate in candidates.iter().flatten() {
        if candidate.exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}
