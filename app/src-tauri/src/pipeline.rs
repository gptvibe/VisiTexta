use crate::errors::{PipelineError, Result};
use crate::events::{AppEvent, CompletedEvent, ErrorEvent, JobStatus, ProgressEvent};
use crate::formatting::clean_markdown;
use crate::ocr::OcrEngine;
use crate::pdf::render_pdf_to_images;
use image::DynamicImage;
use std::fs;
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

pub fn process_batch(app: &tauri::AppHandle, paths: Vec<String>, dpi: u16) -> Result<Vec<JobResult>> {
    let mut results = Vec::with_capacity(paths.len());
    let ocr = OcrEngine::new("eng")?;

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
            match process_single(app, &job_id, &path, &ocr, dpi) {
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
    ocr: &OcrEngine,
    dpi: u16,
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
    for (idx, img_path) in images.iter().enumerate() {
        let text = ocr.recognize(img_path)?;
        page_texts.push(format!("## Page {}\n\n{}\n", idx + 1, text.trim()));
        let prog = 0.25 + ((idx as f32 + 1.0) / images.len().max(1) as f32) * 0.5;
        emit_progress(app, job_id, JobStatus::Ocr, prog, None, None);
    }

    emit_progress(
        app,
        job_id,
        JobStatus::Formatting,
        0.8,
        Some("Formatting".into()),
        Some(path.to_string_lossy().into()),
    );

    let markdown = clean_markdown(&page_texts.join("\n"));

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
