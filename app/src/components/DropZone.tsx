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
        <div className="dropzone-eyebrow">Supported inputs: PNG, JPG, JPEG, PDF</div>
        <div className="dropzone-title">Add documents</div>
        <div className="dropzone-subtitle">
          Process scans, screenshots, and PDFs locally. Browse for files or paste an image from the clipboard.
        </div>
        <div className="dropzone-actions">
          <button className="btn primary" type="button" onClick={onBrowse} disabled={disabled}>
            Choose files
          </button>
          <button className="btn ghost" type="button" onClick={onPasteImage} disabled={disabled}>
            Paste image
          </button>
        </div>
        <div className="dropzone-hint">Tip: press Ctrl+V when a copied image is ready.</div>
      </div>
    </div>
  )
}
