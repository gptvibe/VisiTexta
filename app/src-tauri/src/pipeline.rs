use crate::errors::{PipelineError, Result};
use crate::events::{
    AppEvent, CompletedEvent, ErrorEvent, JobStatus, PreviewEvent, PreviewKind, ProgressEvent,
    RunnerEvent, RunnerMode, RunnerStage,
};
use crate::formatting::clean_markdown;
use crate::llm::{LlmOcrEngine, OcrStreamEvent};
use crate::pdf::{render_pdf_pages_lazy, RenderedPdfPage};
use crate::settings::Settings;
use base64::Engine;
use image::{DynamicImage, ImageFormat};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
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
const MAX_OCR_DIMENSION: u32 = 1600;
const INITIAL_PROCESS_PROGRESS: f32 = 0.05;
const PDF_RENDER_PROGRESS_WEIGHT: f32 = 0.20;
const OCR_PROGRESS_WEIGHT: f32 = 0.55;
const OCR_ACTIVE_PAGE_FRACTION: f32 = 0.1;

static ACTIVE_JOBS: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
struct PdfProgressState {
    total_pages: usize,
    rendered_pages: usize,
    completed_ocr_pages: usize,
    active_ocr_page: Option<usize>,
}

impl PdfProgressState {
    fn new(total_pages: usize) -> Self {
        Self {
            total_pages: total_pages.max(1),
            rendered_pages: 0,
            completed_ocr_pages: 0,
            active_ocr_page: None,
        }
    }

    fn current_status(&self) -> JobStatus {
        if self.active_ocr_page.is_some() || self.completed_ocr_pages > 0 {
            JobStatus::Ocr
        } else {
            JobStatus::Rendering
        }
    }

    fn combined_progress(&self, active_page_fraction: f32) -> f32 {
        let total = self.total_pages.max(1) as f32;
        let render_progress = self.rendered_pages as f32 / total;
        let ocr_progress =
            (self.completed_ocr_pages as f32 + active_page_fraction.min(1.0)) / total;

        INITIAL_PROCESS_PROGRESS
            + render_progress * PDF_RENDER_PROGRESS_WEIGHT
            + ocr_progress * OCR_PROGRESS_WEIGHT
    }

    fn ocr_suffix(&self) -> String {
        if self.rendered_pages < self.total_pages {
            format!(" (rendered {}/{})", self.rendered_pages, self.total_pages)
        } else {
            String::new()
        }
    }
}

#[derive(Debug)]
struct OcrPageJob {
    page_number: usize,
    total_pages: usize,
    image_path: PathBuf,
}

#[derive(Debug)]
enum PdfPipelineEvent {
    RenderStarted {
        total_pages: usize,
    },
    PageRendered(RenderedPdfPage),
    RenderFinished,
    RenderFailed(PipelineError),
    OcrPageStarted {
        page_number: usize,
        total_pages: usize,
    },
    OcrRunnerEvent {
        page_number: usize,
        total_pages: usize,
        mode: RunnerMode,
        stage: RunnerStage,
        message: Option<String>,
        chunk: Option<String>,
        will_fallback: Option<bool>,
    },
    OcrPageFinished {
        page_number: usize,
        page_markdown: String,
    },
    OcrFinished,
    OcrFailed(PipelineError),
}

struct ActiveJobGuard {
    job_id: String,
}

impl ActiveJobGuard {
    fn register(job_id: String) -> Self {
        ACTIVE_JOBS.lock().insert(job_id.clone(), false);
        Self { job_id }
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        ACTIVE_JOBS.lock().remove(&self.job_id);
    }
}

pub fn request_cancel(job_id: &str) -> bool {
    let mut jobs = ACTIVE_JOBS.lock();
    if let Some(cancel_requested) = jobs.get_mut(job_id) {
        *cancel_requested = true;
        return true;
    }
    false
}

fn is_cancel_requested(job_id: &str) -> bool {
    ACTIVE_JOBS.lock().get(job_id).copied().unwrap_or(false)
}

