use crate::errors::{PipelineError, Result};
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;

const DEFAULT_MODEL: &str = "GLM-OCR-Q4_K_M.gguf";
const HF_API_BASE: &str = "https://huggingface.co/api/models";
const HF_RESOLVE_BASE: &str = "https://huggingface.co";
const APP_DIR: &str = "VisiTexta";

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

#[derive(Debug, Deserialize, Clone)]
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
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(is_supported_vision_model_name)
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

pub fn has_vision_model(settings: &Settings) -> bool {
    resolve_active_vision_model_path(settings)
        .map(|path| is_model_runtime_ready(&path))
        .unwrap_or(false)
}

pub fn resolve_active_model_path(settings: &Settings) -> Result<PathBuf> {
    if let Some(model_file) = settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        for dir in model_dir_candidates() {
            let candidate = dir.join(&model_file);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        return Err(PipelineError::InvalidInput(format!(
            "configured model not found: {}",
            model_file
        )));
    }

    let mut candidates = Vec::new();
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
                    candidates.push(path);
                }
            }
        }
    }

    candidates.sort();
    if let Some(path) = candidates
        .iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(is_vision_model_name)
                .unwrap_or(false)
        })
        .cloned()
    {
        return Ok(path);
    }

    candidates.into_iter().next().ok_or_else(|| {
        PipelineError::InvalidInput("no .gguf model found in models directory".into())
    })
}

pub fn resolve_active_vision_model_path(settings: &Settings) -> Result<PathBuf> {
    if let Some(model_file) = settings
        .model_file
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        for dir in model_dir_candidates() {
            let candidate = dir.join(&model_file);
            if candidate.exists() {
                if candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(is_supported_vision_model_name)
                    .unwrap_or(false)
                {
                    return Ok(candidate);
                }
                break;
            }
        }
    }

    let mut candidates = Vec::new();
    for dir in model_dir_candidates() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                    .unwrap_or(false)
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(is_supported_vision_model_name)
                        .unwrap_or(false)
                {
                    candidates.push(path);
                }
            }
        }
    }

    candidates.sort();
    candidates.into_iter().next().ok_or_else(|| {
        PipelineError::InvalidInput(
            "no vision .gguf model found in models directory (expected filename containing Vision, -VL, or LLaVA)".into(),
        )
    })
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
    let model_already_exists = target_path.exists();
    if model_already_exists {
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
    } else {
        download_file_with_progress(app, &client, &repo, &file_name, &target_path).await?;
    }

    // Some vision models (Qwen-VL / GLM-OCR / LLaVA) require a companion mmproj file.
    let lowered_main = file_name.to_ascii_lowercase();
    let requires_mmproj = (lowered_main.contains("qwen") && lowered_main.contains("-vl"))
        || lowered_main.contains("glm-ocr")
        || lowered_main.contains("llava");

    if requires_mmproj {
        let mmproj_file = select_mmproj_file(&client, &repo).await?.ok_or_else(|| {
            PipelineError::InvalidInput(
                "this vision model requires a mmproj companion file, but none was found in the repo"
                    .into(),
            )
        })?;

        let mmproj_target = models_dir.join(&mmproj_file);
        if !mmproj_target.exists() {
            emit_download(
                app,
                &repo,
                &mmproj_file,
                0,
                None,
                0.0,
                "starting",
                Some("downloading companion mmproj".into()),
            );
            download_file_with_progress(app, &client, &repo, &mmproj_file, &mmproj_target)
                .await?;
        }
    }

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
        "model input must look like owner/repo or owner/repo/file.gguf".into(),
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
    let all_ggufs: Vec<HfSibling> = info
        .siblings
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
        .collect();

    let supported_vision: Vec<HfSibling> = all_ggufs
        .iter()
        .filter(|entry| is_supported_vision_model_name(&entry.rfilename))
        .cloned()
        .collect();

    if !supported_vision.is_empty() {
        let mut vision = supported_vision;
        vision.sort_by_key(|entry| {
            let name = entry.rfilename.to_ascii_lowercase();
            let score = if name.contains("glm-ocr") && name.contains("q4_k_m") {
                0
            } else if name.contains("q3_k") {
                2
            } else if name.contains("q4_k_m") {
                1
            } else if name.contains("q4_k") {
                3
            } else if name.contains("q5_k") {
                4
            } else if name.contains("q6_k") {
                5
            } else if name.contains("q8_0") {
                6
            } else if name.contains("f16") {
                7
            } else {
                8
            };
            let size = entry.size.unwrap_or(u64::MAX);
            (score, size)
        });
        return Ok(vision[0].rfilename.clone());
    }

    let mut ggufs: Vec<HfSibling> = all_ggufs
        .iter()
        .filter(|entry| {
            let lowered = entry.rfilename.to_ascii_lowercase();
            !lowered.starts_with("mmproj") && !lowered.contains("/mmproj")
        })
        .cloned()
        .collect();

    if ggufs.is_empty() {
        ggufs = all_ggufs;
    }

    if ggufs.is_empty() {
        return Err(PipelineError::InvalidInput(
            "no supported vision .gguf files found in this repo (expected Qwen-VL GGUF)".into(),
        ));
    }

    ggufs.sort_by_key(|entry| {
        let name = entry.rfilename.to_ascii_lowercase();
        let score = if name.contains("q4_k_m") {
            0
        } else if name.contains("q4_k") {
            1
        } else if name.contains("q5_k") {
            2
        } else if name.contains("q6_k") {
            3
        } else if name.contains("q8_0") {
            4
        } else if name.contains("f16") {
            5
        } else if name.starts_with("mmproj") || name.contains("/mmproj") {
            10
        } else {
            6
        };
        let size = entry.size.unwrap_or(u64::MAX);
        (score, size)
    });

    Ok(ggufs[0].rfilename.clone())
}

