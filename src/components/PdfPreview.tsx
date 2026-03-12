import React, { useEffect, useMemo, useState } from "react";
import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import workerSrc from "pdfjs-dist/build/pdf.worker.min.mjs?url";

GlobalWorkerOptions.workerSrc = workerSrc;

type PdfPreviewProps = {
  file: File;
};

type Thumbnail = {
  pageIndex: number;
  dataUrl: string;
};

const MAX_PAGES = 4;
const PREVIEW_SCALE = 0.28;

export default function PdfPreview({ file }: PdfPreviewProps) {
  const [thumbnails, setThumbnails] = useState<Thumbnail[]>([]);
  const [pageCount, setPageCount] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setThumbnails([]);
    setPageCount(null);
    setError(null);
    setLoading(true);

    const renderPreview = async () => {
      const data = await file.arrayBuffer();
      const pdf = await getDocument({ data }).promise;
      if (cancelled) return;

      setPageCount(pdf.numPages);

      const pagesToRender = Math.min(pdf.numPages, MAX_PAGES);
      const rendered: Thumbnail[] = [];

      for (let pageNumber = 1; pageNumber <= pagesToRender; pageNumber += 1) {
        const page = await pdf.getPage(pageNumber);
        const viewport = page.getViewport({ scale: PREVIEW_SCALE });
        const canvas = document.createElement("canvas");
        const context = canvas.getContext("2d");
        if (!context) {
          continue;
        }
        canvas.width = viewport.width;
        canvas.height = viewport.height;
        await page.render({ canvasContext: context, viewport }).promise;
        const dataUrl = canvas.toDataURL("image/png");
        rendered.push({ pageIndex: pageNumber, dataUrl });
        if (!cancelled) {
          setThumbnails([...rendered]);
        }
      }
    };

    renderPreview()
      .catch(() => {
        if (!cancelled) {
          setError("Preview unavailable");
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [file]);

  const previewLabel = useMemo(() => {
    if (pageCount && pageCount > thumbnails.length) {
      return `Showing ${thumbnails.length} of ${pageCount} pages`;
    }
    if (pageCount) {
      return `${pageCount} page${pageCount === 1 ? "" : "s"}`;
    }
    return "Loading pages...";
  }, [pageCount, thumbnails.length]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between text-xs text-white/60">
        <span>PDF Preview</span>
        <span>{error ? "Preview failed" : previewLabel}</span>
      </div>
      {error ? (
        <div className="rounded-xl border border-dashed border-white/10 bg-ink-900/60 p-4 text-xs text-white/60">
          Unable to render PDF preview.
        </div>
      ) : (
        <div className="grid grid-cols-2 gap-3">
          {thumbnails.map((thumb) => (
            <div
              key={thumb.pageIndex}
              className="rounded-xl border border-white/10 bg-ink-900/70 p-2"
            >
              <img
                src={thumb.dataUrl}
                alt={`Page ${thumb.pageIndex} preview`}
                className="w-full rounded-lg object-contain"
              />
              <div className="mt-2 text-[10px] uppercase tracking-widest text-white/50">
                Page {thumb.pageIndex}
              </div>
            </div>
          ))}
          {loading && thumbnails.length === 0
            ? Array.from({ length: 4 }).map((_, index) => (
                <div
                  key={`skeleton-${index}`}
                  className="h-32 animate-pulse rounded-xl border border-white/10 bg-white/5"
                />
              ))
            : null}
        </div>
      )}
    </div>
  );
}
