import React, { useEffect, useMemo, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import GlowCard from "../components/GlowCard";
import DropZone from "../components/DropZone";
import PdfPreview from "../components/PdfPreview";
import ModelSelector from "../components/ModelSelector";
import OutputPanel from "../components/OutputPanel";
import ProcessingStatus from "../components/ProcessingStatus";
import { MODEL_OPTIONS } from "../lib/models";
import {
  backendEnsureModel,
  backendGetSystemInfo,
  backendMergePages,
  backendProcessImage,
  backendProcessPdf,
} from "../lib/tauri";
import type { BackendStatus } from "../lib/tauri";
import type { FileKind, PageResult, ProcessingStatus as StatusType } from "../types";

const defaultStatus: StatusType = {
  phase: "idle",
  message: "Waiting for input",
  progress: 0,
};

export default function App() {
  const [selectedTier, setSelectedTier] = useState<"minimum" | "medium" | "high" | "custom">(
    "minimum"
  );
  const [customName, setCustomName] = useState("");
  const [memoryLimitGb, setMemoryLimitGb] = useState(8);
  const [fileKind, setFileKind] = useState<FileKind>(null);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [fileObject, setFileObject] = useState<File | null>(null);
  const [pageResults, setPageResults] = useState<PageResult[]>([]);
  const [mergedText, setMergedText] = useState("");
  const [status, setStatus] = useState<StatusType>(defaultStatus);
  const [gpuInfo, setGpuInfo] = useState<string | null>(null);
  const [backendName, setBackendName] = useState<string>("llama.cpp");
  const [modelCached, setModelCached] = useState<boolean | null>(null);
  const [processing, setProcessing] = useState(false);
  const [currentPage, setCurrentPage] = useState<number | undefined>(undefined);
  const [pagesTotal, setPagesTotal] = useState(0);

  const modelId = useMemo(() => {
    if (selectedTier === "custom" && customName.trim()) {
      return customName.trim();
    }
    const option = MODEL_OPTIONS.find((item) => item.id === selectedTier);
    return option?.hfName ? option.hfName : MODEL_OPTIONS[0].hfName;
  }, [selectedTier, customName]);

  useEffect(() => {
    backendGetSystemInfo()
      .then((info) => {
        setGpuInfo(info.gpu);
        setBackendName(info.backend);
      })
      .catch(() => {
        setGpuInfo(null);
      });
  }, []);

  useEffect(() => {
    let unlistenStatus: UnlistenFn | null = null;
    let unlistenPage: UnlistenFn | null = null;

    const setupListeners = async () => {
      unlistenStatus = await listen<BackendStatus>("ocr://status", (event) => {
        const payload = event.payload;
        setStatus({ phase: payload.phase, message: payload.message, progress: payload.progress });
        if (payload.currentPage !== undefined) {
          setCurrentPage(payload.currentPage);
        }
        if (payload.pagesTotal !== undefined) {
          setPagesTotal(payload.pagesTotal);
        }
      });

      unlistenPage = await listen<PageResult>("ocr://page", (event) => {
        const payload = event.payload;
        setPageResults((previous) => {
          const index = previous.findIndex((page) => page.pageIndex === payload.pageIndex);
          if (index >= 0) {
            const next = [...previous];
            next[index] = payload;
            return next.sort((a, b) => a.pageIndex - b.pageIndex);
          }
          return [...previous, payload].sort((a, b) => a.pageIndex - b.pageIndex);
        });
      });
    };

    setupListeners().catch(() => undefined);

    return () => {
      unlistenStatus?.();
      unlistenPage?.();
    };
  }, []);

  const handlePickFile = async (file: File) => {
    const kind = file.type === "application/pdf" ? "pdf" : "image";
    setFileKind(kind);
    setFileName(file.name);
    setFileObject(file);
    const path = (file as unknown as { path?: string }).path;
    if (path) {
      setFilePath(path);
    } else {
      setFilePath(null);
    }
  };

  const clearSelection = () => {
    setFileKind(null);
    setFileName(null);
    setFilePath(null);
    setFileObject(null);
    setPageResults([]);
    setMergedText("");
    setStatus(defaultStatus);
    setProcessing(false);
    setCurrentPage(undefined);
    setPagesTotal(0);
    setModelCached(null);
  };

  const ensureModel = async () => {
    setStatus({ phase: "loading", message: "Loading model", progress: 5 });
    const result = await backendEnsureModel(modelId, memoryLimitGb);
    if (result.cached) {
      setStatus({ phase: "loading", message: "Model cached", progress: 20 });
      setModelCached(true);
    } else {
      setModelCached(false);
    }
    return result;
  };

  const handleProcess = async () => {
    if (!filePath || !fileKind) {
      setStatus({ phase: "error", message: "Pick a file first", progress: 0 });
      return;
    }
    setProcessing(true);
    setMergedText("");
    setPageResults([]);
    setCurrentPage(undefined);
    setPagesTotal(0);
    try {
      await ensureModel();
      setStatus({ phase: "processing", message: "Running OCR", progress: 40 });

      if (fileKind === "image") {
        setPagesTotal(1);
        setCurrentPage(1);
        const response = await backendProcessImage({ modelId, filePath });
        setPageResults(response.pages);
        if (response.status) {
          setStatus({
            phase: response.status.phase,
            message: response.status.message,
            progress: response.status.progress,
          });
          if (response.status.pagesTotal !== undefined) {
            setPagesTotal(response.status.pagesTotal);
          }
          if (response.status.currentPage !== undefined) {
            setCurrentPage(response.status.currentPage);
          }
        } else {
          setStatus({ phase: "complete", message: "OCR complete", progress: 100 });
        }
      } else {
        const response = await backendProcessPdf({ modelId, filePath });
        setPageResults(response.pages);
        if (response.status) {
          setStatus({
            phase: response.status.phase,
            message: response.status.message,
            progress: response.status.progress,
          });
          if (response.status.pagesTotal !== undefined) {
            setPagesTotal(response.status.pagesTotal);
          } else {
            setPagesTotal(response.pages.length);
          }
          if (response.status.currentPage !== undefined) {
            setCurrentPage(response.status.currentPage);
          }
        } else {
          setStatus({ phase: "complete", message: "OCR complete", progress: 100 });
          setPagesTotal(response.pages.length);
        }
      }
    } catch (error) {
      setStatus({ phase: "error", message: "Processing failed", progress: 100 });
    } finally {
      setProcessing(false);
    }
  };

  const handleMerge = async () => {
    if (!pageResults.length) {
      return;
    }
    setStatus({ phase: "processing", message: "Merging pages", progress: 70 });
    setProcessing(true);
    try {
      const response = await backendMergePages({ modelId, pages: pageResults });
      setMergedText(response.merged);
      setStatus({ phase: "complete", message: "Merge complete", progress: 100 });
    } catch (error) {
      setStatus({ phase: "error", message: "Merge failed", progress: 100 });
    } finally {
      setProcessing(false);
    }
  };

  const handleCopy = async () => {
    const combined = pageResults.map((page) => page.text).join("\n\n");
    await navigator.clipboard.writeText(mergedText || combined);
  };

  const handleExport = async (extension: "txt" | "md") => {
    const combined = pageResults.map((page) => page.text).join("\n\n");
    const content = mergedText || combined;
    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `visitexta-export.${extension}`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const handleKeyPress = (event: React.KeyboardEvent) => {
    if (event.key === "Enter" && !processing) {
      handleProcess();
    }
  };

  return (
    <div className="min-h-screen neo-gradient text-white">
      <header className="mx-auto flex max-w-6xl flex-col gap-4 px-6 pt-10">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.3em] text-neon-400">VisiTexta</p>
            <h1 className="text-3xl font-semibold">Local Vision OCR Studio</h1>
            <p className="mt-2 max-w-2xl text-sm text-white/60">
              Scan images and PDFs with fully local vision-language models. Auto-downloads the first time,
              then runs fast on your GPU or CPU.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs text-white/60">
            <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
              GPU: {gpuInfo || "Not detected"}
            </div>
            <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
              Backend: {backendName}
            </div>
            <div className="rounded-2xl border border-white/10 bg-white/5 px-4 py-3">
              Model: {modelCached == null ? "—" : modelCached ? "Cached" : "Downloaded"}
            </div>
          </div>
        </div>
      </header>

      <main className="mx-auto grid max-w-6xl gap-6 px-6 py-10 lg:grid-cols-[1.2fr_1fr]">
        <div className="flex flex-col gap-6">
          <GlowCard title="Source" subtitle="Drop an image or PDF to begin.">
            <DropZone
              fileName={fileName || undefined}
              fileKind={fileKind}
              onPick={handlePickFile}
              onClear={clearSelection}
              processing={processing}
            />
            {fileKind === "pdf" && fileObject ? (
              <div className="mt-4 rounded-2xl border border-white/10 bg-white/5 p-4">
                <PdfPreview file={fileObject} />
              </div>
            ) : null}
            <div className="mt-4 flex flex-wrap gap-3">
              <button
                type="button"
                onClick={handleProcess}
                disabled={!filePath || processing}
                className="rounded-full bg-neon-500/90 px-6 py-3 text-sm font-semibold text-ink-900 hover:bg-neon-500 disabled:opacity-40"
              >
                {processing ? "Processing" : "Start OCR"}
              </button>
              <button
                type="button"
                className="rounded-full border border-white/10 px-5 py-3 text-sm text-white/70 hover:border-neon-500/70"
                onClick={clearSelection}
              >
                Reset
              </button>
            </div>
            <div className="mt-4">
              <ProcessingStatus status={status} pagesTotal={pagesTotal} currentPage={currentPage} />
            </div>
          </GlowCard>
          <GlowCard title="Model Lab" subtitle="Pick a model and tune the memory cap.">
            <ModelSelector
              options={MODEL_OPTIONS}
              selected={selectedTier}
              customName={customName}
              onSelect={setSelectedTier}
              onCustomName={(value) => {
                setCustomName(value);
                if (value.trim().length > 0) {
                  setSelectedTier("custom");
                }
              }}
              memoryLimitGb={memoryLimitGb}
              onMemoryLimit={setMemoryLimitGb}
            />
          </GlowCard>
        </div>

        <div className="flex flex-col gap-6" onKeyDown={handleKeyPress}>
          <OutputPanel
            fileKind={fileKind}
            pageResults={pageResults}
            mergedText={mergedText}
            onCopy={handleCopy}
            onExportTxt={() => handleExport("txt")}
            onExportMd={() => handleExport("md")}
            onMerge={handleMerge}
            processing={processing}
          />
          <GlowCard title="Live Pipeline" subtitle="Real-time view of OCR stages.">
            <div className="space-y-3 text-sm text-white/70">
              <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-ink-900/60 px-4 py-3">
                <span>1. Decode input</span>
                <span className="text-xs text-white/50">image / PDF</span>
              </div>
              <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-ink-900/60 px-4 py-3">
                <span>2. Slice pages</span>
                <span className="text-xs text-white/50">PDF -> images</span>
              </div>
              <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-ink-900/60 px-4 py-3">
                <span>3. Run vision model</span>
                <span className="text-xs text-white/50">GPU or CPU</span>
              </div>
              <div className="flex items-center justify-between rounded-2xl border border-white/10 bg-ink-900/60 px-4 py-3">
                <span>4. Compose text</span>
                <span className="text-xs text-white/50">Layout preserved</span>
              </div>
            </div>
          </GlowCard>
        </div>
      </main>
    </div>
  );
}
