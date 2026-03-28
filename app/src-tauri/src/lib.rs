#[cfg(debug_assertions)]
pub mod benchmark;
mod defaults;
mod errors;
mod events;
mod formatting;
mod history;
mod llm;
mod models;
mod pdf;
mod pipeline;
mod runtime;
mod settings;
mod storage;

use base64::Engine;
use pipeline::JobResult;
use settings::Settings;
use tauri::Wry;
use tauri_plugin_clipboard_manager::Clipboard;
use uuid::Uuid;

#[derive(serde::Serialize)]
struct OnboardingInfo {
    storage_mode: storage::StorageMode,
    app_storage_path: String,
    settings_storage_path: String,
    history_storage_path: String,
    model_storage_path: String,
    temp_storage_path: String,
    pasted_inputs_path: String,
    output_description: String,
    recommended_model_profile_id: String,
    recommended_model_label: String,
    recommended_model_file: String,
    recommended_model_repo: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    runtime::hydrate_path_for_binaries();

    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let _ = app.handle().plugin(tauri_plugin_dialog::init());
            let _ = app.handle().plugin(tauri_plugin_clipboard_manager::init());
            if let Err(err) = storage::prepare_startup() {
                log::warn!("failed to prepare app storage: {err}");
            }
            if let Err(err) = history::recover_interrupted_jobs() {
                log::warn!("failed to recover interrupted jobs: {err}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            enqueue_jobs,
            enqueue_pasted_image,
            cancel_job,
            get_settings,
            get_app_defaults,
            get_runtime_status,
            get_onboarding_info,
            get_storage_info,
            get_recommended_setup_info,
            get_job_history,
            set_settings,
            copy_file_to_clipboard,
            open_output_folder,
            reveal_in_explorer,
            check_model_exists,
            get_model_catalog,
            list_models,
            download_model,
            read_markdown_file,
            save_markdown_as
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn enqueue_jobs(
    app: tauri::AppHandle,
    paths: Vec<String>,
    prompt: Option<String>,
    dpi: Option<u16>,
    run_options: Option<pipeline::PipelineRunOptions>,
) -> Result<Vec<JobResult>, String> {
    enqueue_paths(app, paths, prompt, dpi, run_options).await
}

#[tauri::command]
async fn enqueue_pasted_image(
    app: tauri::AppHandle,
    image_base64: String,
    mime_type: String,
    prompt: Option<String>,
    dpi: Option<u16>,
    run_options: Option<pipeline::PipelineRunOptions>,
) -> Result<Vec<JobResult>, String> {
    let path = write_pasted_image(&image_base64, &mime_type)?;
    enqueue_paths(app, vec![path], prompt, dpi, run_options).await
}

async fn enqueue_paths(
    app: tauri::AppHandle,
    paths: Vec<String>,
    prompt: Option<String>,
    dpi: Option<u16>,
    run_options: Option<pipeline::PipelineRunOptions>,
) -> Result<Vec<JobResult>, String> {
    let settings = Settings::load();
    let dpi = dpi.unwrap_or(settings.dpi);
    let results = tauri::async_runtime::spawn_blocking(move || {
        // Wrap in catch_unwind so an unexpected panic inside the OCR worker
        // surfaces as a readable error string rather than an opaque JoinError.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pipeline::process_batch(&app, paths, dpi, prompt, run_options)
        }))
        .unwrap_or_else(|payload| {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unexpected panic in OCR worker".into());
            Err(crate::errors::PipelineError::InvalidInput(msg))
        })
    })
    .await
    .map_err(|e| format!("background task failed: {e}"))?
    .map_err(|e| e.to_string())?;

    Ok(results)
}

#[tauri::command]
fn cancel_job(job_id: String) -> bool {
    pipeline::request_cancel(&job_id)
}

#[tauri::command]
fn copy_file_to_clipboard(
    clipboard: tauri::State<Clipboard<Wry>>,
    path: String,
) -> Result<(), String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    clipboard.write_text(content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_output_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let folder = p.parent().ok_or_else(|| "no parent folder".to_string())?;
    std::process::Command::new("explorer")
        .arg(folder)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn reveal_in_explorer(path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path);
    if !target.exists() {
        return Err("file does not exist".to_string());
    }

    std::process::Command::new("explorer")
        .arg("/select,")
        .arg(target)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn check_model_exists(profile: Option<runtime::RuntimeProfile>) -> bool {
    let settings = Settings::load();
    let selected = profile.unwrap_or(settings.runtime_profile);
    let mut effective_settings = settings.clone();
    effective_settings.runtime_profile = selected;
    runtime::runtime_has_ocr_runner(selected) && models::has_vision_model(&effective_settings)
}

#[tauri::command]
fn get_runtime_status(profile: Option<runtime::RuntimeProfile>) -> runtime::RuntimeStatus {
    let settings = Settings::load();
    let selected = profile.unwrap_or(settings.runtime_profile);
    runtime::runtime_status(selected)
}

#[tauri::command]
fn get_model_catalog() -> Result<models::ModelCatalog, String> {
    models::get_model_catalog().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_models() -> Result<Vec<String>, String> {
    models::list_models().map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    model: String,
) -> Result<models::DownloadResult, String> {
    models::download_model(&app, &model)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn read_markdown_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_markdown_as(src_path: String, dest_path: String) -> Result<(), String> {
    let content = std::fs::read_to_string(&src_path).map_err(|e| e.to_string())?;
    storage::atomic_write(std::path::Path::new(&dest_path), content.as_bytes())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings() -> Settings {
    Settings::load()
}

#[tauri::command]
fn get_app_defaults() -> defaults::AppDefaults {
    defaults::app_defaults()
}

#[tauri::command]
fn get_onboarding_info() -> Result<OnboardingInfo, String> {
    let storage_info = storage::storage_info().map_err(|e| e.to_string())?;

    Ok(OnboardingInfo {
        storage_mode: storage_info.mode,
        app_storage_path: storage_info.root_path,
        settings_storage_path: storage_info.settings_path,
        history_storage_path: storage_info.history_path,
        model_storage_path: storage_info.models_path,
        temp_storage_path: storage_info.temp_path,
        pasted_inputs_path: storage_info.pasted_inputs_path,
        output_description: storage_info.outputs_description,
        recommended_model_profile_id: models::recommended_model_profile_id().to_string(),
        recommended_model_label: models::recommended_model_label().to_string(),
        recommended_model_file: models::recommended_model_file_name().to_string(),
        recommended_model_repo: models::recommended_model_repo().to_string(),
    })
}

#[tauri::command]
fn get_storage_info() -> Result<storage::StorageInfo, String> {
    storage::storage_info().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_recommended_setup_info() -> models::RecommendedSetupInfo {
    models::recommended_setup_info().await
}

#[tauri::command]
fn get_job_history() -> Result<Vec<JobResult>, String> {
    history::load_recent_jobs().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_settings(settings: Settings) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())?;
    llm::shutdown_persistent_worker();
    Ok(())
}

fn write_pasted_image(image_base64: &str, mime_type: &str) -> Result<String, String> {
    let encoded = image_base64
        .split_once(',')
        .map(|(_, payload)| payload)
        .unwrap_or(image_base64);

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;

    let dir = storage::pasted_inputs_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let extension = match mime_type.to_ascii_lowercase().as_str() {
        value if value.contains("jpeg") || value.contains("jpg") => "jpg",
        value if value.contains("webp") => "webp",
        _ => "png",
    };

    let filename = format!(
        "pasted-image-{}-{}.{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &Uuid::new_v4().simple().to_string()[..8],
        extension
    );
    let path = dir.join(filename);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}
