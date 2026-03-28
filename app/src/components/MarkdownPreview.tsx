import { useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import type { JobPreviewPage, JobResult, JobStreamState } from '../types'

type MarkdownPreviewProps = {
  job?: JobResult | null
  renderedMarkdown: string
  stream?: JobStreamState | null
  onRetry?: () => void
  onCancel?: () => void
  onOpenOutputFolder?: () => void
  onRevealInExplorer?: () => void
  onCopyMarkdown?: () => void
  isCancelRequested?: boolean
}

function getFileName(path?: string | null) {
  if (!path) return 'Nothing selected'
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

export function MarkdownPreview({
  job,
  renderedMarkdown,
  stream,
  onRetry,
  onCancel,
  onOpenOutputFolder,
  onRevealInExplorer,
  onCopyMarkdown,
  isCancelRequested,
}: MarkdownPreviewProps) {
  const streamText = stream?.streamed_markdown?.trim() || ''
  const markdown = renderedMarkdown || streamText
  const isStreaming = job ? !['Done', 'Failed', 'Canceled'].includes(job.status) : false
  const streamRef = useRef<HTMLPreElement | null>(null)
  const [selectedPageNumber, setSelectedPageNumber] = useState<number | null>(null)
  const canRetry = Boolean(job && !isStreaming)
  const canCancel = Boolean(job && isStreaming && !isCancelRequested)
  const canOpenFolder = Boolean(job?.output_path)
  const canReveal = Boolean(job?.output_path || job?.source)
  const canCopy = Boolean(markdown.trim())

  const pages = useMemo<JobPreviewPage[]>(() => {
    if (stream?.pages?.length) {
      return stream.pages
    }

    if (stream?.preview_image_data_url) {
      return [
        {
          page_number: stream.current_page || 1,
          image_data_url: stream.preview_image_data_url,
        },
      ]
    }

    return []
  }, [stream])

  const resolvedSelectedPageNumber =
    selectedPageNumber && pages.some((page) => page.page_number === selectedPageNumber)
      ? selectedPageNumber
      : stream?.current_page || pages[0]?.page_number || null

  const activePage = useMemo(() => {
    if (!pages.length) return null
    return (
      pages.find((page) => page.page_number === resolvedSelectedPageNumber) ||
      pages.find((page) => page.page_number === stream?.current_page) ||
      pages[0]
    )
  }, [pages, resolvedSelectedPageNumber, stream?.current_page])

  useEffect(() => {
    const element = streamRef.current
    if (!element) return
    element.scrollTop = element.scrollHeight
  }, [streamText])

  const streamStatus = useMemo(() => {
    if (!stream) return isStreaming ? 'Working' : 'Ready'

    if (stream.runner_stage === 'WorkerStarting') {
      return stream.runner_mode === 'Persistent' ? 'Starting local engine' : 'Starting OCR'
    }

    if (stream.runner_stage === 'ModelReady') {
      return stream.runner_mode === 'Persistent' ? 'Local engine ready' : 'OCR ready'
    }

    if (stream.runner_stage === 'FirstToken' || stream.runner_stage === 'Chunk') {
      return 'Reading text'
    }

    if (stream.runner_stage === 'Error') {
      return stream.runner_message || 'Trying again'
    }

    return isStreaming ? 'Working' : 'Ready'
  }, [isStreaming, stream])

  const previewStateLabel = useMemo(() => {
    if (!job) return 'Ready'
    if (isCancelRequested) return 'Stopping'
    if (job.status === 'Done') return 'Ready'
    if (job.status === 'Failed') return 'Needs attention'
    if (job.status === 'Canceled') return 'Canceled'
    if (job.status === 'Rendering') return 'Preparing pages'
    if (job.status === 'Formatting') return 'Cleaning text'
    if (job.status === 'Writing') return 'Saving'
    return 'Reading text'
  }, [isCancelRequested, job])

  const feedback = useMemo(() => {
    if (!job) return null
    if (job.status === 'Done') {
      return { tone: 'success', message: 'Markdown is ready. You can copy it or open the folder.' }
    }
    if (job.status === 'Failed') {
      return { tone: 'error', message: job.error || 'This file could not be completed.' }
    }
    if (job.status === 'Canceled') {
      return { tone: 'warning', message: 'This job was canceled before it finished.' }
    }
    if (isCancelRequested) {
      return { tone: 'warning', message: 'Stopping after the current step finishes.' }
    }
    return null
  }, [isCancelRequested, job])

  return (
    <div className="preview">
      <div className="panel-title">Preview</div>
      {!job && <div className="preview-empty">Choose a job to see pages, live text, and markdown.</div>}
      {job && (
        <div className="preview-content">
          <div className="preview-header">
            <div>
              <div className="preview-name">{getFileName(job.source)}</div>
              <div className="preview-path">{job.output_path || stream?.source || 'Working locally on this PC'}</div>
            </div>
            <div className={`preview-state ${isStreaming ? 'live' : 'done'}`}>
              {previewStateLabel}
            </div>
          </div>
          <div className="preview-actions">
            <button className="btn ghost" onClick={onRetry} disabled={!canRetry}>
              Retry
            </button>
            <button className="btn ghost" onClick={onCancel} disabled={!canCancel}>
              {isCancelRequested ? 'Stopping...' : 'Cancel'}
            </button>
            <button className="btn ghost" onClick={onOpenOutputFolder} disabled={!canOpenFolder}>
              Open output folder
            </button>
            <button className="btn ghost" onClick={onRevealInExplorer} disabled={!canReveal}>
              Reveal in Explorer
            </button>
            <button className="btn primary" onClick={onCopyMarkdown} disabled={!canCopy}>
              Copy markdown
            </button>
          </div>
          {feedback && <div className={`preview-feedback ${feedback.tone}`}>{feedback.message}</div>}
          <div className="preview-grid">
            <div className="preview-stage">
              <div className="preview-section-header">
                <span>Pages</span>
                {activePage && stream?.total_pages && (
                  <span>{`Page ${activePage.page_number} / ${stream.total_pages}`}</span>
                )}
              </div>
              {pages.length > 0 && (
                <div className="preview-page-nav">
                  <button
                    className="btn ghost"
                    onClick={() => {
                      if (!activePage) return
                      setSelectedPageNumber(Math.max(1, activePage.page_number - 1))
                    }}
                    disabled={!activePage || activePage.page_number <= 1}
                  >
                    Previous
                  </button>
                  <div className="preview-page-label">
                    {activePage ? `Showing page ${activePage.page_number}` : 'Waiting for a page'}
                  </div>
                  <button
                    className="btn ghost"
                    onClick={() => {
                      if (!activePage) return
                      setSelectedPageNumber(
                        Math.min(pages.length, activePage.page_number + 1)
                      )
                    }}
                    disabled={!activePage || activePage.page_number >= pages.length}
                  >
                    Next
                  </button>
                </div>
              )}
              <div className="preview-frame">
                {activePage?.image_data_url ? (
                  <img
                    className="preview-image"
                    src={activePage.image_data_url}
                    alt={job.source}
                  />
                ) : (
                  <div className="preview-placeholder">
                    The current page will appear here while text is being extracted.
                  </div>
                )}
              </div>
              {pages.length > 1 && (
                <div className="preview-thumbnails" role="tablist" aria-label="Scanned pages">
                  {pages.map((page) => (
                    <button
                      key={page.page_number}
                      className={`preview-thumbnail ${page.page_number === activePage?.page_number ? 'selected' : ''}`}
                      onClick={() => setSelectedPageNumber(page.page_number)}
                      role="tab"
                      aria-selected={page.page_number === activePage?.page_number}
                    >
                      <img src={page.image_data_url} alt={`${job.source} page ${page.page_number}`} />
                      <span>{`P${page.page_number}`}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="preview-stack">
              <div className="preview-console">
                <div className="preview-section-header">
                  <span>Live text</span>
                  <span>{streamStatus}</span>
                </div>
                <pre ref={streamRef} className="preview-stream">
                  {streamText ? <span className="preview-stream-text">{streamText}</span> : 'Waiting for extracted text...'}
                  {isStreaming && <span className="preview-caret" aria-hidden="true" />}
                </pre>
              </div>

              <div className="preview-rendered">
                <div className="preview-section-header">
                  <span>Markdown</span>
                  <span>{previewStateLabel}</span>
                </div>
                <div className="preview-markdown">
                  <ReactMarkdown>{markdown || 'Markdown will appear here when text is ready.'}</ReactMarkdown>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
