mod errors;
mod events;
mod pipeline;
mod settings;
mod pdf;
mod llm;
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
            if let Err(err) = models::ensure_models_dir() {
                log::warn!("failed to prepare models directory: {err}");
            }
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

            let mut first_bin: Option<std::path::PathBuf> = None;

            for cand in &bin_candidates {
                if cand.exists() {
                    if first_bin.is_none() {
                        first_bin = Some(cand.clone());
                    }
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

            // Windows: use SetDllDirectoryW so that native-library dependencies
            // (e.g. the dozens of lib*.dll files that pdfium.dll imports) are
            // resolved from our bin/ folder BEFORE the EXE directory.
            // PATH alone does not guarantee this because the standard DLL search
            // order starts with the application (EXE) directory, not the directory
            // of the DLL being loaded.
            #[cfg(target_os = "windows")]
            if let Some(ref bin_dir) = first_bin {
                win_set_dll_directory(bin_dir);
                // Pre-load pdfium.dll on the main thread so its dependencies
                // (lib*.dll in bin/) are resolved using SetDllDirectory before
                // any background worker threads call LoadLibraryW for it.
                win_preload_dll(bin_dir, "pdfium.dll");
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn win_set_dll_directory(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe { SetDllDirectoryW(wide.as_ptr()) };
}

#[cfg(target_os = "windows")]
fn win_preload_dll(bin_dir: &std::path::Path, dll_name: &str) {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
    }
    let dll_path = bin_dir.join(dll_name);
    if !dll_path.exists() {
        return;
    }
    let wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // HMODULE is intentionally not freed — keep the DLL resident for the
    // process lifetime so worker threads always find it already loaded.
    unsafe { LoadLibraryW(wide.as_ptr()) };
}

#[tauri::command]
async fn enqueue_jobs(
    app: tauri::AppHandle,
    paths: Vec<String>,
    prompt: Option<String>,
) -> Result<Vec<JobResult>, String> {
    let settings = Settings::load();
    let dpi = settings.dpi;
    tauri::async_runtime::spawn_blocking(move || {
        // Wrap in catch_unwind so an unexpected panic inside the OCR worker
        // surfaces as a readable error string rather than an opaque JoinError.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pipeline::process_batch(&app, paths, dpi, prompt)
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
    .map_err(|e| e.to_string())
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
    llm::runtime_has_llama_cli() && models::has_vision_model(&settings)
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











