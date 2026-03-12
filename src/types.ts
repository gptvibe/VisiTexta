export type ModelTier = "minimum" | "medium" | "high" | "custom";

export type ModelOption = {
  id: ModelTier;
  label: string;
  hfName: string;
  memoryHint: string;
};

export type FileKind = "image" | "pdf" | null;

export type PageResult = {
  pageIndex: number;
  text: string;
  status: "queued" | "processing" | "done" | "error";
};

export type ProcessingStatus = {
  phase: "idle" | "loading" | "downloading" | "processing" | "complete" | "error";
  message: string;
  progress: number;
};
