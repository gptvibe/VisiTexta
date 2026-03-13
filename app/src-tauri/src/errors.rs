use thiserror::Error;

pub type Result<T> = std::result::Result<T, PipelineError>;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("pdf render failed: {0}")]
    Pdf(String),
    #[error("ocr failed: {0}")]
    Ocr(String),
    #[error("llm failed: {0}")]
    Llm(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("operation canceled")]
    Canceled,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
