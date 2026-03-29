type FirstRunWizardProps = {
  open: boolean
  statusLabel: string
  statusTone: 'info' | 'success' | 'warning' | 'error'
  title: string
  description: string
  storageModeLabel: string
  storagePath: string
  storageHint: string
  estimatedDiskUse: string
  modelLabel: string
  modelFamily: string
  modelFile: string
  mmprojFile?: string | null
  validationStatus: string
  validationTone: 'info' | 'success' | 'warning' | 'error'
  downloadStatus: string
  helperMessage?: string | null
  progressPercent: number
  showProgress: boolean
  canStart: boolean
  canRetry: boolean
  isWorking: boolean
  onStart: () => void
  onRetry: () => void
  onCancel: () => void
  onOpenSettings: () => void
}

export function FirstRunWizard({
  open,
  statusLabel,
  statusTone,
  title,
  description,
  storageModeLabel,
  storagePath,
  storageHint,
  estimatedDiskUse,
  modelLabel,
  modelFamily,
  modelFile,
  mmprojFile,
  validationStatus,
  validationTone,
  downloadStatus,
  helperMessage,
  progressPercent,
  showProgress,
  canStart,
  canRetry,
  isWorking,
  onStart,
  onRetry,
  onCancel,
  onOpenSettings,
}: FirstRunWizardProps) {
  if (!open) return null

  return (
    <div className="modal-shell first-run-modal-shell" role="dialog" aria-modal="true">
      <div className="modal-overlay" onClick={onCancel} />
      <section className="modal-panel first-run-modal" aria-label="First-run OCR setup">
        <div className="first-run-header">
          <div>
            <div className="section-title">First-run setup</div>
            <div className="first-run-title">{title}</div>
          </div>
          <div className={`setup-card-badge first-run-status-badge ${statusTone}`}>
            {statusLabel}
          </div>
        </div>

        <p className="first-run-body">{description}</p>

        <div className="first-run-grid">
          <article className="first-run-card">
            <div className="section-title">Storage</div>
            <div className="first-run-card-title">{storageModeLabel}</div>
            <div className="field-note">{storageHint}</div>
            <div className="model-profile-meta">
              <span>Model files</span>
              <strong>{storagePath}</strong>
            </div>
            <div className="model-profile-meta">
              <span>Estimated disk use</span>
              <strong>{estimatedDiskUse}</strong>
            </div>
          </article>

          <article className="first-run-card">
            <div className="section-title">Recommended model</div>
            <div className="first-run-card-title">{modelLabel}</div>
            <div className="field-note">{modelFamily}</div>
            <div className="model-profile-meta">
              <span>Main file</span>
              <strong>{modelFile}</strong>
            </div>
            {mmprojFile && (
              <div className="model-profile-meta">
                <span>Companion file</span>
                <strong>{mmprojFile}</strong>
              </div>
            )}
          </article>

          <article className="first-run-card">
            <div className="section-title">Validation</div>
            <div className={`status-pill ${validationTone === 'success' ? 'ok' : validationTone === 'error' ? 'bad' : validationTone === 'warning' ? 'warn' : ''}`}>
              {validationStatus}
            </div>
            <div className="field-note">{downloadStatus}</div>
            {helperMessage && <div className="field-note">{helperMessage}</div>}
          </article>
        </div>

        {showProgress && (
          <div className="model-progress first-run-progress">
            <div className="model-progress-bar">
              <div className="model-progress-fill" style={{ width: `${progressPercent}%` }} />
            </div>
            <div className="model-progress-text">{downloadStatus}</div>
          </div>
        )}

        <div className="first-run-actions">
          <button className="btn ghost" type="button" onClick={onOpenSettings}>
            Advanced settings
          </button>
          <div className="first-run-action-group">
            <button className="btn ghost" type="button" onClick={onCancel}>
              {isWorking ? 'Hide' : 'Cancel'}
            </button>
            {canRetry ? (
              <button className="btn primary" type="button" onClick={onRetry}>
                Retry download
              </button>
            ) : canStart ? (
              <button className="btn primary" type="button" onClick={onStart} disabled={isWorking}>
                {isWorking ? 'Preparing...' : 'Download recommended model'}
              </button>
            ) : null}
          </div>
        </div>
      </section>
    </div>
  )
}
