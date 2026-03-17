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

export type JobStreamState = {
  streamed_markdown: string
  preview_image_data_url?: string | null
  current_page?: number | null
  total_pages?: number | null
  source?: string | null
  pages?: JobPreviewPage[]
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
      type: 'Preview'
      data: {
        job_id: string
        source?: string | null
        page_number: number
        total_pages: number
        image_data_url: string
        text_chunk?: string | null
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
