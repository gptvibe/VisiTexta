import { StatusChips } from './StatusChips'

type TopBarProps = {
  statusItems: Array<{
    label: string
    value: string | number
    wide?: boolean
  }>
  themeLabel: string
  onToggleTheme: () => void
}

export function TopBar({ statusItems, themeLabel, onToggleTheme }: TopBarProps) {
  return (
    <header className="topbar">
      <div className="brand-block">
        <div className="subtitle">Offline OCR</div>
        <div className="title-row">
          <div className="title">VisiTexta</div>
          <div className="mode-pill">Local only</div>
        </div>
        <div className="headline">
          Turn PDFs, scans, and screenshots into markdown on this PC. Choose a preset,
          then drop, paste, or pick a file to start.
        </div>
      </div>
      <div className="topbar-side">
        <button className="theme-toggle" type="button" onClick={onToggleTheme}>
          <span>Theme</span>
          <strong>{themeLabel}</strong>
        </button>
        <StatusChips items={statusItems} />
      </div>
    </header>
  )
}
