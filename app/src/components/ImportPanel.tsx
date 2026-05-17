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

function compactExportLabel(exportOption: WorkflowModeExport) {
  switch (exportOption.id) {
    case 'markdown':
      return 'Markdown'
    case 'text':
      return 'Text'
    case 'json':
      return 'JSON'
    case 'pdf':
      return 'PDF'
    case 'csv':
      return exportOption.label.toLowerCase().includes('anki') ? 'Anki CSV' : 'CSV'
    default:
      return exportOption.label
  }
}

function templateOutputChips(template: ExtractTemplateDefinition) {
  switch (template.id) {
    case 'invoice_receipt':
      return ['Markdown', 'JSON', 'CSV rows']
    case 'table_to_csv':
      return ['Markdown', 'JSON table', 'CSV']
    case 'meeting_whiteboard':
      return ['Markdown', 'JSON', 'CSV actions']
    case 'contract_key_points':
      return ['Markdown', 'JSON', 'CSV fields']
    default:
      return ['Markdown', 'JSON']
  }
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
        {modeDefinition.helper} Choose a preset, then add a file to begin.
      </div>

      <section className="import-section quickstart-section" aria-label="Run OCR">
        <div className="import-section-header">
          <div className="section-title">Run OCR</div>
          <div className="import-section-copy">
            Start here. Add a file first, then fine-tune workflow and preset details below when
            you need them.
          </div>
        </div>
        <div className="signal-grid quickstart-signals">
          <div className="signal-card">
            <span>Workflow</span>
            <strong>{modeDefinition.label}</strong>
            <span>{presetSummary}</span>
          </div>
          <div className="signal-card">
            <span>Model</span>
            <strong>{activeModelTitle}</strong>
            <span>
              {modelMissing ? 'Recommended setup will finish before extraction starts.' : activeModelSupportLabel}
            </span>
          </div>
          <div className="signal-card">
            <span>Runtime</span>
            <strong>{runtimeLabel}</strong>
            <span>{effectiveRuntimeLabel}</span>
          </div>
        </div>

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
              <button className="btn primary" type="button" onClick={onOpenSetupWizard}>
                Open setup
              </button>
            </div>
          </section>
        )}

        <DropZone
          disabled={busy || modelMissing}
          onBrowse={onBrowseFiles}
          onPasteImage={onPasteImage}
          onFiles={onFiles}
        />
      </section>

      <section className="import-section workflow-section" aria-label="Workflow modes">
        <div className="import-section-header">
          <div className="section-title">Workflow modes</div>
          <div className="import-section-copy">
            Choose how VisiTexta should shape the result before you start.
          </div>
        </div>
        <div className="workflow-grid">
          {modeOptions.map((mode) => {
            const selected = selectedMode === mode.id
            return (
              <button
                key={mode.id}
                type="button"
                className={`import-card workflow-card ${selected ? 'selected' : ''}`}
                aria-pressed={selected}
                onClick={() => onSelectMode(mode.id)}
              >
                <div className="workflow-card-body">
                  <span className="workflow-card-title">{mode.label}</span>
                  <span className="workflow-card-description">{mode.description}</span>
                </div>
                <div className="workflow-chip-list">
                  {mode.available_exports.map((exportOption) => (
                    <span key={exportOption.id} className="import-chip workflow-chip">
                      {compactExportLabel(exportOption)}
                    </span>
                  ))}
                </div>
              </button>
            )
          })}
        </div>
      </section>

      <section className="import-section speed-section" aria-label="Extraction presets">
        <div className="import-section-header">
          <div className="section-title">Speed presets</div>
          <div className="import-section-copy">{presetSummary}</div>
        </div>
        <div className="speed-grid">
          {presetOrder.map((key) => {
            const preset = presetOptions.find((option) => option.id === key)
            if (!preset) return null

            const selected = selectedPreset === key
            return (
              <button
                key={key}
                type="button"
                className={`import-card speed-card ${selected ? 'selected' : ''}`}
                aria-pressed={selected}
                onClick={() => onSelectPreset(key, preset.label)}
              >
                <div className="speed-card-top">
                  <span className="speed-card-title">{preset.label}</span>
                  <span className="import-chip speed-chip">{preset.dpi} DPI</span>
                </div>
                <span className="speed-card-summary">{preset.meta}</span>
                <span className="speed-card-detail">{preset.description}</span>
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

      <div className="advanced-toggle">
        <button
          className="btn ghost"
          type="button"
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
            <section className="import-section template-section" aria-label="Extract templates">
              <div className="import-section-header">
                <div className="section-title">Extract templates</div>
                <div className="import-section-copy">
                  Choose the output shape that best matches the document you are reviewing.
                </div>
              </div>
              <div className="template-grid">
                {extractTemplates.map((template) => {
                  const selected = selectedExtractTemplateId === template.id
                  return (
                    <button
                      key={template.id}
                      type="button"
                      className={`import-card template-card ${selected ? 'selected' : ''}`}
                      aria-pressed={selected}
                      onClick={() => onSelectExtractTemplate(template.id)}
                    >
                      <div className="template-card-body">
                        <span className="template-card-title">{template.label}</span>
                        <span className="template-card-usecase">{template.helper}</span>
                      </div>
                      <div className="template-chip-list">
                        {templateOutputChips(template).map((chip) => (
                          <span key={chip} className="import-chip template-chip">
                            {chip}
                          </span>
                        ))}
                      </div>
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
            <button className="btn ghost" type="button" onClick={onOpenSettings}>
              Advanced settings
            </button>
            {modeDefinition.available_exports.map((exportOption) => (
              <button
                key={exportOption.id}
                className={`btn ${exportOption.primary ? 'primary' : 'ghost'}`}
                type="button"
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
