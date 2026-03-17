use crate::errors::{PipelineError, Result};
use crate::events::{
    AppEvent, CompletedEvent, ErrorEvent, JobStatus, PreviewEvent, ProgressEvent,
};
use crate::formatting::clean_markdown;
use crate::llm::LlmOcrEngine;
use crate::pdf::render_pdf_to_images;
use base64::Engine;
use image::{DynamicImage, ImageFormat};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, Clone)]
pub struct JobResult {
    pub job_id: String,
    pub source: String,
    pub output_path: Option<String>,
    pub status: JobStatus,
    pub error: Option<String>,
}

const ALLOWED_EXT: &[&str] = &["png", "jpg", "jpeg", "pdf"];
const DEFAULT_PROMPT: &str = "Extract all text from the image and return it as markdown.";

pub fn process_batch(app: &tauri::AppHandle, paths: Vec<String>, dpi: u16, prompt: Option<String>) -> Result<Vec<JobResult>> {
    let mut results = Vec::with_capacity(paths.len());
    let settings = crate::settings::Settings::load();
    let model_path = crate::models::resolve_active_vision_model_path(&settings)?;
    let ocr = LlmOcrEngine::new(model_path, settings.threads)?;
    let effective_prompt = prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PROMPT);

    for raw in paths {
        let path = PathBuf::from(&raw);
        let job_id = Uuid::new_v4().to_string();

        emit_progress(
            app,
            &job_id,
            JobStatus::Queued,
            0.0,
            Some("Queued".into()),
            Some(raw.clone()),
        );

        let res = if !path.exists() {
            fail(job_id, raw, "file does not exist".into(), app)
        } else if !is_allowed(&path) {
            fail(job_id, raw, "unsupported file type".into(), app)
        } else {
            match process_single(app, &job_id, &path, &ocr, dpi, effective_prompt) {
                Ok(out) => {
                    emit_complete(app, &job_id, &out);
                    JobResult {
                        job_id,
                        source: raw,
                        output_path: Some(out.to_string_lossy().into_owned()),
                        status: JobStatus::Done,
                        error: None,
                    }
                }
                Err(err) => fail(job_id, raw, err.to_string(), app),
            }
        };

        results.push(res);
    }

    Ok(results)
}

fn fail(job_id: String, source: String, msg: String, app: &tauri::AppHandle) -> JobResult {
    emit_error(app, &job_id, &msg);
    JobResult {
        job_id,
        source,
        output_path: None,
        status: JobStatus::Failed,
        error: Some(msg),
    }
}

