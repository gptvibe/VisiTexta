# Benchmarks

Run the OCR benchmark suite from the repo root with:

```bash
npm --prefix app run benchmark
```

To compare cold versus prewarmed time-to-first-text runs:

```bash
npm --prefix app run benchmark -- --prewarm off
npm --prefix app run benchmark -- --prewarm on
```

What it does:

- Loads `app/benchmarks/fixtures/manifest.json`
- Covers the default workflow plus low-end Starter-style cases and a multi-page scan PDF case
- Runs each fixture serially through the existing OCR pipeline
- Records `time_to_first_preview`, `time_to_first_text`, `total_time`, `peak_memory_bytes`, and `normalized_output_diff`
- Writes pretty JSON reports under `app/benchmarks/out/` and refreshes `app/benchmarks/out/latest.json`

Regression-gate commands:

```bash
npm --prefix app run benchmark:gate:cold
npm --prefix app run benchmark:gate:warm
```

Those commands compare the new run against:

- `app/benchmarks/baselines/cold.json`
- `app/benchmarks/baselines/warm.json`
- `app/benchmarks/gate.json`

The checked-in thresholds currently gate:

- `time_to_first_text`
- `total_time`
- `peak_memory_bytes`

The warm baseline is intentionally an envelope baseline rather than a single lucky run. It keeps the slowest approved warm timings and highest approved PDF memory peaks observed on the release workstation so the gate is useful without flapping on ordinary local variance.

Fixture layout:

- Inputs live under `app/benchmarks/fixtures/inputs/`
- Expected markdown lives under `app/benchmarks/fixtures/expected/`
- The manifest points each fixture at its input and expected output file

Notes:

- `npm --prefix app run release:qa` turns the benchmark suite into a release gate by running frontend build checks, `cargo check`, and both benchmark gate modes before packaging.
- The command assumes the usual Tauri/Rust development toolchain is installed and available.
- OCR prompts, model args, and markdown output semantics still come from the shipping pipeline.
- `normalized_output_diff` ignores whitespace-only differences and injected page header formatting such as `## Page 1`.
- Add `-- --bless` after the command to refresh expected markdown baselines for newly added fixtures.
- `-- --prewarm off` resets the persistent OCR worker before each fixture to capture cold-start TTFT.
- `-- --prewarm on` prewarms the persistent OCR worker before each fixture to approximate the post-idle experience after model selection or installation.
- The multi-page PDF fixture is generated from a browser-rendered two-page document so Pdfium sees a realistic, standards-compliant scan input.
- If you want a tighter gate on a quieter dedicated runner, edit `app/benchmarks/gate.json` and refresh the baselines intentionally on that machine.
