import React, { useCallback, useMemo, useRef } from "react";
import type { FileKind } from "../types";

type DropZoneProps = {
  fileName?: string;
  fileKind: FileKind;
  onPick: (file: File) => void;
  onClear: () => void;
  processing: boolean;
};

export default function DropZone({ fileName, fileKind, onPick, onClear, processing }: DropZoneProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  const accept = useMemo(() => "image/*,application/pdf", []);

  const handlePick = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (file) {
        onPick(file);
      }
    },
    [onPick]
  );

  const handleDrop = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const file = event.dataTransfer.files?.[0];
      if (file) {
        onPick(file);
      }
    },
    [onPick]
  );

  const handlePaste = useCallback(
    (event: React.ClipboardEvent<HTMLDivElement>) => {
      const file = Array.from(event.clipboardData.files)[0];
      if (file) {
        onPick(file);
      }
    },
    [onPick]
  );

  return (
    <div
      className="relative flex h-full min-h-[320px] flex-col items-center justify-center gap-4 rounded-[32px] border border-dashed border-white/20 bg-white/5 px-6 py-10 text-center"
      onDragOver={(event) => event.preventDefault()}
      onDrop={handleDrop}
      onPaste={handlePaste}
      role="button"
      tabIndex={0}
      onClick={() => inputRef.current?.click()}
    >
      {processing ? <div className="scanline pointer-events-none" aria-hidden="true" /> : null}
      <input
        ref={inputRef}
        type="file"
        accept={accept}
        className="hidden"
        onChange={handlePick}
      />
      <div className="text-4xl">▲</div>
      <div className="text-lg font-semibold">Drop an image or PDF to scan</div>
      <div className="text-sm text-white/60">Drag & drop, click to browse, or paste from clipboard</div>
      {fileName ? (
        <div className="mt-4 rounded-full border border-white/10 bg-ink-900/70 px-4 py-2 text-xs text-white/70">
          {fileKind === "pdf" ? "PDF" : "Image"} · {fileName}
        </div>
      ) : null}
      {fileName && !processing ? (
        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            onClear();
          }}
          className="mt-3 rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs text-white/70 hover:border-neon-500/60"
        >
          Clear
        </button>
      ) : null}
    </div>
  );
}
