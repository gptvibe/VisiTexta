import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { DropZone } from './components/DropZone'
import { FileQueue } from './components/FileQueue'
import { MarkdownPreview } from './components/MarkdownPreview'
import { SettingsDrawer } from './components/SettingsDrawer'
import { ToastNotifications, type Toast } from './components/ToastNotifications'
import type { AppEvent, JobResult, ModelDownloadEvent } from './types'
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

const DEFAULT_MODEL_REPO = 'unsloth/Qwen3.5-0.8B-GGUF'

function App() {
  const [busy, setBusy] = useState(false)
  const [jobs, setJobs] = useState<JobResult[]>([])
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
  const [autoDownloadAttempted, setAutoDownloadAttempted] = useState(false)

  const selectedJob = useMemo(
    () => jobs.find((job) => job.job_id === selectedId) || null,
    [jobs, selectedId]
  )

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
    if (autoDownloadAttempted) return
    if (downloadState.status !== 'idle') return
    if (models.length === 0 && !settings.model_file) {
      setAutoDownloadAttempted(true)
      setModelInput(DEFAULT_MODEL_REPO)
      enqueueToast(`Downloading ${DEFAULT_MODEL_REPO}...`, 'info')
      onDownloadModel(DEFAULT_MODEL_REPO)
    }
  }, [models, autoDownloadAttempted, downloadState.status, settings.model_file])

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
      if (!selectedId) setSelectedId(job_id)
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
      setLog('No model found. Download one in Settings or place a .gguf in models/.')
      enqueueToast('No model found. Download one in Settings.', 'error')
      return
    }

    setBusy(true)
    setLog(`Processing ${paths.length} file(s)...`)

    try {
      const result = (await invoke('enqueue_jobs', { paths })) as JobResult[]
      setJobs((prev) => [...result, ...prev])
      if (result.length && !selectedId) setSelectedId(result[0].job_id)
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
        <div>
          <div className="title">VisiTexta</div>
          <div className="subtitle">Offline OCR to clean Markdown</div>
        </div>
        <div className="topbar-actions">
          <button className="btn ghost" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
        </div>
      </header>

      {modelMissing && (
        <div className="warning">
          No model found. Download a model in Settings or place a .gguf file in{' '}
          <code>models/</code>.
        </div>
      )}

      <main className="workspace">
        <section className="panel queue-panel">
          <FileQueue
            jobs={jobs}
            selectedId={selectedId}
            onSelect={(id) => setSelectedId(id)}
          />
        </section>

        <section className="panel drop-panel">
          <div className="panel-title">Upload</div>
          <DropZone
            disabled={busy || modelMissing}
            onBrowse={onBrowseFiles}
            onFiles={handlePaths}
          />
        </section>

        <section className="panel preview-panel">
          <MarkdownPreview job={selectedJob} markdown={markdown} />
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









