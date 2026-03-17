import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { DropZone } from './components/DropZone'
import { FileQueue } from './components/FileQueue'
import { MarkdownPreview } from './components/MarkdownPreview'
import { SettingsDrawer } from './components/SettingsDrawer'
import { ToastNotifications, type Toast } from './components/ToastNotifications'
import type {
  AppEvent,
  JobPreviewPage,
  JobResult,
  JobStatus,
  JobStreamState,
  ModelDownloadEvent,
} from './types'
import './App.css'

type Settings = {
  threads: number
  dpi: number
  chunk_size: number
  auto_open: boolean
  theme?: string | null
  model_file?: string | null
}

type ModelDownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'done' | 'error'
  progress: number
  message?: string | null
  file_name?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

const defaultSettings: Settings = {
  threads: 4,
  dpi: 300,
  chunk_size: 3000,
  auto_open: false,
  theme: 'dark',
  model_file: null,
}

const defaultDownloadState: ModelDownloadState = {
  status: 'idle',
  progress: 0,
}

const DEFAULT_MODEL_REPO = 'unsloth/Qwen2.5-VL-3B-Instruct-GGUF'
const DEFAULT_PROMPT = 'Extract all text from the image and return it as markdown.'

function isActiveStatus(status: JobStatus) {
  return !['Done', 'Failed', 'Canceled'].includes(status)
}

function mergeJobs(previous: JobResult[], incoming: JobResult[]) {
  const next = [...previous]

  for (const job of incoming) {
    const index = next.findIndex((item) => item.job_id === job.job_id)
    if (index === -1) {
      next.unshift(job)
      continue
    }

    next[index] = {
      ...next[index],
      ...job,
      progress: job.progress ?? next[index].progress,
      message: job.message ?? next[index].message,
      error: job.error ?? next[index].error,
    }
  }

  return next
}

function upsertPreviewPage(
  pages: JobPreviewPage[] | undefined,
  update: JobPreviewPage
) {
  const current = pages ?? []
  const index = current.findIndex((page) => page.page_number === update.page_number)

  if (index === -1) {
    return [...current, update].sort((left, right) => left.page_number - right.page_number)
  }

  const next = [...current]
  next[index] = {
    ...next[index],
    ...update,
    text_chunk: update.text_chunk ?? next[index].text_chunk ?? null,
  }
  return next
}

