mod errors;
mod events;
mod pipeline;
mod settings;
mod pdf;
mod ocr;
mod formatting;
mod models;

use pipeline::JobResult;
use settings::Settings;
use tauri::Wry;
use tauri_plugin_clipboard_manager::Clipboard;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    hydrate_path_for_binaries();

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            enqueue_jobs,
            get_settings,
            set_settings,
            copy_file_to_clipboard,
            open_output_folder,
            check_model_exists,
            list_models,
            download_model,
            read_markdown_file,
            save_markdown_as
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn hydrate_path_for_binaries() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let mut bin_candidates = vec![dir.join("bin"), dir.join("resources").join("bin")];
            if let Some(parent) = dir.parent() {
                bin_candidates.push(parent.join("bin"));
            }
            for cand in bin_candidates {
                if cand.exists() {
                    if let Some(cand_str) = cand.to_str() {
                        if let Some(path) = std::env::var_os("PATH") {
                            let mut new_path = std::ffi::OsString::from(cand_str);
                            new_path.push(";");
                            new_path.push(path);
                            std::env::set_var("PATH", new_path);
                        } else {
                            std::env::set_var("PATH", cand_str);
                        }
                    }
                }
            }
        }
    }
}

#[tauri::command]
fn enqueue_jobs(app: tauri::AppHandle, paths: Vec<String>) -> Result<Vec<JobResult>, String> {
    let settings = Settings::load();
    pipeline::process_batch(&app, paths, settings.dpi).map_err(|e| e.to_string())
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
    let folder = p
        .parent()
        .ok_or_else(|| "no parent folder".to_string())?;
    std::process::Command::new("explorer")
        .arg(folder)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn check_model_exists() -> bool {
    let settings = Settings::load();
    models::model_exists(&settings)
}

#[tauri::command]
fn list_models() -> Result<Vec<String>, String> {
    models::list_models().map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_model(app: tauri::AppHandle, model: String) -> Result<models::DownloadResult, String> {
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
    std::fs::write(dest_path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_settings() -> Settings {
    Settings::load()
}

#[tauri::command]
fn set_settings(settings: Settings) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())
}











