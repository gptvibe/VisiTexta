import { useEffect, useMemo, useState } from 'react'
import type {
  AppDefaults,
  LocalModelInfo,
  ModelCatalog,
  ModelProfile,
  RuntimeProfile,
  RuntimeStatus,
  Settings,
  StorageInfo,
} from '../types'

type ModelDownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'verifying' | 'done' | 'error'
  progress: number
  message?: string | null
  file_name?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

type SettingsDrawerProps = {
  open: boolean
  settings: Settings
  appDefaults: AppDefaults | null
  modelCatalog: ModelCatalog | null
  runtimeStatus?: RuntimeStatus | null
  storageInfo?: StorageInfo | null
  modelInput: string
  modelStoragePath?: string | null
  downloadState: ModelDownloadState
  onModelInputChange: (value: string) => void
  onDownloadModel: (value?: string | null) => void
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

function supportLabel(input: Pick<LocalModelInfo, 'support_tier'>) {
  switch (input.support_tier) {
    case 'recommended':
      return 'Recommended'
    case 'tested':
      return 'Tested'
    case 'legacy':
      return 'Legacy'
    default:
      return 'Experimental'
  }
}

function formatRunnerCompatibility(
  compatibility: ModelProfile['runner_compatibility'] | LocalModelInfo['runner_compatibility']
) {
  const labels = []
  if (compatibility.transient_cli) labels.push('CLI')
  if (compatibility.persistent_server) labels.push('Warm server')
  return labels.join(' + ')
}

function describeLocalOption(model: LocalModelInfo) {
  return `${supportLabel(model)}: ${model.label}`
}

function describeRuntimeProfile(
  options: AppDefaults['runtime_profiles']['options'],
  profile: RuntimeProfile
) {
  return options.find((option) => option.id === profile)?.description ?? ''
}

function renderProfileCard(
  profile: ModelProfile,
  installedModel: LocalModelInfo | null,
  isDownloading: boolean,
  onDownloadModel: (value?: string | null) => void
) {
  return (
    <article
      key={profile.id}
      className={`model-profile-card ${profile.recommended ? 'recommended' : ''}`}
    >
      <div className="model-profile-header">
        <div>
          <div className="model-badge-row">
            <span className={`model-badge ${profile.recommended ? 'recommended' : 'tested'}`}>
              {profile.recommended ? 'Recommended' : 'Tested'}
            </span>
            {profile.requires_mmproj && <span className="model-badge subtle">mmproj</span>}
            {installedModel && (
              <span className={`model-badge ${installedModel.runtime_ready ? 'ready' : 'warning'}`}>
                {installedModel.runtime_ready ? 'Installed' : 'Needs mmproj'}
              </span>
            )}
          </div>
          <div className="model-profile-title">{profile.label}</div>
          <div className="model-profile-subtitle">{profile.family}</div>
        </div>
        <button
          className="btn ghost"
          onClick={() => onDownloadModel(profile.id)}
          disabled={isDownloading}
        >
          {isDownloading ? 'Downloading...' : installedModel ? 'Use / re-download' : 'Download'}
        </button>
      </div>
      <div className="model-profile-meta">
        <span>Default file</span>
        <strong>{profile.default_file}</strong>
      </div>
      <div className="model-profile-meta">
        <span>Repo</span>
        <strong>{profile.repo}</strong>
      </div>
      <div className="model-profile-meta">
        <span>Runner</span>
        <strong>{formatRunnerCompatibility(profile.runner_compatibility)}</strong>
      </div>
      <div className="field-note">{profile.notes}</div>
    </article>
  )
}

export function SettingsDrawer({
  open,
  settings,
  appDefaults,
  modelCatalog,
  runtimeStatus,
  storageInfo,
  modelInput,
  modelStoragePath,
  downloadState,
  onModelInputChange,
  onDownloadModel,
  onRefreshModels,
  onClose,
  onSave,
}: SettingsDrawerProps) {
  const [draft, setDraft] = useState(settings)
  const runtimeProfileOptions = appDefaults?.runtime_profiles.options ?? []
  const themeOptions = appDefaults?.theme.options ?? []

  useEffect(() => {
    setDraft(settings)
  }, [settings])

  const defaultProfile = useMemo(() => {
    if (!modelCatalog) return null
    return (
      modelCatalog.profiles.find((profile) => profile.id === modelCatalog.default_profile_id) ??
      null
    )
  }, [modelCatalog])

  const selectedProfile = useMemo(() => {
    if (!modelCatalog || !draft.model_profile_id) return null
    return (
      modelCatalog.profiles.find((profile) => profile.id === draft.model_profile_id) ?? null
    )
  }, [draft.model_profile_id, modelCatalog])

  const supportedProfiles = modelCatalog?.profiles ?? []
  const localModels: LocalModelInfo[] = modelCatalog?.local_models ?? []
  const supportedLocalModels = localModels.filter(
    (model) => model.support_tier === 'recommended' || model.support_tier === 'tested'
  )
  const experimentalLocalModels = localModels.filter(
    (model) => model.support_tier === 'experimental' || model.support_tier === 'legacy'
  )

  const selectedLocalModel = useMemo(() => {
    if (!draft.model_file) return null
    return localModels.find((model) => model.file_name === draft.model_file) ?? null
  }, [draft.model_file, localModels])

  const selectedProfileInstall = useMemo(() => {
    if (!selectedProfile) return null
    return localModels.find((model) => model.profile_id === selectedProfile.id) ?? null
  }, [localModels, selectedProfile])

  const missingModel = useMemo(() => {
    if (!draft.model_file) return null
    return selectedLocalModel ? null : draft.model_file
  }, [draft.model_file, selectedLocalModel])

  const selectedSupportLabel = selectedLocalModel
    ? supportLabel(selectedLocalModel)
    : selectedProfile
      ? selectedProfile.recommended
        ? 'Recommended'
        : 'Tested'
    : defaultProfile
      ? defaultProfile.recommended
        ? 'Recommended'
        : 'Tested'
      : 'Recommended'

  const percent = Math.min(100, Math.max(0, Math.round(downloadState.progress * 100)))
  const downloadedLabel = formatBytes(downloadState.downloaded_bytes)
  const totalLabel = formatBytes(downloadState.total_bytes)
  const isDownloading =
    downloadState.status === 'starting' ||
    downloadState.status === 'downloading' ||
    downloadState.status === 'verifying'

  let progressText = ''
  if (downloadState.status === 'error') {
    progressText = downloadState.message || 'Download failed.'
  } else if (downloadState.status === 'done') {
    progressText = downloadState.message || 'Download complete.'
  } else if (downloadState.status === 'verifying') {
    progressText = downloadState.message || 'Verifying download...'
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
      <div className="drawer-panel model-drawer-panel">
        <div className="drawer-header">
          <div className="panel-title">Advanced settings</div>
          <button className="btn ghost" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="drawer-body">
          <label className="field">
            <span>CPU threads</span>
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
            <span>Render DPI</span>
            <input
              type="number"
              min={150}
              max={600}
              value={draft.dpi}
              onChange={(event) => setDraft({ ...draft, dpi: Number(event.target.value) })}
            />
          </label>
          <label className="field">
            <span>Max OCR image size</span>
            <input
              type="number"
              min={800}
              max={2400}
              step={100}
              value={draft.max_ocr_dimension}
              onChange={(event) =>
                setDraft({ ...draft, max_ocr_dimension: Number(event.target.value) })
              }
            />
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.lazy_preview_thumbnails}
              onChange={(event) =>
                setDraft({ ...draft, lazy_preview_thumbnails: event.target.checked })
              }
            />
            <span>Generate preview thumbnails lazily</span>
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.disable_rich_preview_for_large_jobs}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  disable_rich_preview_for_large_jobs: event.target.checked,
                })
              }
            />
            <span>Disable rich preview for large jobs</span>
          </label>
          <label className="field">
            <span>Large-job page threshold</span>
            <input
              type="number"
              min={2}
              max={200}
              value={draft.large_job_page_threshold}
              onChange={(event) =>
                setDraft({
                  ...draft,
                  large_job_page_threshold: Number(event.target.value),
                })
              }
            />
          </label>
          <div className="field-note">
            Lower OCR image size and large-job preview limits reduce memory pressure, but page previews and tiny text detail can be less rich.
          </div>
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
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.idle_model_prewarm}
              onChange={(event) =>
                setDraft({ ...draft, idle_model_prewarm: event.target.checked })
              }
            />
            <span>Prewarm the local OCR engine after model changes</span>
          </label>
          <div className="field-note">
            When enabled, VisiTexta quietly starts the warm OCR worker a few seconds after you
            install or switch models so the next extraction reaches first text sooner.
          </div>
          <label className="field">
            <span>Theme</span>
            <select
              value={draft.theme || appDefaults?.theme.default_theme || 'system'}
              onChange={(event) =>
                setDraft({ ...draft, theme: event.target.value || null })
              }
            >
              {themeOptions.map((option) => (
                <option key={option.id} value={option.id}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>

          <div className="drawer-section">
            <div className="section-title">Runtime profile</div>
            <label className="field">
              <span>Local runtime</span>
              <select
                value={draft.runtime_profile}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    runtime_profile: event.target.value as RuntimeProfile,
                  })
                }
              >
                {runtimeProfileOptions.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <div className="field-note">
              {describeRuntimeProfile(runtimeProfileOptions, draft.runtime_profile)}
            </div>
            <div className="field-note">
              Acceleration changes speed only. OCR semantics stay tied to the same model,
              prompt, and preprocessing path.
            </div>
            {runtimeStatus && (
              <div className="model-selection-card">
                <div className="model-badge-row">
                  <span className={`model-badge ${runtimeStatus.usable_runtime ? 'ready' : 'warning'}`}>
                    {runtimeStatus.usable_runtime ? 'Usable' : 'Needs runtime'}
                  </span>
                  <span
                    className={`model-badge ${
                      runtimeStatus.cpu_runtime_available ? 'tested' : 'warning'
                    }`}
                  >
                    {runtimeStatus.cpu_runtime_available ? 'CPU bundled' : 'CPU missing'}
                  </span>
                  {runtimeStatus.accelerated_runtime_available && (
                    <span
                      className={`model-badge ${
                        runtimeStatus.accelerated_runtime_compatible ? 'ready' : 'warning'
                      }`}
                    >
                      {runtimeStatus.accelerated_runtime_compatible
                        ? runtimeStatus.accelerated_runtime_label || 'Acceleration detected'
                        : runtimeStatus.accelerated_runtime_label || 'Acceleration bundled'}
                    </span>
                  )}
                </div>
                <div className="model-profile-title">{runtimeStatus.effective_runtime_label}</div>
                <div className="field-note">{runtimeStatus.summary}</div>
              </div>
            )}
          </div>

          {storageInfo && (
            <div className="drawer-section">
              <div className="section-title">Windows storage</div>
              <div className="field-note">
                {storageInfo.mode === 'portable'
                  ? 'Portable mode keeps VisiTexta data beside the executable.'
                  : 'Installer mode keeps VisiTexta data under %LOCALAPPDATA%.'}
              </div>
              <div className="model-profile-meta">
                <span>Settings</span>
                <strong>{storageInfo.settings_path}</strong>
              </div>
              <div className="model-profile-meta">
                <span>History</span>
                <strong>{storageInfo.history_path}</strong>
              </div>
              <div className="model-profile-meta">
                <span>Models</span>
                <strong>{storageInfo.models_path}</strong>
              </div>
              <div className="model-profile-meta">
                <span>Temp work files</span>
                <strong>{storageInfo.temp_path}</strong>
              </div>
              <div className="field-note">{storageInfo.outputs_description}</div>
            </div>
          )}

          <div className="drawer-section">
            <div className="section-title">Supported profiles</div>
            <div className="field-note">
              The default path stays GLM-first. Other listed profiles are explicitly curated and
              tested with VisiTexta&apos;s bundled llama.cpp multimodal runners.
            </div>
            <div className="model-profile-grid">
              {supportedProfiles.map((profile) =>
                renderProfileCard(
                  profile,
                  localModels.find((model) => model.profile_id === profile.id) ?? null,
                  isDownloading,
                  onDownloadModel
                )
              )}
            </div>
          </div>

          <div className="drawer-section">
            <div className="section-title">Preferred installed model</div>
            <label className="field">
              <span>Active local model</span>
              <select
                value={draft.model_file || ''}
                onChange={(event) => {
                  const nextFile = event.target.value || null
                  const nextModel =
                    localModels.find((model) => model.file_name === nextFile) || null
                  setDraft({
                    ...draft,
                    model_file: nextFile,
                    model_profile_id: nextModel?.profile_id || null,
                  })
                }}
              >
                <option value="">
                  {`Auto (recommended: ${defaultProfile?.label || 'GLM-OCR'})`}
                </option>
                {missingModel && (
                  <option value={missingModel}>{`Missing: ${missingModel}`}</option>
                )}
                {supportedLocalModels.length > 0 && (
                  <optgroup label="Installed supported models">
                    {supportedLocalModels.map((model) => (
                      <option key={model.file_name} value={model.file_name}>
                        {describeLocalOption(model)}
                      </option>
                    ))}
                  </optgroup>
                )}
                {experimentalLocalModels.length > 0 && (
                  <optgroup label="Installed experimental / legacy models">
                    {experimentalLocalModels.map((model) => (
                      <option key={model.file_name} value={model.file_name}>
                        {describeLocalOption(model)}
                      </option>
                    ))}
                  </optgroup>
                )}
              </select>
            </label>
            <div className="field-note">
              Auto mode prefers {defaultProfile?.label || 'the recommended GLM-OCR profile'} and
              then falls back to other installed compatible local models.
            </div>
            <div className="model-selection-card">
              <div className="model-badge-row">
                <span
                  className={`model-badge ${
                    selectedSupportLabel === 'Recommended'
                      ? 'recommended'
                      : selectedSupportLabel === 'Tested'
                        ? 'tested'
                        : 'warning'
                  }`}
                >
                  {selectedSupportLabel}
                </span>
                {selectedLocalModel?.requires_mmproj && (
                  <span className="model-badge subtle">mmproj</span>
                )}
                {selectedLocalModel && (
                  <span
                    className={`model-badge ${
                      selectedLocalModel.runtime_ready ? 'ready' : 'warning'
                    }`}
                  >
                    {selectedLocalModel.runtime_ready ? 'Ready' : 'Needs mmproj'}
                  </span>
                )}
              </div>
              <div className="model-profile-title">
                {selectedLocalModel?.label ||
                  selectedProfile?.label ||
                  defaultProfile?.label ||
                  'Recommended default'}
              </div>
              <div className="model-profile-subtitle">
                {selectedLocalModel?.family ||
                  selectedProfile?.family ||
                  defaultProfile?.family ||
                  'Supported OCR vision model'}
              </div>
              <div className="model-profile-meta">
                <span>File</span>
                <strong>
                  {selectedLocalModel?.file_name ||
                    selectedProfile?.default_file ||
                    defaultProfile?.default_file ||
                    'Auto-selects the recommended profile'}
                </strong>
              </div>
              <div className="model-profile-meta">
                <span>Runner</span>
                <strong>
                  {formatRunnerCompatibility(
                    selectedLocalModel?.runner_compatibility ||
                      selectedProfile?.runner_compatibility ||
                      defaultProfile?.runner_compatibility || {
                        transient_cli: true,
                        persistent_server: true,
                        notes: '',
                      }
                  )}
                </strong>
              </div>
              {(selectedLocalModel?.repo || selectedProfile?.repo || defaultProfile?.repo) && (
                <div className="model-profile-meta">
                  <span>Repo</span>
                  <strong>
                    {selectedLocalModel?.repo || selectedProfile?.repo || defaultProfile?.repo}
                  </strong>
                </div>
              )}
              <div className="field-note">
                {selectedLocalModel?.notes ||
                  selectedProfile?.notes ||
                  defaultProfile?.notes ||
                  'Use the recommended profile unless you have a specific reason to change it.'}
              </div>
            </div>
            {!draft.model_file &&
              selectedProfile &&
              !selectedProfileInstall && (
                <div className="field-note field-note-warning">
                  {selectedProfile.label} is selected, but it is not downloaded yet.
                </div>
              )}
            {!draft.model_file &&
              selectedProfile &&
              selectedProfileInstall &&
              !selectedProfileInstall.runtime_ready && (
                <div className="field-note field-note-warning">
                  {selectedProfile.label} is installed, but its mmproj companion is missing.
                </div>
              )}
            {missingModel && (
              <div className="field-note field-note-warning">
                The saved model selection is not installed right now: {missingModel}
              </div>
            )}
          </div>

          <div className="drawer-section">
            <div className="section-title">Experimental custom download</div>
            <div className="field">
              <span>Unlisted model path</span>
              <div className="model-row">
                <input
                  type="text"
                  placeholder="owner/repo/file.gguf"
                  value={modelInput}
                  onChange={(event) => onModelInputChange(event.target.value)}
                />
                <button
                  className="btn ghost"
                  onClick={() => onDownloadModel()}
                  disabled={isDownloading || !modelInput.trim()}
                >
                  {isDownloading ? 'Downloading...' : 'Download'}
                </button>
              </div>
              <div className="field-note">
                Use this only for power-user experiments. For unlisted models, enter a full
                `owner/repo/file.gguf` path or Hugging Face file URL. Repo-only auto-selection is
                intentionally limited to the curated supported profiles above.
              </div>
              {modelStoragePath && (
                <div className="field-note">Stored in: {modelStoragePath}</div>
              )}
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
              Refresh local catalog
            </button>
          </div>
        </div>
        <div className="drawer-footer">
          <button className="btn ghost" onClick={() => setDraft(settings)}>
            Reset
          </button>
          <button className="btn primary" onClick={() => onSave(draft)}>
            Save
          </button>
        </div>
      </div>
    </div>
  )
}