pub fn process_batch(
    app: &tauri::AppHandle,
    paths: Vec<String>,
    dpi: u16,
    prompt: Option<String>,
) -> Result<Vec<JobResult>> {
    let settings = Settings::load();
    let mut observer = TauriObserver { app };
    process_batch_with_observer(paths, &settings, dpi, prompt, &mut observer)
}

pub(crate) fn process_batch_with_observer(
    paths: Vec<String>,
    settings: &Settings,
    dpi: u16,
    prompt: Option<String>,
    observer: &mut dyn PipelineObserver,
) -> Result<Vec<JobResult>> {
    let mut results = Vec::with_capacity(paths.len());
    let model_path = crate::models::resolve_active_vision_model_path(settings)?;
    let ocr = LlmOcrEngine::new(model_path, settings.threads)?;
    let effective_prompt = prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PROMPT);

    for raw in paths {
        let path = PathBuf::from(&raw);
        let job_id = Uuid::new_v4().to_string();
        let _job_guard = ActiveJobGuard::register(job_id.clone());

        emit_progress(
            observer,
            &job_id,
            JobStatus::Queued,
            0.0,
            Some("Waiting to start".into()),
            Some(raw.clone()),
            None,
            None,
            None,
            None,
        );

        let res = if is_cancel_requested(&job_id) {
            canceled(job_id, raw, observer)
        } else if !path.exists() {
            fail(job_id, raw, "file does not exist".into(), observer)
        } else if !is_allowed(&path) {
            fail(job_id, raw, "unsupported file type".into(), observer)
        } else {
            match process_single(observer, &job_id, &path, &ocr, dpi, effective_prompt) {
                Ok(out) => {
                    emit_complete(observer, &job_id, &out);
                    JobResult {
                        job_id,
                        source: raw,
                        output_path: Some(out.to_string_lossy().into_owned()),
                        status: JobStatus::Done,
                        error: None,
                    }
                }
                Err(PipelineError::Canceled) => canceled(job_id, raw, observer),
                Err(err) => fail(job_id, raw, err.to_string(), observer),
            }
        };

        results.push(res);
    }

    Ok(results)
}

fn canceled(job_id: String, source: String, observer: &mut dyn PipelineObserver) -> JobResult {
    emit_progress(
        observer,
        &job_id,
        JobStatus::Canceled,
        1.0,
        Some("Canceled".into()),
        Some(source.clone()),
        None,
        None,
        None,
        None,
    );

    JobResult {
        job_id,
        source,
        output_path: None,
        status: JobStatus::Canceled,
        error: None,
    }
}

pub(crate) trait PipelineObserver {
    fn on_progress(&mut self, event: ProgressEvent);
    fn on_preview(&mut self, event: PreviewEvent);
    fn on_runner(&mut self, event: RunnerEvent);
    fn on_completed(&mut self, event: CompletedEvent);
    fn on_error(&mut self, event: ErrorEvent);
}

struct TauriObserver<'a> {
    app: &'a tauri::AppHandle,
}

impl PipelineObserver for TauriObserver<'_> {
    fn on_progress(&mut self, event: ProgressEvent) {
        let payload = AppEvent::Progress(event);
        let _ = self.app.emit("job-progress", &payload);
    }

    fn on_preview(&mut self, event: PreviewEvent) {
        let payload = AppEvent::Preview(event);
        let _ = self.app.emit("job-preview", &payload);
    }

    fn on_runner(&mut self, event: RunnerEvent) {
        let payload = AppEvent::Runner(event);
        let _ = self.app.emit("job-runner", &payload);
    }

    fn on_completed(&mut self, event: CompletedEvent) {
        let payload = AppEvent::Completed(event);
        let _ = self.app.emit("job-complete", &payload);
    }

    fn on_error(&mut self, event: ErrorEvent) {
        let payload = AppEvent::Error(event);
        let _ = self.app.emit("job-error", &payload);
    }
}

