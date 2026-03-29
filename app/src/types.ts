export type JobStatus =
  | 'Queued'
  | 'Rendering'
  | 'Ocr'
  | 'Formatting'
  | 'Writing'
  | 'Done'
  | 'Failed'
  | 'Canceled'

export type JobResult = {
  job_id: string
  source: string
  output_path?: string | null
  workflow_mode: WorkflowMode
  status: JobStatus
  error?: string | null
  progress?: number
  message?: string | null
}

export type JobPreviewPage = {
  page_number: number
  image_data_url: string
  text_chunk?: string | null
}

export type PreviewKind = 'Rendered' | 'Ocr'
export type RunnerMode = 'Persistent' | 'Transient'
export type RunnerStage =
  | 'WorkerStarting'
  | 'ModelReady'
  | 'FirstToken'
  | 'Chunk'
  | 'Error'

export type JobStreamState = {
  streamed_markdown: string
  preview_image_data_url?: string | null
  current_page?: number | null
  total_pages?: number | null
  source?: string | null
  pages?: JobPreviewPage[]
  runner_mode?: RunnerMode | null
  runner_stage?: RunnerStage | null
  runner_message?: string | null
  first_token_received?: boolean
  lazy_preview_thumbnails?: boolean
  disable_rich_preview_for_large_jobs?: boolean
  large_job_page_threshold?: number | null
}

export type AppEvent =
  | {
      type: 'Progress'
      data: {
      job_id: string
      status: JobStatus
      progress: number
      message?: string | null
      source?: string | null
      page_number?: number | null
      total_pages?: number | null
      rendered_pages?: number | null
      recognized_pages?: number | null
    }
  }
  | {
      type: 'Preview'
      data: {
        job_id: string
        source?: string | null
        kind: PreviewKind
        page_number: number
        total_pages: number
        image_data_url: string
        text_chunk?: string | null
      }
    }
  | {
      type: 'Runner'
      data: {
        job_id: string
        source?: string | null
        page_number?: number | null
        total_pages?: number | null
        mode: RunnerMode
        stage: RunnerStage
        message?: string | null
        chunk?: string | null
        will_fallback?: boolean | null
      }
    }
  | {
      type: 'Completed'
      data: { job_id: string; output_path: string }
    }
  | {
      type: 'Error'
      data: { job_id: string; message: string }
    }

export type ModelDownloadEvent = {
  repo: string
  file_name: string
  downloaded_bytes: number
  total_bytes?: number | null
  progress: number
  status: string
  message?: string | null
}

export type Settings = {
  threads: number
  dpi: number
  chunk_size: number
  auto_open: boolean
  idle_model_prewarm: boolean
  study_boost: boolean
  workflow_mode: WorkflowMode
  extract_template_id: string
  runtime_profile: RuntimeProfile
  max_ocr_dimension: number
  lazy_preview_thumbnails: boolean
  disable_rich_preview_for_large_jobs: boolean
  large_job_page_threshold: number
  theme?: string | null
  model_profile_id?: string | null
  model_file?: string | null
}

export type RunnerCompatibility = {
  transient_cli: boolean
  persistent_server: boolean
  notes: string
}

export type RuntimeProfile = 'auto' | 'cpu_compatible' | 'accelerated_if_available'
export type WorkflowMode = 'exact_ocr' | 'notes' | 'extract'

export type RuntimeStatus = {
  selected_profile: RuntimeProfile
  safe_default_profile: RuntimeProfile
  usable_runtime: boolean
  cpu_runtime_available: boolean
  accelerated_runtime_available: boolean
  accelerated_runtime_compatible: boolean
  accelerated_runtime_label?: string | null
  effective_runtime_label: string
  summary: string
}

export type ThemeOption = {
  id: string
  label: string
}

export type ThemeDefaults = {
  default_theme: string
  options: ThemeOption[]
}

export type RuntimeProfileOption = {
  id: RuntimeProfile
  label: string
  description: string
}

export type RuntimeProfileDefaults = {
  default_profile: RuntimeProfile
  options: RuntimeProfileOption[]
}

