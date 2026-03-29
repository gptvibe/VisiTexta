import { StatusChips } from './StatusChips'

type TopBarProps = {
  contextLabel: string
  contextDetail: string
  statusItems: Array<{
    label: string
    value: string | number
    wide?: boolean
  }>
  themeLabel: string
  onToggleTheme: () => void
  onOpenSettings: () => void
}

export function TopBar({
  contextLabel,
  contextDetail,
  statusItems,
  themeLabel,
  onToggleTheme,
  onOpenSettings,
}: TopBarProps) {
  return (
    <header className="topbar">
      <div className="topbar-primary">
        <div className="app-mark" aria-hidden="true">
          V
        </div>
        <div className="brand-block">
          <div className="title-row">
            <div className="title">VisiTexta</div>
            <div className="mode-pill">Local OCR</div>
          </div>
          <div className="topbar-context">{contextLabel}</div>
          <div className="topbar-detail">{contextDetail}</div>
        </div>
      </div>
      <div className="topbar-side">
        <StatusChips items={statusItems} />
        <div className="topbar-toolbar">
          <button className="chrome-button" type="button" onClick={onToggleTheme}>
            <span>Theme</span>
            <strong>{themeLabel}</strong>
          </button>
          <button className="chrome-button settings-button" type="button" onClick={onOpenSettings}>
            <span>Open</span>
            <strong>Settings</strong>
          </button>
        </div>
      </div>
    </header>
  )
}
