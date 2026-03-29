import { useEffect, useEffectEvent, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { AppShell } from './components/AppShell'
import { FirstRunWizard } from './components/FirstRunWizard'
import { ImportPanel } from './components/ImportPanel'
import { JobQueue } from './components/JobQueue'
import { PreviewWorkspace } from './components/PreviewWorkspace'
import { SettingsDrawer } from './components/SettingsDrawer'
import { TopBar } from './components/TopBar'
import { ToastNotifications, type Toast } from './components/ToastNotifications'
import type {
  AppDefaults,
  AppEvent,
  ExtractTemplateDefinition,
  ExtractionPreset,
  JobPreviewPage,
  JobResult,
  JobStatus,
  JobStreamState,
  ModelCatalog,
  ModelDownloadEvent,
  RecommendedSetupInfo,
  RunOptions,
  RuntimeStatus,
  RunnerMode,
  RunnerStage,
  Settings,
  StorageInfo,
  OnboardingInfo,
  WorkflowMode,
  WorkflowModeDefinition,
  WorkflowModeExport,
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

type PresetKey = 'starter' | 'recommended' | 'quality' | 'faster'
type ThemeChoice = 'light' | 'dark' | 'system'
type ResolvedTheme = 'light' | 'dark'

type StructuredExtractExport = {
  template_id: string
  template_label: string
  source_page_count: number
  summary: Array<{ text: string; source_pages: number[] }>
  fields: Array<{
    key: string
    label: string
    value?: string | null
    source_pages: number[]
    needs_verification: boolean
    verification_note?: string | null
  }>
  rows: Array<{
    cells: Array<{ column: string; value: string }>
    source_pages: number[]
    needs_verification: boolean
    verification_note?: string | null
  }>
  verification: Array<{ text: string; source_pages: number[] }>
  csv_export?: {
    mode: string
    columns: string[]
    rows: string[][]
  } | null
}

const fallbackWorkflowMode: WorkflowModeDefinition = {
  id: 'exact_ocr',
  label: 'Exact OCR',
  short_label: 'Exact',
  description: 'Preserve the current OCR-to-markdown behavior.',
  helper: 'Use the current OCR-first markdown flow.',
  result_label: 'Markdown',
  empty_state_copy: 'Markdown will appear here when text is ready.',
  copy_action_label: 'Copy markdown',
  save_action_label: 'Export markdown',
  advanced_panel_copy: 'Use custom instructions only when you want to override the default OCR behavior.',
  prompt_label: 'Custom OCR override',
  prompt_hint: 'Optional. Leave blank to keep the default OCR behavior.',
  prompt_placeholder: 'Extract all text from the image and return it as markdown.',
  default_prompt: 'Extract all text from the image and return it as markdown.',
  available_exports: [
    {
      id: 'markdown',
      label: 'Markdown',
      extension: 'md',
      description: 'Faithful OCR markdown output.',
      primary: true,
    },
  ],
}

const defaultDownloadState: ModelDownloadState = {
  status: 'idle',
  progress: 0,
}

function isActiveStatus(status: JobStatus) {
  return !isTerminalStatus(status)
}

function isTerminalStatus(status: JobStatus) {
  return ['Done', 'Failed', 'Canceled'].includes(status)
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

function workflowModeDefinition(
  appDefaults: AppDefaults | null,
  mode?: WorkflowMode | null
) {
  if (!mode) return fallbackWorkflowMode
  return (
    appDefaults?.workflow_modes.find((candidate) => candidate.id === mode) || fallbackWorkflowMode
  )
}

function normalizeExportExtension(path: string, exportOption: WorkflowModeExport) {
  if (/\.[^.\\/]+$/.test(path)) {
    return path
  }
  return `${path}.${exportOption.extension}`
}

function markdownToPlainText(markdown: string) {
  return markdown
    .replace(/```[\s\S]*?```/g, '')
    .replace(/^#{1,6}\s+/gm, '')
    .replace(/^\s*[-*+]\s+/gm, '')
    .replace(/^\s*\d+\.\s+/gm, '')
    .replace(/\[(.*?)\]\(.*?\)/g, '$1')
    .replace(/[*_`>#-]/g, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

function escapeRegex(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

function deriveExportTitle(
  markdown: string,
  job: JobResult | null,
  modeDefinition: WorkflowModeDefinition
) {
  const headingMatch = markdown.match(/^#\s+(.+)$/m)
  if (headingMatch?.[1]?.trim()) {
    return headingMatch[1].trim()
  }

  const sourceName = getFileName(job?.source)
  const withoutExtension = sourceName.replace(/\.[^.]+$/, '').trim()
  return withoutExtension || modeDefinition.label
}

function readMarkdownSection(markdown: string, heading: string) {
  const pattern = new RegExp(
    `^##\\s+${escapeRegex(heading)}\\s*$([\\s\\S]*?)(?=^##\\s+|\\Z)`,
    'im'
  )
  const match = markdown.match(pattern)
  return match?.[1]?.trim() || ''
}

function stripSourceReferenceSuffix(value: string) {
  return value
    .replace(/\s*_\(\s*Source:\s*.+\)_\s*$/i, '')
    .trim()
}

function readSectionBullets(markdown: string, heading: string) {
  return readMarkdownSection(markdown, heading)
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.startsWith('- '))
    .map((line) => line.slice(2).trim())
    .map(stripSourceReferenceSuffix)
    .filter(Boolean)
}

function csvEscape(value: string) {
  return `"${value.replace(/"/g, '""')}"`
}

function notesToAnkiCsv(markdown: string, fallbackTitle: string) {
  const deck = deriveExportTitle(markdown, null, {
    ...fallbackWorkflowMode,
    label: fallbackTitle,
  })
  const rows: Array<[string, string, string, string]> = []
  const glossaryItems = readSectionBullets(markdown, 'Glossary')
  const formulaItems = readSectionBullets(markdown, 'Formulas')
  const reviewQuestions = readSectionBullets(markdown, 'Review Questions')
  const keyPoints = readSectionBullets(markdown, 'Key Points')
  const examples = readSectionBullets(markdown, 'Examples')

  glossaryItems.forEach((item) => {
    const match = item.match(/^\*\*(.+?)\*\*:\s*(.+)$/)
    if (match) {
      rows.push([match[1].trim(), match[2].trim(), deck, 'glossary'])
    }
  })

  formulaItems.forEach((item) => {
    const formula = item.replace(/^`|`$/g, '').trim()
    if (formula) {
      rows.push([`When do you use ${formula}?`, formula, deck, 'formula'])
    }
  })

  reviewQuestions.forEach((item) => {
    rows.push([item, '', deck, 'review'])
  })

  keyPoints.forEach((item) => {
    rows.push([`Key point from ${deck}`, item, deck, 'key-point'])
  })

  examples.forEach((item) => {
    rows.push([`Example from ${deck}`, item, deck, 'example'])
  })

  if (!rows.length) {
    rows.push([`Summary of ${deck}`, markdownToPlainText(markdown), deck, 'summary'])
  }

  return ['Front,Back,Deck,Tags', ...rows.map((row) => row.map(csvEscape).join(','))].join('\n')
}

function readStructuredExtract(markdown: string): StructuredExtractExport | null {
  const match = markdown.match(/<!--\s*visitexta-extract:\s*([\s\S]*?)\s*-->/i)
  if (!match?.[1]) return null

  try {
    return JSON.parse(match[1]) as StructuredExtractExport
  } catch {
    return null
  }
}

function structuredExtractToCsv(data: StructuredExtractExport) {
  const columns = data.csv_export?.columns ?? []
  const rows = data.csv_export?.rows ?? []

  if (!columns.length) {
    return ''
  }

  return [columns, ...rows]
    .map((row) => row.map((value) => csvEscape(value ?? '')).join(','))
    .join('\n')
}

function exportContent(
  exportOption: WorkflowModeExport,
  modeDefinition: WorkflowModeDefinition,
  job: JobResult | null,
  renderedMarkdown: string
) {
  const markdown = renderedMarkdown.trim()
  const structuredExtract =
    modeDefinition.id === 'extract' ? readStructuredExtract(markdown) : null
  if (exportOption.id === 'markdown') {
    return markdown
  }

  const plainText = markdownToPlainText(markdown)
  if (exportOption.id === 'text') {
    return plainText
  }

  if (exportOption.id === 'csv') {
    if (modeDefinition.id === 'extract' && structuredExtract) {
      return structuredExtractToCsv(structuredExtract)
    }
    return notesToAnkiCsv(markdown, modeDefinition.label)
  }

  if (exportOption.id === 'json' && structuredExtract) {
    return JSON.stringify(structuredExtract, null, 2)
  }

  return JSON.stringify(
    {
      workflow_mode: modeDefinition.id,
      workflow_label: modeDefinition.label,
      source: job?.source ?? null,
      output_path: job?.output_path ?? null,
      markdown,
      plain_text: plainText,
    },
    null,
    2
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

function isDownloadActive(status: ModelDownloadState['status']) {
  return status === 'starting' || status === 'downloading' || status === 'verifying'
}

function isNetworkError(message?: string | null) {
  const value = message?.toLowerCase() ?? ''
  return (
    value.includes('dns') ||
    value.includes('network') ||
    value.includes('timeout') ||
    value.includes('timed out') ||
    value.includes('connection') ||
    value.includes('offline') ||
    value.includes('failed to send request')
  )
}

function storageModeLabel(mode?: StorageInfo['mode'] | OnboardingInfo['storage_mode']) {
  return mode === 'portable' ? 'Portable storage' : 'Installer storage'
}

function storageModeHint(mode?: StorageInfo['mode'] | OnboardingInfo['storage_mode']) {
  return mode === 'portable'
    ? 'Model files stay beside the portable app so you can move the whole setup together.'
    : 'Model files stay under your local app data folder so the installed app can reuse them.'
}

function describeValidationStatus(downloadState: ModelDownloadState) {
  const message = downloadState.message?.toLowerCase() ?? ''

  if (downloadState.status === 'verifying') {
    return {
      tone: 'info' as const,
      label: 'Validating download',
    }
  }

  if (downloadState.status === 'done') {
    if (message.includes('verified')) {
      return {
        tone: 'success' as const,
        label: 'Checksum verified',
      }
    }

    return {
      tone: 'success' as const,
      label: 'Ready to use',
    }
  }

  if (downloadState.status === 'error') {
    if (message.includes('checksum') || message.includes('mismatch')) {
      return {
        tone: 'error' as const,
        label: 'Validation failed',
      }
    }

    return {
      tone: 'warning' as const,
      label: 'Waiting to retry',
    }
  }

  if (message.includes('resume')) {
    return {
      tone: 'info' as const,
      label: 'Resume supported',
    }
  }

  return {
    tone: 'info' as const,
    label: 'SHA-256 validation',
  }
}

function prioritizeExportOption(
  exportOptions: WorkflowModeExport[],
  preferredId?: WorkflowModeExport['id']
) {
  if (!preferredId) return exportOptions
  const preferred = exportOptions.find((exportOption) => exportOption.id === preferredId)
  if (!preferred) return exportOptions
  return [preferred, ...exportOptions.filter((exportOption) => exportOption.id !== preferredId)]
}

function describeDownloadStatus(
  downloadState: ModelDownloadState,
  formatBytesLabel: (value?: number | null) => string | null
) {
  if (downloadState.status === 'error') {
    return downloadState.message || 'Download failed.'
  }

  if (downloadState.status === 'verifying') {
    return downloadState.message || 'Verifying the downloaded files.'
  }

  if (downloadState.status === 'done') {
    return downloadState.message || 'Model download is complete.'
  }

  if (downloadState.total_bytes) {
    return `${Math.round(downloadState.progress * 100)}% (${formatBytesLabel(downloadState.downloaded_bytes)} / ${formatBytesLabel(downloadState.total_bytes)})`
  }

  if (downloadState.downloaded_bytes) {
    return `${formatBytesLabel(downloadState.downloaded_bytes)} downloaded`
  }

  return downloadState.message || 'Ready to download the recommended model.'
}

function setupHelperMessage(
  downloadState: ModelDownloadState,
  recommendedSetupInfo: RecommendedSetupInfo | null
) {
  if (downloadState.message?.toLowerCase().includes('resum')) {
    return 'A previous partial download was found, so setup can resume instead of starting over.'
  }

  if (downloadState.status === 'error') {
    if (isNetworkError(downloadState.message)) {
      return 'The network request did not complete. Check your connection and try again. Any partial model file can resume later.'
    }

    if (downloadState.message?.toLowerCase().includes('checksum')) {
      return 'The download did not validate cleanly, so VisiTexta will fetch a fresh copy on retry.'
    }
  }

  if (recommendedSetupInfo?.availability_error) {
    return 'Size estimates could not be refreshed right now, but setup can still start when the network is available.'
  }

  return 'The recommended setup uses the same curated model catalog and downloader as Advanced settings.'
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

function applyRunPreferencesToStreams(
  previous: Record<string, JobStreamState>,
  jobs: JobResult[],
  runOptions: RunOptions | null
) {
  if (!jobs.length || !runOptions) return previous

  const next = { ...previous }
  for (const job of jobs) {
    const current = next[job.job_id] ?? createEmptyStreamState()
    next[job.job_id] = {
      ...current,
      lazy_preview_thumbnails: runOptions.lazy_preview_thumbnails ?? false,
      disable_rich_preview_for_large_jobs:
        runOptions.disable_rich_preview_for_large_jobs ?? false,
      large_job_page_threshold: runOptions.large_job_page_threshold ?? null,
    }
  }
  return next
}

function App() {
  const [appDefaults, setAppDefaults] = useState<AppDefaults | null>(null)
  const [busy, setBusy] = useState(false)
  const [jobs, setJobs] = useState<JobResult[]>([])
  const [streams, setStreams] = useState<Record<string, JobStreamState>>({})
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [markdown, setMarkdown] = useState('')
  const [log, setLog] = useState('Choose a mode and preset, then drop, paste, or pick a file to begin.')
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
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [selectedPreset, setSelectedPreset] = useState<PresetKey | null>(null)
  const [onboardingInfo, setOnboardingInfo] = useState<OnboardingInfo | null>(null)
  const [recommendedSetupInfo, setRecommendedSetupInfo] = useState<RecommendedSetupInfo | null>(null)
  const [setupWizardOpen, setSetupWizardOpen] = useState(false)
  const [cancelingJobs, setCancelingJobs] = useState<Record<string, boolean>>({})
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(() => readSystemTheme())
  const effectiveSettings = settings ?? appDefaults?.settings ?? null
  const drawerSettings = effectiveSettings ?? appDefaults?.settings ?? null
  const selectedWorkflowMode =
    effectiveSettings?.workflow_mode ?? appDefaults?.settings.workflow_mode ?? 'exact_ocr'
  const selectedModeDefinition = workflowModeDefinition(appDefaults, selectedWorkflowMode)
  const workflowModes = appDefaults?.workflow_modes ?? [selectedModeDefinition]
  const extractTemplates = appDefaults?.extract_templates ?? []
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

  const selectedJobModeDefinition = useMemo(
    () => workflowModeDefinition(appDefaults, selectedJob?.workflow_mode ?? selectedWorkflowMode),
    [appDefaults, selectedJob?.workflow_mode, selectedWorkflowMode]
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
  const configuredModelLabel =
    explicitModelFile || selectedProfile?.label || activeModelTitle || 'Selected OCR model'

  const activeJobs = useMemo(
    () => jobs.filter((job) => isActiveStatus(job.status)).length,
    [jobs]
  )

  const finishedJobs = useMemo(
    () => jobs.filter((job) => isTerminalStatus(job.status)).length,
    [jobs]
  )

  const selectedRenderedMarkdown = useMemo(() => {
    const streamText = selectedStream?.streamed_markdown?.trim() || ''
    if (selectedJob?.status === 'Done') {
      return markdown || streamText
    }
    return streamText || markdown
  }, [markdown, selectedJob?.status, selectedStream?.streamed_markdown])

  const selectedPresetConfig = useMemo(
    () => presetOptions.find((preset) => preset.id === selectedPreset) ?? null,
    [presetOptions, selectedPreset]
  )

  const selectedExtractTemplate = useMemo<ExtractTemplateDefinition | null>(() => {
    if (!extractTemplates.length) return null
    const templateId = effectiveSettings?.extract_template_id
    return (
      extractTemplates.find((template) => template.id === templateId) ||
      extractTemplates[0] ||
      null
    )
  }, [effectiveSettings?.extract_template_id, extractTemplates])

  const effectiveDpi = useMemo(
    () =>
      selectedPresetConfig
        ? selectedPresetConfig.dpi
        : effectiveSettings?.dpi ?? 300,
    [effectiveSettings?.dpi, selectedPresetConfig]
  )

  const effectiveRunOptions = useMemo<RunOptions | null>(() => {
    if (!effectiveSettings) return null

    return {
      workflow_mode: effectiveSettings.workflow_mode,
      study_boost: effectiveSettings.study_boost,
      extract_template_id: effectiveSettings.extract_template_id,
      runtime_profile:
        selectedPresetConfig?.runtime_profile_override ?? effectiveSettings.runtime_profile,
      max_ocr_dimension:
        selectedPresetConfig?.max_ocr_dimension ?? effectiveSettings.max_ocr_dimension,
      lazy_preview_thumbnails:
        selectedPresetConfig?.lazy_preview_thumbnails ?? effectiveSettings.lazy_preview_thumbnails,
      disable_rich_preview_for_large_jobs:
        selectedPresetConfig?.disable_rich_preview_for_large_jobs ??
        effectiveSettings.disable_rich_preview_for_large_jobs,
      large_job_page_threshold:
        selectedPresetConfig?.large_job_page_threshold ??
        effectiveSettings.large_job_page_threshold,
    }
  }, [effectiveSettings, selectedPresetConfig])

  const effectiveRuntimeProfile =
    effectiveRunOptions?.runtime_profile ?? effectiveSettings?.runtime_profile ?? null

  const runtimeSetupIssue = Boolean(runtimeStatus && !runtimeStatus.usable_runtime)

  const presetSummary = useMemo(() => {
    const templateSummary =
      selectedWorkflowMode === 'extract' && selectedExtractTemplate
        ? ` · Template: ${selectedExtractTemplate.label}`
        : ''
    if (!selectedPreset) {
      return `Custom DPI active · ${effectiveSettings?.dpi ?? 300} DPI${templateSummary}`
    }
    return `${selectedPresetConfig?.label || 'Selected'} preset · ${selectedPresetConfig?.dpi ?? effectiveSettings?.dpi ?? 300} DPI${templateSummary}`
  }, [
    effectiveSettings?.dpi,
    selectedExtractTemplate,
    selectedPreset,
    selectedPresetConfig,
    selectedWorkflowMode,
  ])

  const selectedJobName = getFileName(selectedJob?.source)
  const downloadProgressPercent = Math.min(
    100,
    Math.max(0, Math.round(downloadState.progress * 100))
  )
  const setupDownloadActive = isDownloadActive(downloadState.status)
  const validationStatus = describeValidationStatus(downloadState)
  const recommendedSetupModel =
    recommendedSetupInfo ||
    (defaultModelProfile
      ? {
          label: defaultModelProfile.label,
          family: defaultModelProfile.family,
          file_name: defaultModelProfile.default_file,
          mmproj_file: null,
          notes: defaultModelProfile.notes,
          availability_error: null,
          estimated_download_bytes: null,
        }
      : null)
  const setupStorageMode = storageInfo?.mode || onboardingInfo?.storage_mode
  const setupStoragePath =
    storageInfo?.models_path || onboardingInfo?.model_storage_path || 'Loading...'
  const estimatedDiskUse =
    formatBytes(recommendedSetupInfo?.estimated_download_bytes) ||
    'Checking size estimate...'
  const setupStatusLabel = runtimeSetupIssue
    ? 'Runtime required'
    : downloadState.status === 'error'
      ? 'Retry needed'
      : setupDownloadActive
        ? downloadState.message?.toLowerCase().includes('resum')
          ? 'Resuming download'
          : 'Downloading model'
        : modelMissing
          ? 'Model required'
          : 'Ready'
  const setupStatusTone =
    runtimeSetupIssue
      ? 'warning'
      : downloadState.status === 'error'
        ? 'error'
        : downloadState.status === 'done'
          ? 'success'
          : 'info'
  const setupDownloadText = describeDownloadStatus(downloadState, formatBytes)
  const setupHelper = setupHelperMessage(downloadState, recommendedSetupInfo)

  const missingModelMessage =
    runtimeSetupIssue
      ? runtimeStatus?.summary || 'A usable local OCR runtime was not found.'
      : downloadState.status === 'error'
        ? downloadState.message || 'First-run model setup needs another try.'
        : setupDownloadActive
          ? 'First-run setup is still downloading the recommended OCR model.'
          : `${configuredModelLabel} is selected, but no supported OCR model is ready yet.`

  const setupCardTitle =
    runtimeSetupIssue
      ? 'Local OCR runtime needs attention'
      : downloadState.status === 'error'
        ? 'Recommended model download needs another try'
        : setupDownloadActive
          ? 'Downloading the recommended OCR model'
          : `${configuredModelLabel} is not ready yet`

  const setupCardBody =
    runtimeSetupIssue
      ? runtimeStatus?.summary ||
        'No usable local OCR runtime is bundled right now.'
      : downloadState.status === 'error'
        ? downloadState.message || 'Open setup to retry the recommended model download.'
        : setupDownloadActive
          ? 'This only happens on first setup or after model files are removed.'
          : 'Open setup to install the recommended OCR model before starting extraction.'

  const runtimeLabel = effectiveRuntimeProfile
    ? runtimeProfileLabel(appDefaults, effectiveRuntimeProfile)
    : 'Loading...'
  const effectiveRuntimeLabel =
    runtimeStatus?.effective_runtime_label || 'Checking local runtime...'
  const modelStorageLabel =
    storageInfo?.models_path || onboardingInfo?.model_storage_path || 'Loading...'
  const selectedPresetLabel =
    selectedPresetConfig?.label || (selectedPreset ? 'Selected' : 'Advanced custom')
  const topBarStatusItems = [
    {
      label: 'Queue',
      value: activeJobs ? `${activeJobs} active` : 'Idle',
    },
    {
      label: 'Finished',
      value: finishedJobs,
    },
    {
      label: 'Runtime',
      value: runtimeSetupIssue ? 'Needs attention' : runtimeLabel,
    },
    {
      label: 'Model',
      value: activeModelTitle,
      wide: true,
    },
  ]
  const topBarContextLabel = `${selectedModeDefinition.label} workflow`
  const topBarContextDetail = [
    `Preset: ${selectedPresetLabel}`,
    selectedWorkflowMode === 'extract' && selectedExtractTemplate
      ? `Template: ${selectedExtractTemplate.label}`
      : null,
    modelMissing
      ? downloadState.status === 'error'
        ? 'Setup needs attention'
        : 'First-run setup required'
      : 'Processing stays on this PC',
  ]
    .filter(Boolean)
    .join(' • ')

  const handlePathsEvent = useEffectEvent((paths: string[]) => {
    void handlePaths(paths)
  })

  const handleAppEventListener = useEffectEvent((payload: AppEvent) => {
    handleAppEvent(payload)
  })

  const handlePastedImageEvent = useEffectEvent((blob: Blob) => {
    void handlePastedImage(blob)
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
    effectiveRuntimeProfile,
  ])

  useEffect(() => {
    if (!effectiveRuntimeProfile) return
    void loadRuntimeStatus(effectiveRuntimeProfile)
  }, [effectiveRuntimeProfile])

  useEffect(() => {
    void loadModelCatalog()
    invoke<OnboardingInfo>('get_onboarding_info')
      .then((info) => setOnboardingInfo(info))
      .catch(() => setOnboardingInfo(null))
    invoke<StorageInfo>('get_storage_info')
      .then((info) => setStorageInfo(info))
      .catch(() => setStorageInfo(null))
    invoke<RecommendedSetupInfo>('get_recommended_setup_info')
      .then((info) => setRecommendedSetupInfo(info))
      .catch(() => setRecommendedSetupInfo(null))
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
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((toast) => toast.id !== id))
    }, 4000)
  }

  function clearCancelRequest(jobId: string) {
    setCancelingJobs((prev) => removeCancelingJob(prev, jobId))
  }

  function openSetupWizard() {
    setSetupWizardOpen(true)
  }

  function closeSetupWizard() {
    setSetupWizardOpen(false)
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
            workflow_mode: update.workflow_mode ?? selectedWorkflowMode,
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
      const exists = await invoke<boolean>('check_model_exists', {
        profile: effectiveRuntimeProfile,
      })
      setModelMissing(!exists)
    } catch {
      setModelMissing(false)
    }
  }

  async function loadRuntimeStatus(profile?: Settings['runtime_profile']) {
    const nextProfile = profile ?? effectiveRuntimeProfile
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
    await Promise.all([loadModelCatalog(), loadRuntimeStatus(effectiveRuntimeProfile ?? undefined)])
  }

  async function handleClearFinishedJobs() {
    if (!finishedJobs) return

    try {
      const clearedCount = await invoke<number>('clear_terminal_job_history')
      if (!clearedCount) return

      const nextJobs = await invoke<JobResult[]>('get_job_history').catch(() =>
        jobs.filter((job) => !isTerminalStatus(job.status))
      )
      const remainingIds = new Set(nextJobs.map((job) => job.job_id))

      setJobs(nextJobs)
      setStreams((prev) =>
        Object.fromEntries(
          Object.entries(prev).filter(([jobId]) => remainingIds.has(jobId))
        )
      )
      setCancelingJobs((prev) =>
        Object.fromEntries(
          Object.entries(prev).filter(([jobId]) => remainingIds.has(jobId))
        )
      )
      setSelectedId((current) => {
        if (!current || remainingIds.has(current)) {
          return current
        }
        return nextJobs[0]?.job_id ?? null
      })
      enqueueToast(
        `Cleared ${clearedCount} finished job${clearedCount === 1 ? '' : 's'}`,
        'success'
      )
    } catch (err) {
      console.error(err)
      const message = err instanceof Error ? err.message : String(err)
      enqueueToast(message || 'Could not clear finished jobs.', 'error')
    }
  }

  async function handlePaths(paths: string[]) {
    if (!paths.length) return
    if (modelMissing) {
      const downloading = isDownloadActive(downloadState.status)
      const blockedByRuntime = runtimeSetupIssue
      openSetupWizard()
      setLog(
        blockedByRuntime
          ? runtimeStatus?.summary || 'A usable local OCR runtime is required before extraction can start.'
          : downloading
          ? 'First-time setup is still downloading the OCR model.'
          : 'A local OCR model is required before extraction can start.'
      )
      enqueueToast(
        blockedByRuntime
          ? runtimeStatus?.summary || 'Add a local OCR runtime bundle before starting extraction.'
          : downloading
            ? 'Setup is still downloading the OCR model.'
            : 'Finish first-run model setup before starting extraction.',
        'error'
      )
      return
    }

    setBusy(true)
    setLog(
      paths.length === 1
        ? `Starting ${selectedModeDefinition.label.toLowerCase()}...`
        : `Starting ${selectedModeDefinition.label.toLowerCase()} for ${paths.length} files...`
    )

    try {
      const result = (await invoke('enqueue_jobs', {
        paths,
        prompt: prompt.trim() || null,
        dpi: effectiveDpi,
        runOptions: effectiveRunOptions,
      })) as JobResult[]
      setJobs((prev) => mergeJobs(prev, result))
      setStreams((prev) => applyRunPreferencesToStreams(prev, result, effectiveRunOptions))
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
      const downloading = isDownloadActive(downloadState.status)
      const blockedByRuntime = runtimeSetupIssue
      openSetupWizard()
      enqueueToast(
        blockedByRuntime
          ? runtimeStatus?.summary || 'Add a local OCR runtime bundle before starting extraction.'
          : downloading
          ? 'Wait for the recommended model download to finish before pasting an image.'
            : 'Finish first-run model setup before pasting an image.',
        'info'
      )
      return
    }

    setBusy(true)
    setLog(`Starting ${selectedModeDefinition.label.toLowerCase()} from the pasted image...`)

    try {
      const imageBase64 = await blobToDataUrl(blob)
      const result = (await invoke('enqueue_pasted_image', {
        imageBase64,
        mimeType: blob.type || 'image/png',
        prompt: prompt.trim() || null,
        dpi: effectiveDpi,
        runOptions: effectiveRunOptions,
      })) as JobResult[]
      setJobs((prev) => mergeJobs(prev, result))
      setStreams((prev) => applyRunPreferencesToStreams(prev, result, effectiveRunOptions))
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
      enqueueToast(`Select a job with ${selectedJobModeDefinition.result_label.toLowerCase()} first.`, 'info')
      return
    }

    try {
      if (text) {
        await navigator.clipboard.writeText(text)
      } else if (selectedJob?.output_path) {
        await invoke('copy_file_to_clipboard', { path: selectedJob.output_path })
      }
      enqueueToast(`${selectedJobModeDefinition.result_label} copied.`, 'success')
    } catch (err) {
      console.error(err)
      if (selectedJob?.output_path) {
        try {
          await invoke('copy_file_to_clipboard', { path: selectedJob.output_path })
          enqueueToast(`${selectedJobModeDefinition.result_label} copied.`, 'success')
          return
        } catch (fallbackError) {
          console.error(fallbackError)
        }
      }
      enqueueToast('Copy failed.', 'error')
    }
  }

  async function onSaveMarkdown(preferredExportId?: WorkflowModeExport['id']) {
    const text = selectedRenderedMarkdown.trim()
    const exportOptions = selectedJobModeDefinition.available_exports
    const prioritizedExports = prioritizeExportOption(exportOptions, preferredExportId)
    const primaryExport =
      prioritizedExports.find((exportOption) => exportOption.primary) || prioritizedExports[0]

    if (!selectedJob?.output_path && !text) {
      enqueueToast('Select a completed job first.', 'info')
      return
    }

    const dest = await save({
      defaultPath: normalizeExportExtension(
        selectedJob?.output_path || getFileName(selectedJob?.source),
        primaryExport
      ),
      filters: prioritizedExports.map((exportOption) => ({
        name: exportOption.label,
        extensions: [exportOption.extension],
      })),
    })
    if (!dest) return
    try {
      const rendered =
        text ||
        (selectedJob?.output_path
          ? await invoke<string>('read_markdown_file', { path: selectedJob.output_path }).catch(
              () => ''
            )
          : '')
      const selectedExport =
        exportOptions.find((exportOption) =>
          dest.toLowerCase().endsWith(`.${exportOption.extension}`)
        ) ||
        primaryExport
      const normalizedPath = normalizeExportExtension(dest, selectedExport)
      if (selectedExport.id === 'pdf') {
        await invoke('save_pdf_as', {
          title: deriveExportTitle(rendered, selectedJob, selectedJobModeDefinition),
          content: rendered,
          destPath: normalizedPath,
        })
      } else {
        await invoke('save_text_as', {
          content: exportContent(
            selectedExport,
            selectedJobModeDefinition,
            selectedJob,
            rendered
          ),
          destPath: normalizedPath,
        })
      }
      enqueueToast(`${selectedExport.label} saved.`, 'success')
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

  async function handleWorkflowModeChange(nextMode: WorkflowMode) {
    if (!effectiveSettings || nextMode === effectiveSettings.workflow_mode) return

    const previous = effectiveSettings
    const next = {
      ...effectiveSettings,
      workflow_mode: nextMode,
    }

    setSettings(next)
    setLog(`${workflowModeDefinition(appDefaults, nextMode).label} mode selected.`)
    try {
      await persistSettings(next)
    } catch (err) {
      console.error(err)
      setSettings(previous)
      enqueueToast('Failed to switch mode.', 'error')
    }
  }

  async function handleStudyBoostChange(nextStudyBoost: boolean) {
    if (!effectiveSettings || nextStudyBoost === effectiveSettings.study_boost) return

    const previous = effectiveSettings
    const next = {
      ...effectiveSettings,
      study_boost: nextStudyBoost,
    }

    setSettings(next)
    setLog(
      nextStudyBoost
        ? 'Study boost enabled for Notes mode.'
        : 'Study boost disabled for Notes mode.'
    )
    try {
      await persistSettings(next)
    } catch (err) {
      console.error(err)
      setSettings(previous)
      enqueueToast('Failed to update Study boost.', 'error')
    }
  }

  async function handleExtractTemplateChange(nextTemplateId: string) {
    if (!effectiveSettings || nextTemplateId === effectiveSettings.extract_template_id) return

    const previous = effectiveSettings
    const next = {
      ...effectiveSettings,
      extract_template_id: nextTemplateId,
    }

    setSettings(next)
    const templateLabel =
      extractTemplates.find((template) => template.id === nextTemplateId)?.label ||
      'Extract template'
    setLog(`${templateLabel} template selected.`)
    try {
      await persistSettings(next)
    } catch (err) {
      console.error(err)
      setSettings(previous)
      enqueueToast('Failed to switch extract template.', 'error')
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
    setSetupWizardOpen(true)

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
      setLog('First-time setup is complete. Drop, paste, or choose a file to begin.')
      setModelInput('')
      await refreshLocalCatalog()
      invoke<RecommendedSetupInfo>('get_recommended_setup_info')
        .then((info) => setRecommendedSetupInfo(info))
        .catch(() => {})
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
      setLog('First-time setup could not finish. Retry the download or open Advanced settings.')
      enqueueToast(message || 'Download failed.', 'error')
    }
  }

  async function startRecommendedSetup() {
    await onDownloadModel(
      recommendedSetupInfo?.profile_id ||
        onboardingInfo?.recommended_model_profile_id ||
        appDefaults?.recommended_model_profile_id ||
        null,
      true
    )
  }

  return (
    <AppShell
      topBar={
        <TopBar
          contextLabel={topBarContextLabel}
          contextDetail={topBarContextDetail}
          statusItems={topBarStatusItems}
          themeLabel={currentThemeLabel}
          onToggleTheme={handleThemeToggle}
          onOpenSettings={() => setSettingsOpen(true)}
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
          activeCount={activeJobs}
          finishedCount={finishedJobs}
          selectedId={selectedId}
          streams={streams}
          onClearFinished={handleClearFinishedJobs}
          onSelect={setSelectedId}
        />
      }
      importPanel={
        <ImportPanel
          modeDefinition={selectedModeDefinition}
          modeOptions={workflowModes}
          selectedMode={selectedWorkflowMode}
          onSelectMode={handleWorkflowModeChange}
          showSetupCard={modelMissing || downloadState.status === 'error'}
          runtimeSetupIssue={runtimeSetupIssue}
          setupCardTitle={setupCardTitle}
          setupCardBody={setupCardBody}
          downloadState={downloadState}
          downloadProgressPercent={downloadProgressPercent}
          formatBytes={formatBytes}
          onOpenSetupWizard={openSetupWizard}
          presetSummary={presetSummary}
          presetTradeoff={selectedPresetConfig?.tradeoff ?? null}
          presetOrder={presetOrder}
          presetOptions={presetOptions}
          selectedPreset={selectedPreset}
          onSelectPreset={(preset, label) => {
            setSelectedPreset(preset)
            setLog(`${label} preset selected.`)
          }}
          extractTemplates={extractTemplates}
          selectedExtractTemplateId={selectedExtractTemplate?.id ?? null}
          onSelectExtractTemplate={handleExtractTemplateChange}
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
          studyBoost={effectiveSettings?.study_boost ?? false}
          onToggleStudyBoost={handleStudyBoostChange}
          activeModelTitle={activeModelTitle}
          activeModelSupportLabel={activeModelSupportLabel}
          runtimeLabel={runtimeLabel}
          effectiveRuntimeLabel={effectiveRuntimeLabel}
          modelStorageLabel={modelStorageLabel}
          onOpenSettings={() => setSettingsOpen(true)}
          onExportResult={onSaveMarkdown}
          canExportResult={Boolean(selectedJob?.output_path || selectedRenderedMarkdown.trim())}
        />
      }
      preview={
        <PreviewWorkspace
          selectedJob={selectedJob}
          renderedMarkdown={selectedRenderedMarkdown}
          modeDefinition={selectedJobModeDefinition}
          selectedStream={selectedStream}
          activeModelLabel={activeModelTitle}
          runtimeLabel={effectiveRuntimeLabel}
          storageModeLabel={storageModeLabel(setupStorageMode)}
          onRetry={onRetryJob}
          onCancel={onCancelJob}
          onOpenOutputFolder={onOpenOutputFolder}
          onRevealInExplorer={onRevealInExplorer}
          onCopyMarkdown={onCopyMarkdown}
          onExportResult={onSaveMarkdown}
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
      overlay={
        <FirstRunWizard
          open={setupWizardOpen}
          statusLabel={setupStatusLabel}
          statusTone={setupStatusTone}
          title={
            runtimeSetupIssue
              ? 'A local OCR runtime is required before setup can continue'
              : downloadState.status === 'done'
                ? 'Recommended OCR model is ready'
                : 'Install the recommended OCR model to finish first-run setup'
          }
          description={
            runtimeSetupIssue
              ? runtimeStatus?.summary ||
                'VisiTexta needs a usable local OCR runtime bundle before it can download and run models.'
              : downloadState.status === 'error'
                ? downloadState.message ||
                  'The recommended download did not finish. Retry when you are ready.'
                : 'VisiTexta runs OCR locally. The first setup downloads the recommended model once, stores it on this PC, and then reuses it for later jobs.'
          }
          storageModeLabel={storageModeLabel(setupStorageMode)}
          storagePath={setupStoragePath}
          storageHint={storageModeHint(setupStorageMode)}
          estimatedDiskUse={estimatedDiskUse}
          modelLabel={recommendedSetupModel?.label || activeModelTitle}
          modelFamily={recommendedSetupModel?.family || 'Supported OCR vision model'}
          modelFile={
            recommendedSetupModel?.file_name ||
            onboardingInfo?.recommended_model_file ||
            appDefaults?.recommended_model_file ||
            'Loading...'
          }
          mmprojFile={recommendedSetupModel?.mmproj_file}
          validationStatus={validationStatus.label}
          validationTone={validationStatus.tone}
          downloadStatus={setupDownloadText}
          helperMessage={setupHelper}
          progressPercent={downloadProgressPercent}
          showProgress={setupDownloadActive || downloadState.status === 'done'}
          canStart={!runtimeSetupIssue && !setupDownloadActive && downloadState.status !== 'done'}
          canRetry={!runtimeSetupIssue && downloadState.status === 'error'}
          isWorking={setupDownloadActive}
          onStart={startRecommendedSetup}
          onRetry={startRecommendedSetup}
          onCancel={closeSetupWizard}
          onOpenSettings={() => setSettingsOpen(true)}
        />
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









