import { useEffect, useEffectEvent, useMemo, useState } from 'react'
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
  ModelCatalog,
  ModelDownloadEvent,
  RunnerMode,
  RunnerStage,
  StorageInfo,
} from './types'
import './App.css'

type Settings = {
  threads: number
  dpi: number
  auto_open: boolean
  theme?: string | null
  model_profile_id?: string | null
  model_file?: string | null
}

type ModelDownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'verifying' | 'done' | 'error'
  progress: number
  message?: string | null
  file_name?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

type OnboardingInfo = {
  model_storage_path: string
  recommended_model_profile_id: string
  recommended_model_label: string
  recommended_model_file: string
  recommended_model_repo: string
}

type PresetKey = 'recommended' | 'quality' | 'faster'

type Preset = {
  label: string
  dpi: number
  description: string
  meta: string
}

const defaultSettings: Settings = {
  threads: 4,
  dpi: 300,
  auto_open: false,
  theme: 'dark',
  model_profile_id: null,
  model_file: null,
}

const defaultDownloadState: ModelDownloadState = {
  status: 'idle',
  progress: 0,
}

const DEFAULT_MODEL_PROFILE_ID = 'glm-ocr'
const DEFAULT_PROMPT = 'Extract all text from the image and return it as markdown.'
const ONBOARDING_STORAGE_KEY = 'visitexta.onboarding.dismissed'

const PRESETS: Record<PresetKey, Preset> = {
  recommended: {
    label: 'Recommended',
    dpi: 300,
    description: 'Best default for most screenshots, scans, and PDFs.',
    meta: 'Balanced speed and readability',
  },
  quality: {
    label: 'Higher quality',
    dpi: 360,
    description: 'Sharper rendering for small print and dense documents.',
    meta: 'Slower, but easier on tiny text',
  },
  faster: {
    label: 'Faster',
    dpi: 220,
    description: 'Quickest option for clean documents and large batches.',
    meta: 'Fastest turnaround',
  },
}

const PRESET_ORDER: PresetKey[] = ['recommended', 'quality', 'faster']

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

function createEmptyStreamState(): JobStreamState {
  return {
    streamed_markdown: '',
    preview_image_data_url: null,
    current_page: null,
    total_pages: null,
    source: null,
    pages: [],
    runner_mode: null,
    runner_stage: null,
    runner_message: null,
    first_token_received: false,
  }
}

function getPresetForDpi(dpi: number): PresetKey | null {
  const match = PRESET_ORDER.find((key) => PRESETS[key].dpi === dpi)
  return match ?? null
}

function getFileName(path?: string | null) {
  if (!path) return 'Nothing selected'
  const parts = path.split(/[/\\]/)
  return parts[parts.length - 1] || path
}

function formatBytes(value?: number | null) {
  if (value === undefined || value === null) return null
  const mb = value / (1024 * 1024)
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  return `${(mb / 1024).toFixed(2)} GB`
}

function buildRunnerMessage(
  mode: RunnerMode,
  stage: RunnerStage,
  pageNumber?: number | null,
  totalPages?: number | null,
  willFallback?: boolean | null,
  backendMessage?: string | null
) {
  const pageLabel =
    pageNumber && totalPages ? `page ${pageNumber}/${totalPages}` : 'this file'

  if (stage === 'WorkerStarting') {
    return mode === 'Persistent'
      ? `Starting the local OCR engine for ${pageLabel}.`
      : `Starting OCR for ${pageLabel}.`
  }

  if (stage === 'ModelReady') {
    return mode === 'Persistent'
      ? `The local OCR engine is ready for ${pageLabel}.`
      : `OCR is ready for ${pageLabel}.`
  }

  if (stage === 'FirstToken' || stage === 'Chunk') {
    return `Reading ${pageLabel}.`
  }

  if (stage === 'Error') {
    if (willFallback) {
      return 'The first OCR pass failed. Trying the backup OCR path.'
    }
    return backendMessage || 'OCR could not continue.'
  }

  return backendMessage || 'Working locally.'
}

function readOnboardingDismissed() {
  try {
    return window.localStorage.getItem(ONBOARDING_STORAGE_KEY) === '1'
  } catch {
    return false
  }
}

function writeOnboardingDismissed() {
  try {
    window.localStorage.setItem(ONBOARDING_STORAGE_KEY, '1')
  } catch {
    // Ignore local storage failures in restricted environments.
  }
}

function removeCancelingJob(
  previous: Record<string, boolean>,
  jobId: string
) {
  if (!previous[jobId]) return previous
  const next = { ...previous }
  delete next[jobId]
  return next
}

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(reader.error ?? new Error('Failed to read image.'))
    reader.onload = () => {
      if (typeof reader.result === 'string') {
        resolve(reader.result)
        return
      }
      reject(new Error('Clipboard image could not be read.'))
    }
    reader.readAsDataURL(blob)
  })
}

