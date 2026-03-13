use crate::errors::{PipelineError, Result};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

const DEFAULT_MODEL: &str = "Qwen_Qwen3.5-0.8B-Q4_K_M.gguf";
const HF_API_BASE: &str = "https://huggingface.co/api/models";
const HF_RESOLVE_BASE: &str = "https://huggingface.co";

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
}

#[derive(Debug, Deserialize)]
struct HfModelInfo {
    siblings: Option<Vec<HfSibling>>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    size: Option<u64>,
}

pub fn list_models() -> Result<Vec<String>> {
    let dir = match resolve_models_dir(false) {
        Ok(path) => path,
        Err(_) => return Ok(Vec::new()),
    };
    let mut models = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                .unwrap_or(false)
            {
                models.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    models.sort();
    Ok(models)
}

pub fn model_exists(settings: &Settings) -> bool {
    if let Some(model_file) = settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        for dir in model_dir_candidates() {
            let candidate = dir.join(&model_file);
            if candidate.exists() {
                return true;
            }
        }
        return false;
    }

    for dir in model_dir_candidates() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
    }
    false
}


pub async fn download_model(app: &tauri::AppHandle, input: &str) -> Result<DownloadResult> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PipelineError::InvalidInput("model name is required".into()));
    }

    let (repo, file_hint) = parse_model_input(trimmed)?;
    emit_download(
        app,
        &repo,
        file_hint.as_deref().unwrap_or_default(),
        0,
        None,
        0.0,
        "starting",
        None,
    );

    let client = reqwest::Client::builder()
        .user_agent("VisiTexta/1.0")
        .build()
        .map_err(|e| PipelineError::Other(e.into()))?;

    let file_name = match file_hint {
        Some(file) => file,
        None => select_gguf_file(&client, &repo).await?,
    };

    let file_name = sanitize_file_name(&file_name)?;
    let models_dir = resolve_models_dir(true)?;
    let target_path = models_dir.join(&file_name);
    if target_path.exists() {
        emit_download(
            app,
            &repo,
            &file_name,
            0,
            None,
            1.0,
            "done",
            Some("model already exists".into()),
        );
        return Ok(DownloadResult {
            repo,
            file_name,
            file_path: target_path.to_string_lossy().into_owned(),
        });
    }

    let url = format!("{}/{}/resolve/main/{}", HF_RESOLVE_BASE, repo, file_name);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    if !response.status().is_success() {
        emit_download(
            app,
            &repo,
            &file_name,
            0,
            None,
            0.0,
            "error",
            Some(format!("download failed: {}", response.status())),
        );
        return Err(PipelineError::InvalidInput(format!(
            "download failed: {}",
            response.status()
        )));
    }

    let total = response.content_length();
    let temp_path = target_path.with_extension("part");
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut downloaded: u64 = 0;
    let mut last_emit_bytes: u64 = 0;
    let mut last_emit_progress: f32 = 0.0;
    let mut stream = response;

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
                &repo,
                &file_name,
                downloaded,
                total,
                progress.min(1.0),
                "downloading",
                None,
            );
        }
    }

    file.flush().await?;
    tokio::fs::rename(&temp_path, &target_path).await?;

    emit_download(
        app,
        &repo,
        &file_name,
        downloaded,
        total,
        1.0,
        "done",
        Some("download complete".into()),
    );

    Ok(DownloadResult {
        repo,
        file_name,
        file_path: target_path.to_string_lossy().into_owned(),
    })
}

pub fn active_model_file(settings: &Settings) -> String {
    settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
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

fn parse_model_input(input: &str) -> Result<(String, Option<String>)> {
    let trimmed = input.trim();
    if trimmed.ends_with(".gguf") {
        if let Some((repo, file)) = trimmed.rsplit_once('/') {
            if repo.is_empty() || file.is_empty() {
                return Err(PipelineError::InvalidInput("invalid model input".into()));
            }
            return Ok((repo.to_string(), Some(file.to_string())));
        }
        return Err(PipelineError::InvalidInput(
            "model input must include repo and file".into(),
        ));
    }
    Ok((trimmed.to_string(), None))
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

async fn select_gguf_file(client: &reqwest::Client, repo: &str) -> Result<String> {
    let url = format!("{}/{}", HF_API_BASE, repo);
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
    let info: HfModelInfo = response
        .json()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;
    let mut ggufs: Vec<HfSibling> = info
        .siblings
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
        .collect();

    if ggufs.is_empty() {
        return Err(PipelineError::InvalidInput(
            "no .gguf files found in this repo".into(),
        ));
    }

    ggufs.sort_by_key(|entry| {
        let name = entry.rfilename.as_str();
        let score = if name.contains("Q4_K_M") {
            0
        } else if name.contains("Q4_K") {
            1
        } else if name.contains("Q5_K") {
            2
        } else if name.contains("Q6_K") {
            3
        } else {
            4
        };
        let size = entry.size.unwrap_or(u64::MAX);
        (score, size)
    });

    Ok(ggufs[0].rfilename.clone())
}

fn model_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("models"));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("models"));
            }
        }
    }
    candidates
}

fn resolve_models_dir(create: bool) -> Result<PathBuf> {
    for candidate in model_dir_candidates() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if create {
        let target = model_dir_candidates()
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("models"));
        std::fs::create_dir_all(&target)?;
        return Ok(target);
    }
    Err(PipelineError::InvalidInput(
        "models directory not found".into(),
    ))
}


