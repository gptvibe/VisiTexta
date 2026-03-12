#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tokio::time::{sleep, Duration};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemInfo {
    gpu: Option<String>,
    backend: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnsureModelResponse {
    cached: bool,
    path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum PageStatus {
    Queued,
    Processing,
    Done,
    Error,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PageResult {
    page_index: usize,
    text: String,
    status: PageStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Idle,
    Loading,
    Downloading,
    Processing,
    Complete,
    Error,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BackendStatus {
    phase: Phase,
    message: String,
    progress: f32,
    current_page: Option<usize>,
    pages_total: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnsureModelPayload {
    model_id: String,
    memory_limit_gb: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessPayload {
    model_id: String,
    file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MergePayload {
    model_id: String,
    pages: Vec<PageResult>,
}

#[derive(Default)]
struct ModelState {
    active_model: Option<String>,
    cache_root: Option<PathBuf>,
}

type SharedState = Arc<Mutex<ModelState>>;

#[tauri::command]
async fn get_system_info() -> SystemInfo {
    SystemInfo {
        gpu: detect_gpu().await,
        backend: "llama.cpp".to_string(),
    }
}

#[tauri::command]
async fn ensure_model(
    app: tauri::AppHandle,
    state: State<'_, SharedState>,
    payload: EnsureModelPayload,
) -> Result<EnsureModelResponse, String> {
    let cache_root = resolve_cache_root(&app).map_err(|err| err.to_string())?;
    let model_dir = cache_root.join(sanitize_model_id(&payload.model_id));

    {
        let mut guard = state.lock().map_err(|_| "State lock poisoned")?;
        guard.cache_root = Some(cache_root.clone());
        guard.active_model = Some(payload.model_id.clone());
    }

    if model_dir.exists() {
        return Ok(EnsureModelResponse {
            cached: true,
            path: model_dir.to_string_lossy().to_string(),
        });
    }

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Downloading,
            message: format!("Downloading {}", payload.model_id),
            progress: 10.0,
            current_page: None,
            pages_total: None,
        },
    );

    std::fs::create_dir_all(&model_dir).map_err(|err| err.to_string())?;
    let placeholder = model_dir.join("MODEL_PLACEHOLDER.txt");
    let note = format!(
        "Model placeholder for {}. Replace with actual download logic. Memory limit: {} GB.",
        payload.model_id, payload.memory_limit_gb
    );
    std::fs::write(&placeholder, note).map_err(|err| err.to_string())?;

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Loading,
            message: "Model cached".to_string(),
            progress: 25.0,
            current_page: None,
            pages_total: None,
        },
    );

    Ok(EnsureModelResponse {
        cached: false,
        path: model_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
async fn process_image(
    app: tauri::AppHandle,
    _state: State<'_, SharedState>,
    payload: ProcessPayload,
) -> Result<ProcessResponse, String> {
    let file_name = Path::new(&payload.file_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Processing,
            message: "Processing image".to_string(),
            progress: 55.0,
            current_page: Some(1),
            pages_total: Some(1),
        },
    );

    sleep(Duration::from_millis(450)).await;

    let text = format!(
        "[placeholder OCR] {}\nModel: {}\nReplace with actual vision inference.",
        file_name, payload.model_id
    );
    let pages = vec![PageResult {
        page_index: 1,
        text,
        status: PageStatus::Done,
    }];

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Complete,
            message: "OCR complete".to_string(),
            progress: 100.0,
            current_page: Some(1),
            pages_total: Some(1),
        },
    );

    Ok(ProcessResponse {
        pages,
        status: BackendStatus {
            phase: Phase::Complete,
            message: "OCR complete".to_string(),
            progress: 100.0,
            current_page: Some(1),
            pages_total: Some(1),
        },
    })
}

#[tauri::command]
async fn process_pdf(
    app: tauri::AppHandle,
    _state: State<'_, SharedState>,
    payload: ProcessPayload,
) -> Result<ProcessResponse, String> {
    let total_pages = 3usize;
    let mut pages = Vec::with_capacity(total_pages);

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Processing,
            message: "Splitting PDF".to_string(),
            progress: 35.0,
            current_page: Some(0),
            pages_total: Some(total_pages),
        },
    );

    for page in 1..=total_pages {
        emit_status(
            &app,
            BackendStatus {
                phase: Phase::Processing,
                message: format!("Processing page {}/{}", page, total_pages),
                progress: 35.0 + (page as f32 / total_pages as f32) * 60.0,
                current_page: Some(page),
                pages_total: Some(total_pages),
            },
        );

        emit_page(
            &app,
            PageResult {
                page_index: page,
                text: "".to_string(),
                status: PageStatus::Processing,
            },
        );

        sleep(Duration::from_millis(350)).await;

        let text = format!(
            "[placeholder OCR] Page {} of {}\nModel: {}\nSource: {}",
            page,
            total_pages,
            payload.model_id,
            payload.file_path
        );
        let result = PageResult {
            page_index: page,
            text,
            status: PageStatus::Done,
        };
        emit_page(&app, result.clone());
        pages.push(result);
    }

    emit_status(
        &app,
        BackendStatus {
            phase: Phase::Complete,
            message: "OCR complete".to_string(),
            progress: 100.0,
            current_page: Some(total_pages),
            pages_total: Some(total_pages),
        },
    );

    Ok(ProcessResponse {
        pages,
        status: BackendStatus {
            phase: Phase::Complete,
            message: "OCR complete".to_string(),
            progress: 100.0,
            current_page: Some(total_pages),
            pages_total: Some(total_pages),
        },
    })
}

#[tauri::command]
async fn merge_pages(payload: MergePayload) -> Result<MergeResponse, String> {
    let mut merged = String::new();
    for (index, page) in payload.pages.iter().enumerate() {
        if index > 0 {
            merged.push_str("\n\n");
        }
        merged.push_str(page.text.trim());
    }

    merged.push_str("\n\n[placeholder merge] Smooth transitions can be added here.");

    Ok(MergeResponse { merged })
}

#[derive(Debug, Serialize)]
struct ProcessResponse {
    pages: Vec<PageResult>,
    status: BackendStatus,
}

#[derive(Debug, Serialize)]
struct MergeResponse {
    merged: String,
}

fn emit_status(app: &tauri::AppHandle, status: BackendStatus) {
    let _ = app.emit("ocr://status", status);
}

fn emit_page(app: &tauri::AppHandle, page: PageResult) {
    let _ = app.emit("ocr://page", page);
}

fn resolve_cache_root(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let base = app
        .path()
        .app_cache_dir()
        .or_else(|_| dirs_next::cache_dir().ok_or_else(|| anyhow::anyhow!("cache dir")))?;
    Ok(base.join("visitexta").join("models"))
}

fn sanitize_model_id(model_id: &str) -> String {
    model_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

async fn detect_gpu() -> Option<String> {
    if cfg!(target_os = "windows") {
        return Some("GPU detection TODO".to_string());
    }
    if cfg!(target_os = "macos") {
        return Some("Apple Silicon / Metal (TODO)".to_string());
    }
    None
}

fn main() {
    let state: SharedState = Arc::new(Mutex::new(ModelState::default()));

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            ensure_model,
            process_image,
            process_pdf,
            merge_pages
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}