export type PromptDefaults = {
  default_prompt: string
  system_prompt: string
  placeholder: string
  hint: string
}

export type WorkflowModeExport = {
  id: 'markdown' | 'text' | 'json' | 'pdf' | 'csv'
  label: string
  extension: string
  description: string
  primary: boolean
}

export type WorkflowModeDefinition = {
  id: WorkflowMode
  label: string
  short_label: string
  description: string
  helper: string
  result_label: string
  empty_state_copy: string
  copy_action_label: string
  save_action_label: string
  advanced_panel_copy: string
  prompt_label: string
  prompt_hint: string
  prompt_placeholder: string
  default_prompt: string
  available_exports: WorkflowModeExport[]
}

export type ExtractTemplateDefinition = {
  id: string
  label: string
  description: string
  helper: string
  csv_hint: string
}

export type ExtractionPreset = {
  id: string
  label: string
  dpi: number
  description: string
  meta: string
  tradeoff: string
  runtime_profile_override?: RuntimeProfile | null
  max_ocr_dimension?: number | null
  lazy_preview_thumbnails: boolean
  disable_rich_preview_for_large_jobs: boolean
  large_job_page_threshold?: number | null
}

export type RunOptions = {
  workflow_mode?: WorkflowMode | null
  study_boost?: boolean | null
  extract_template_id?: string | null
  runtime_profile?: RuntimeProfile | null
  max_ocr_dimension?: number | null
  lazy_preview_thumbnails?: boolean | null
  disable_rich_preview_for_large_jobs?: boolean | null
  large_job_page_threshold?: number | null
}

export type AppDefaults = {
  settings: Settings
  theme: ThemeDefaults
  runtime_profiles: RuntimeProfileDefaults
  prompt: PromptDefaults
  workflow_modes: WorkflowModeDefinition[]
  extract_templates: ExtractTemplateDefinition[]
  extraction_presets: ExtractionPreset[]
  recommended_model_profile_id: string
  recommended_model_label: string
  recommended_model_file: string
  recommended_model_repo: string
}

export type ModelProfile = {
  id: string
  label: string
  family: string
  repo: string
  default_file: string
  requires_mmproj: boolean
  tested: boolean
  recommended: boolean
  notes: string
  runner_compatibility: RunnerCompatibility
  installed: boolean
  runtime_ready: boolean
}

export type LocalModelInfo = {
  file_name: string
  label: string
  family: string
  repo?: string | null
  profile_id?: string | null
  requires_mmproj: boolean
  runtime_ready: boolean
  tested: boolean
  recommended: boolean
  experimental: boolean
  notes?: string | null
  support_tier: 'recommended' | 'tested' | 'legacy' | 'experimental'
  source: 'registry' | 'custom' | 'heuristic' | 'legacy'
  auto_selectable: boolean
  runner_compatibility: RunnerCompatibility
}

export type ModelCatalog = {
  default_profile_id: string
  profiles: ModelProfile[]
  local_models: LocalModelInfo[]
}

export type StorageInfo = {
  mode: 'portable' | 'installer'
  root_path: string
  settings_path: string
  history_path: string
  models_path: string
  temp_path: string
  pasted_inputs_path: string
  outputs_description: string
}

export type OnboardingInfo = {
  storage_mode: 'portable' | 'installer'
  app_storage_path: string
  settings_storage_path: string
  history_storage_path: string
  model_storage_path: string
  temp_storage_path: string
  pasted_inputs_path: string
  output_description: string
  recommended_model_profile_id: string
  recommended_model_label: string
  recommended_model_file: string
  recommended_model_repo: string
}

export type RecommendedSetupInfo = {
  profile_id: string
  label: string
  family: string
  repo: string
  file_name: string
  mmproj_file?: string | null
  requires_mmproj: boolean
  estimated_download_bytes?: number | null
  primary_download_bytes?: number | null
  companion_download_bytes?: number | null
  validation: string
  notes: string
  availability_error?: string | null
}