fn process_single(
    app: &tauri::AppHandle,
    job_id: &str,
    path: &Path,
    ocr: &LlmOcrEngine,
    dpi: u16,
    prompt: &str,
) -> Result<PathBuf> {
    let mut images: Vec<PathBuf> = Vec::new();
    let mut _temp_holder: Option<tempfile::TempDir> = None;

    if is_pdf(path) {
        emit_progress(
            app,
            job_id,
            JobStatus::Rendering,
            0.05,
            Some("Rendering PDF".into()),
            Some(path.to_string_lossy().into()),
        );
        let rendered = render_pdf_to_images(path, dpi)?;
        images = rendered.images.clone();
        _temp_holder = Some(rendered._tempdir);
    } else {
        // preprocess standalone image into temp png (grayscale)
        let img = image::open(path).map_err(|e| PipelineError::InvalidInput(e.to_string()))?;
        let gray = DynamicImage::ImageLuma8(img.to_luma8());
        let tempdir = tempfile::tempdir()?;
        let out = tempdir.path().join("image.png");
        gray.save(&out)
            .map_err(|e| PipelineError::InvalidInput(e.to_string()))?;
        images.push(out);
        _temp_holder = Some(tempdir);
    }

    emit_progress(
        app,
        job_id,
        JobStatus::Ocr,
        0.25,
        Some("Running OCR".into()),
        Some(path.to_string_lossy().into()),
    );

    let mut page_texts = Vec::new();
    let total_pages = images.len().max(1);

    for (idx, img_path) in images.iter().enumerate() {
        let page_number = idx + 1;
        let preview_image = encode_preview_image_data_url(img_path)?;

        emit_progress(
            app,
            job_id,
            JobStatus::Ocr,
            0.25 + (idx as f32 / total_pages as f32) * 0.5,
            Some(format!("Scanning page {page_number}/{total_pages}")),
            Some(path.to_string_lossy().into()),
        );
        emit_preview(
            app,
            job_id,
            path,
            page_number,
            total_pages,
            &preview_image,
            None,
        );

        let text = ocr.recognize(img_path, prompt)?;
        let chunk = format!("## Page {}\n\n{}\n", page_number, text.trim());
        page_texts.push(chunk.clone());
        emit_preview(
            app,
            job_id,
            path,
            page_number,
            total_pages,
            &preview_image,
            Some(chunk),
        );

        let prog = 0.25 + (page_number as f32 / total_pages as f32) * 0.5;
        emit_progress(
            app,
            job_id,
            JobStatus::Ocr,
            prog,
            Some(format!("Recognized page {page_number}/{total_pages}")),
            Some(path.to_string_lossy().into()),
        );
    }

    emit_progress(
        app,
        job_id,
        JobStatus::Formatting,
        0.8,
        Some("Formatting".into()),
        Some(path.to_string_lossy().into()),
    );

    let body = clean_markdown(&page_texts.join("\n"));
    let markdown = if prompt != DEFAULT_PROMPT {
        format!("<!-- prompt: {} -->\n\n{}", prompt, body)
    } else {
        body
    };

    emit_progress(
        app,
        job_id,
        JobStatus::Writing,
        0.9,
        Some("Writing Markdown".into()),
        Some(path.to_string_lossy().into()),
    );

    let parent = path
        .parent()
        .ok_or_else(|| PipelineError::InvalidInput("missing parent directory".into()))?;
    let stem = path
        .file_stem()
        .ok_or_else(|| PipelineError::InvalidInput("invalid file name".into()))?;

    let mut out = parent.to_path_buf();
    out.push(format!("{}.md", stem.to_string_lossy()));
    fs::write(&out, markdown)?;

    emit_progress(
        app,
        job_id,
        JobStatus::Done,
        1.0,
        Some("Done".into()),
        Some(path.to_string_lossy().into()),
    );
    Ok(out)
}

fn is_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ALLOWED_EXT.iter().any(|allowed| allowed.eq_ignore_ascii_case(ext)))
        .unwrap_or(false)
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn emit_progress(
    app: &tauri::AppHandle,
    job_id: &str,
    status: JobStatus,
    progress: f32,
    message: Option<String>,
    source: Option<String>,
) {
    let payload = AppEvent::Progress(ProgressEvent {
        job_id: job_id.to_string(),
        status,
        progress,
        message,
        source,
    });
    let _ = app.emit("job-progress", &payload);
}

fn emit_complete(app: &tauri::AppHandle, job_id: &str, path: &Path) {
    let payload = AppEvent::Completed(CompletedEvent {
        job_id: job_id.to_string(),
        output_path: path.to_string_lossy().into(),
    });
    let _ = app.emit("job-complete", &payload);
}

fn emit_error(app: &tauri::AppHandle, job_id: &str, message: &str) {
    let payload = AppEvent::Error(ErrorEvent {
        job_id: job_id.to_string(),
        message: message.into(),
    });
    let _ = app.emit("job-error", &payload);
}

fn emit_preview(
    app: &tauri::AppHandle,
    job_id: &str,
    source: &Path,
    page_number: usize,
    total_pages: usize,
    image_data_url: &str,
    text_chunk: Option<String>,
) {
    let payload = AppEvent::Preview(PreviewEvent {
        job_id: job_id.to_string(),
        source: Some(source.to_string_lossy().into()),
        page_number,
        total_pages,
        image_data_url: image_data_url.to_string(),
        text_chunk,
    });
    let _ = app.emit("job-preview", &payload);
}

fn encode_preview_image_data_url(path: &Path) -> Result<String> {
    let preview = image::open(path)
        .map_err(|e| PipelineError::InvalidInput(e.to_string()))?
        .thumbnail(1200, 1600);

    let mut bytes = Cursor::new(Vec::new());
    preview
        .write_to(&mut bytes, ImageFormat::Png)
        .map_err(|e| PipelineError::InvalidInput(e.to_string()))?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    Ok(format!("data:image/png;base64,{encoded}"))
}