function App() {
  const [busy, setBusy] = useState(false)
  const [jobs, setJobs] = useState<JobResult[]>([])
  const [streams, setStreams] = useState<Record<string, JobStreamState>>({})
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [markdown, setMarkdown] = useState('')
  const [log, setLog] = useState('Drop files or select to start.')
  const [modelMissing, setModelMissing] = useState(false)
  const [settings, setSettings] = useState<Settings>(defaultSettings)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [toasts, setToasts] = useState<Toast[]>([])
  const [models, setModels] = useState<string[]>([])
  const [modelInput, setModelInput] = useState(DEFAULT_MODEL_REPO)
  const [downloadState, setDownloadState] = useState<ModelDownloadState>(defaultDownloadState)
  const [prompt, setPrompt] = useState('')

  const selectedJob = useMemo(
    () => jobs.find((job) => job.job_id === selectedId) || null,
    [jobs, selectedId]
  )

  const selectedStream = useMemo(
    () => (selectedId ? streams[selectedId] || null : null),
    [selectedId, streams]
  )

  const activeJobs = useMemo(
    () => jobs.filter((job) => isActiveStatus(job.status)).length,
    [jobs]
  )

  const completedJobs = useMemo(
    () => jobs.filter((job) => job.status === 'Done').length,
    [jobs]
  )

  const selectedRenderedMarkdown = useMemo(() => {
    const streamText = selectedStream?.streamed_markdown?.trim() || ''
    if (selectedJob?.status === 'Done') {
      return markdown || streamText
    }
    return streamText || markdown
  }, [markdown, selectedJob?.status, selectedStream?.streamed_markdown])

  useEffect(() => {
    const dropListener = listen<string[]>('tauri://file-drop', async (event) => {
      if (Array.isArray(event.payload)) {
        await handlePaths(event.payload)
      }
    })

    return () => {
      dropListener.then((unlisten) => unlisten())
    }
  }, [])

  useEffect(() => {
    const registrations: Promise<() => void>[] = [
      listen<AppEvent>('job-progress', (event) => handleAppEvent(event.payload)),
      listen<AppEvent>('job-preview', (event) => handleAppEvent(event.payload)),
      listen<AppEvent>('job-complete', (event) => handleAppEvent(event.payload)),
      listen<AppEvent>('job-error', (event) => handleAppEvent(event.payload)),
      listen<ModelDownloadEvent>('model-download-progress', (event) => {
        const payload = event.payload
        setDownloadState({
          status: (payload.status as ModelDownloadState['status']) || 'downloading',
          progress: payload.progress ?? 0,
          message: payload.message ?? null,
          file_name: payload.file_name ?? null,
          downloaded_bytes: payload.downloaded_bytes ?? 0,
          total_bytes: payload.total_bytes ?? null,
        })
      }),
    ]

    return () => {
      registrations.forEach((promise) => promise.then((unlisten) => unlisten()))
    }
  }, [])

  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((result) => setSettings(result))
      .catch(() => setSettings(defaultSettings))
  }, [])

  useEffect(() => {
    refreshModelStatus()
  }, [settings.model_file])

  useEffect(() => {
    loadModels()
  }, [])

  useEffect(() => {
    if (settingsOpen) loadModels()
  }, [settingsOpen])

  useEffect(() => {
    if (!selectedJob?.output_path) {
      setMarkdown('')
      return
    }

    invoke<string>('read_markdown_file', { path: selectedJob.output_path })
      .then((content) => setMarkdown(content))
      .catch(() => setMarkdown('Failed to load markdown.'))
  }, [selectedJob?.output_path])

  function enqueueToast(message: string, tone: Toast['tone'] = 'info') {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    setToasts((prev) => [...prev, { id, message, tone }])
    setTimeout(() => {
      setToasts((prev) => prev.filter((toast) => toast.id !== id))
    }, 4000)
  }

  function upsertJob(update: Partial<JobResult> & { job_id: string }) {
    setJobs((prev) => {
      const idx = prev.findIndex((job) => job.job_id === update.job_id)
      if (idx === -1) {
        return [
          {
            job_id: update.job_id,
            source: update.source ?? 'Unknown',
            status: update.status ?? 'Queued',
            output_path: update.output_path ?? null,
            error: update.error ?? null,
            progress: update.progress ?? 0,
            message: update.message ?? null,
          },
          ...prev,
        ]
      }
      const clone = [...prev]
      clone[idx] = { ...clone[idx], ...update }
      return clone
    })
  }

  function handleAppEvent(event?: AppEvent | null) {
    if (!event) return

    if (event.type === 'Progress') {
      const { job_id, status, progress, message, source } = event.data
      upsertJob({
        job_id,
        status,
        progress,
        message: message ?? null,
        source: source ?? undefined,
      })
      setSelectedId((current) => current ?? job_id)
      if (message) {
        setLog(`${status} • ${message}`)
      }
    } else if (event.type === 'Preview') {
      const { job_id, image_data_url, page_number, total_pages, text_chunk, source } = event.data
      setStreams((prev) => {
        const current = prev[job_id] || { streamed_markdown: '' }
        const nextStream = text_chunk
          ? `${current.streamed_markdown}${current.streamed_markdown ? '\n' : ''}${text_chunk}`
          : current.streamed_markdown
        const nextPages = upsertPreviewPage(current.pages, {
          page_number,
          image_data_url,
          text_chunk: text_chunk ?? null,
        })

        return {
          ...prev,
          [job_id]: {
            ...current,
            source: source ?? current.source ?? null,
            current_page: page_number,
            total_pages: total_pages,
            preview_image_data_url: image_data_url,
            streamed_markdown: nextStream,
            pages: nextPages,
          },
        }
      })
      setSelectedId((current) => current ?? job_id)
      setLog(`Scanning page ${page_number}/${total_pages}`)
    } else if (event.type === 'Completed') {
      const { job_id, output_path } = event.data
      upsertJob({
        job_id,
        status: 'Done',
        output_path,
        progress: 1,
        message: 'Done',
      })
      setLog(`Finished ${output_path}`)
      enqueueToast('Markdown ready.', 'success')
      setSelectedId((current) => current ?? job_id)
    } else if (event.type === 'Error') {
      const { job_id, message } = event.data
      upsertJob({
        job_id,
        status: 'Failed',
        error: message,
        message,
      })
      setLog(`Error: ${message}`)
      enqueueToast(message, 'error')
    }
  }

  async function refreshModelStatus() {
    try {
      const exists = await invoke<boolean>('check_model_exists')
      setModelMissing(!exists)
    } catch {
      setModelMissing(false)
    }
  }

  async function loadModels() {
    try {
      const list = await invoke<string[]>('list_models')
      setModels(list)
    } catch (err) {
      console.error(err)
      setModels([])
    }
  }

  async function handlePaths(paths: string[]) {
    if (!paths.length) return
    if (modelMissing) {
      setLog('Model missing. Open Settings and download a vision model to continue.')
      enqueueToast('Download a vision model in Settings.', 'error')
      return
    }

    setBusy(true)
    setLog(`Processing ${paths.length} file(s)...`)

    try {
      const result = (await invoke('enqueue_jobs', { paths, prompt: prompt.trim() || null })) as JobResult[]
      setJobs((prev) => mergeJobs(prev, result))
      if (result.length) {
        setSelectedId((current) => current ?? result[0].job_id)
      }
    } catch (err) {
      console.error(err)
      setLog('Failed to enqueue jobs.')
      enqueueToast('Failed to enqueue jobs.', 'error')
    } finally {
      setBusy(false)
    }
  }

  async function onBrowseFiles() {
    const selection = await open({
      multiple: true,
      filters: [
        { name: 'All files', extensions: ['*'] },
        { name: 'Images', extensions: ['png', 'jpg', 'jpeg'] },
        { name: 'PDF', extensions: ['pdf'] },
      ],
    })
    if (!selection) return
    const paths = Array.isArray(selection) ? selection : [selection]
    await handlePaths(paths as string[])
  }

  async function onCopyMarkdown() {
    if (!selectedJob?.output_path) {
      enqueueToast('Select a completed job first.', 'info')
      return
    }
    try {
      await invoke('copy_file_to_clipboard', { path: selectedJob.output_path })
      enqueueToast('Markdown copied.', 'success')
    } catch (err) {
      console.error(err)
      enqueueToast('Copy failed.', 'error')
    }
  }

  async function onSaveMarkdown() {
    if (!selectedJob?.output_path) {
      enqueueToast('Select a completed job first.', 'info')
      return
    }
    const dest = await save({
      defaultPath: selectedJob.output_path,
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    })
    if (!dest) return
    try {
      await invoke('save_markdown_as', {
        srcPath: selectedJob.output_path,
        destPath: dest,
      })
      enqueueToast('Markdown saved.', 'success')
    } catch (err) {
      console.error(err)
      enqueueToast('Save failed.', 'error')
    }
  }

  async function handleSettingsSave(next: Settings) {
    setSettings(next)
    setSettingsOpen(false)
    try {
      await invoke('set_settings', { settings: next })
      enqueueToast('Settings saved.', 'success')
      refreshModelStatus()
    } catch (err) {
      console.error(err)
      enqueueToast('Failed to save settings.', 'error')
    }
  }

  async function onDownloadModel(repoOverride?: string | null) {
    const repoSource = typeof repoOverride === 'string' ? repoOverride : modelInput
    const repo = repoSource.trim()
    if (!repo) {
      enqueueToast('Enter a Hugging Face model name.', 'info')
      return
    }

    setDownloadState({ status: 'starting', progress: 0 })

    try {
      const result = await invoke<{ file_name: string }>('download_model', { model: repo })
      setDownloadState({
        status: 'done',
        progress: 1,
        message: 'Download complete.',
        file_name: result.file_name,
      })
      enqueueToast(`Downloaded ${result.file_name}.`, 'success')
      setModelInput('')
      await loadModels()
      const next = { ...settings, model_file: result.file_name }
      setSettings(next)
      await invoke('set_settings', { settings: next })
      setModelMissing(false)
    } catch (err) {
      console.error(err)
      const message = err instanceof Error ? err.message : String(err)
      setDownloadState((prev) => ({ ...prev, status: 'error', message }))
      enqueueToast(message || 'Download failed.', 'error')
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand-block">
          <div className="subtitle">Offline vision pipeline</div>
          <div className="title-row">
            <div className="title">VisiTexta</div>
            <div className="mode-pill">Streaming OCR</div>
          </div>
          <div className="headline">
            Live page-aware OCR with a running transcript and markdown render.
          </div>
        </div>
        <div className="topbar-actions">
          <div className="telemetry-card">
            <span>Active jobs</span>
            <strong>{activeJobs}</strong>
          </div>
          <div className="telemetry-card">
            <span>Completed</span>
            <strong>{completedJobs}</strong>
          </div>
          <div className="telemetry-card wide">
            <span>Runtime</span>
            <strong>{modelMissing ? 'Missing' : 'Ready'}</strong>
          </div>
          <button className="btn ghost" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
        </div>
      </header>

      {modelMissing && (
        <div className="warning">
          No vision model detected. Open Settings and download one from Hugging Face.
        </div>
      )}

      <main className="workspace">
        <section className="panel queue-panel">
          <FileQueue
            jobs={jobs}
            selectedId={selectedId}
            streams={streams}
            onSelect={(id) => setSelectedId(id)}
          />
        </section>

        <section className="panel command-panel">
          <div className="panel-title">Mission Control</div>
          <div className="command-copy">
            Route images or PDFs into the OCR pipeline and monitor the active page as it is being read.
          </div>
          <div className="prompt-block">
            <label className="prompt-label">
              Extraction Prompt
              <span className="prompt-hint">optional — leave blank for default</span>
            </label>
            <textarea
              className="prompt-input"
              placeholder={DEFAULT_PROMPT}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={3}
            />
          </div>
          <div className="signal-grid">
            <div className="signal-card">
              <span>Selection</span>
              <strong>{selectedJob?.source || 'No file selected'}</strong>
            </div>
            <div className="signal-card">
              <span>Phase</span>
              <strong>{selectedJob?.status || 'Idle'}</strong>
            </div>
            <div className="signal-card">
              <span>Live page</span>
              <strong>
                {selectedStream?.current_page && selectedStream?.total_pages
                  ? `${selectedStream.current_page}/${selectedStream.total_pages}`
                  : 'Waiting'}
              </strong>
            </div>
          </div>
          <DropZone
            disabled={busy || modelMissing}
            onBrowse={onBrowseFiles}
            onFiles={handlePaths}
          />
        </section>

        <section className="panel preview-panel">
          <MarkdownPreview
            job={selectedJob}
            renderedMarkdown={selectedRenderedMarkdown}
            stream={selectedStream}
          />
        </section>
      </main>

      <footer className="bottom-bar">
        <div className="log">{log}</div>
        <div className="bottom-actions">
          <button className="btn ghost" onClick={onCopyMarkdown}>
            Copy Markdown
          </button>
          <button className="btn primary" onClick={onSaveMarkdown}>
            Save Markdown
          </button>
        </div>
      </footer>

      <SettingsDrawer
        open={settingsOpen}
        settings={settings}
        models={models}
        modelInput={modelInput}
        downloadState={downloadState}
        onModelInputChange={setModelInput}
        onDownloadModel={onDownloadModel}
        onRefreshModels={loadModels}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSettingsSave}
      />

      <ToastNotifications
        toasts={toasts}
        onDismiss={(id) => setToasts((prev) => prev.filter((t) => t.id !== id))}
      />
    </div>
  )
}

export default App









