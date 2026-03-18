use crate::errors::{PipelineError, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct RenderedPdf {
    pub images: Vec<PathBuf>,
    pub _tempdir: TempDir, // keeps files alive
}

pub fn render_pdf_to_images(path: &Path, dpi: u16) -> Result<RenderedPdf> {
    let pdfium = Pdfium::new(resolve_pdfium_bindings()?);

    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| PipelineError::Pdf(format!("load failed: {e}")))?;

    let tempdir = tempfile::tempdir()?;
    let mut images = Vec::new();

    for (idx, page) in doc.pages().iter().enumerate() {
        let width_px = ((page.width().value as f32 / 72.0) * dpi as f32).round() as i32;
        let height_px = ((page.height().value as f32 / 72.0) * dpi as f32).round() as i32;

        let bitmap = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(width_px)
                    .set_target_height(height_px),
            )
            .map_err(|e| PipelineError::Pdf(format!("render page {idx} failed: {e}")))?;

        let image = bitmap.as_image();
        let out_path = tempdir
            .path()
            .join(format!("page-{}.png", idx + 1));
        image
            .save(&out_path)
            .map_err(|e| PipelineError::Pdf(format!("save page {idx} failed: {e}")))?;
        images.push(out_path);
    }

    Ok(RenderedPdf { images, _tempdir: tempdir })
}

fn resolve_pdfium_bindings() -> Result<Box<dyn PdfiumLibraryBindings>> {
    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        search_dirs.push(cwd.clone());
        search_dirs.push(cwd.join("bin"));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            search_dirs.push(exe_dir.to_path_buf());
            search_dirs.push(exe_dir.join("bin"));
            search_dirs.push(exe_dir.join("resources"));
            search_dirs.push(exe_dir.join("resources").join("bin"));
            if let Some(parent) = exe_dir.parent() {
                search_dirs.push(parent.to_path_buf());
                search_dirs.push(parent.join("bin"));
                search_dirs.push(parent.join("resources"));
                search_dirs.push(parent.join("resources").join("bin"));
            }
        }
    }

    // Helpful during local development.
    search_dirs.push(PathBuf::from("src-tauri").join("bin"));

    let mut seen = std::collections::HashSet::new();
    search_dirs.retain(|dir| seen.insert(dir.clone()));

    let mut attempted = Vec::new();
    let mut last_err: Option<String> = None;

    for dir in search_dirs {
        let pdfium_dll = dir.join("pdfium.dll");
        attempted.push(pdfium_dll.clone());
        if !pdfium_dll.exists() {
            continue;
        }

        let platform_name = Pdfium::pdfium_platform_library_name_at_path(&dir);
        match Pdfium::bind_to_library(platform_name) {
            Ok(bindings) => return Ok(bindings),
            Err(e) => {
                last_err = Some(format!("{} ({e})", pdfium_dll.to_string_lossy()));
            }
        }
    }

    if let Ok(system) = Pdfium::bind_to_system_library() {
        return Ok(system);
    }

    let tried = attempted
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("; ");

    let detail = last_err.unwrap_or_else(|| "pdfium.dll was not found in known runtime locations".to_string());
    Err(PipelineError::Pdf(format!(
        "pdfium bind failed: {detail}. Searched: {tried}. Place pdfium.dll in app bin/resources/bin."
    )))
}