fn fail(
    job_id: String,
    source: String,
    msg: String,
    observer: &mut dyn PipelineObserver,
) -> JobResult {
    emit_error(observer, &job_id, &msg);
    JobResult {
        job_id,
        source,
        output_path: None,
        status: JobStatus::Failed,
        error: Some(msg),
    }
}

fn process_single(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    path: &Path,
    ocr: &LlmOcrEngine,
    dpi: u16,
    prompt: &str,
) -> Result<PathBuf> {
    ensure_not_canceled(observer, job_id, path, 0.0, None, None, None, None)?;

    let page_texts = if is_pdf(path) {
        process_pdf_pages(observer, job_id, path, ocr, dpi, prompt)?
    } else {
        process_image_pages(observer, job_id, path, ocr, prompt)?
    };

    ensure_not_canceled(
        observer,
        job_id,
        path,
        0.8,
        None,
        Some(page_texts.len().max(1)),
        Some(page_texts.len()),
        Some(page_texts.len()),
    )?;

    emit_progress(
        observer,
        job_id,
        JobStatus::Formatting,
        0.8,
        Some("Cleaning up text".into()),
        Some(path.to_string_lossy().into()),
        None,
        Some(page_texts.len().max(1)),
        Some(page_texts.len()),
        Some(page_texts.len()),
    );

    let body = clean_markdown(&page_texts.join("\n"));
    if !has_substantive_ocr_text(&body) {
        return Err(PipelineError::Llm(
            "OCR produced empty markdown. Verify the selected model supports vision OCR and try again."
                .into(),
        ));
    }

    let markdown = if prompt != DEFAULT_PROMPT {
        format!("<!-- prompt: {} -->\n\n{}", prompt, body)
    } else {
        body
    };

    emit_progress(
        observer,
        job_id,
        JobStatus::Writing,
        0.9,
        Some("Saving markdown".into()),
        Some(path.to_string_lossy().into()),
        None,
        Some(page_texts.len().max(1)),
        Some(page_texts.len()),
        Some(page_texts.len()),
    );

    ensure_not_canceled(
        observer,
        job_id,
        path,
        0.9,
        None,
        Some(page_texts.len().max(1)),
        Some(page_texts.len()),
        Some(page_texts.len()),
    )?;

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
        observer,
        job_id,
        JobStatus::Done,
        1.0,
        Some("Markdown ready".into()),
        Some(path.to_string_lossy().into()),
        None,
        Some(page_texts.len().max(1)),
        Some(page_texts.len()),
        Some(page_texts.len()),
    );
    Ok(out)
}

