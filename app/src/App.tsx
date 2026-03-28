import { useEffect, useEffectEvent, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { AppShell } from './components/AppShell'
import { ImportPanel } from './components/ImportPanel'
import { JobQueue } from './components/JobQueue'
import { PreviewWorkspace } from './components/PreviewWorkspace'
import { SettingsDrawer } from './components/SettingsDrawer'
import { TopBar } from './components/TopBar'
import { ToastNotifications, type Toast } from './components/ToastNotifications'
import type {
  AppDefaults,
  AppEvent,
  ExtractionPreset,
  JobPreviewPage,
  JobResult,
  JobStatus,
  JobStreamState,
  ModelCatalog,
  ModelDownloadEvent,
  RuntimeStatus,
  RunnerMode,
  RunnerStage,
  Settings,
  StorageInfo,
  OnboardingInfo,
} from './types'
import './App.css'

type ModelDownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'verifying' | 'done' | 'error'
  progress: number
  message?: string | null
  file_name?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

type PresetKey = 'recommended' | 'quality' | 'faster'
type ThemeChoice = 'light' | 'dark' | 'system'
type ResolvedTheme = 'light' | 'dark'

const defaultDownloadState: ModelDownloadState = {
  status: 'idle',
  progress: 0,
}

const ONBOARDING_STORAGE_KEY = 'visitexta.onboarding.dismissed'

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

function getPresetForDpi(presets: ExtractionPreset[], dpi: number): PresetKey | null {
  const match = presets.find((preset) => preset.dpi === dpi)
  return (match?.id as PresetKey | undefined) ?? null
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

function normalizeThemeChoice(value?: string | null): ThemeChoice {
  if (value === 'light' || value === 'dark' || value === 'system') {
    return value
  }
  return 'system'
}

function readSystemTheme(): ResolvedTheme {
  if (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  ) {
    return 'dark'
  }
  return 'light'
}

function resolveTheme(choice: ThemeChoice, systemTheme: ResolvedTheme): ResolvedTheme {
  return choice === 'system' ? systemTheme : choice
}

function themeLabel(choice: ThemeChoice, resolvedTheme: ResolvedTheme) {
  if (choice === 'system') {
    return `System (${resolvedTheme === 'dark' ? 'Dark' : 'Light'})`
  }
  return choice === 'dark' ? 'Dark' : 'Light'
}

function runtimeProfileLabel(appDefaults: AppDefaults | null, profile: Settings['runtime_profile']) {
  return (
    appDefaults?.runtime_profiles.options.find((option) => option.id === profile)?.label ??
    profile
  )
}

function buildRunnerMessage(
  mode: RunnerMode,
  stage: RunnerStage,
  pageNumber?: number | null,
  totalPages?: number | null,
  willFallback?: boolean | null,
  backendMessage?: string | null
) {
  const explicitMessage = backendMessage?.trim()
  if (explicitMessage && stage !== 'FirstToken' && stage !== 'Chunk') {
    return explicitMessage
  }

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
  const [appDefaults, setAppDefaults] = useState<AppDefaults | null>(null)
  const [busy, setBusy] = useState(false)
  const [jobs, setJobs] = useState<JobResult[]>([])
  const [streams, setStreams] = useState<Record<string, JobStreamState>>({})
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [markdown, setMarkdown] = useState('')
  const [log, setLog] = useState('Choose a preset, then drop, paste, or pick a file to begin.')
  const [modelMissing, setModelMissing] = useState(false)
  const [settings, setSettings] = useState<Settings | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [toasts, setToasts] = useState<Toast[]>([])
  const [modelCatalog, setModelCatalog] = useState<ModelCatalog | null>(null)
  const [modelInput, setModelInput] = useState('')
  const [downloadState, setDownloadState] = useState<ModelDownloadState>(defaultDownloadState)
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null)
  const [runtimeStatus, setRuntimeStatus] = useState<RuntimeStatus | null>(null)
  const [prompt, setPrompt] = useState('')
  const [autoDownloadAttempted, setAutoDownloadAttempted] = useState(false)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [selectedPreset, setSelectedPreset] = useState<PresetKey | null>(null)
  const [onboardingInfo, setOnboardingInfo] = useState<OnboardingInfo | null>(null)
  const [onboardingOpen, setOnboardingOpen] = useState(!readOnboardingDismissed())
  const [onboardingStep, setOnboardingStep] = useState(0)
  const [cancelingJobs, setCancelingJobs] = useState<Record<string, boolean>>({})
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() => readSystemTheme())
  const effectiveSettings = settings ?? appDefaults?.settings ?? null
  const drawerSettings = effectiveSettings ?? appDefaults?.settings ?? null
  const presetOptions = appDefaults?.extraction_presets ?? []
  const presetOrder = presetOptions.map((preset) => preset.id as PresetKey)
  const selectedThemeChoice = normalizeThemeChoice(
    effectiveSettings?.theme ?? appDefaults?.theme.default_theme
  )
  const resolvedTheme = resolveTheme(selectedThemeChoice, systemTheme)
  const currentThemeLabel = themeLabel(selectedThemeChoice, resolvedTheme)

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
    if (!modelCatalog || !effectiveSettings?.model_profile_id) return null
    return (
      modelCatalog.profiles.find((profile) => profile.id === effectiveSettings.model_profile_id) ||
      null
    )
  }, [effectiveSettings?.model_profile_id, modelCatalog])

  const selectedLocalModel = useMemo(() => {
    if (!modelCatalog || !effectiveSettings?.model_file) return null
    return (
      modelCatalog.local_models.find((model) => model.file_name === effectiveSettings.model_file) ||
      null
    )
  }, [effectiveSettings?.model_file, modelCatalog])

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

  const explicitModelFile = effectiveSettings?.model_file?.trim() || ''
  const explicitModelProfileId = effectiveSettings?.model_profile_id?.trim() || ''
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
    () =>
      selectedPreset
        ? (presetOptions.find((preset) => preset.id === selectedPreset)?.dpi ?? effectiveSettings?.dpi ?? 300)
        : effectiveSettings?.dpi ?? 300,
    [effectiveSettings?.dpi, presetOptions, selectedPreset]
  )

  const hasUsableRuntime = runtimeStatus?.usable_runtime ?? true
  const runtimeSetupIssue = Boolean(runtimeStatus && !runtimeStatus.usable_runtime)

  const presetSummary = useMemo(() => {
    if (!selectedPreset) {
      return `Using advanced custom DPI (${effectiveSettings?.dpi ?? 300}).`
    }
    const preset = presetOptions.find((option) => option.id === selectedPreset)
    return `${preset?.label || 'Selected'} preset (${preset?.dpi ?? effectiveSettings?.dpi ?? 300} DPI).`
  }, [effectiveSettings?.dpi, presetOptions, selectedPreset])

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
        body: `${onboardingInfo?.recommended_model_label || appDefaults?.recommended_model_label || 'GLM-OCR (Q4_K_M)'} uses ${onboardingInfo?.recommended_model_file || appDefaults?.recommended_model_file || 'GLM-OCR.Q4_K_M.gguf'} from ${onboardingInfo?.recommended_model_repo || appDefaults?.recommended_model_repo || 'mradermacher/GLM-OCR-GGUF'} as the default path for most users.`,
        detail: 'When setup is ready, choose a preset, then drop or paste a file and the app starts extracting right away.',
      },
    ],
    [appDefaults, downloadState.message, downloadState.status, modelMissing, onboardingInfo]
  )

  const selectedJobName = getFileName(selectedJob?.source)
  const downloadProgressPercent = Math.min(
    100,
    Math.max(0, Math.round(downloadState.progress * 100))
  )
  const shouldAutoDownloadRecommended = useMemo(() => {
    if (!hasUsableRuntime) return false

    const recommendedProfileId =
      onboardingInfo?.recommended_model_profile_id ||
      appDefaults?.recommended_model_profile_id ||
      ''

    if (!recommendedProfileId) return false
    if (explicitModelFile) return false
    if (explicitModelProfileId && explicitModelProfileId !== recommendedProfileId) return false
    return true
  }, [
    explicitModelFile,
    explicitModelProfileId,
    hasUsableRuntime,
    appDefaults?.recommended_model_profile_id,
    onboardingInfo?.recommended_model_profile_id,
  ])

  const missingModelMessage =
    runtimeSetupIssue
      ? runtimeStatus?.summary || 'A usable local OCR runtime was not found.'
      : downloadState.status === 'error'
      ? downloadState.message ||
        (shouldAutoDownloadRecommended
          ? 'First-time setup needs another try in Advanced settings.'
          : 'Open Advanced settings to install or choose a different supported model.')
      : shouldAutoDownloadRecommended
        ? 'First-time setup is downloading the recommended OCR model.'
        : `${configuredModelLabel} is selected, but it is not ready yet.`

  const setupCardTitle =
    runtimeSetupIssue
      ? 'Local OCR runtime needs attention'
      : downloadState.status === 'error'
      ? shouldAutoDownloadRecommended
        ? 'Recommended model download needs another try'
        : 'Selected model needs attention'
      : shouldAutoDownloadRecommended
        ? 'Downloading the recommended OCR model'
        : `${configuredModelLabel} is not ready yet`

  const setupCardBody =
    runtimeSetupIssue
      ? runtimeStatus?.summary ||
        'No usable local OCR runtime is bundled right now.'
      : downloadState.status === 'error'
      ? downloadState.message ||
        (shouldAutoDownloadRecommended
          ? 'Open Advanced settings to retry the download.'
          : 'Open Advanced settings to install the selected model or switch back to a curated profile.')
      : shouldAutoDownloadRecommended
        ? 'This only happens on first setup or after models are removed.'
        : 'The current selection is missing locally or is missing its required mmproj companion.'

  const topBarStatusItems = [
    {
      label: 'Setup',
      value: modelMissing
        ? downloadState.status === 'error'
          ? 'Needs attention'
          : 'First run'
        : 'Ready',
    },
    { label: 'In progress', value: activeJobs },
    { label: 'Finished', value: completedJobs },
    {
      label: 'Preset',
      value: selectedPreset
        ? presetOptions.find((preset) => preset.id === selectedPreset)?.label || 'Selected'
        : 'Advanced custom',
      wide: true,
    },
  ]

  const runtimeLabel = effectiveSettings
    ? runtimeProfileLabel(appDefaults, effectiveSettings.runtime_profile)
    : 'Loading...'
  const effectiveRuntimeLabel =
    runtimeStatus?.effective_runtime_label || 'Checking local runtime...'
  const modelStorageLabel =
    storageInfo?.models_path || onboardingInfo?.model_storage_path || 'Loading...'

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
      onboardingInfo?.recommended_model_profile_id ||
        appDefaults?.recommended_model_profile_id ||
        null,
      true
    )
  })

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return

    const query = window.matchMedia('(prefers-color-scheme: dark)')
    const updateTheme = () => {
      setSystemTheme(query.matches ? 'dark' : 'light')
    }

    updateTheme()
    query.addEventListener('change', updateTheme)
    return () => {
      query.removeEventListener('change', updateTheme)
    }
  }, [])

  useEffect(() => {
    const root = document.documentElement
    root.dataset.theme = resolvedTheme
    root.dataset.themePreference = selectedThemeChoice
    root.style.colorScheme = resolvedTheme
  }, [resolvedTheme, selectedThemeChoice])

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
    Promise.all([
      invoke<AppDefaults>('get_app_defaults'),
      invoke<Settings>('get_settings').catch(() => null),
    ])
      .then(([defaults, result]) => {
        const nextSettings = result ?? defaults.settings
        setAppDefaults(defaults)
        setSettings(nextSettings)
        setSelectedPreset(getPresetForDpi(defaults.extraction_presets, nextSettings.dpi))
      })
      .catch((err) => {
        console.error(err)
        setAppDefaults(null)
        setSettings(null)
        setSelectedPreset(null)
      })
  }, [])

  useEffect(() => {
    if (!effectiveSettings) return
    void refreshModelStatus()
  }, [
    effectiveSettings,
    effectiveSettings?.model_file,
    effectiveSettings?.model_profile_id,
    effectiveSettings?.runtime_profile,
  ])

  useEffect(() => {
    if (!effectiveSettings) return
    void loadRuntimeStatus(effectiveSettings.runtime_profile)
  }, [effectiveSettings, effectiveSettings?.runtime_profile])

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
      void refreshLocalCatalog()
    }
  }, [settingsOpen])

  useEffect(() => {
    const handlePaste = (event: ClipboardEvent) => {
      if (busy) return
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

  async function persistSettings(next: Settings) {
    setSettings(next)
    await invoke('set_settings', { settings: next })
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

  async function loadRuntimeStatus(profile?: Settings['runtime_profile']) {
    const nextProfile = profile ?? effectiveSettings?.runtime_profile
    if (!nextProfile) {
      setRuntimeStatus(null)
      return
    }

    try {
      const status = await invoke<RuntimeStatus>('get_runtime_status', { profile: nextProfile })
      setRuntimeStatus(status)
    } catch (err) {
      console.error(err)
      setRuntimeStatus(null)
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

  async function refreshLocalCatalog() {
    await Promise.all([loadModelCatalog(), loadRuntimeStatus(effectiveSettings?.runtime_profile)])
  }

  async function handlePaths(paths: string[]) {
    if (!paths.length) return
    if (modelMissing) {
      const downloading = ['starting', 'downloading', 'verifying'].includes(downloadState.status)
      const blockedByRuntime = runtimeSetupIssue
      setLog(
        blockedByRuntime
          ? runtimeStatus?.summary || 'A usable local OCR runtime is required before extraction can start.'
          : downloading
          ? 'First-time setup is still downloading the OCR model.'
          : shouldAutoDownloadRecommended
            ? 'A local OCR model is required before extraction can start.'
            : `${configuredModelLabel} is selected, but it is not ready yet.`
      )
      enqueueToast(
        blockedByRuntime
          ? runtimeStatus?.summary || 'Add a local OCR runtime bundle before starting extraction.'
          : downloading
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
      const blockedByRuntime = runtimeSetupIssue
      enqueueToast(
        blockedByRuntime
          ? runtimeStatus?.summary || 'Add a local OCR runtime bundle before starting extraction.'
          : downloading
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
    setSelectedPreset(getPresetForDpi(presetOptions, next.dpi))
    setSettingsOpen(false)
    try {
      await persistSettings(next)
      enqueueToast('Settings saved.', 'success')
      await Promise.all([
        refreshModelStatus(),
        loadModelCatalog(),
        loadRuntimeStatus(next.runtime_profile),
      ])
    } catch (err) {
      console.error(err)
      enqueueToast('Failed to save settings.', 'error')
    }
  }

  async function handleThemeChange(nextTheme: ThemeChoice) {
    if (!effectiveSettings) return

    const previous = effectiveSettings
    const next = {
      ...effectiveSettings,
      theme: nextTheme,
    }

    setSettings(next)
    try {
      await persistSettings(next)
    } catch (err) {
      console.error(err)
      setSettings(previous)
      enqueueToast('Failed to update theme.', 'error')
    }
  }

  async function handleThemeToggle() {
    const nextTheme = resolvedTheme === 'dark' ? 'light' : 'dark'
    await handleThemeChange(nextTheme)
  }

  async function onDownloadModel(targetOverride?: string | null, isAuto = false) {
    if (!effectiveSettings) {
      enqueueToast('App defaults are still loading.', 'info')
      return
    }

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
      await refreshLocalCatalog()
      const next = {
        ...effectiveSettings,
        model_profile_id: result.profile_id || null,
        model_file: result.file_name,
      }
      setSettings(next)
      if (!selectedPreset) {
        setSelectedPreset(getPresetForDpi(presetOptions, next.dpi))
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
    <AppShell
      topBar={
        <TopBar
          statusItems={topBarStatusItems}
          themeLabel={currentThemeLabel}
          onToggleTheme={handleThemeToggle}
        />
      }
      warning={
        modelMissing || downloadState.status === 'error' ? (
          <div className="warning">{missingModelMessage}</div>
        ) : undefined
      }
      queue={
        <JobQueue
          jobs={jobs}
          selectedId={selectedId}
          streams={streams}
          onSelect={setSelectedId}
        />
      }
      importPanel={
        <ImportPanel
          onboardingOpen={onboardingOpen}
          onboardingStep={onboardingStep}
          onboardingSteps={onboardingSteps}
          onDismissOnboarding={() => dismissOnboarding(true)}
          onBackOnboarding={() =>
            setOnboardingStep((current) => Math.max(0, current - 1))
          }
          onNextOnboarding={() => {
            if (onboardingStep === onboardingSteps.length - 1) {
              dismissOnboarding(true)
              return
            }
            setOnboardingStep((current) =>
              Math.min(onboardingSteps.length - 1, current + 1)
            )
          }}
          showSetupCard={modelMissing || downloadState.status === 'error'}
          runtimeSetupIssue={runtimeSetupIssue}
          setupCardTitle={setupCardTitle}
          setupCardBody={setupCardBody}
          downloadState={downloadState}
          downloadProgressPercent={downloadProgressPercent}
          formatBytes={formatBytes}
          presetSummary={presetSummary}
          presetOrder={presetOrder}
          presetOptions={presetOptions}
          selectedPreset={selectedPreset}
          onSelectPreset={(preset, label) => {
            setSelectedPreset(preset)
            setLog(`${label} preset selected.`)
          }}
          busy={busy}
          modelMissing={modelMissing}
          onBrowseFiles={onBrowseFiles}
          onPasteImage={onPasteImage}
          onFiles={handlePaths}
          advancedOpen={advancedOpen}
          onToggleAdvanced={() => setAdvancedOpen((current) => !current)}
          appDefaults={appDefaults}
          prompt={prompt}
          onPromptChange={setPrompt}
          activeModelTitle={activeModelTitle}
          activeModelSupportLabel={activeModelSupportLabel}
          runtimeLabel={runtimeLabel}
          effectiveRuntimeLabel={effectiveRuntimeLabel}
          modelStorageLabel={modelStorageLabel}
          onOpenSettings={() => setSettingsOpen(true)}
          onSaveMarkdown={onSaveMarkdown}
          canSaveMarkdown={Boolean(selectedJob?.output_path)}
        />
      }
      preview={
        <PreviewWorkspace
          selectedJob={selectedJob}
          renderedMarkdown={selectedRenderedMarkdown}
          selectedStream={selectedStream}
          onRetry={onRetryJob}
          onCancel={onCancelJob}
          onOpenOutputFolder={onOpenOutputFolder}
          onRevealInExplorer={onRevealInExplorer}
          onCopyMarkdown={onCopyMarkdown}
          isCancelRequested={
            selectedJob ? Boolean(cancelingJobs[selectedJob.job_id]) : false
          }
        />
      }
      footer={
        <footer className="bottom-bar" aria-live="polite">
          <div className="log">{log}</div>
          <div className="bottom-note">{presetSummary}</div>
        </footer>
      }
      drawer={
        drawerSettings ? (
          <SettingsDrawer
            open={settingsOpen}
            settings={drawerSettings}
            appDefaults={appDefaults}
            modelCatalog={modelCatalog}
            runtimeStatus={runtimeStatus}
            storageInfo={storageInfo}
            modelInput={modelInput}
            modelStoragePath={onboardingInfo?.model_storage_path || null}
            downloadState={downloadState}
            onModelInputChange={setModelInput}
            onDownloadModel={onDownloadModel}
            onRefreshModels={refreshLocalCatalog}
            onClose={() => setSettingsOpen(false)}
            onSave={handleSettingsSave}
          />
        ) : undefined
      }
      toasts={
        <ToastNotifications
          toasts={toasts}
          onDismiss={(id) =>
            setToasts((prev) => prev.filter((toast) => toast.id !== id))
          }
        />
      }
    />
  )
}

export default App









