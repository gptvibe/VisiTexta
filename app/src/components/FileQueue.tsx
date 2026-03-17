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

type FileQueueProps = {
  jobs: JobResult[]
  selectedId?: string | null
  streams?: Record<string, JobStreamState>
  onSelect: (jobId: string) => void
}

export function FileQueue({ jobs, selectedId, streams, onSelect }: FileQueueProps) {
  return (
    <div className="queue">
      <div className="panel-title">File Queue</div>
      <div className="queue-list">
        {jobs.length === 0 && (
          <div className="queue-empty">Drop a file to start.</div>
        )}
        {jobs.map((job) => {
          const progress = Math.round((job.progress ?? 0) * 100)
          const isSelected = job.job_id === selectedId
          const stream = streams?.[job.job_id]
          const pageLabel =
            stream?.current_page && stream?.total_pages
              ? `Page ${stream.current_page}/${stream.total_pages}`
              : null
          const detail = job.error ?? pageLabel ?? job.message ?? ''

          return (
            <button
              key={job.job_id}
              className={`queue-item ${isSelected ? 'selected' : ''}`}
              onClick={() => onSelect(job.job_id)}
            >
              <div className="queue-row">
                <div className="queue-name">{job.source}</div>
                <span className={`status-pill ${statusTone[job.status] || 'warn'}`}>
                  {job.status}
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


