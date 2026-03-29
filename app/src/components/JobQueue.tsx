import { FileQueue } from './FileQueue'
import type { JobResult, JobStreamState } from '../types'

type JobQueueProps = {
  jobs: JobResult[]
  activeCount: number
  finishedCount: number
  selectedId: string | null
  streams: Record<string, JobStreamState>
  onClearFinished: () => void
  onSelect: (id: string) => void
}

export function JobQueue({
  jobs,
  activeCount,
  finishedCount,
  selectedId,
  streams,
  onClearFinished,
  onSelect,
}: JobQueueProps) {
  return (
    <section className="panel queue-panel">
      <FileQueue
        jobs={jobs}
        activeCount={activeCount}
        finishedCount={finishedCount}
        selectedId={selectedId}
        streams={streams}
        onClearFinished={onClearFinished}
        onSelect={onSelect}
      />
    </section>
  )
}
