import { useEffect, useMemo, useState } from 'react'

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

type SettingsDrawerProps = {
  open: boolean
  settings: Settings
  models: string[]
  modelInput: string
  downloadState: ModelDownloadState
  onModelInputChange: (value: string) => void
  onDownloadModel: () => void
  onRefreshModels: () => void
  onClose: () => void
  onSave: (settings: Settings) => void
}

function formatBytes(value?: number | null) {
  if (value === undefined || value === null) return null
  const mb = value / (1024 * 1024)
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  const gb = mb / 1024
  return `${gb.toFixed(2)} GB`
}

export function SettingsDrawer({
  open,
  settings,
  models,
  modelInput,
  downloadState,
  onModelInputChange,
  onDownloadModel,
  onRefreshModels,
  onClose,
  onSave,
}: SettingsDrawerProps) {
  const [draft, setDraft] = useState(settings)

  useEffect(() => {
    setDraft(settings)
  }, [settings])

  const missingModel = useMemo(() => {
    if (!draft.model_file) return null
    return models.includes(draft.model_file) ? null : draft.model_file
  }, [draft.model_file, models])

  const percent = Math.min(100, Math.max(0, Math.round(downloadState.progress * 100)))
  const downloadedLabel = formatBytes(downloadState.downloaded_bytes)
  const totalLabel = formatBytes(downloadState.total_bytes)
  const isDownloading = downloadState.status === 'starting' || downloadState.status === 'downloading'

  let progressText = ''
  if (downloadState.status === 'error') {
    progressText = downloadState.message || 'Download failed.'
  } else if (downloadState.status === 'done') {
    progressText = 'Download complete.'
  } else if (downloadState.status !== 'idle') {
    if (downloadedLabel && totalLabel) {
      progressText = `${percent}% (${downloadedLabel} / ${totalLabel})`
    } else if (downloadedLabel) {
      progressText = `${downloadedLabel} downloaded`
    } else {
      progressText = `${percent}%`
    }
  }

  return (
    <div className={`drawer ${open ? 'open' : ''}`}>
      <div className="drawer-overlay" onClick={onClose} />
      <div className="drawer-panel">
        <div className="drawer-header">
          <div className="panel-title">Settings</div>
          <button className="btn ghost" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="drawer-body">
          <label className="field">
            <span>Threads</span>
            <input
              type="number"
              min={1}
              value={draft.threads}
              onChange={(event) =>
                setDraft({ ...draft, threads: Number(event.target.value) })
              }
            />
          </label>
          <label className="field">
            <span>DPI</span>
            <input
              type="number"
              min={150}
              max={600}
              value={draft.dpi}
              onChange={(event) =>
                setDraft({ ...draft, dpi: Number(event.target.value) })
              }
            />
          </label>
          <label className="field">
            <span>Chunk size</span>
            <input
              type="number"
              min={500}
              max={8000}
              value={draft.chunk_size}
              onChange={(event) =>
                setDraft({ ...draft, chunk_size: Number(event.target.value) })
              }
            />
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.auto_open}
              onChange={(event) =>
                setDraft({ ...draft, auto_open: event.target.checked })
              }
            />
            <span>Auto-open output folder</span>
          </label>

          <div className="drawer-section">
            <div className="section-title">Models</div>
            <label className="field">
              <span>Active model</span>
              <select
                value={draft.model_file || ''}
                onChange={(event) =>
                  setDraft({ ...draft, model_file: event.target.value || null })
                }
              >
                <option value="">Auto (download unsloth/Qwen3.5-0.8B-GGUF)</option>
                {missingModel && (
                  <option value={missingModel}>{`Missing: ${missingModel}`}</option>
                )}
                {models.map((model) => (
                  <option key={model} value={model}>
                    {model}
                  </option>
                ))}
              </select>
            </label>
            <div className="field">
              <span>Download from Hugging Face</span>
              <div className="model-row">
                <input
                  type="text"
                  placeholder="unsloth/Qwen3.5-0.8B-GGUF"
                  value={modelInput}
                  onChange={(event) => onModelInputChange(event.target.value)}
                />
                <button
                  className="btn ghost"
                  onClick={onDownloadModel}
                  disabled={isDownloading || !modelInput.trim()}
                >
                  {isDownloading ? 'Downloading...' : 'Download'}
                </button>
              </div>
              {downloadState.status !== 'idle' && (
                <div className="model-progress">
                  <div className="model-progress-bar">
                    <div
                      className="model-progress-fill"
                      style={{ width: `${percent}%` }}
                    />
                  </div>
                  <div className="model-progress-text">{progressText}</div>
                </div>
              )}
            </div>
            <button className="btn ghost" onClick={onRefreshModels}>
              Refresh models
            </button>
          </div>
        </div>
        <div className="drawer-footer">
          <button className="btn ghost" onClick={() => setDraft(settings)}>
            Reset
          </button>
          <button className="btn primary" onClick={() => onSave(draft)}>
            Save settings
          </button>
        </div>
      </div>
    </div>
  )
}
