import React from "react";
import type { ProcessingStatus as ProcessingStatusType } from "../types";

type ProcessingStatusProps = {
  status: ProcessingStatusType;
  pagesTotal: number;
  currentPage?: number;
};

export default function ProcessingStatus({ status, pagesTotal, currentPage }: ProcessingStatusProps) {
  if (status.phase === "idle") {
    return null;
  }

  const pageLabel = currentPage && currentPage > 0 ? currentPage : 1;
  const safeTotal = pagesTotal > 0 ? pagesTotal : 1;

  return (
    <div className="relative overflow-hidden rounded-2xl border border-white/10 bg-ink-900/60 p-4">
      <div className="scanline" aria-hidden="true" />
      <div className="relative z-10 flex flex-col gap-2">
        <div className="text-xs uppercase tracking-widest text-white/60">Status</div>
        <div className="text-sm font-semibold text-white">{status.message}</div>
        {status.phase === "processing" ? (
          <div className="text-xs text-white/60">
            Page {pageLabel} / {safeTotal}
          </div>
        ) : null}
        <div className="h-2 w-full overflow-hidden rounded-full bg-white/10">
          <div
            className="h-full rounded-full bg-neon-500 transition-all"
            style={{ width: `${Math.max(2, status.progress)}%` }}
          />
        </div>
      </div>
    </div>
  );
}