async fn select_mmproj_file(client: &reqwest::Client, repo: &str) -> Result<Option<String>> {
    let url = format!("{}/{}", HF_API_BASE, repo);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let info: HfModelInfo = response
        .json()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    let mut mmproj: Vec<HfSibling> = info
        .siblings
        .unwrap_or_default()
        .into_iter()
        .filter(|s| {
            let lowered = s.rfilename.to_ascii_lowercase();
            lowered.ends_with(".gguf") && lowered.contains("mmproj")
        })
        .collect();

    if mmproj.is_empty() {
        return Ok(None);
    }

    mmproj.sort_by_key(|entry| {
        let name = entry.rfilename.to_ascii_lowercase();
        if name.contains("f16") {
            0
        } else if name.contains("bf16") {
            1
        } else if name.contains("f32") {
            2
        } else {
            3
        }
    });

    Ok(mmproj.first().map(|entry| entry.rfilename.clone()))
}

async fn download_file_with_progress(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    repo: &str,
    file_name: &str,
    target_path: &Path,
) -> Result<()> {
    let url = format!("{}/{}/resolve/main/{}", HF_RESOLVE_BASE, repo, file_name);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| PipelineError::Other(e.into()))?;

    if !response.status().is_success() {
        emit_download(
            app,
            repo,
            file_name,
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
                repo,
                file_name,
                downloaded,
                total,
                progress.min(1.0),
                "downloading",
                None,
            );
        }
    }

    file.flush().await?;
    tokio::fs::rename(&temp_path, target_path).await?;

    emit_download(
        app,
        repo,
        file_name,
        downloaded,
        total,
        1.0,
        "done",
        Some("download complete".into()),
    );

    Ok(())
}

fn model_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app_data) = app_models_dir() {
        candidates.push(app_data);
    }
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
    candidates
}

fn model_dir_creation_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("models"));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("models"));
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models"));
    }

    if let Some(app_data) = app_models_dir() {
        candidates.push(app_data);
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|dir| seen.insert(dir.clone()));
    candidates
}

fn app_models_dir() -> Option<PathBuf> {
    let mut path = dirs::data_local_dir()?;
    path.push(APP_DIR);
    path.push("models");
    Some(path)
}

pub fn ensure_models_dir() -> Result<PathBuf> {
    resolve_models_dir(true)
}

fn is_vision_model_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    lowered.contains("vision") || lowered.contains("-vl") || lowered.contains("llava")
}

fn is_supported_vision_model_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    if lowered.contains("mmproj") {
        return false;
    }
    lowered.ends_with(".gguf")
        && (
            (lowered.contains("qwen") && lowered.contains("-vl"))
            || lowered.contains("glm-ocr")
            || lowered.contains("vision")
            || lowered.contains("llava")
        )
}

fn model_requires_mmproj(model_name: &str) -> bool {
    let lowered = model_name.to_ascii_lowercase();
    (lowered.contains("qwen") && lowered.contains("-vl"))
        || lowered.contains("glm-ocr")
        || lowered.contains("llava")
}

fn is_model_runtime_ready(model_path: &Path) -> bool {
    let model_name = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if !model_requires_mmproj(model_name) {
        return true;
    }

    has_mmproj_for_model(model_path)
}

fn has_mmproj_for_model(model_path: &Path) -> bool {
    let model_name = model_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut search_dirs = Vec::new();
    if let Some(parent) = model_path.parent() {
        search_dirs.push(parent.to_path_buf());
    }
    search_dirs.extend(model_dir_candidates());

    let mut seen = std::collections::HashSet::new();
    search_dirs.retain(|d| seen.insert(d.clone()));

    for dir in search_dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_ascii_lowercase(),
                None => continue,
            };

            if !name.ends_with(".gguf") || !name.contains("mmproj") {
                continue;
            }

            if model_name.contains("glm-ocr") && name.contains("glm-ocr") {
                return true;
            }
            if model_name.contains("qwen") && model_name.contains("-vl") && name.contains("qwen") {
                return true;
            }
            if model_name.contains("llava") && name.contains("llava") {
                return true;
            }

            // Final fallback if family tag is ambiguous.
            return true;
        }
    }

    false
}

fn resolve_models_dir(create: bool) -> Result<PathBuf> {
    for candidate in model_dir_candidates() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    if create {
        let mut last_error: Option<std::io::Error> = None;
        for target in model_dir_creation_candidates() {
            match fs::create_dir_all(&target) {
                Ok(()) => return Ok(target),
                Err(err) => last_error = Some(err),
            }
        }

        return Err(PipelineError::InvalidInput(
            last_error
                .map(|err| format!("failed to create models directory: {err}"))
                .unwrap_or_else(|| "failed to create models directory".into()),
        ));
    }
    Err(PipelineError::InvalidInput(
        "models directory not found".into(),
    ))
}


