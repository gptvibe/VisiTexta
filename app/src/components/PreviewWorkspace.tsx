import { MarkdownPreview } from './MarkdownPreview'
import type { JobResult, JobStreamState } from '../types'

type PreviewWorkspaceProps = {
  selectedJob: JobResult | null
  renderedMarkdown: string
  selectedStream: JobStreamState | null
  onRetry: () => void | Promise<void>
  onCancel: () => void | Promise<void>
  onOpenOutputFolder: () => void | Promise<void>
  onRevealInExplorer: () => void | Promise<void>
  onCopyMarkdown: () => void | Promise<void>
  isCancelRequested: boolean
}

export function PreviewWorkspace({
  selectedJob,
  renderedMarkdown,
  selectedStream,
  onRetry,
  onCancel,
  onOpenOutputFolder,
  onRevealInExplorer,
  onCopyMarkdown,
  isCancelRequested,
}: PreviewWorkspaceProps) {
  return (
    <section className="panel preview-panel">
      <MarkdownPreview
        key={selectedJob?.job_id || 'empty-preview'}
        job={selectedJob}
        renderedMarkdown={renderedMarkdown}
        stream={selectedStream}
        onRetry={onRetry}
        onCancel={onCancel}
        onOpenOutputFolder={onOpenOutputFolder}
        onRevealInExplorer={onRevealInExplorer}
        onCopyMarkdown={onCopyMarkdown}
        isCancelRequested={isCancelRequested}
      />
    </section>
  )
}
