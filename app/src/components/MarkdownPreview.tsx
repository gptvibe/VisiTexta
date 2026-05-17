import { useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import type {
  JobPreviewPage,
  JobResult,
  JobStreamState,
  WorkflowModeDefinition,
  WorkflowModeExport,
} from '../types'

type PreviewTab = 'original' | 'ocr' | 'result' | 'export'

type MarkdownPreviewProps = {
  job?: JobResult | null
  renderedMarkdown: string
  modeDefinition: WorkflowModeDefinition
  stream?: JobStreamState | null
  activeModelLabel: string
  runtimeLabel: string
  storageModeLabel: string
  onRetry?: () => void
  onCancel?: () => void
  onOpenOutputFolder?: () => void
  onRevealInExplorer?: () => void
  onCopyMarkdown?: () => void
  onExportResult?: (exportId?: WorkflowModeExport['id']) => void
  isCancelRequested?: boolean
}

function getFileName(path?: string | null) {
  if (!path) return 'Nothing selected'
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

function readSourcePageNumber(href?: string | null) {
  const match = href?.match(/^#source-page-(\d+)$/i)
  if (!match) return null
  return Number.parseInt(match[1], 10)
}

function isJobStreaming(job?: JobResult | null) {
  return job ? !['Done', 'Failed', 'Canceled'].includes(job.status) : false
}

function previewStateLabel(
  job?: JobResult | null,
  isCancelRequested?: boolean
) {
  if (!job) return 'Ready'
  if (isCancelRequested) return 'Stopping'
  if (job.status === 'Done') return 'Ready'
  if (job.status === 'Failed') return 'Needs attention'
  if (job.status === 'Canceled') return 'Canceled'
  if (job.status === 'Rendering') return 'Preparing pages'
  if (job.status === 'Formatting') return 'Cleaning text'
  if (job.status === 'Writing') return 'Saving'
  return 'Reading text'
}

export function MarkdownPreview({
  job,
  renderedMarkdown,
  modeDefinition,
  stream,
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
}: MarkdownPreviewProps) {
  const streamText = stream?.streamed_markdown?.trim() || ''
  const markdown = renderedMarkdown || streamText
  const isStreaming = isJobStreaming(job)
  const streamRef = useRef<HTMLPreElement | null>(null)
  const [selectedPageNumber, setSelectedPageNumber] = useState<number | null>(null)
  const [activeTab, setActiveTab] = useState<PreviewTab>('result')
  const canRetry = Boolean(job && !isStreaming)
  const canCancel = Boolean(job && isStreaming && !isCancelRequested)
  const canOpenFolder = Boolean(job?.output_path)
  const canReveal = Boolean(job?.output_path || job?.source)
  const canCopy = Boolean(markdown.trim())
  const canExport = Boolean(markdown.trim() || job?.output_path)
  const lazyThumbnailLimit = 8

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

  useEffect(() => {
     setActiveTab(isStreaming ? 'ocr' : 'result')
    setSelectedPageNumber(null)
  }, [isStreaming, job?.job_id])

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

  const availablePageNumbers = useMemo(
    () => new Set(pages.map((page) => page.page_number)),
    [pages]
  )

  const richPreviewSuppressed = Boolean(
    stream?.disable_rich_preview_for_large_jobs &&
      (stream?.total_pages ?? 0) >= (stream?.large_job_page_threshold ?? Number.MAX_SAFE_INTEGER) &&
      pages.length === 0
  )

  const visibleThumbnails = useMemo(() => {
    if (!stream?.lazy_preview_thumbnails || pages.length <= lazyThumbnailLimit) {
      return pages
    }

    const anchor = activePage?.page_number ?? pages[0]?.page_number ?? 1
    const anchorIndex = Math.max(
      0,
      pages.findIndex((page) => page.page_number === anchor)
    )
    const start = Math.max(0, anchorIndex - Math.floor(lazyThumbnailLimit / 2))
    return pages.slice(start, start + lazyThumbnailLimit)
  }, [activePage?.page_number, pages, stream?.lazy_preview_thumbnails])

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

  const stateLabel = previewStateLabel(job, isCancelRequested)
  const progressPercent = Math.min(100, Math.max(0, Math.round((job?.progress ?? 0) * 100)))
  const activeTabId = `preview-tab-${activeTab}`
  const activePanelId = `preview-panel-${activeTab}`
  const progressMessage =
    stream?.runner_message || job?.message || (job ? stateLabel : 'Choose a file to begin.')
  const pageStatus =
    activePage && stream?.total_pages ? `Page ${activePage.page_number} / ${stream.total_pages}` : null

  const feedback = useMemo(() => {
    if (!job) return null
    if (job.status === 'Failed') {
      return { tone: 'error', message: job.error || 'This file could not be completed.' }
    }
    if (job.status === 'Canceled') {
      return { tone: 'warning', message: 'This job was canceled before it finished.' }
    }
    if (isCancelRequested) {
      return { tone: 'warning', message: 'Stopping after the current step finishes.' }
    }
    if (job.status === 'Done') {
      return {
        tone: 'success',
        message: `${modeDefinition.result_label} is ready.`,
      }
    }
    return null
  }, [isCancelRequested, job, modeDefinition.result_label])

  const tabOptions: Array<{ id: PreviewTab; label: string; note?: string }> = [
    { id: 'original', label: 'Original', note: pageStatus || 'Page preview' },
    { id: 'ocr', label: 'OCR', note: streamStatus },
    { id: 'result', label: modeDefinition.result_label, note: stateLabel },
    { id: 'export', label: 'Export', note: `${modeDefinition.available_exports.length} formats` },
  ]

  const markdownComponents = useMemo<Components>(
    () => ({
      a: ({ href, children, node: _node, ...props }) => {
        const sourcePageNumber = readSourcePageNumber(href)
        if (sourcePageNumber) {
          const canJump = availablePageNumbers.has(sourcePageNumber)
          if (!canJump) {
            return (
              <span
                className="source-page-link unavailable"
                title="Source preview is not available in this session."
              >
                {children}
              </span>
            )
          }

          return (
            <a
              {...props}
              href={href}
              className="source-page-link"
              onClick={(event) => {
                event.preventDefault()
                setSelectedPageNumber(sourcePageNumber)
                setActiveTab('original')
              }}
              title={`Jump to source page ${sourcePageNumber}`}
            >
              {children}
            </a>
          )
        }

        return (
          <a {...props} href={href} target="_blank" rel="noreferrer">
            {children}
          </a>
        )
      },
    }),
    [availablePageNumbers]
  )

  const tabBody = (() => {
    if (activeTab === 'original') {
      return (
        <div
          className="preview-tab-panel"
          id={activePanelId}
          role="tabpanel"
          aria-labelledby={activeTabId}
        >
          <div className="preview-panel-header">
            <div>
              <div className="preview-panel-title">Original</div>
              <div className="preview-panel-copy">
                {pageStatus || 'The scanned page preview will appear here.'}
              </div>
            </div>
            {pages.length > 0 && (
              <div className="preview-page-nav compact">
                <button
                  className="btn ghost"
                  type="button"
                  onClick={() => {
                    if (!activePage) return
                    setSelectedPageNumber(Math.max(1, activePage.page_number - 1))
                  }}
                  disabled={!activePage || activePage.page_number <= 1}
                >
                  Previous
                </button>
                <button
                  className="btn ghost"
                  type="button"
                  onClick={() => {
                    if (!activePage) return
                    setSelectedPageNumber(Math.min(pages.length, activePage.page_number + 1))
                  }}
                  disabled={!activePage || activePage.page_number >= pages.length}
                >
                  Next
                </button>
              </div>
            )}
          </div>
          <div className="preview-focus-frame">
            {activePage?.image_data_url ? (
              <img className="preview-image" src={activePage.image_data_url} alt={job?.source} />
            ) : richPreviewSuppressed ? (
              <div className="preview-placeholder">
                Rich preview is disabled for this large job to keep memory use lower.
              </div>
            ) : (
              <div className="preview-placeholder">
                The current page will appear here while text is being extracted.
              </div>
            )}
          </div>
          {visibleThumbnails.length > 1 && (
            <div className="preview-thumbnail-strip" role="tablist" aria-label="Scanned pages">
              {visibleThumbnails.map((page) => (
                <button
                  key={page.page_number}
                  className={`preview-thumbnail ${page.page_number === activePage?.page_number ? 'selected' : ''}`}
                  type="button"
                  onClick={() => setSelectedPageNumber(page.page_number)}
                  role="tab"
                  aria-selected={page.page_number === activePage?.page_number}
                >
                  <img src={page.image_data_url} alt={`${job?.source} page ${page.page_number}`} />
                  <span>{`P${page.page_number}`}</span>
                </button>
              ))}
            </div>
          )}
          {stream?.lazy_preview_thumbnails && pages.length > visibleThumbnails.length && (
            <div className="preview-inline-note">
              {`Showing ${visibleThumbnails.length} nearby page thumbnails to keep the preview lighter.`}
            </div>
          )}
        </div>
      )
    }

    if (activeTab === 'ocr') {
      return (
        <div
          className="preview-tab-panel"
          id={activePanelId}
          role="tabpanel"
          aria-labelledby={activeTabId}
        >
          <div className="preview-panel-header">
            <div>
              <div className="preview-panel-title">Live OCR</div>
              <div className="preview-panel-copy">
                {streamStatus}
                {pageStatus ? ` • ${pageStatus}` : ''}
              </div>
            </div>
          </div>
          <pre ref={streamRef} className="preview-stream focus">
            {streamText ? <span className="preview-stream-text">{streamText}</span> : 'Waiting for extracted text...'}
            {isStreaming && <span className="preview-caret" aria-hidden="true" />}
          </pre>
        </div>
      )
    }

    if (activeTab === 'export') {
      return (
        <div
          className="preview-tab-panel"
          id={activePanelId}
          role="tabpanel"
          aria-labelledby={activeTabId}
        >
          <div className="preview-panel-header">
            <div>
              <div className="preview-panel-title">Export</div>
              <div className="preview-panel-copy">
                Save the current {modeDefinition.result_label.toLowerCase()} in the format you need.
              </div>
            </div>
          </div>
          <div className="preview-export-grid">
            {modeDefinition.available_exports.map((exportOption) => (
              <button
                key={exportOption.id}
                className={`preview-export-card ${exportOption.primary ? 'primary' : ''}`}
                onClick={() => onExportResult?.(exportOption.id)}
                disabled={!canExport}
                type="button"
              >
                <span className="preview-export-label">{exportOption.label}</span>
                <strong>{`.${exportOption.extension}`}</strong>
                <span>{exportOption.description}</span>
              </button>
            ))}
          </div>
          <div className="preview-export-actions">
            <button
              className="btn primary"
              type="button"
              onClick={onCopyMarkdown}
              disabled={!canCopy}
            >
              {modeDefinition.copy_action_label}
            </button>
            <button
              className="btn ghost"
              type="button"
              onClick={onOpenOutputFolder}
              disabled={!canOpenFolder}
            >
              Open output folder
            </button>
            <button
              className="btn ghost"
              type="button"
              onClick={onRevealInExplorer}
              disabled={!canReveal}
            >
              Reveal in Explorer
            </button>
          </div>
        </div>
      )
    }

    return (
      <div
        className="preview-tab-panel"
        id={activePanelId}
        role="tabpanel"
        aria-labelledby={activeTabId}
      >
        <div className="preview-panel-header">
          <div>
            <div className="preview-panel-title">{modeDefinition.result_label}</div>
            <div className="preview-panel-copy">
              Source page links will jump back to Original when preview pages are available.
            </div>
          </div>
        </div>
        <div className="preview-markdown focus">
          <ReactMarkdown components={markdownComponents}>
            {markdown || modeDefinition.empty_state_copy}
          </ReactMarkdown>
        </div>
      </div>
    )
  })()

  return (
    <div className="preview calm-preview">
      <div className="panel-title">Workspace</div>
      {!job && (
        <div className="preview-empty">
          {`Select a job from Recent jobs to inspect source pages, live OCR, ${modeDefinition.result_label.toLowerCase()}, and export actions.`}
        </div>
      )}
      {job && (
        <div className="preview-content calm">
          <div className="preview-hero">
            <div className="preview-hero-copy">
              <div className="preview-name">{getFileName(job.source)}</div>
              <div className="preview-path">{job.output_path || stream?.source || 'Working locally on this PC'}</div>
            </div>
            <div className={`preview-state ${isStreaming ? 'live' : 'done'}`}>{stateLabel}</div>
          </div>

          <div className="preview-status-bar">
            <div className="preview-status-item">
              <span>Model</span>
              <strong>{activeModelLabel}</strong>
            </div>
            <div className="preview-status-item">
              <span>Runtime</span>
              <strong>{runtimeLabel}</strong>
            </div>
            <div className="preview-status-item">
              <span>Storage</span>
              <strong>{storageModeLabel}</strong>
            </div>
            <div className="preview-status-item progress">
              <span>Progress</span>
              <strong>{`${progressPercent}%`}</strong>
              <div className="preview-progress-bar">
                <div className="preview-progress-fill" style={{ width: `${progressPercent}%` }} />
              </div>
            </div>
          </div>

          <div className="preview-live-strip">
            <div className="preview-live-copy">
              <span className="preview-live-label">Live status</span>
              <strong>{progressMessage}</strong>
            </div>
            <div className="preview-actions compact">
              <button className="btn ghost" type="button" onClick={onRetry} disabled={!canRetry}>
                Retry
              </button>
              <button className="btn ghost" type="button" onClick={onCancel} disabled={!canCancel}>
                {isCancelRequested ? 'Stopping...' : 'Cancel'}
              </button>
            </div>
          </div>

          {feedback && <div className={`preview-feedback ${feedback.tone}`}>{feedback.message}</div>}

          <div className="preview-tabs" role="tablist" aria-label="Preview workspace sections">
            {tabOptions.map((tab) => (
              <button
                key={tab.id}
                type="button"
                id={`preview-tab-${tab.id}`}
                role="tab"
                aria-controls={`preview-panel-${tab.id}`}
                className={`preview-tab ${activeTab === tab.id ? 'active' : ''}`}
                aria-selected={activeTab === tab.id}
                onClick={() => setActiveTab(tab.id)}
              >
                <span>{tab.label}</span>
                {tab.note ? <strong>{tab.note}</strong> : null}
              </button>
            ))}
          </div>

          <div className="preview-focus-shell">{tabBody}</div>
        </div>
      )}
    </div>
  )
}
