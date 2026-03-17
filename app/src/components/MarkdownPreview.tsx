import { useEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import type { JobPreviewPage, JobResult, JobStreamState } from '../types'

type MarkdownPreviewProps = {
  job?: JobResult | null
  renderedMarkdown: string
  stream?: JobStreamState | null
}

export function MarkdownPreview({ job, renderedMarkdown, stream }: MarkdownPreviewProps) {
  const streamText = stream?.streamed_markdown?.trim() || ''
  const markdown = renderedMarkdown || streamText
  const isStreaming = job ? !['Done', 'Failed', 'Canceled'].includes(job.status) : false
  const streamRef = useRef<HTMLPreElement | null>(null)
  const [selectedPageNumber, setSelectedPageNumber] = useState<number | null>(null)
  const [animatedStream, setAnimatedStream] = useState('')

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
  }, [stream?.current_page, stream?.pages, stream?.preview_image_data_url])

  const activePage = useMemo(() => {
    if (!pages.length) return null
    return (
      pages.find((page) => page.page_number === selectedPageNumber) ||
      pages.find((page) => page.page_number === stream?.current_page) ||
      pages[0]
    )
  }, [pages, selectedPageNumber, stream?.current_page])

  useEffect(() => {
    setSelectedPageNumber(stream?.current_page || pages[0]?.page_number || null)
  }, [job?.job_id])

  useEffect(() => {
    if (!pages.length) {
      setSelectedPageNumber(null)
      return
    }

    if (isStreaming && stream?.current_page) {
      setSelectedPageNumber(stream.current_page)
      return
    }

    if (!selectedPageNumber || !pages.some((page) => page.page_number === selectedPageNumber)) {
      setSelectedPageNumber(stream?.current_page || pages[0].page_number)
    }
  }, [isStreaming, pages, selectedPageNumber, stream?.current_page])

  useEffect(() => {
    if (!streamText) {
      setAnimatedStream('')
      return
    }

    if (!streamText.startsWith(animatedStream)) {
      setAnimatedStream(streamText)
      return
    }

    if (animatedStream === streamText) {
      return
    }

    const pendingLength = streamText.length - animatedStream.length
    const step = Math.min(36, Math.max(6, Math.ceil(pendingLength / 14)))
    const timer = window.setTimeout(() => {
      setAnimatedStream(streamText.slice(0, animatedStream.length + step))
    }, 18)

    return () => window.clearTimeout(timer)
  }, [animatedStream, streamText])

  useEffect(() => {
    const element = streamRef.current
    if (!element) return
    element.scrollTop = element.scrollHeight
  }, [animatedStream])

  return (
    <div className="preview">
      <div className="panel-title">Live Workspace</div>
      {!job && <div className="preview-empty">Select a job to preview.</div>}
      {job && (
        <div className="preview-content">
          <div className="preview-header">
            <div>
              <div className="preview-name">{job.source}</div>
              <div className="preview-path">{job.output_path || stream?.source || 'Streaming locally'}</div>
            </div>
            <div className={`preview-state ${isStreaming ? 'live' : 'done'}`}>
              {isStreaming ? 'Live' : job.status}
            </div>
          </div>
          <div className="preview-grid">
            <div className="preview-stage">
              <div className="preview-section-header">
                <span>Page browser</span>
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
                    {activePage ? `Viewing page ${activePage.page_number}` : 'Waiting for page'}
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
                    The current PDF page or image will appear here while OCR runs.
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
                  <span>Streaming OCR</span>
                  <span>{isStreaming ? 'Typing live' : 'Finalized'}</span>
                </div>
                <pre ref={streamRef} className="preview-stream">
                  {animatedStream ? <span className="preview-stream-text">{animatedStream}</span> : 'Waiting for OCR output...'}
                  {isStreaming && <span className="preview-caret" aria-hidden="true" />}
                </pre>
              </div>

              <div className="preview-rendered">
                <div className="preview-section-header">
                  <span>Markdown render</span>
                  <span>{job.status}</span>
                </div>
                <div className="preview-markdown">
                  <ReactMarkdown>{markdown || 'No markdown loaded yet.'}</ReactMarkdown>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
