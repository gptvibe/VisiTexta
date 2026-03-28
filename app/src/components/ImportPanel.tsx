import { DropZone } from './DropZone'
import { FirstRunWizard } from './FirstRunWizard'
import type { AppDefaults, ExtractionPreset } from '../types'

type PresetKey = 'recommended' | 'quality' | 'faster'

type DownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'verifying' | 'done' | 'error'
  progress: number
  message?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

type ImportPanelProps = {
  onboardingOpen: boolean
  onboardingStep: number
  onboardingSteps: Array<{
    title: string
    body: string
    detail: string
  }>
  onDismissOnboarding: () => void
  onBackOnboarding: () => void
  onNextOnboarding: () => void
  showSetupCard: boolean
  runtimeSetupIssue: boolean
  setupCardTitle: string
  setupCardBody: string
  downloadState: DownloadState
  downloadProgressPercent: number
  formatBytes: (value?: number | null) => string | null
  presetSummary: string
  presetOrder: PresetKey[]
  presetOptions: ExtractionPreset[]
  selectedPreset: PresetKey | null
  onSelectPreset: (preset: PresetKey, label: string) => void
  busy: boolean
  modelMissing: boolean
  onBrowseFiles: () => void | Promise<void>
  onPasteImage: () => void | Promise<void>
  onFiles: (paths: string[]) => void | Promise<void>
  advancedOpen: boolean
  onToggleAdvanced: () => void
  appDefaults: AppDefaults | null
  prompt: string
  onPromptChange: (value: string) => void
  activeModelTitle: string
  activeModelSupportLabel: string
  runtimeLabel: string
  effectiveRuntimeLabel: string
  modelStorageLabel: string
  onOpenSettings: () => void
  onSaveMarkdown: () => void | Promise<void>
  canSaveMarkdown: boolean
}

export function ImportPanel({
  onboardingOpen,
  onboardingStep,
  onboardingSteps,
  onDismissOnboarding,
  onBackOnboarding,
  onNextOnboarding,
  showSetupCard,
  runtimeSetupIssue,
  setupCardTitle,
  setupCardBody,
  downloadState,
  downloadProgressPercent,
  formatBytes,
  presetSummary,
  presetOrder,
  presetOptions,
  selectedPreset,
  onSelectPreset,
  busy,
  modelMissing,
  onBrowseFiles,
  onPasteImage,
  onFiles,
  advancedOpen,
  onToggleAdvanced,
  appDefaults,
  prompt,
  onPromptChange,
  activeModelTitle,
  activeModelSupportLabel,
  runtimeLabel,
  effectiveRuntimeLabel,
  modelStorageLabel,
  onOpenSettings,
  onSaveMarkdown,
  canSaveMarkdown,
}: ImportPanelProps) {
  return (
    <section className="panel command-panel">
      <div className="panel-title">Start extraction</div>
      <div className="command-copy">
        Pick the speed and quality you want, then drop files, paste an image, or browse
        for files from your computer.
      </div>

      <FirstRunWizard
        open={onboardingOpen}
        step={onboardingStep}
        steps={onboardingSteps}
        onBack={onBackOnboarding}
        onNext={onNextOnboarding}
        onSkip={onDismissOnboarding}
      />

      {showSetupCard && (
        <section className="setup-card" aria-live="polite">
          <div className="setup-card-header">
            <div>
              <div className="section-title">Setup status</div>
              <div className="setup-card-title">{setupCardTitle}</div>
            </div>
            <div className="setup-card-badge">
              {runtimeSetupIssue
                ? 'Runtime'
                : downloadState.status === 'error'
                  ? 'Paused'
                  : `${downloadProgressPercent}%`}
            </div>
          </div>
          <div className="setup-card-copy">{setupCardBody}</div>
          {!runtimeSetupIssue && downloadState.status !== 'error' && (
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
          {presetOrder.map((key) => {
            const preset = presetOptions.find((option) => option.id === key)
            if (!preset) return null

            const selected = selectedPreset === key
            return (
              <button
                key={key}
                type="button"
                className={`preset-card ${selected ? 'selected' : ''}`}
                aria-pressed={selected}
                onClick={() => onSelectPreset(key, preset.label)}
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
        onFiles={onFiles}
      />

      <div className="advanced-toggle">
        <button
          className="btn ghost"
          aria-expanded={advancedOpen}
          aria-controls="advanced-panel"
          onClick={onToggleAdvanced}
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
              <span className="prompt-hint">
                {appDefaults?.prompt.hint || 'Optional. Leave blank for the standard OCR prompt.'}
              </span>
            </label>
            <textarea
              className="prompt-input"
              placeholder={appDefaults?.prompt.placeholder || ''}
              value={prompt}
              onChange={(event) => onPromptChange(event.target.value)}
              rows={4}
            />
          </div>
          <div className="signal-grid advanced-grid">
            <div className="signal-card">
              <span>Active model</span>
              <strong>{activeModelTitle}</strong>
              <span>{activeModelSupportLabel}</span>
            </div>
            <div className="signal-card">
              <span>Runtime</span>
              <strong>{runtimeLabel}</strong>
              <span>{effectiveRuntimeLabel}</span>
            </div>
            <div className="signal-card wide">
              <span>Model storage</span>
              <strong>{modelStorageLabel}</strong>
            </div>
          </div>
          <div className="advanced-actions">
            <button className="btn ghost" onClick={onOpenSettings}>
              Advanced settings
            </button>
            <button className="btn ghost" onClick={onSaveMarkdown} disabled={!canSaveMarkdown}>
              Save a copy
            </button>
          </div>
        </section>
      )}
    </section>
  )
}
