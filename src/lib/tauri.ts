import { invoke } from "@tauri-apps/api/core";
import type { PageResult, ProcessingStatus } from "../types";

export type BackendStatus = ProcessingStatus & {
  currentPage?: number;
  pagesTotal?: number;
};

export async function backendGetSystemInfo() {
  return invoke("get_system_info") as Promise<{
    gpu: string | null;
    backend: string;
  }>;
}

export async function backendEnsureModel(modelId: string, memoryLimitGb: number) {
  return invoke("ensure_model", { modelId, memoryLimitGb }) as Promise<{
    cached: boolean;
    path: string;
  }>;
}

export async function backendProcessImage(payload: {
  modelId: string;
  filePath: string;
}) {
  return invoke("process_image", payload) as Promise<{
    pages: PageResult[];
    status: BackendStatus;
  }>;
}

export async function backendProcessPdf(payload: {
  modelId: string;
  filePath: string;
}) {
  return invoke("process_pdf", payload) as Promise<{
    pages: PageResult[];
    status: BackendStatus;
  }>;
}

export async function backendMergePages(payload: {
  modelId: string;
  pages: PageResult[];
}) {
  return invoke("merge_pages", payload) as Promise<{ merged: string }>;
}
