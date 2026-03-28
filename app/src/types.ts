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

export type RunnerCompatibility = {
  transient_cli: boolean
  persistent_server: boolean
  notes: string
}

export type RuntimeProfile = 'auto' | 'cpu_compatible' | 'accelerated_if_available'

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
