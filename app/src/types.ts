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

export type AppEvent =
  | {
      type: 'Progress'
      data: {
        job_id: string
        status: JobStatus
        progress: number
        message?: string | null
        source?: string | null
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
