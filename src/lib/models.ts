import type { ModelOption } from "../types";

export const MODEL_OPTIONS: ModelOption[] = [
  {
    id: "minimum",
    label: "Minimum",
    hfName: "Qwen/Qwen3.5-0.8B",
    memoryHint: "~4 GB VRAM · Fastest",
  },
  {
    id: "medium",
    label: "Medium",
    hfName: "Qwen/Qwen3.5-4B-Base",
    memoryHint: "~10 GB VRAM · Balanced",
  },
  {
    id: "high",
    label: "High Quality",
    hfName: "Qwen/Qwen3.5-27B",
    memoryHint: "~48 GB VRAM · Best quality",
  },
];
