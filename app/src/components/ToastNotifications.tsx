export type Toast = {
  id: string
  message: string
  tone?: 'info' | 'success' | 'error'
}

type ToastNotificationsProps = {
  toasts: Toast[]
  onDismiss: (id: string) => void
}

export function ToastNotifications({ toasts, onDismiss }: ToastNotificationsProps) {
  return (
    <div className="toast-stack">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast ${toast.tone || 'info'}`}>
          <span>{toast.message}</span>
          <button className="btn ghost" onClick={() => onDismiss(toast.id)}>
            Dismiss
          </button>
        </div>
      ))}
    </div>
  )
}
