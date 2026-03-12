import React from "react";
import type { ModelOption, ModelTier } from "../types";

type ModelSelectorProps = {
  options: ModelOption[];
  selected: ModelTier;
  customName: string;
  onSelect: (tier: ModelTier) => void;
  onCustomName: (value: string) => void;
  memoryLimitGb: number;
  onMemoryLimit: (value: number) => void;
};

export default function ModelSelector({
  options,
  selected,
  customName,
  onSelect,
  onCustomName,
  memoryLimitGb,
  onMemoryLimit,
}: ModelSelectorProps) {
  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-3 md:grid-cols-3">
        {options.map((option) => (
          <button
            key={option.id}
            type="button"
            onClick={() => onSelect(option.id)}
            className={`rounded-2xl border px-4 py-4 text-left transition hover:border-neon-500/80 hover:shadow-glow ${
              selected === option.id
                ? "border-neon-500 bg-neon-500/10"
                : "border-white/10 bg-white/5"
            }`}
          >
            <div className="text-sm uppercase tracking-widest text-white/60">{option.label}</div>
            <div className="mt-2 text-sm font-semibold text-white">{option.hfName}</div>
            <div className="mt-1 text-xs text-white/50">{option.memoryHint}</div>
          </button>
        ))}
      </div>
      <div
        className={`rounded-2xl border bg-white/5 p-4 ${
          selected === "custom"
            ? "border-neon-500/70 shadow-glow"
            : "border-white/10"
        }`}
      >
        <label className="text-xs uppercase tracking-widest text-white/60">Custom Model</label>
        <input
          value={customName}
          onChange={(event) => onCustomName(event.target.value)}
          placeholder="HuggingFace repo id (e.g. Qwen/Qwen3.5-VL-7B)"
          className="mt-2 w-full rounded-xl border border-white/10 bg-ink-900/60 px-3 py-2 text-sm text-white outline-none focus:border-neon-500"
        />
        <p className="mt-2 text-xs text-white/50">
          Custom models must be vision-capable and compatible with the runtime.
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-4">
        <div className="text-xs uppercase tracking-widest text-white/60">Memory Limit</div>
        <input
          type="range"
          min={2}
          max={64}
          step={2}
          value={memoryLimitGb}
          onChange={(event) => onMemoryLimit(Number(event.target.value))}
          className="w-56"
        />
        <div className="text-sm text-white">{memoryLimitGb} GB</div>
      </div>
    </div>
  );
}
