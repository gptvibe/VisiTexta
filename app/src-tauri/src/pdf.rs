use crate::errors::{PipelineError, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct RenderedPdf {
    pub images: Vec<PathBuf>,
    pub _tempdir: TempDir, // keeps files alive
}

pub fn render_pdf_to_images(path: &Path, dpi: u16) -> Result<RenderedPdf> {
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./bin")))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| PipelineError::Pdf(format!("pdfium bind failed: {e}")))?,
    );

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
