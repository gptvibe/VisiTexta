use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum AppEvent {
    Progress(ProgressEvent),
    Preview(PreviewEvent),
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
}

#[derive(Debug, Serialize, Clone)]
pub struct PreviewEvent {
    pub job_id: String,
    pub markdown_chunk: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CompletedEvent {
    pub job_id: String,
    pub output_path: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ErrorEvent {
    pub job_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
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
