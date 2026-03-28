use crate::errors::Result;
use crate::events::JobStatus;
use crate::pipeline::JobResult;
use crate::storage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;

const HISTORY_VERSION: u8 = 1;
const MAX_HISTORY_ENTRIES: usize = 100;
const INTERRUPTED_MESSAGE: &str = "VisiTexta closed before this job finished.";

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct JobHistoryFile {
    version: u8,
    entries: Vec<HistoryEntry>,
}

impl Default for JobHistoryFile {
    fn default() -> Self {
        Self {
            version: HISTORY_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryEntry {
    recorded_at: String,
    job: JobResult,
}

pub fn load_recent_jobs() -> Result<Vec<JobResult>> {
    let history = read_history_file()?;
    Ok(history.entries.into_iter().map(|entry| entry.job).collect())
}

pub fn record_job(job: &JobResult) -> Result<()> {
    let mut history = read_history_file()?;
    history
        .entries
        .retain(|entry| entry.job.job_id != job.job_id);
    history.entries.insert(
        0,
        HistoryEntry {
            recorded_at: Utc::now().to_rfc3339(),
            job: job.clone(),
        },
    );
    history.entries.truncate(MAX_HISTORY_ENTRIES);
    write_history_file(&history)
}

pub fn recover_interrupted_jobs() -> Result<()> {
    let mut history = read_history_file()?;
    let mut changed = false;

    for entry in &mut history.entries {
        if is_terminal_status(entry.job.status) {
            continue;
        }

        entry.job.status = JobStatus::Failed;
        entry.job.output_path = None;
        entry.job.error = Some(INTERRUPTED_MESSAGE.into());
        entry.job.progress = Some(1.0);
        entry.job.message = Some("Needs attention".into());
        entry.recorded_at = Utc::now().to_rfc3339();
        changed = true;
    }

    if changed {
        write_history_file(&history)?;
    }

    Ok(())
}

fn read_history_file() -> Result<JobHistoryFile> {
    let path = storage::storage_paths()?.history_path;
    let Ok(bytes) = fs::read(path) else {
        return Ok(JobHistoryFile::default());
    };

    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

fn write_history_file(history: &JobHistoryFile) -> Result<()> {
    let path = storage::storage_paths()?.history_path;
    storage::ensure_parent_dir(&path)?;
    let bytes = serde_json::to_vec_pretty(history)?;
    storage::atomic_write(&path, &bytes)
}

fn is_terminal_status(status: JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Done | JobStatus::Failed | JobStatus::Canceled
    )
}
