import { useState } from 'react'

type DropZoneProps = {
  disabled?: boolean
  onBrowse: () => void
  onFiles: (paths: string[]) => void
}

export function DropZone({ disabled, onBrowse, onFiles }: DropZoneProps) {
  const [active, setActive] = useState(false)

  return (
    <div
      className={`dropzone ${active ? 'is-active' : ''} ${disabled ? 'is-disabled' : ''}`}
      onDragOver={(event) => {
        event.preventDefault()
        if (!disabled) setActive(true)
      }}
      onDragLeave={() => setActive(false)}
      onDrop={(event) => {
        event.preventDefault()
        if (disabled) return
        setActive(false)
        const files = Array.from(event.dataTransfer.files || [])
        const paths = files
          .map((file: any) => (typeof file.path === 'string' ? file.path : null))
          .filter(Boolean) as string[]
        onFiles(paths)
      }}
    >
      <div className="dropzone-inner">
        <div className="dropzone-title">Feed the vision pipeline</div>
        <div className="dropzone-subtitle">Drop PNG, JPG, JPEG, or PDF to start a live OCR stream.</div>
        <div className="dropzone-actions">
          <button className="btn primary" onClick={onBrowse} disabled={disabled}>
            Browse files
          </button>
          <button className="btn ghost" disabled>
            Paste (soon)
          </button>
        </div>
      </div>
    </div>
  )
}