function App() {
  const [busy, setBusy] = useState(false)
  const [jobs, setJobs] = useState<JobResult[]>([])
  const [streams, setStreams] = useState<Record<string, JobStreamState>>({})
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [markdown, setMarkdown] = useState('')
  const [log, setLog] = useState('Choose a preset, then drop, paste, or pick a file to begin.')
  const [modelMissing, setModelMissing] = useState(false)
  const [settings, setSettings] = useState<Settings>(defaultSettings)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [toasts, setToasts] = useState<Toast[]>([])
  const [modelCatalog, setModelCatalog] = useState<ModelCatalog | null>(null)
  const [modelInput, setModelInput] = useState('')
  const [downloadState, setDownloadState] = useState<ModelDownloadState>(defaultDownloadState)
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)
  const [prompt, setPrompt] = useState('')
  const [autoDownloadAttempted, setAutoDownloadAttempted] = useState(false)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [selectedPreset, setSelectedPreset] = useState<PresetKey | null>(
    getPresetForDpi(defaultSettings.dpi)
  )
  const [onboardingInfo, setOnboardingInfo] = useState<OnboardingInfo | null>(null)
  const [onboardingOpen, setOnboardingOpen] = useState(!readOnboardingDismissed())
  const [onboardingStep, setOnboardingStep] = useState(0)
  const [cancelingJobs, setCancelingJobs] = useState<Record<string, boolean>>({})

  const selectedJob = useMemo(
    () => jobs.find((job) => job.job_id === selectedId) || null,
    [jobs, selectedId]
  )

  const selectedStream = useMemo(
    () => (selectedId ? streams[selectedId] || null : null),
    [selectedId, streams]
  )

  const defaultModelProfile = useMemo(() => {
    if (!modelCatalog) return null
    return (
      modelCatalog.profiles.find((profile) => profile.id === modelCatalog.default_profile_id) ||
      null
    )
  }, [modelCatalog])

  const selectedProfile = useMemo(() => {
    if (!modelCatalog || !settings.model_profile_id) return null
    return (
      modelCatalog.profiles.find((profile) => profile.id === settings.model_profile_id) || null
    )
  }, [modelCatalog, settings.model_profile_id])

  const selectedLocalModel = useMemo(() => {
    if (!modelCatalog || !settings.model_file) return null
    return (
      modelCatalog.local_models.find((model) => model.file_name === settings.model_file) || null
    )
  }, [modelCatalog, settings.model_file])

  const activeModelTitle =
    selectedLocalModel?.label ||
    selectedProfile?.label ||
    defaultModelProfile?.label ||
    onboardingInfo?.recommended_model_label ||
    'Recommended OCR model'

  const activeModelSupportLabel = selectedLocalModel
    ? selectedLocalModel.support_tier === 'recommended'
      ? 'Recommended'
      : selectedLocalModel.support_tier === 'tested'
        ? 'Tested'
        : selectedLocalModel.support_tier === 'legacy'
          ? 'Legacy'
          : 'Experimental'
    : selectedProfile
      ? selectedProfile.recommended
        ? 'Recommended'
        : 'Tested'
      : 'Recommended'

  const explicitModelFile = settings.model_file?.trim() || ''
  const explicitModelProfileId = settings.model_profile_id?.trim() || ''
  const configuredModelLabel =
    explicitModelFile || selectedProfile?.label || activeModelTitle || 'Selected OCR model'

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

  const effectiveDpi = useMemo(
    () => (selectedPreset ? PRESETS[selectedPreset].dpi : settings.dpi),
    [selectedPreset, settings.dpi]
  )

  const presetSummary = useMemo(() => {
    if (!selectedPreset) {
      return `Using advanced custom DPI (${settings.dpi}).`
    }
    return `${PRESETS[selectedPreset].label} preset (${PRESETS[selectedPreset].dpi} DPI).`
  }, [selectedPreset, settings.dpi])

  const onboardingSteps = useMemo(
    () => [
      {
        title: 'Everything runs locally',
        body: 'VisiTexta renders pages, reads text, and writes markdown on this PC. It is designed for offline OCR, not a cloud OCR service.',
        detail: 'You can keep working even when you are offline after the model is downloaded.',
      },
      {
        title: 'The first run downloads a model once',
        body:
          modelMissing || ['starting', 'downloading', 'verifying'].includes(downloadState.status)
            ? 'The recommended OCR model is being downloaded for first-time setup. Keep the app open until it finishes.'
            : 'If no local OCR model is installed, VisiTexta downloads the recommended model once and reuses it later.',
        detail:
          downloadState.status === 'error'
            ? downloadState.message || 'The model download needs another try from Advanced settings.'
            : 'After setup, future runs stay local unless you choose another model.',
      },
      {
        title: 'Downloaded models are stored here',
        body: onboardingInfo?.model_storage_path || 'Loading the storage location...',
        detail: 'This is where downloaded OCR models are kept so the app can reuse them.',
      },
      {
        title: 'Start with the recommended model',
        body: `${onboardingInfo?.recommended_model_label || 'GLM-OCR (Q4_K_M)'} uses ${onboardingInfo?.recommended_model_file || 'GLM-OCR.Q4_K_M.gguf'} from ${onboardingInfo?.recommended_model_repo || 'mradermacher/GLM-OCR-GGUF'} as the default path for most users.`,
        detail: 'When setup is ready, choose a preset, then drop or paste a file and the app starts extracting right away.',
      },
    ],
    [downloadState.message, downloadState.status, modelMissing, onboardingInfo]
  )

  const onboardingStepData = onboardingSteps[onboardingStep] ?? onboardingSteps[0]
  const selectedJobName = getFileName(selectedJob?.source)
  const downloadProgressPercent = Math.min(
    100,
    Math.max(0, Math.round(downloadState.progress * 100))
  )
  const shouldAutoDownloadRecommended = useMemo(() => {
    const recommendedProfileId =
      onboardingInfo?.recommended_model_profile_id || DEFAULT_MODEL_PROFILE_ID

    if (explicitModelFile) return false
    if (explicitModelProfileId && explicitModelProfileId !== recommendedProfileId) return false
    return true
  }, [
    explicitModelFile,
    explicitModelProfileId,
    onboardingInfo?.recommended_model_profile_id,
  ])

  const missingModelMessage =
    downloadState.status === 'error'
      ? downloadState.message ||
        (shouldAutoDownloadRecommended
          ? 'First-time setup needs another try in Advanced settings.'
          : 'Open Advanced settings to install or choose a different supported model.')
      : shouldAutoDownloadRecommended
        ? 'First-time setup is downloading the recommended OCR model.'
        : `${configuredModelLabel} is selected, but it is not ready yet.`

  const setupCardTitle =
    downloadState.status === 'error'
      ? shouldAutoDownloadRecommended
        ? 'Recommended model download needs another try'
        : 'Selected model needs attention'
      : shouldAutoDownloadRecommended
        ? 'Downloading the recommended OCR model'
        : `${configuredModelLabel} is not ready yet`

  const setupCardBody =
    downloadState.status === 'error'
      ? downloadState.message ||
        (shouldAutoDownloadRecommended
          ? 'Open Advanced settings to retry the download.'
          : 'Open Advanced settings to install the selected model or switch back to a curated profile.')
      : shouldAutoDownloadRecommended
        ? 'This only happens on first setup or after models are removed.'
        : 'The current selection is missing locally or is missing its required mmproj companion.'

  const handlePathsEvent = useEffectEvent((paths: string[]) => {
    void handlePaths(paths)
  })

  const handleAppEventListener = useEffectEvent((payload: AppEvent) => {
    handleAppEvent(payload)
  })

  const handlePastedImageEvent = useEffectEvent((blob: Blob) => {
    void handlePastedImage(blob)
  })

  const triggerAutoDownload = useEffectEvent(() => {
    void onDownloadModel(
      onboardingInfo?.recommended_model_profile_id || DEFAULT_MODEL_PROFILE_ID,
      true
    )
  })

  useEffect(() => {
    const dropListener = listen<string[]>('tauri://file-drop', async (event) => {
      if (Array.isArray(event.payload)) {
        handlePathsEvent(event.payload)
      }
    })

    return () => {
      dropListener.then((unlisten) => unlisten())
    }
  }, [])

  useEffect(() => {
    const registrations: Promise<() => void>[] = [
      listen<AppEvent>('job-progress', (event) => handleAppEventListener(event.payload)),
      listen<AppEvent>('job-preview', (event) => handleAppEventListener(event.payload)),
      listen<AppEvent>('job-runner', (event) => handleAppEventListener(event.payload)),
      listen<AppEvent>('job-complete', (event) => handleAppEventListener(event.payload)),
      listen<AppEvent>('job-error', (event) => handleAppEventListener(event.payload)),
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
    invoke<JobResult[]>('get_job_history')
      .then((history) => setJobs(history))
      .catch(() => setJobs([]))
  }, [])

  useEffect(() => {
    invoke<Settings>('get_settings')
      .then((result) => {
        setSettings(result)
        setSelectedPreset(getPresetForDpi(result.dpi))
      })
      .catch(() => {
        setSettings(defaultSettings)
        setSelectedPreset(getPresetForDpi(defaultSettings.dpi))
      })
  }, [])

  useEffect(() => {
    void refreshModelStatus()
  }, [settings.model_file, settings.model_profile_id])

  useEffect(() => {
    void loadModelCatalog()
    invoke<OnboardingInfo>('get_onboarding_info')
      .then((info) => setOnboardingInfo(info))
      .catch(() => setOnboardingInfo(null))
    invoke<StorageInfo>('get_storage_info')
      .then((info) => setStorageInfo(info))
      .catch(() => setStorageInfo(null))
  }, [])

  useEffect(() => {
    if (settingsOpen) {
      void loadModelCatalog()
    }
  }, [settingsOpen])

  useEffect(() => {
    const handlePaste = (event: ClipboardEvent) => {
      if (busy || modelMissing) return
      const items = Array.from(event.clipboardData?.items ?? [])
      const imageItem = items.find((item) => item.type.startsWith('image/'))
      if (!imageItem) return

      const file = imageItem.getAsFile()
      if (!file) return

      event.preventDefault()
      handlePastedImageEvent(file)
    }

    window.addEventListener('paste', handlePaste)
    return () => {
      window.removeEventListener('paste', handlePaste)
    }
  }, [busy, modelMissing])

  useEffect(() => {
    if (!modelMissing && autoDownloadAttempted) {
      setAutoDownloadAttempted(false)
    }
  }, [autoDownloadAttempted, modelMissing])

  useEffect(() => {
    if (!modelMissing || autoDownloadAttempted || !shouldAutoDownloadRecommended) return

    setAutoDownloadAttempted(true)
    setLog('First-time setup is downloading the recommended OCR model.')
    enqueueToast('Downloading the recommended OCR model for first-time setup.', 'info')
    triggerAutoDownload()
  }, [autoDownloadAttempted, modelMissing, shouldAutoDownloadRecommended])

  useEffect(() => {
    if (!selectedJob?.output_path) {
      setMarkdown('')
      return
    }

    invoke<string>('read_markdown_file', { path: selectedJob.output_path })
      .then((content) => setMarkdown(content))
      .catch(() => setMarkdown('Failed to load markdown.'))
  }, [selectedJob?.output_path])

  useEffect(() => {
    if (!onboardingOpen || jobs.length === 0) return
    dismissOnboarding(true)
  }, [jobs.length, onboardingOpen])

  function enqueueToast(message: string, tone: Toast['tone'] = 'info') {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    setToasts((prev) => [...prev, { id, message, tone }])
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((toast) => toast.id !== id))
    }, 4000)
  }

  function dismissOnboarding(persist: boolean) {
    setOnboardingOpen(false)
    if (persist) {
      writeOnboardingDismissed()
    }
  }

  function clearCancelRequest(jobId: string) {
    setCancelingJobs((prev) => removeCancelingJob(prev, jobId))
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
      const {
        job_id,
        status,
        progress,
        message,
        source,
        page_number,
        total_pages,
      } = event.data
      upsertJob({
        job_id,
        status,
        progress,
        message: message ?? null,
        source: source ?? undefined,
      })
      if (source || total_pages) {
        setStreams((prev) => {
          const existing = prev[job_id]
          if (!existing && !source && !total_pages) return prev
          const current = existing ?? createEmptyStreamState()

          return {
            ...prev,
            [job_id]: {
              ...current,
              current_page:
                status === 'Ocr'
                  ? page_number ?? current.current_page ?? null
                  : current.current_page ?? null,
              total_pages: total_pages ?? current.total_pages ?? null,
              source: source ?? current.source ?? null,
            },
          }
        })
      }
      if (status === 'Done' || status === 'Canceled') {
        clearCancelRequest(job_id)
      }
      setSelectedId((current) => current ?? job_id)
      if (message) {
        setLog(message)
      }
    } else if (event.type === 'Preview') {
      const {
        job_id,
        image_data_url,
        kind,
        page_number,
        total_pages,
        text_chunk,
        source,
      } = event.data
      setStreams((prev) => {
        const current = prev[job_id] ?? createEmptyStreamState()
        const nextStream = text_chunk
          ? `${current.streamed_markdown}${text_chunk}`
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
            current_page: kind === 'Ocr' ? page_number : current.current_page ?? null,
            total_pages,
            preview_image_data_url:
              kind === 'Ocr'
                ? image_data_url
                : current.preview_image_data_url ?? image_data_url,
            streamed_markdown: nextStream,
            pages: nextPages,
          },
        }
      })
      setSelectedId((current) => current ?? job_id)
      setLog(
        kind === 'Rendered'
          ? `Prepared page ${page_number}/${total_pages}.`
          : `Reading page ${page_number}/${total_pages}.`
      )
    } else if (event.type === 'Runner') {
      const {
        job_id,
        source,
        page_number,
        total_pages,
        mode,
        stage,
        message,
        chunk,
        will_fallback,
      } = event.data

      const runnerMessage = buildRunnerMessage(
        mode,
        stage,
        page_number,
        total_pages,
        will_fallback,
        message
      )

      setStreams((prev) => {
        const current = prev[job_id] ?? createEmptyStreamState()

        return {
          ...prev,
          [job_id]: {
            ...current,
            source: source ?? current.source ?? null,
            current_page: page_number ?? current.current_page ?? null,
            total_pages: total_pages ?? current.total_pages ?? null,
            streamed_markdown: chunk
              ? `${current.streamed_markdown}${chunk}`
              : current.streamed_markdown,
            runner_mode: mode,
            runner_stage: stage,
            runner_message: runnerMessage,
            first_token_received:
              current.first_token_received || stage === 'FirstToken',
          },
        }
      })
      setSelectedId((current) => current ?? job_id)
      setLog(runnerMessage)
    } else if (event.type === 'Completed') {
      const { job_id, output_path } = event.data
      upsertJob({
        job_id,
        status: 'Done',
        output_path,
        progress: 1,
        message: 'Markdown ready',
      })
      clearCancelRequest(job_id)
      setLog(`Markdown is ready for ${getFileName(output_path)}.`)
      enqueueToast('Markdown is ready.', 'success')
      setSelectedId((current) => current ?? job_id)
    } else if (event.type === 'Error') {
      const { job_id, message } = event.data
      upsertJob({
        job_id,
        status: 'Failed',
        error: message,
        message,
      })
      clearCancelRequest(job_id)
      setLog(message)
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

  async function loadModelCatalog() {
    try {
      const catalog = await invoke<ModelCatalog>('get_model_catalog')
      setModelCatalog(catalog)
    } catch (err) {
      console.error(err)
      setModelCatalog(null)
    }
  }

  async function handlePaths(paths: string[]) {
    if (!paths.length) return
    if (modelMissing) {
      const downloading = ['starting', 'downloading', 'verifying'].includes(downloadState.status)
      setLog(
        downloading
          ? 'First-time setup is still downloading the OCR model.'
          : shouldAutoDownloadRecommended
            ? 'A local OCR model is required before extraction can start.'
            : `${configuredModelLabel} is selected, but it is not ready yet.`
      )
      enqueueToast(
        downloading
          ? 'Setup is still downloading the OCR model.'
          : shouldAutoDownloadRecommended
            ? 'Open Advanced settings to retry the recommended model download.'
            : 'Open Advanced settings to install the selected model or choose another curated profile.',
        'error'
      )
      return
    }

    setBusy(true)
    setLog(paths.length === 1 ? 'Starting extraction...' : `Starting extraction for ${paths.length} files...`)

    try {
      const result = (await invoke('enqueue_jobs', {
        paths,
        prompt: prompt.trim() || null,
        dpi: effectiveDpi,
      })) as JobResult[]
      setJobs((prev) => mergeJobs(prev, result))
      if (result.length) {
        setSelectedId(result[0].job_id)
      }
    } catch (err) {
      console.error(err)
      const message =
        typeof err === 'string'
          ? err
          : err instanceof Error
            ? err.message
            : String(err)
      setLog(message)
      enqueueToast(message || 'Failed to start extraction.', 'error')
    } finally {
      setBusy(false)
    }
  }

  async function handlePastedImage(blob: Blob) {
    if (modelMissing) {
      const downloading = ['starting', 'downloading', 'verifying'].includes(downloadState.status)
      enqueueToast(
        downloading
          ? 'Wait for the recommended model download to finish before pasting an image.'
          : shouldAutoDownloadRecommended
            ? 'Open Advanced settings to finish setting up the recommended model first.'
            : 'Open Advanced settings to install the selected model or choose another curated profile.',
        'info'
      )
      return
    }

    setBusy(true)
    setLog('Starting extraction from the pasted image...')

    try {
      const imageBase64 = await blobToDataUrl(blob)
      const result = (await invoke('enqueue_pasted_image', {
        imageBase64,
        mimeType: blob.type || 'image/png',
        prompt: prompt.trim() || null,
        dpi: effectiveDpi,
      })) as JobResult[]
      setJobs((prev) => mergeJobs(prev, result))
      if (result.length) {
        setSelectedId(result[0].job_id)
      }
      enqueueToast('Pasted image added.', 'success')
    } catch (err) {
      console.error(err)
      const message = err instanceof Error ? err.message : String(err)
      setLog(message)
      enqueueToast(message || 'Could not read the pasted image.', 'error')
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

  async function onPasteImage() {
    if (!navigator.clipboard?.read) {
      enqueueToast('Clipboard image access is not available here. Try Ctrl+V instead.', 'info')
      return
    }

    try {
      const items = await navigator.clipboard.read()
      const imageItem = items.find((item) =>
        item.types.some((type) => type.startsWith('image/'))
      )

      if (!imageItem) {
        enqueueToast('Your clipboard does not contain an image right now.', 'info')
        return
      }

      const imageType = imageItem.types.find((type) => type.startsWith('image/'))
      if (!imageType) {
        enqueueToast('Your clipboard image format is not supported.', 'info')
        return
      }

      const blob = await imageItem.getType(imageType)
      await handlePastedImage(blob)
    } catch (err) {
      console.error(err)
      enqueueToast('Could not read the clipboard image.', 'error')
    }
  }

  async function onCopyMarkdown() {
    const text = selectedRenderedMarkdown.trim()
    if (!text && !selectedJob?.output_path) {
      enqueueToast('Select a job with markdown first.', 'info')
      return
    }

    try {
      if (text) {
        await navigator.clipboard.writeText(text)
      } else if (selectedJob?.output_path) {
        await invoke('copy_file_to_clipboard', { path: selectedJob.output_path })
      }
      enqueueToast('Markdown copied.', 'success')
    } catch (err) {
      console.error(err)
      if (selectedJob?.output_path) {
        try {
          await invoke('copy_file_to_clipboard', { path: selectedJob.output_path })
          enqueueToast('Markdown copied.', 'success')
          return
        } catch (fallbackError) {
          console.error(fallbackError)
        }
      }
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

  async function onRetryJob() {
    if (!selectedJob?.source) {
      enqueueToast('Select a job to retry.', 'info')
      return
    }
    await handlePaths([selectedJob.source])
  }

  async function onCancelJob() {
    if (!selectedJob || !isActiveStatus(selectedJob.status)) {
      enqueueToast('Only active jobs can be canceled.', 'info')
      return
    }

    setCancelingJobs((prev) => ({ ...prev, [selectedJob.job_id]: true }))
    setLog(`Stopping ${selectedJobName} after the current step...`)

    try {
      const accepted = await invoke<boolean>('cancel_job', { jobId: selectedJob.job_id })
      if (!accepted) {
        clearCancelRequest(selectedJob.job_id)
        enqueueToast('This job already finished.', 'info')
        return
      }
      enqueueToast('Cancellation requested.', 'info')
    } catch (err) {
      console.error(err)
      clearCancelRequest(selectedJob.job_id)
      enqueueToast('Cancel failed.', 'error')
    }
  }

  async function onOpenOutputFolder() {
    if (!selectedJob?.output_path) {
      enqueueToast('This job has no saved markdown folder yet.', 'info')
      return
    }
    try {
      await invoke('open_output_folder', { path: selectedJob.output_path })
    } catch (err) {
      console.error(err)
      enqueueToast('Could not open the output folder.', 'error')
    }
  }

  async function onRevealInExplorer() {
    const path = selectedJob?.output_path || selectedJob?.source
    if (!path) {
      enqueueToast('Select a job first.', 'info')
      return
    }
    try {
      await invoke('reveal_in_explorer', { path })
    } catch (err) {
      console.error(err)
      enqueueToast('Could not reveal the file in Explorer.', 'error')
    }
  }

  async function handleSettingsSave(next: Settings) {
    setSettings(next)
    setSelectedPreset(getPresetForDpi(next.dpi))
    setSettingsOpen(false)
    try {
      await invoke('set_settings', { settings: next })
      enqueueToast('Settings saved.', 'success')
      await refreshModelStatus()
    } catch (err) {
      console.error(err)
      enqueueToast('Failed to save settings.', 'error')
    }
  }

  async function onDownloadModel(targetOverride?: string | null, isAuto = false) {
    const targetSource = typeof targetOverride === 'string' ? targetOverride : modelInput
    const target = targetSource.trim()
    if (!target) {
      enqueueToast('Choose a supported profile or enter a full custom model path.', 'info')
      return
    }

    setDownloadState({ status: 'starting', progress: 0 })

    try {
      const result = await invoke<{ file_name: string; profile_id?: string | null }>(
        'download_model',
        { model: target }
      )
      setDownloadState({
        status: 'done',
        progress: 1,
        message: 'Download complete.',
        file_name: result.file_name,
      })
      enqueueToast(
        isAuto
          ? `First-time setup is complete. ${result.file_name} is ready.`
          : `${result.file_name} downloaded.`,
        'success'
      )
      if (isAuto) {
        setLog('First-time setup is complete. Drop, paste, or choose a file to begin.')
      }
      setModelInput('')
      await loadModelCatalog()
      const next = {
        ...settings,
        model_profile_id: result.profile_id || null,
        model_file: result.file_name,
      }
      setSettings(next)
      if (!selectedPreset) {
        setSelectedPreset(getPresetForDpi(next.dpi))
      }
      await invoke('set_settings', { settings: next })
      setModelMissing(false)
    } catch (err) {
      console.error(err)
      const message = err instanceof Error ? err.message : String(err)
      setDownloadState((prev) => ({ ...prev, status: 'error', message }))
      if (isAuto) {
        setLog('First-time setup could not finish automatically. Open Advanced settings to retry.')
      }
      enqueueToast(message || 'Download failed.', 'error')
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand-block">
          <div className="subtitle">Offline OCR</div>
          <div className="title-row">
            <div className="title">VisiTexta</div>
            <div className="mode-pill">Local only</div>
          </div>
          <div className="headline">
            Turn PDFs, scans, and screenshots into markdown on this PC. Choose a preset,
            then drop, paste, or pick a file to start.
          </div>
        </div>
        <div className="topbar-actions">
          <div className="telemetry-card">
            <span>Setup</span>
            <strong>
              {modelMissing
                ? downloadState.status === 'error'
                  ? 'Needs attention'
                  : 'First run'
                : 'Ready'}
            </strong>
          </div>
          <div className="telemetry-card">
            <span>In progress</span>
            <strong>{activeJobs}</strong>
          </div>
          <div className="telemetry-card">
            <span>Finished</span>
            <strong>{completedJobs}</strong>
          </div>
          <div className="telemetry-card wide">
            <span>Preset</span>
            <strong>{selectedPreset ? PRESETS[selectedPreset].label : 'Advanced custom'}</strong>
          </div>
        </div>
      </header>

      {(modelMissing || downloadState.status === 'error') && (
        <div className="warning">{missingModelMessage}</div>
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
          <div className="panel-title">Start extraction</div>
          <div className="command-copy">
            Pick the speed and quality you want, then drop files, paste an image, or browse
            for files from your computer.
          </div>

          {onboardingOpen && (
            <section className="onboarding-card" aria-label="First-run guide">
              <div className="onboarding-header">
                <div>
                  <div className="section-title">First-run guide</div>
                  <div className="onboarding-title">{onboardingStepData.title}</div>
                </div>
                <button className="btn ghost" onClick={() => dismissOnboarding(true)}>
                  Skip
                </button>
              </div>
              <p className="onboarding-body">{onboardingStepData.body}</p>
              <div className="onboarding-detail">{onboardingStepData.detail}</div>
              <div className="onboarding-progress">
                <span>{`Step ${onboardingStep + 1} of ${onboardingSteps.length}`}</span>
                <div className="onboarding-progress-bar" aria-hidden="true">
                  <div
                    className="onboarding-progress-fill"
                    style={{
                      width: `${((onboardingStep + 1) / onboardingSteps.length) * 100}%`,
                    }}
                  />
                </div>
              </div>
              <div className="onboarding-actions">
                <button
                  className="btn ghost"
                  onClick={() => setOnboardingStep((current) => Math.max(0, current - 1))}
                  disabled={onboardingStep === 0}
                >
                  Back
                </button>
                <button
                  className="btn primary"
                  onClick={() => {
                    if (onboardingStep === onboardingSteps.length - 1) {
                      dismissOnboarding(true)
                      return
                    }
                    setOnboardingStep((current) => Math.min(onboardingSteps.length - 1, current + 1))
                  }}
                >
                  {onboardingStep === onboardingSteps.length - 1
                    ? 'Start extraction'
                    : 'Next'}
                </button>
              </div>
            </section>
          )}

          {(modelMissing || downloadState.status === 'error') && (
            <section className="setup-card" aria-live="polite">
              <div className="setup-card-header">
                <div>
                  <div className="section-title">Setup status</div>
                  <div className="setup-card-title">
                    {setupCardTitle}
                  </div>
                </div>
                <div className="setup-card-badge">
                  {downloadState.status === 'error' ? 'Paused' : `${downloadProgressPercent}%`}
                </div>
              </div>
              <div className="setup-card-copy">{setupCardBody}</div>
              {downloadState.status !== 'error' && (
                <div className="model-progress">
                  <div className="model-progress-bar">
                    <div
                      className="model-progress-fill"
                      style={{ width: `${downloadProgressPercent}%` }}
                    />
                  </div>
                  <div className="model-progress-text">
                    {downloadState.status === 'verifying'
                      ? downloadState.message || 'Verifying download...'
                      : downloadState.total_bytes
                      ? `${downloadProgressPercent}% (${formatBytes(downloadState.downloaded_bytes)} / ${formatBytes(downloadState.total_bytes)})`
                      : `${downloadProgressPercent}%`}
                  </div>
                </div>
              )}
            </section>
          )}

          <section className="preset-section" aria-label="Extraction presets">
            <div className="preset-header">
              <div className="section-title">Presets</div>
              <div className="preset-note">{presetSummary}</div>
            </div>
            <div className="preset-grid">
              {PRESET_ORDER.map((key) => {
                const preset = PRESETS[key]
                const selected = selectedPreset === key
                return (
                  <button
                    key={key}
                    type="button"
                    className={`preset-card ${selected ? 'selected' : ''}`}
                    aria-pressed={selected}
                    onClick={() => {
                      setSelectedPreset(key)
                      setLog(`${preset.label} preset selected.`)
                    }}
                  >
                    <span className="preset-name">{preset.label}</span>
                    <span className="preset-copy">{preset.description}</span>
                    <span className="preset-meta">{preset.meta}</span>
                  </button>
                )
              })}
            </div>
            {selectedPreset === null && (
              <div className="preset-custom-note">
                Advanced custom settings are active for the next run.
              </div>
            )}
          </section>

          <DropZone
            disabled={busy || modelMissing}
            onBrowse={onBrowseFiles}
            onPasteImage={onPasteImage}
            onFiles={handlePaths}
          />

          <div className="advanced-toggle">
            <button
              className="btn ghost"
              aria-expanded={advancedOpen}
              aria-controls="advanced-panel"
              onClick={() => setAdvancedOpen((current) => !current)}
            >
              {advancedOpen ? 'Hide advanced' : 'Advanced'}
            </button>
          </div>

          {advancedOpen && (
            <section id="advanced-panel" className="advanced-panel">
              <div className="advanced-copy">
                Use custom instructions, switch models, or change lower-level OCR settings.
              </div>
              <div className="prompt-block">
                <label className="prompt-label">
                  Custom instructions
                  <span className="prompt-hint">Optional. Leave blank for the standard OCR prompt.</span>
                </label>
                <textarea
                  className="prompt-input"
                  placeholder={DEFAULT_PROMPT}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  rows={4}
                />
              </div>
              <div className="signal-grid advanced-grid">
                <div className="signal-card">
                  <span>Active model</span>
                  <strong>{activeModelTitle}</strong>
                  <span>{activeModelSupportLabel}</span>
                </div>
                <div className="signal-card wide">
                  <span>Model storage</span>
                  <strong>{storageInfo?.models_path || onboardingInfo?.model_storage_path || 'Loading...'}</strong>
                </div>
              </div>
              <div className="advanced-actions">
                <button className="btn ghost" onClick={() => setSettingsOpen(true)}>
                  Advanced settings
                </button>
                <button className="btn ghost" onClick={onSaveMarkdown} disabled={!selectedJob?.output_path}>
                  Save a copy
                </button>
              </div>
            </section>
          )}
        </section>

        <section className="panel preview-panel">
          <MarkdownPreview
            key={selectedJob?.job_id || 'empty-preview'}
            job={selectedJob}
            renderedMarkdown={selectedRenderedMarkdown}
            stream={selectedStream}
            onRetry={onRetryJob}
            onCancel={onCancelJob}
            onOpenOutputFolder={onOpenOutputFolder}
            onRevealInExplorer={onRevealInExplorer}
            onCopyMarkdown={onCopyMarkdown}
            isCancelRequested={selectedJob ? Boolean(cancelingJobs[selectedJob.job_id]) : false}
          />
        </section>
      </main>

      <footer className="bottom-bar" aria-live="polite">
        <div className="log">{log}</div>
        <div className="bottom-note">{presetSummary}</div>
      </footer>

      <SettingsDrawer
        open={settingsOpen}
        settings={settings}
        modelCatalog={modelCatalog}
        storageInfo={storageInfo}
        modelInput={modelInput}
        modelStoragePath={onboardingInfo?.model_storage_path || null}
        downloadState={downloadState}
        onModelInputChange={setModelInput}
        onDownloadModel={onDownloadModel}
        onRefreshModels={loadModelCatalog}
        onClose={() => setSettingsOpen(false)}
        onSave={handleSettingsSave}
      />

      <ToastNotifications
        toasts={toasts}
        onDismiss={(id) => setToasts((prev) => prev.filter((toast) => toast.id !== id))}
      />
    </div>
  )
}

export default App









