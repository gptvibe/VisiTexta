import type { JobResult, JobStreamState } from '../types'

const statusTone: Record<string, string> = {
  Done: 'ok',
  Failed: 'bad',
  Queued: 'warn',
  Rendering: 'warn',
  Ocr: 'warn',
  Formatting: 'warn',
  Writing: 'warn',
  Canceled: 'bad',
}

const statusLabel: Record<string, string> = {
  Done: 'Ready',
  Failed: 'Needs attention',
  Queued: 'Waiting',
  Rendering: 'Preparing pages',
  Ocr: 'Reading text',
  Formatting: 'Cleaning text',
  Writing: 'Saving',
  Canceled: 'Canceled',
}

type FileQueueProps = {
  jobs: JobResult[]
  activeCount: number
  finishedCount: number
  selectedId?: string | null
  streams?: Record<string, JobStreamState>
  onClearFinished: () => void
  onSelect: (jobId: string) => void
}

function getFileName(path: string) {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

function formatCount(value: number, label: string) {
  return `${value} ${label}${value === 1 ? '' : 's'}`
}

export function FileQueue({
  jobs,
  activeCount,
  finishedCount,
  selectedId,
  streams,
  onClearFinished,
  onSelect,
}: FileQueueProps) {
  return (
    <div className="queue">
      <div className="queue-header">
        <div className="panel-title queue-title">Recent jobs</div>
        <div className="queue-header-side">
          <div className="queue-counts">
            <span>{formatCount(activeCount, 'active')}</span>
            <span>{formatCount(finishedCount, 'finished')}</span>
          </div>
          <button
            className="btn ghost queue-clear-btn"
            disabled={finishedCount === 0}
            onClick={() => onClearFinished()}
            type="button"
          >
            Clear finished
          </button>
        </div>
      </div>
      <div className="queue-list">
        {jobs.length === 0 && (
          <div className="queue-empty">No files yet. Drop, paste, or choose a file to start.</div>
        )}
        {jobs.map((job) => {
          const progress = Math.round((job.progress ?? 0) * 100)
          const isSelected = job.job_id === selectedId
          const stream = streams?.[job.job_id]
          const pageLabel =
            stream?.current_page && stream?.total_pages
              ? `Page ${stream.current_page}/${stream.total_pages}`
              : null
          const detail =
            job.error ?? stream?.runner_message ?? job.message ?? pageLabel ?? ''
          const displayName = getFileName(job.source)

          return (
            <button
              key={job.job_id}
              className={`queue-item ${isSelected ? 'selected' : ''}`}
              onClick={() => onSelect(job.job_id)}
            >
              <div className="queue-row">
                <div className="queue-primary">
                  <div className="queue-name">{displayName}</div>
                  <div className="queue-secondary">{job.source}</div>
                </div>
                <span className={`status-pill ${statusTone[job.status] || 'warn'}`}>
                  {statusLabel[job.status] || job.status}
                </span>
              </div>
              <div className="queue-progress">
                <div className="queue-bar" style={{ width: `${progress}%` }} />
              </div>
              <div className="queue-meta">
                <span>{progress}%</span>
                <span className="queue-detail">{detail}</span>
              </div>
            </button>
          )
        })}
      </div>
    </div>
  )
}


