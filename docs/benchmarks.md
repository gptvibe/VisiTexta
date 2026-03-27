# Benchmarks

Run the OCR benchmark suite from the repo root with:

```bash
npm --prefix app run benchmark
```

What it does:

- Loads `app/benchmarks/fixtures/manifest.json`
- Runs each fixture serially through the existing OCR pipeline
- Records `time_to_first_preview`, `time_to_first_text`, `total_time`, `peak_memory_bytes`, and `normalized_output_diff`
- Writes pretty JSON reports under `app/benchmarks/out/` and refreshes `app/benchmarks/out/latest.json`

Fixture layout:

- Inputs live under `app/benchmarks/fixtures/inputs/`
- Expected markdown lives under `app/benchmarks/fixtures/expected/`
- The manifest points each fixture at its input and expected output file

Notes:

- The benchmark runner is dev-only and does not change release behavior.
- The command assumes the usual Tauri/Rust development toolchain is installed and available.
- OCR prompts, model args, and markdown output semantics still come from the shipping pipeline.
- `normalized_output_diff` ignores whitespace-only differences and injected page header formatting such as `## Page 1`.
- Add `-- --bless` after the command to refresh expected markdown baselines for newly added fixtures.
