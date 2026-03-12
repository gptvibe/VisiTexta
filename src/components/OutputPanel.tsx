import React from "react";
import type { FileKind, PageResult } from "../types";

type OutputPanelProps = {
  fileKind: FileKind;
  pageResults: PageResult[];
  mergedText: string;
  onCopy: () => void;
  onExportTxt: () => void;
  onExportMd: () => void;
  onMerge: () => void;
  processing: boolean;
};

export default function OutputPanel({
  fileKind,
  pageResults,
  mergedText,
  onCopy,
  onExportTxt,
  onExportMd,
  onMerge,
  processing,
}: OutputPanelProps) {
  const hasResults = pageResults.length > 0;

  return (
    <section className="glass-panel rounded-3xl px-6 py-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">Output Studio</h2>
          <p className="text-sm text-white/60">
            Progressive OCR output, ready to export or merge.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={onCopy}
            disabled={!hasResults}
            className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs text-white/80 hover:border-neon-500/70 disabled:opacity-40"
          >
            Copy Text
          </button>
          <button
            type="button"
            onClick={onExportTxt}
            disabled={!hasResults}
            className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs text-white/80 hover:border-neon-500/70 disabled:opacity-40"
          >
            Export TXT
          </button>
          <button
            type="button"
            onClick={onExportMd}
            disabled={!hasResults}
            className="rounded-full border border-white/10 bg-white/5 px-4 py-2 text-xs text-white/80 hover:border-neon-500/70 disabled:opacity-40"
          >
            Export MD
          </button>
        </div>
      </div>

      {fileKind === "pdf" ? (
        <div className="mt-4 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
          <div>
            <div className="text-sm font-semibold">Merge Pages into Article</div>
            <div className="text-xs text-white/60">
              Connects page paragraphs into a smooth, readable article.
            </div>
          </div>
          <button
            type="button"
            onClick={onMerge}
            disabled={!hasResults || processing}
            className="rounded-full bg-neon-500/80 px-4 py-2 text-xs font-semibold text-ink-900 hover:bg-neon-500 disabled:opacity-40"
          >
            Merge Pages
          </button>
        </div>
      ) : null}

      <div className="mt-6 grid gap-4 lg:grid-cols-[2fr_3fr]">
        <div className="scrollbar-skin max-h-[520px] space-y-3 overflow-y-auto pr-2">
          {pageResults.map((page) => (
            <div
              key={page.pageIndex}
              className="rounded-2xl border border-white/10 bg-ink-900/60 p-4"
            >
              <div className="flex items-center justify-between text-xs uppercase tracking-widest text-white/60">
                <span>Page {page.pageIndex}</span>
                <span>{page.status}</span>
              </div>
              <p className="mt-3 whitespace-pre-wrap text-sm text-white/80">
                {page.text || (page.status === "processing" ? "Scanning..." : "Waiting")}
              </p>
            </div>
          ))}
          {!hasResults ? (
            <div className="rounded-2xl border border-dashed border-white/10 bg-ink-900/40 p-6 text-sm text-white/60">
              OCR output will appear here as pages finish processing.
            </div>
          ) : null}
        </div>
        <div className="rounded-2xl border border-white/10 bg-ink-900/60 p-4">
          <div className="text-xs uppercase tracking-widest text-white/60">Merged Article</div>
          <div className="mt-3 min-h-[220px] whitespace-pre-wrap text-sm text-white/80">
            {mergedText ||
              (hasResults
                ? "Click Merge Pages to build a single article."
                : "Waiting for OCR output...")}
          </div>
        </div>
      </div>
    </section>
  );
}
