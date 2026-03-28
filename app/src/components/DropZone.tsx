import { useState } from 'react'

type DropZoneProps = {
  disabled?: boolean
  onBrowse: () => void
  onPasteImage: () => void
  onFiles: (paths: string[]) => void
}

type FileWithPath = File & {
  path?: string
}

export function DropZone({ disabled, onBrowse, onPasteImage, onFiles }: DropZoneProps) {
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
        const files = Array.from(event.dataTransfer.files || []) as FileWithPath[]
        const paths = files
          .map((file) => (typeof file.path === 'string' ? file.path : null))
          .filter(Boolean) as string[]
        onFiles(paths)
      }}
    >
      <div className="dropzone-inner">
        <div className="dropzone-eyebrow">PNG, JPG, JPEG, PDF</div>
        <div className="dropzone-title">Drop files here</div>
        <div className="dropzone-subtitle">
          Everything stays on this PC. You can also paste a screenshot or copied image.
        </div>
        <div className="dropzone-actions">
          <button className="btn primary" onClick={onBrowse} disabled={disabled}>
            Choose files
          </button>
          <button className="btn ghost" onClick={onPasteImage} disabled={disabled}>
            Paste image
          </button>
        </div>
        <div className="dropzone-hint">Tip: press Ctrl+V when an image is on your clipboard.</div>
      </div>
    </div>
  )
}
