type WizardStep = {
  title: string
  body: string
  detail: string
}

type FirstRunWizardProps = {
  open: boolean
  step: number
  steps: WizardStep[]
  onBack: () => void
  onNext: () => void
  onSkip: () => void
}

export function FirstRunWizard({
  open,
  step,
  steps,
  onBack,
  onNext,
  onSkip,
}: FirstRunWizardProps) {
  if (!open) return null

  const current = steps[step] ?? steps[0]
  if (!current) return null

  const isLastStep = step === steps.length - 1

  return (
    <section className="onboarding-card" aria-label="First-run guide">
      <div className="onboarding-header">
        <div>
          <div className="section-title">First-run guide</div>
          <div className="onboarding-title">{current.title}</div>
        </div>
        <button className="btn ghost" onClick={onSkip}>
          Skip
        </button>
      </div>
      <p className="onboarding-body">{current.body}</p>
      <div className="onboarding-detail">{current.detail}</div>
      <div className="onboarding-progress">
        <span>{`Step ${step + 1} of ${steps.length}`}</span>
        <div className="onboarding-progress-bar" aria-hidden="true">
          <div
            className="onboarding-progress-fill"
            style={{ width: `${((step + 1) / steps.length) * 100}%` }}
          />
        </div>
      </div>
      <div className="onboarding-actions">
        <button className="btn ghost" onClick={onBack} disabled={step === 0}>
          Back
        </button>
        <button className="btn primary" onClick={onNext}>
          {isLastStep ? 'Start extraction' : 'Next'}
        </button>
      </div>
    </section>
  )
}
