import type { ReactNode } from 'react'

type AppShellProps = {
  topBar: ReactNode
  warning?: ReactNode
  queue: ReactNode
  importPanel: ReactNode
  preview: ReactNode
  footer: ReactNode
  drawer?: ReactNode
  overlay?: ReactNode
  toasts?: ReactNode
}

export function AppShell({
  topBar,
  warning,
  queue,
  importPanel,
  preview,
  footer,
  drawer,
  overlay,
  toasts,
}: AppShellProps) {
  return (
    <div className="app-shell">
      <div className="app">
        <div className="app-topbar-slot">{topBar}</div>
        {warning ? <div className="app-warning-slot">{warning}</div> : null}
        <main className="workspace">
          <aside className="workspace-rail">{queue}</aside>
          <section className="workspace-main">
            <div className="workspace-task">{importPanel}</div>
            <div className="workspace-preview">{preview}</div>
          </section>
        </main>
        <div className="app-footer-slot">{footer}</div>
      </div>
      {drawer}
      {overlay}
      {toasts}
    </div>
  )
}
