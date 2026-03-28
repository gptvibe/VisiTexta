import { FileQueue } from './FileQueue'
import type { JobResult, JobStreamState } from '../types'

type JobQueueProps = {
  jobs: JobResult[]
  selectedId: string | null
  streams: Record<string, JobStreamState>
  onSelect: (id: string) => void
}

export function JobQueue({
  jobs,
  selectedId,
  streams,
  onSelect,
}: JobQueueProps) {
  return (
    <section className="panel queue-panel">
      <FileQueue
        jobs={jobs}
        selectedId={selectedId}
        streams={streams}
        onSelect={onSelect}
      />
    </section>
  )
}