fn process_pdf_pages(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    path: &Path,
    ocr: &LlmOcrEngine,
    dpi: u16,
    prompt: &str,
) -> Result<Vec<String>> {
    emit_progress(
        observer,
        job_id,
        JobStatus::Rendering,
        INITIAL_PROCESS_PROGRESS,
        Some("Preparing pages".into()),
        Some(path.to_string_lossy().into()),
        None,
        None,
        None,
        None,
    );

    let render_dir = tempfile::tempdir()?;
    let preprocess_dir = tempfile::tempdir()?;
    let (event_tx, event_rx) = mpsc::channel::<PdfPipelineEvent>();
    let (ocr_tx, ocr_rx) = mpsc::channel::<OcrPageJob>();

    thread::scope(|scope| -> Result<Vec<String>> {
        let render_path = path.to_path_buf();
        let render_output_dir = render_dir.path().to_path_buf();
        let render_events = event_tx.clone();
        let render_worker = scope.spawn(move || {
            let result = render_pdf_pages_lazy(
                &render_path,
                dpi,
                &render_output_dir,
                |total_pages| {
                    render_events
                        .send(PdfPipelineEvent::RenderStarted { total_pages })
                        .map_err(|_| PipelineError::Canceled)
                },
                |page| {
                    render_events
                        .send(PdfPipelineEvent::PageRendered(page))
                        .map_err(|_| PipelineError::Canceled)
                },
            );

            let _ = render_events.send(match result {
                Ok(()) => PdfPipelineEvent::RenderFinished,
                Err(err) => PdfPipelineEvent::RenderFailed(err),
            });
        });

        let ocr_events = event_tx.clone();
        let ocr_worker = scope.spawn(move || {
            while let Ok(job) = ocr_rx.recv() {
                if ocr_events
                    .send(PdfPipelineEvent::OcrPageStarted {
                        page_number: job.page_number,
                        total_pages: job.total_pages,
                    })
                    .is_err()
                {
                    return;
                }

                let result = ocr.recognize_streaming(&job.image_path, prompt, |event| {
                    let pipeline_event = match event {
                        OcrStreamEvent::WorkerStarting { mode, message } => {
                            PdfPipelineEvent::OcrRunnerEvent {
                                page_number: job.page_number,
                                total_pages: job.total_pages,
                                mode,
                                stage: RunnerStage::WorkerStarting,
                                message: Some(message),
                                chunk: None,
                                will_fallback: None,
                            }
                        }
                        OcrStreamEvent::ModelReady { mode, message } => {
                            PdfPipelineEvent::OcrRunnerEvent {
                                page_number: job.page_number,
                                total_pages: job.total_pages,
                                mode,
                                stage: RunnerStage::ModelReady,
                                message: Some(message),
                                chunk: None,
                                will_fallback: None,
                            }
                        }
                        OcrStreamEvent::FirstToken { mode } => PdfPipelineEvent::OcrRunnerEvent {
                            page_number: job.page_number,
                            total_pages: job.total_pages,
                            mode,
                            stage: RunnerStage::FirstToken,
                            message: None,
                            chunk: None,
                            will_fallback: None,
                        },
                        OcrStreamEvent::TextChunk { mode, chunk } => {
                            PdfPipelineEvent::OcrRunnerEvent {
                                page_number: job.page_number,
                                total_pages: job.total_pages,
                                mode,
                                stage: RunnerStage::Chunk,
                                message: None,
                                chunk: Some(chunk),
                                will_fallback: None,
                            }
                        }
                        OcrStreamEvent::RunnerError {
                            mode,
                            message,
                            will_fallback,
                        } => PdfPipelineEvent::OcrRunnerEvent {
                            page_number: job.page_number,
                            total_pages: job.total_pages,
                            mode,
                            stage: RunnerStage::Error,
                            message: Some(message),
                            chunk: None,
                            will_fallback: Some(will_fallback),
                        },
                    };

                    let _ = ocr_events.send(pipeline_event);
                });

                match result {
                    Ok(streamed_text) => {
                        let mut page_markdown = format!("## Page {}\n\n", job.page_number);
                        page_markdown.push_str(streamed_text.trim());
                        page_markdown.push('\n');

                        if ocr_events
                            .send(PdfPipelineEvent::OcrPageFinished {
                                page_number: job.page_number,
                                page_markdown,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(err) => {
                        let _ = ocr_events.send(PdfPipelineEvent::OcrFailed(err));
                        return;
                    }
                }
            }

            let _ = ocr_events.send(PdfPipelineEvent::OcrFinished);
        });

        drop(event_tx);

        let mut ocr_tx = Some(ocr_tx);
        let mut progress_state: Option<PdfProgressState> = None;
        let mut preview_images: Vec<Option<String>> = Vec::new();
        let mut page_texts: Vec<Option<String>> = Vec::new();
        let mut render_finished = false;
        let mut ocr_finished = false;

        while !(render_finished && ocr_finished) {
            if let Some(state) = progress_state.as_ref() {
                ensure_not_canceled(
                    observer,
                    job_id,
                    path,
                    state.combined_progress(if state.active_ocr_page.is_some() {
                        OCR_ACTIVE_PAGE_FRACTION
                    } else {
                        0.0
                    }),
                    state.active_ocr_page,
                    Some(state.total_pages),
                    Some(state.rendered_pages),
                    Some(state.completed_ocr_pages),
                )?;
            } else {
                ensure_not_canceled(
                    observer,
                    job_id,
                    path,
                    INITIAL_PROCESS_PROGRESS,
                    None,
                    None,
                    None,
                    None,
                )?;
            }

            let event = event_rx.recv().map_err(|_| {
                PipelineError::InvalidInput(
                    "PDF pipeline stopped before all pages were processed".into(),
                )
            })?;

            match event {
                PdfPipelineEvent::RenderStarted { total_pages } => {
                    if total_pages == 0 {
                        return Err(PipelineError::Pdf("PDF has no pages".into()));
                    }

                    progress_state = Some(PdfProgressState::new(total_pages));
                    preview_images = vec![None; total_pages];
                    page_texts = vec![None; total_pages];

                    emit_progress(
                        observer,
                        job_id,
                        JobStatus::Rendering,
                        INITIAL_PROCESS_PROGRESS,
                        Some(format!("Preparing page 1/{total_pages}")),
                        Some(path.to_string_lossy().into()),
                        None,
                        Some(total_pages),
                        Some(0),
                        Some(0),
                    );
                }
                PdfPipelineEvent::PageRendered(page) => {
                    let state = progress_state.as_mut().ok_or_else(|| {
                        PipelineError::Pdf("PDF page rendered before start".into())
                    })?;

                    let processed = preprocess_image_to_png(
                        &page.rendered_path,
                        preprocess_dir.path(),
                        &format!("page-{}", page.page_number),
                    )?;
                    let preview_image = encode_preview_image_data_url(&processed)?;

                    state.rendered_pages = state.rendered_pages.max(page.page_number);
                    preview_images[page.page_number - 1] = Some(preview_image.clone());

                    emit_preview(
                        observer,
                        job_id,
                        path,
                        PreviewKind::Rendered,
                        page.page_number,
                        page.total_pages,
                        &preview_image,
                        None,
                    );

                    emit_progress(
                        observer,
                        job_id,
                        state.current_status(),
                        state.combined_progress(if state.active_ocr_page.is_some() {
                            OCR_ACTIVE_PAGE_FRACTION
                        } else {
                            0.0
                        }),
                        Some(render_progress_message(state, page.page_number)),
                        Some(path.to_string_lossy().into()),
                        Some(page.page_number),
                        Some(page.total_pages),
                        Some(state.rendered_pages),
                        Some(state.completed_ocr_pages),
                    );

                    ocr_tx
                        .as_ref()
                        .ok_or(PipelineError::Canceled)?
                        .send(OcrPageJob {
                            page_number: page.page_number,
                            total_pages: page.total_pages,
                            image_path: processed,
                        })
                        .map_err(|_| PipelineError::Canceled)?;
                }
                PdfPipelineEvent::RenderFinished => {
                    render_finished = true;
                    if let Some(sender) = ocr_tx.take() {
                        drop(sender);
                    }

                    if let Some(state) = progress_state.as_ref() {
                        emit_progress(
                            observer,
                            job_id,
                            state.current_status(),
                            state.combined_progress(if state.active_ocr_page.is_some() {
                                OCR_ACTIVE_PAGE_FRACTION
                            } else {
                                0.0
                            }),
                            Some(render_finished_message(state)),
                            Some(path.to_string_lossy().into()),
                            state.active_ocr_page,
                            Some(state.total_pages),
                            Some(state.rendered_pages),
                            Some(state.completed_ocr_pages),
                        );
                    }
                }
                PdfPipelineEvent::RenderFailed(err) => return Err(err),
                PdfPipelineEvent::OcrPageStarted {
                    page_number,
                    total_pages,
                } => {
                    let state = progress_state
                        .as_mut()
                        .ok_or_else(|| PipelineError::Pdf("OCR started before rendering".into()))?;
                    let preview_image = preview_image_for_page(&preview_images, page_number)?;

                    state.active_ocr_page = Some(page_number);

                    emit_progress(
                        observer,
                        job_id,
                        JobStatus::Ocr,
                        state.combined_progress(OCR_ACTIVE_PAGE_FRACTION),
                        Some(format!(
                            "Reading page {page_number}/{total_pages}{}",
                            state.ocr_suffix()
                        )),
                        Some(path.to_string_lossy().into()),
                        Some(page_number),
                        Some(total_pages),
                        Some(state.rendered_pages),
                        Some(state.completed_ocr_pages),
                    );
                    emit_preview(
                        observer,
                        job_id,
                        path,
                        PreviewKind::Ocr,
                        page_number,
                        total_pages,
                        preview_image,
                        None,
                    );

                    let page_heading = format!("## Page {}\n\n", page_number);
                    emit_preview(
                        observer,
                        job_id,
                        path,
                        PreviewKind::Ocr,
                        page_number,
                        total_pages,
                        preview_image,
                        Some(page_heading),
                    );

                    emit_progress(
                        observer,
                        job_id,
                        JobStatus::Ocr,
                        state.combined_progress(OCR_ACTIVE_PAGE_FRACTION),
                        Some(format!(
                            "Extracting text from page {page_number}/{total_pages}{}",
                            state.ocr_suffix()
                        )),
                        Some(path.to_string_lossy().into()),
                        Some(page_number),
                        Some(total_pages),
                        Some(state.rendered_pages),
                        Some(state.completed_ocr_pages),
                    );
                }
                PdfPipelineEvent::OcrRunnerEvent {
                    page_number,
                    total_pages,
                    mode,
                    stage,
                    message,
                    chunk,
                    will_fallback,
                } => {
                    emit_runner(
                        observer,
                        job_id,
                        path,
                        Some(page_number),
                        Some(total_pages),
                        mode,
                        stage,
                        message,
                        chunk,
                        will_fallback,
                    );
                }
                PdfPipelineEvent::OcrPageFinished {
                    page_number,
                    page_markdown,
                } => {
                    let state = progress_state.as_mut().ok_or_else(|| {
                        PipelineError::Pdf("OCR finished before rendering".into())
                    })?;

                    state.completed_ocr_pages += 1;
                    state.active_ocr_page = None;
                    page_texts[page_number - 1] = Some(page_markdown);

                    emit_progress(
                        observer,
                        job_id,
                        JobStatus::Ocr,
                        state.combined_progress(0.0),
                        Some(format!(
                            "Finished page {page_number}/{}{}",
                            state.total_pages,
                            state.ocr_suffix()
                        )),
                        Some(path.to_string_lossy().into()),
                        Some(page_number),
                        Some(state.total_pages),
                        Some(state.rendered_pages),
                        Some(state.completed_ocr_pages),
                    );
                }
                PdfPipelineEvent::OcrFinished => {
                    ocr_finished = true;
                }
                PdfPipelineEvent::OcrFailed(err) => return Err(err),
            }
        }

        join_worker(render_worker, "PDF render worker")?;
        join_worker(ocr_worker, "OCR worker")?;

        collect_page_texts(page_texts)
    })
}

fn process_image_pages(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    path: &Path,
    ocr: &LlmOcrEngine,
    prompt: &str,
) -> Result<Vec<String>> {
    let tempdir = tempfile::tempdir()?;
    let out = preprocess_image_to_png(path, tempdir.path(), "image")?;
    let preview_image = encode_preview_image_data_url(&out)?;

    emit_progress(
        observer,
        job_id,
        JobStatus::Ocr,
        0.25,
        Some("Reading page 1/1".into()),
        Some(path.to_string_lossy().into()),
        Some(1),
        Some(1),
        Some(1),
        Some(0),
    );
    emit_preview(
        observer,
        job_id,
        path,
        PreviewKind::Ocr,
        1,
        1,
        &preview_image,
        None,
    );

    let mut page_markdown = "## Page 1\n\n".to_string();
    emit_preview(
        observer,
        job_id,
        path,
        PreviewKind::Ocr,
        1,
        1,
        &preview_image,
        Some(page_markdown.clone()),
    );

    emit_progress(
        observer,
        job_id,
        JobStatus::Ocr,
        0.30,
        Some("Extracting text from page 1/1".into()),
        Some(path.to_string_lossy().into()),
        Some(1),
        Some(1),
        Some(1),
        Some(0),
    );

    ensure_not_canceled(
        observer,
        job_id,
        path,
        0.30,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
    )?;

    let streamed_text = ocr.recognize_streaming(&out, prompt, |event| match event {
        OcrStreamEvent::WorkerStarting { mode, message } => emit_runner(
            observer,
            job_id,
            path,
            Some(1),
            Some(1),
            mode,
            RunnerStage::WorkerStarting,
            Some(message),
            None,
            None,
        ),
        OcrStreamEvent::ModelReady { mode, message } => emit_runner(
            observer,
            job_id,
            path,
            Some(1),
            Some(1),
            mode,
            RunnerStage::ModelReady,
            Some(message),
            None,
            None,
        ),
        OcrStreamEvent::FirstToken { mode } => emit_runner(
            observer,
            job_id,
            path,
            Some(1),
            Some(1),
            mode,
            RunnerStage::FirstToken,
            None,
            None,
            None,
        ),
        OcrStreamEvent::TextChunk { mode, chunk } => emit_runner(
            observer,
            job_id,
            path,
            Some(1),
            Some(1),
            mode,
            RunnerStage::Chunk,
            None,
            Some(chunk),
            None,
        ),
        OcrStreamEvent::RunnerError {
            mode,
            message,
            will_fallback,
        } => emit_runner(
            observer,
            job_id,
            path,
            Some(1),
            Some(1),
            mode,
            RunnerStage::Error,
            Some(message),
            None,
            Some(will_fallback),
        ),
    })?;

    ensure_not_canceled(
        observer,
        job_id,
        path,
        0.75,
        Some(1),
        Some(1),
        Some(1),
        Some(0),
    )?;

    page_markdown.push_str(streamed_text.trim());
    page_markdown.push('\n');

    emit_progress(
        observer,
        job_id,
        JobStatus::Ocr,
        0.75,
        Some("Finished page 1/1".into()),
        Some(path.to_string_lossy().into()),
        Some(1),
        Some(1),
        Some(1),
        Some(1),
    );

    Ok(vec![page_markdown])
}

fn collect_page_texts(page_texts: Vec<Option<String>>) -> Result<Vec<String>> {
    page_texts
        .into_iter()
        .enumerate()
        .map(|(idx, page)| {
            page.ok_or_else(|| {
                PipelineError::Ocr(format!("page {} did not produce markdown", idx + 1))
            })
        })
        .collect()
}

fn preview_image_for_page<'a>(
    preview_images: &'a [Option<String>],
    page_number: usize,
) -> Result<&'a str> {
    preview_images
        .get(page_number.saturating_sub(1))
        .and_then(|image| image.as_deref())
        .ok_or_else(|| PipelineError::Pdf(format!("missing preview image for page {page_number}")))
}

fn render_progress_message(state: &PdfProgressState, page_number: usize) -> String {
    if let Some(active_page) = state.active_ocr_page {
        format!(
            "Prepared page {page_number}/{} while page {active_page}/{} is still reading",
            state.total_pages, state.total_pages
        )
    } else if state.completed_ocr_pages > 0 {
        format!(
            "Prepared page {page_number}/{} after finishing {}/{}",
            state.total_pages, state.completed_ocr_pages, state.total_pages
        )
    } else {
        format!("Prepared page {page_number}/{}", state.total_pages)
    }
}

fn render_finished_message(state: &PdfProgressState) -> String {
    if let Some(active_page) = state.active_ocr_page {
        format!(
            "All {} pages are prepared; page {active_page}/{} is still reading",
            state.total_pages, state.total_pages
        )
    } else if state.completed_ocr_pages < state.total_pages {
        format!(
            "All {} pages are prepared; waiting on page {}/{}",
            state.total_pages,
            state.completed_ocr_pages + 1,
            state.total_pages
        )
    } else {
        format!("All {} pages are prepared", state.total_pages)
    }
}

fn ensure_not_canceled(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    source: &Path,
    progress: f32,
    page_number: Option<usize>,
    total_pages: Option<usize>,
    rendered_pages: Option<usize>,
    recognized_pages: Option<usize>,
) -> Result<()> {
    if !is_cancel_requested(job_id) {
        return Ok(());
    }

    emit_progress(
        observer,
        job_id,
        JobStatus::Canceled,
        progress,
        Some("Canceled".into()),
        Some(source.to_string_lossy().into()),
        page_number,
        total_pages,
        rendered_pages,
        recognized_pages,
    );
    Err(PipelineError::Canceled)
}

fn join_worker(handle: thread::ScopedJoinHandle<'_, ()>, label: &str) -> Result<()> {
    handle
        .join()
        .map_err(|_| PipelineError::InvalidInput(format!("{label} panicked")))
}

fn is_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            ALLOWED_EXT
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

fn has_substantive_ocr_text(markdown: &str) -> bool {
    if markdown.trim().is_empty() {
        return false;
    }

    // Ignore page headers that we inject ourselves; require at least some real
    // payload text after stripping those scaffolding lines.
    let page_heading = Regex::new(r"(?m)^##\s+Page\s+\d+\s*$").unwrap();
    let stripped = page_heading.replace_all(markdown, "");
    stripped.trim().chars().any(|c| c.is_alphanumeric())
}

fn preprocess_image_to_png(input: &Path, tempdir: &Path, stem: &str) -> Result<PathBuf> {
    let img = image::open(input).map_err(|e| PipelineError::InvalidInput(e.to_string()))?;
    let gray = DynamicImage::ImageLuma8(img.to_luma8());
    let normalized = if gray.width() > MAX_OCR_DIMENSION || gray.height() > MAX_OCR_DIMENSION {
        gray.thumbnail(MAX_OCR_DIMENSION, MAX_OCR_DIMENSION)
    } else {
        gray
    };

    let out = tempdir.join(format!("{stem}.png"));
    normalized
        .save(&out)
        .map_err(|e| PipelineError::InvalidInput(e.to_string()))?;
    Ok(out)
}

fn emit_progress(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    status: JobStatus,
    progress: f32,
    message: Option<String>,
    source: Option<String>,
    page_number: Option<usize>,
    total_pages: Option<usize>,
    rendered_pages: Option<usize>,
    recognized_pages: Option<usize>,
) {
    observer.on_progress(ProgressEvent {
        job_id: job_id.to_string(),
        status,
        progress,
        message,
        source,
        page_number,
        total_pages,
        rendered_pages,
        recognized_pages,
    });
}

fn emit_complete(observer: &mut dyn PipelineObserver, job_id: &str, path: &Path) {
    observer.on_completed(CompletedEvent {
        job_id: job_id.to_string(),
        output_path: path.to_string_lossy().into(),
    });
}

fn emit_error(observer: &mut dyn PipelineObserver, job_id: &str, message: &str) {
    observer.on_error(ErrorEvent {
        job_id: job_id.to_string(),
        message: message.into(),
    });
}

fn emit_preview(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    source: &Path,
    kind: PreviewKind,
    page_number: usize,
    total_pages: usize,
    image_data_url: &str,
    text_chunk: Option<String>,
) {
    observer.on_preview(PreviewEvent {
        job_id: job_id.to_string(),
        source: Some(source.to_string_lossy().into()),
        kind,
        page_number,
        total_pages,
        image_data_url: image_data_url.to_string(),
        text_chunk,
    });
}

fn emit_runner(
    observer: &mut dyn PipelineObserver,
    job_id: &str,
    source: &Path,
    page_number: Option<usize>,
    total_pages: Option<usize>,
    mode: RunnerMode,
    stage: RunnerStage,
    message: Option<String>,
    chunk: Option<String>,
    will_fallback: Option<bool>,
) {
    observer.on_runner(RunnerEvent {
        job_id: job_id.to_string(),
        source: Some(source.to_string_lossy().into()),
        page_number,
        total_pages,
        mode,
        stage,
        message,
        chunk,
        will_fallback,
    });
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
