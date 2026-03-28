use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum AppEvent {
    Progress(ProgressEvent),
    Preview(PreviewEvent),
    Runner(RunnerEvent),
    Completed(CompletedEvent),
    Error(ErrorEvent),
}

#[derive(Debug, Serialize, Clone)]
pub struct ProgressEvent {
    pub job_id: String,
    pub status: JobStatus,
    pub progress: f32,
    pub message: Option<String>,
    pub source: Option<String>,
    pub page_number: Option<usize>,
    pub total_pages: Option<usize>,
    pub rendered_pages: Option<usize>,
    pub recognized_pages: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PreviewEvent {
    pub job_id: String,
    pub source: Option<String>,
    pub kind: PreviewKind,
    pub page_number: usize,
    pub total_pages: usize,
    pub image_data_url: String,
    pub text_chunk: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RunnerEvent {
    pub job_id: String,
    pub source: Option<String>,
    pub page_number: Option<usize>,
    pub total_pages: Option<usize>,
    pub mode: RunnerMode,
    pub stage: RunnerStage,
    pub message: Option<String>,
    pub chunk: Option<String>,
    pub will_fallback: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum RunnerMode {
    Persistent,
    Transient,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum RunnerStage {
    WorkerStarting,
    ModelReady,
    FirstToken,
    Chunk,
    Error,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum PreviewKind {
    Rendered,
    Ocr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompletedEvent {
    pub job_id: String,
    pub output_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorEvent {
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum JobStatus {
    Queued,
    Rendering,
    Ocr,
    Formatting,
    Writing,
    Done,
    Failed,
    Canceled,
}
