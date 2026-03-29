use crate::errors::Result;
use crate::events::JobStatus;
use crate::pipeline::JobResult;
use crate::storage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

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

pub fn clear_terminal_jobs() -> Result<usize> {
    let path = storage::history_path()?;
    clear_terminal_jobs_at(&path)
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
    let path = storage::history_path()?;
    read_history_file_at(&path)
}

fn read_history_file_at(path: &Path) -> Result<JobHistoryFile> {
    let Ok(bytes) = fs::read(path) else {
        return Ok(JobHistoryFile::default());
    };

    Ok(serde_json::from_slice(&bytes).unwrap_or_default())
}

fn write_history_file(history: &JobHistoryFile) -> Result<()> {
    let path = storage::history_path()?;
    write_history_file_at(&path, history)
}

fn write_history_file_at(path: &Path, history: &JobHistoryFile) -> Result<()> {
    storage::ensure_parent_dir(path)?;
    let bytes = serde_json::to_vec_pretty(history)?;
    storage::atomic_write(path, &bytes)
}

fn clear_terminal_jobs_at(path: &Path) -> Result<usize> {
    let mut history = read_history_file_at(path)?;
    let cleared = clear_terminal_entries(&mut history);

    if cleared > 0 {
        write_history_file_at(path, &history)?;
    }

    Ok(cleared)
}

fn clear_terminal_entries(history: &mut JobHistoryFile) -> usize {
    let before = history.entries.len();
    history
        .entries
        .retain(|entry| !is_terminal_status(entry.job.status));
    before.saturating_sub(history.entries.len())
}

fn is_terminal_status(status: JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Done | JobStatus::Failed | JobStatus::Canceled
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history_entry(job_id: &str, status: JobStatus) -> HistoryEntry {
        HistoryEntry {
            recorded_at: "2026-03-29T00:00:00Z".into(),
            job: JobResult {
                job_id: job_id.into(),
                source: format!("{job_id}.pdf"),
                output_path: None,
                workflow_mode: crate::modes::default_workflow_mode(),
                status,
                error: None,
                progress: Some(0.0),
                message: None,
            },
        }
    }

    #[test]
    fn clear_terminal_entries_removes_only_finished_jobs() {
        let mut history = JobHistoryFile {
            version: HISTORY_VERSION,
            entries: vec![
                history_entry("queued", JobStatus::Queued),
                history_entry("done", JobStatus::Done),
                history_entry("failed", JobStatus::Failed),
                history_entry("canceled", JobStatus::Canceled),
                history_entry("ocr", JobStatus::Ocr),
            ],
        };

        let cleared = clear_terminal_entries(&mut history);

        assert_eq!(cleared, 3);
        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0].job.job_id, "queued");
        assert_eq!(history.entries[1].job.job_id, "ocr");
    }

    #[test]
    fn clear_terminal_jobs_persists_remaining_history() {
        let sandbox = tempfile::tempdir().unwrap();
        let history_path = sandbox.path().join("history.json");
        let history = JobHistoryFile {
            version: HISTORY_VERSION,
            entries: vec![
                history_entry("done", JobStatus::Done),
                history_entry("rendering", JobStatus::Rendering),
                history_entry("failed", JobStatus::Failed),
            ],
        };

        write_history_file_at(&history_path, &history).unwrap();

        let cleared = clear_terminal_jobs_at(&history_path).unwrap();
        let persisted = read_history_file_at(&history_path).unwrap();

        assert_eq!(cleared, 2);
        assert_eq!(persisted.entries.len(), 1);
        assert_eq!(persisted.entries[0].job.job_id, "rendering");
        assert_eq!(persisted.entries[0].job.status, JobStatus::Rendering);
    }
}
