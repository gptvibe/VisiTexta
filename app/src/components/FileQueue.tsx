import { useMemo, useState } from 'react'
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

type QueueFilter = 'all' | 'active' | 'finished'

function getFileName(path: string) {
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

function getFileBadge(path: string) {
  const match = path.match(/\.([^.\\/]+)$/)
  const ext = match?.[1]?.toUpperCase() || 'FILE'

  if (ext === 'JPEG') return 'JPG'
  if (ext.length <= 4) return ext
  return ext.slice(0, 4)
}

function formatCount(value: number, label: string) {
  return `${value} ${label}${value === 1 ? '' : 's'}`
}

function isTerminalStatus(status: JobResult['status']) {
  return ['Done', 'Failed', 'Canceled'].includes(status)
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
  const [filter, setFilter] = useState<QueueFilter>('all')
  const visibleJobs = useMemo(() => {
    if (filter === 'active') {
      return jobs.filter((job) => !isTerminalStatus(job.status))
    }
    if (filter === 'finished') {
      return jobs.filter((job) => isTerminalStatus(job.status))
    }
    return jobs
  }, [filter, jobs])

  return (
    <div className="queue">
      <div className="queue-header">
        <div className="queue-header-copy">
          <div className="panel-title queue-title">Recent jobs</div>
          <div className="queue-counts">
            <span className="queue-count-pill">{formatCount(activeCount, 'active')}</span>
            <span className="queue-count-pill">{formatCount(finishedCount, 'finished')}</span>
          </div>
        </div>
        <div className="queue-header-side">
          <div className="queue-filters" role="group" aria-label="Job list filters">
            <button
              type="button"
              className={`queue-filter ${filter === 'all' ? 'active' : ''}`}
              aria-pressed={filter === 'all'}
              onClick={() => setFilter('all')}
            >
              All
            </button>
            <button
              type="button"
              className={`queue-filter ${filter === 'active' ? 'active' : ''}`}
              aria-pressed={filter === 'active'}
              onClick={() => setFilter('active')}
            >
              Active
            </button>
            <button
              type="button"
              className={`queue-filter ${filter === 'finished' ? 'active' : ''}`}
              aria-pressed={filter === 'finished'}
              onClick={() => setFilter('finished')}
            >
              Finished
            </button>
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
          <div className="queue-empty">
            No recent jobs yet. Start from the workspace to review pages, OCR output, and exports
            here.
          </div>
        )}
        {jobs.length > 0 && visibleJobs.length === 0 && (
          <div className="queue-empty">
            {filter === 'active'
              ? 'No active jobs right now.'
              : 'No finished jobs in the current history.'}
          </div>
        )}
        {visibleJobs.map((job) => {
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
          const fileBadge = getFileBadge(job.source)

          return (
            <button
              key={job.job_id}
              className={`queue-item ${isSelected ? 'selected' : ''}`}
              onClick={() => onSelect(job.job_id)}
              type="button"
            >
              <div className="queue-item-shell">
                <div className={`queue-file-icon ${statusTone[job.status] || 'warn'}`}>
                  {fileBadge}
                </div>
                <div className="queue-item-copy">
                  <div className="queue-row queue-item-top">
                    <div className="queue-primary">
                      <div className="queue-name">{displayName}</div>
                    </div>
                    <div className="queue-status-stack">
                      <span className={`status-pill ${statusTone[job.status] || 'warn'}`}>
                        {statusLabel[job.status] || job.status}
                      </span>
                      <span className="queue-progress-value">{progress}%</span>
                    </div>
                  </div>
                  <div className="queue-secondary" title={job.source}>
                    {job.source}
                  </div>
                  <div className="queue-meta">
                    <span className="queue-detail" title={detail || statusLabel[job.status] || job.status}>
                      {detail || statusLabel[job.status] || job.status}
                    </span>
                  </div>
                  <div className="queue-progress" aria-hidden="true">
                    <div className="queue-bar" style={{ width: `${progress}%` }} />
                  </div>
                </div>
              </div>
            </button>
          )
        })}
      </div>
    </div>
  )
}


