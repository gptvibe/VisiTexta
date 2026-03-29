import { DropZone } from './DropZone'
import type {
  AppDefaults,
  ExtractTemplateDefinition,
  ExtractionPreset,
  WorkflowMode,
  WorkflowModeDefinition,
  WorkflowModeExport,
} from '../types'

type PresetKey = 'starter' | 'recommended' | 'quality' | 'faster'

type DownloadState = {
  status: 'idle' | 'starting' | 'downloading' | 'verifying' | 'done' | 'error'
  progress: number
  message?: string | null
  downloaded_bytes?: number
  total_bytes?: number | null
}

type ImportPanelProps = {
  modeDefinition: WorkflowModeDefinition
  modeOptions: WorkflowModeDefinition[]
  selectedMode: WorkflowMode
  onSelectMode: (mode: WorkflowMode) => void
  showSetupCard: boolean
  runtimeSetupIssue: boolean
  setupCardTitle: string
  setupCardBody: string
  downloadState: DownloadState
  downloadProgressPercent: number
  formatBytes: (value?: number | null) => string | null
  onOpenSetupWizard: () => void
  presetSummary: string
  presetTradeoff?: string | null
  presetOrder: PresetKey[]
  presetOptions: ExtractionPreset[]
  selectedPreset: PresetKey | null
  onSelectPreset: (preset: PresetKey, label: string) => void
  extractTemplates: ExtractTemplateDefinition[]
  selectedExtractTemplateId: string | null
  onSelectExtractTemplate: (templateId: string) => void
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
  studyBoost: boolean
  onToggleStudyBoost: (value: boolean) => void
  activeModelTitle: string
  activeModelSupportLabel: string
  runtimeLabel: string
  effectiveRuntimeLabel: string
  modelStorageLabel: string
  onOpenSettings: () => void
  onExportResult: (exportId?: WorkflowModeExport['id']) => void | Promise<void>
  canExportResult: boolean
}

export function ImportPanel({
  modeDefinition,
  modeOptions,
  selectedMode,
  onSelectMode,
  showSetupCard,
  runtimeSetupIssue,
  setupCardTitle,
  setupCardBody,
  downloadState,
  downloadProgressPercent,
  formatBytes,
  onOpenSetupWizard,
  presetSummary,
  presetTradeoff,
  presetOrder,
  presetOptions,
  selectedPreset,
  onSelectPreset,
  extractTemplates,
  selectedExtractTemplateId,
  onSelectExtractTemplate,
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
  studyBoost,
  onToggleStudyBoost,
  activeModelTitle,
  activeModelSupportLabel,
  runtimeLabel,
  effectiveRuntimeLabel,
  modelStorageLabel,
  onOpenSettings,
  onExportResult,
  canExportResult,
}: ImportPanelProps) {
  return (
    <section className="panel command-panel">
      <div className="panel-title">{modeDefinition.label}</div>
      <div className="command-copy">
        {modeDefinition.helper} Pick the speed and quality you want, then drop files, paste an
        image, or browse for files from your computer.
      </div>

      <section className="preset-section" aria-label="Workflow modes">
        <div className="preset-header">
          <div className="section-title">Modes</div>
          <div className="preset-note">{modeDefinition.description}</div>
        </div>
        <div className="preset-grid">
          {modeOptions.map((mode) => {
            const selected = selectedMode === mode.id
            return (
              <button
                key={mode.id}
                type="button"
                className={`preset-card ${selected ? 'selected' : ''}`}
                aria-pressed={selected}
                onClick={() => onSelectMode(mode.id)}
              >
                <span className="preset-name">{mode.label}</span>
                <span className="preset-copy">{mode.description}</span>
                <span className="preset-meta">
                  {mode.available_exports.map((exportOption) => exportOption.label).join(' + ')}
                </span>
              </button>
            )
          })}
        </div>
      </section>

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
          <div className="advanced-actions">
            <button className="btn primary" onClick={onOpenSetupWizard}>
              Open setup
            </button>
          </div>
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
        {selectedPreset !== null && presetTradeoff && (
          <div className="preset-tradeoff-note">{presetTradeoff}</div>
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
          <div className="advanced-copy">{modeDefinition.advanced_panel_copy}</div>
          <div className="prompt-block">
            <label className="prompt-label">
              {modeDefinition.prompt_label}
              <span className="prompt-hint">
                {modeDefinition.prompt_hint ||
                  appDefaults?.prompt.hint ||
                  'Optional. Leave blank for the standard OCR prompt.'}
              </span>
            </label>
            <textarea
              className="prompt-input"
              placeholder={modeDefinition.prompt_placeholder || appDefaults?.prompt.placeholder || ''}
              value={prompt}
              onChange={(event) => onPromptChange(event.target.value)}
              rows={4}
            />
          </div>
          {selectedMode === 'notes' && (
            <label className="field checkbox">
              <input
                type="checkbox"
                checked={studyBoost}
                onChange={(event) => onToggleStudyBoost(event.target.checked)}
              />
              <span>Study boost</span>
              <span className="prompt-hint">
                Slower second pass that adds extra glossary cues, review prompts, and memory checks.
              </span>
            </label>
          )}
          {selectedMode === 'extract' && extractTemplates.length > 0 && (
            <section className="preset-section" aria-label="Extract templates">
              <div className="preset-header">
                <div className="section-title">Extract templates</div>
                <div className="preset-note">
                  Choose the output shape that best matches the document you are reviewing.
                </div>
              </div>
              <div className="preset-grid">
                {extractTemplates.map((template) => {
                  const selected = selectedExtractTemplateId === template.id
                  return (
                    <button
                      key={template.id}
                      type="button"
                      className={`preset-card ${selected ? 'selected' : ''}`}
                      aria-pressed={selected}
                      onClick={() => onSelectExtractTemplate(template.id)}
                    >
                      <span className="preset-name">{template.label}</span>
                      <span className="preset-copy">{template.description}</span>
                      <span className="preset-meta">{template.csv_hint}</span>
                    </button>
                  )
                })}
              </div>
            </section>
          )}
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
            {modeDefinition.available_exports.map((exportOption) => (
              <button
                key={exportOption.id}
                className={`btn ${exportOption.primary ? 'primary' : 'ghost'}`}
                onClick={() => onExportResult(exportOption.id)}
                disabled={!canExportResult}
                title={exportOption.description}
              >
                {exportOption.label}
              </button>
            ))}
          </div>
          {selectedMode === 'notes' && (
            <div className="field-note">
              PDF export stays text-based so the notes remain searchable. Source page jumps stay interactive in the app preview, and exported files keep the page references as readable text.
            </div>
          )}
          {selectedMode === 'extract' && (
            <div className="field-note">
              Extract templates always produce readable markdown plus structured JSON. CSV exports use detected rows when possible and fall back to a review-friendly field/value sheet.
            </div>
          )}
        </section>
      )}
    </section>
  )
}
