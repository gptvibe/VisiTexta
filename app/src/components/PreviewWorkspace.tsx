import { MarkdownPreview } from './MarkdownPreview'
import type {
  JobResult,
  JobStreamState,
  WorkflowModeDefinition,
  WorkflowModeExport,
} from '../types'

type PreviewWorkspaceProps = {
  selectedJob: JobResult | null
  renderedMarkdown: string
  modeDefinition: WorkflowModeDefinition
  selectedStream: JobStreamState | null
  activeModelLabel: string
  runtimeLabel: string
  storageModeLabel: string
  onRetry: () => void | Promise<void>
  onCancel: () => void | Promise<void>
  onOpenOutputFolder: () => void | Promise<void>
  onRevealInExplorer: () => void | Promise<void>
  onCopyMarkdown: () => void | Promise<void>
  onExportResult: (exportId?: WorkflowModeExport['id']) => void | Promise<void>
  isCancelRequested: boolean
}

export function PreviewWorkspace({
  selectedJob,
  renderedMarkdown,
  modeDefinition,
  selectedStream,
  activeModelLabel,
  runtimeLabel,
  storageModeLabel,
  onRetry,
  onCancel,
  onOpenOutputFolder,
  onRevealInExplorer,
  onCopyMarkdown,
  onExportResult,
  isCancelRequested,
}: PreviewWorkspaceProps) {
  return (
    <section className="panel preview-panel">
      <MarkdownPreview
        key={selectedJob?.job_id || 'empty-preview'}
        job={selectedJob}
        renderedMarkdown={renderedMarkdown}
        modeDefinition={modeDefinition}
        stream={selectedStream}
        activeModelLabel={activeModelLabel}
        runtimeLabel={runtimeLabel}
        storageModeLabel={storageModeLabel}
        onRetry={onRetry}
        onCancel={onCancel}
        onOpenOutputFolder={onOpenOutputFolder}
        onRevealInExplorer={onRevealInExplorer}
        onCopyMarkdown={onCopyMarkdown}
        onExportResult={onExportResult}
        isCancelRequested={isCancelRequested}
      />
    </section>
  )
}
