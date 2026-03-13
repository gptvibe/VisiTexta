import ReactMarkdown from 'react-markdown'
import type { JobResult } from '../types'

type MarkdownPreviewProps = {
  job?: JobResult | null
  markdown: string
}

export function MarkdownPreview({ job, markdown }: MarkdownPreviewProps) {
  return (
    <div className="preview">
      <div className="panel-title">Markdown Preview</div>
      {!job && <div className="preview-empty">Select a job to preview.</div>}
      {job && (
        <div className="preview-content">
          <div className="preview-header">
            <div className="preview-name">{job.source}</div>
            {job.output_path && (
              <div className="preview-path">{job.output_path}</div>
            )}
          </div>
          <div className="preview-markdown">
            <ReactMarkdown>{markdown || 'No markdown loaded yet.'}</ReactMarkdown>
          </div>
        </div>
      )}
    </div>
  )
}
