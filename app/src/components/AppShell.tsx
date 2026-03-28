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
    <div className="app">
      {topBar}
      {warning}
      <main className="workspace">
        {queue}
        {importPanel}
        {preview}
      </main>
      {footer}
      {drawer}
      {overlay}
      {toasts}
    </div>
  )
}
