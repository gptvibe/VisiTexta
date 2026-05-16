# VisiTexta Native Rebuild Plan

Audit date: 2026-05-16  
Audited repositories:

- VisiTexta: `C:\Users\fungk\Desktop\GitHub\VisiTexta`, branch `main`, commit `f4c1ae9`
- QuietScribe / Audio-to-Text: `C:\Users\fungk\Desktop\GitHub\Audio to Text`, branch `main`, commit `d4fe539`

## Executive Direction

VisiTexta should be rebuilt as a native Windows app rather than reskinned. The native version should keep the existing VisiTexta product value - local image/PDF OCR with streaming output, workflow modes, exports, model management, history, portable behavior, and offline-first processing - while adopting QuietScribe's WinUI 3 structure, service layering, local worker process boundary, release packaging style, and calm Fluent UI.

The target v1 should not add cloud OCR, AI chat, account features, sync, or an installer-first packaging strategy. Network access should remain limited to model/runtime downloads and optional model metadata checks. Once models and runtimes are present, OCR and document processing should run locally.

## Current VisiTexta Feature Inventory

### Current Stack

The existing app is a Tauri 2 desktop app:

- React/TypeScript frontend under `app/src`
- Rust backend under `app/src-tauri/src`
- Rust OCR pipeline and service-like modules for storage, settings, history, PDF rendering, model downloads, llama runtime discovery, formatting, modes, study notes, extract templates, and benchmarks
- Tauri commands bridge the frontend to Rust for jobs, settings, downloads, file operations, and exports

This stack should be replaced for the native rebuild. The product behavior should be ported, not the Tauri shell.

### Inputs and Job Flow

Current supported source files:

- PNG
- JPG
- JPEG
- PDF

Current flow:

- User selects, drops, queues, or pastes files.
- Backend validates extension and source existence.
- PDF pages are rendered lazily with PDFium.
- Images and rendered pages are preprocessed to PNG and capped by a max OCR dimension.
- OCR streams text page by page.
- UI receives progress, preview images, runner status, text chunks, completion, and errors.
- Jobs can be canceled.
- Interrupted non-terminal history entries are marked failed on next launch.
- OCR output is written next to the source file.
- Output naming is duplicate-safe:
  - first output: `name.ocr.md`
  - later outputs: `name (ocr 2).md`, `name (ocr 3).md`, etc.

The native app must preserve this workflow and keep heavy work off the UI thread. The current Rust implementation processes a batch serially while PDF rendering and OCR can overlap through background threads; the native version should move this work behind the separate OCR worker process.

### Workflow Modes

The current app has three workflow modes:

- Exact OCR: faithful OCR-to-Markdown output.
- Notes: turns OCR page text into study notes with page references such as `Source: p. 3`.
- Extract: turns OCR page text into structured document extraction.

Extract presets currently include:

- Invoice / Receipt
- Table to CSV
- Meeting Photo / Whiteboard
- Contract Key Points

Notes mode currently supports:

- Headings
- Key points
- Glossary
- Formulas
- Examples
- Review questions
- Optional Study Boost memory checks
- Markdown, plain text, searchable text-based PDF, and flashcard/Anki CSV export paths

Extract mode currently supports:

- Markdown output
- Hidden structured JSON metadata embedded in Markdown
- JSON export
- CSV export from extracted rows or fields
- Uncertainty / verification notes for fields and rows needing review

The C# port should keep the mode model explicit in `App.Models`, and should port the deterministic post-processing logic into `App.Core` or `App.Services` with tests.

### Model and Runtime Behavior

The current app uses local GGUF vision models and llama runtimes.

Curated model profiles:

- Recommended: GLM-OCR
  - repo: `mradermacher/GLM-OCR-GGUF`
  - default file: `GLM-OCR.Q4_K_M.gguf`
  - requires mmproj
- Tested alternative: Qwen2-VL OCR 2B
  - repo: `mradermacher/Qwen2-VL-OCR-2B-Instruct-GGUF`
  - default file: `Qwen2-VL-OCR-2B-Instruct.Q4_K_M.gguf`
  - requires mmproj
- Tested alternative: Qwen2.5-VL 3B
  - repo: `mradermacher/Qwen2.5-VL-3B-Instruct-GGUF`
  - default file: `Qwen2.5-VL-3B-Instruct.Q4_K_M.gguf`
  - requires mmproj

Current model behavior:

- First-run setup can auto-download the recommended GLM-OCR profile when no supported model is installed.
- Curated model downloads fetch the main GGUF and companion mmproj where required.
- Downloads use `.part` files and resume with HTTP Range when supported.
- Curated downloads require SHA-256 validation from Hugging Face LFS metadata.
- A local manifest tracks downloaded model metadata.
- Legacy filename-based model discovery remains available for upgrades.
- Experimental custom downloads are allowed only with an explicit `owner/repo/file.gguf` path and are best-effort.

Current runtime profiles:

- CPU compatible
- Auto
- Accelerated if available

Current runtime discovery:

- Searches app and working-directory `bin` and `resources/bin` locations.
- Recognizes `llama-server`, `llama-mtmd-cli`, and `llama-cli`.
- Detects CPU-compatible and accelerated runtime groups such as CUDA, DirectML, Vulkan, and generic accelerated builds.
- Tries persistent `llama-server` first when available.
- Falls back to transient CLI runners when the persistent path fails or is unavailable.
- Keeps a warm persistent worker for lower time to first text.

The native design should preserve runtime profile semantics but move process ownership into the OCR worker boundary.

### Storage, History, and Offline Behavior

Current storage modes:

- Portable mode: app data under `portable-data` beside the executable.
- Installer mode: app data under `%LOCALAPPDATA%\VisiTexta`.

Current app data:

- `settings.json`
- `history.json`
- `models`
- `temp`
- `pasted-inputs`

Native v1 should add the requested local diagnostics and logs locations while keeping the same storage-mode rules:

- `settings`
- `history`
- `models`
- `downloads`
- `temp`
- `logs`
- `diagnostics`
- `pasted-inputs`

Current history stores recent jobs with job id, source, output path, workflow mode, status, progress, error, and message. The native rebuild should expand history records to include file name, mode, date, model, runtime, page count, status, output path, and retry metadata as requested.

### Exports

Current primary output is Markdown next to the source file. The frontend also supports export actions depending on mode:

- Markdown
- TXT
- searchable text-based PDF for Notes
- JSON for Extract
- CSV for Notes flashcards and Extract structured rows/fields

Native v1 should centralize this in `ExportService` instead of splitting export derivation between TypeScript and Rust. Export support should be mode-aware and tested.

### Diagnostics and QA

Current VisiTexta has runtime status reporting and benchmark gates. Benchmarks cover time to first preview, time to first text, total time, peak memory, and normalized output differences for fixture inputs.

Native v1 should add a real Diagnostics/About page:

- App version
- Windows version
- .NET version
- Runtime files detected and missing
- PDFium detected and missing
- Model folder
- Storage mode and paths
- Last errors
- Open logs folder
- Copy diagnostic report

## QuietScribe Patterns to Copy

QuietScribe already has the target native shape:

- `QuietScribe.slnx` solution with layered projects under `/src`
- `App.Desktop` for WinUI 3 pages and app shell
- `App.Models` for immutable records and enums
- `App.Core` for contracts and pure formatting/merge logic
- `App.Inference` for hardware detection and worker clients
- `App.Services` for paths, settings, model downloads, export, history, diagnostics, and storage helpers
- `App.Tests` for focused unit tests
- `workers/transcription-worker` as a separate local worker process
- `scripts/build-release.ps1` for portable release packaging

Product/UI patterns to copy:

- WinUI 3 `Window` with `MicaBackdrop`
- custom title bar using `TitleBar`
- `NavigationView` sidebar
- focused pages rather than one huge frontend file
- shared Fluent-ish card/text styles in `App.xaml`
- calm local-first language
- Models, History, Settings, and diagnostics surfaces
- async UI workflows using `CancellationTokenSource`
- background worker client with JSON-lines parsing
- local history stored as JSON files
- diagnostic report copied to clipboard
- portable zip as the first release artifact

Service patterns to copy:

- `AppServices` static composition root for v1 simplicity
- `JsonFileStore<T>` for atomic JSON persistence
- service interfaces in `App.Core.Contracts`
- model catalog plus local model info objects
- resumable Hugging Face downloads
- delete and open-folder actions
- tests around history, paths, worker protocol parsing, and formatting

Patterns to adapt rather than copy directly:

- QuietScribe's Hugging Face token storage is useful, but VisiTexta's curated OCR models should remain public-first. Add token support only if a curated OCR model later requires gated access.
- QuietScribe downloads whole model repos into per-repo folders. VisiTexta currently downloads specific GGUF and mmproj files into a flat models folder. The native OCR app should keep VisiTexta's file-level GGUF model registry because mmproj pairing and active file selection matter.
- QuietScribe's worker is Python. The OCR worker should be a .NET console process unless a later spike proves a native C++ bridge is necessary. This keeps the rebuild in C#/.NET and avoids adding Python as a core OCR dependency.

## Proposed Native Architecture

### Repository Layout

Target layout:

```text
/src
  /App.Desktop
  /App.Core
  /App.Inference
  /App.Models
  /App.Services
  /App.Tests

/workers
  /ocr-worker

/docs
  architecture.md
  model-support.md
  troubleshooting.md
  release.md
  native-rebuild-plan.md
```

Recommended solution name: `VisiTexta.slnx`.

Recommended worker shape:

```text
/workers/ocr-worker
  OcrWorker.csproj
  Program.cs
  WorkerProtocol.cs
  LlamaRuntimeInvoker.cs
  PdfiumPageRenderer.cs
  ImagePreprocessor.cs
```

The worker should be built and copied into the publish output under:

```text
workers/ocr-worker/ocr-worker.exe
```

Bundled native dependencies should live under predictable runtime folders:

```text
bin/
  cpu/
    llama-server.exe
    llama-mtmd-cli.exe
    pdfium.dll
  accelerated/
    vulkan/
    directml/
```

Portable data should be included in release zips:

```text
portable-data/
  .keep
```

### Layer Responsibilities

`App.Models`

- Domain records and enums only.
- Suggested records:
  - `OcrJob`
  - `OcrJobOptions`
  - `OcrWorkflowMode`
  - `OcrExportFormat`
  - `OcrJobStatus`
  - `OcrWorkerEvent`
  - `OcrPageResult`
  - `OcrHistoryItem`
  - `OcrModelProfile`
  - `LocalOcrModelInfo`
  - `RuntimeProfile`
  - `RuntimeStatus`
  - `StorageMode`
  - `AppSettings`
  - `DiagnosticReport`

`App.Core`

- Contracts and pure logic.
- Suggested contracts:
  - `ISettingsService`
  - `IHistoryService`
  - `IModelRegistryService`
  - `IModelDownloadService`
  - `IExportService`
  - `IDiagnosticsService`
  - `IRuntimeDetectionService`
  - `IStoragePathService`
  - `IOcrWorkerClient`
- Suggested pure logic:
  - output naming
  - Markdown cleanup
  - Notes post-processing
  - Extract post-processing
  - worker event parsing
  - model registry validation

`App.Services`

- File system and app-data services.
- Implement:
  - `SettingsService`
  - `HistoryService`
  - `ModelRegistryService`
  - `ModelDownloadService`
  - `ExportService`
  - `DiagnosticsService`
  - `StoragePathService`
  - JSON store and atomic writes
- The runtime detection contract can live in `App.Core`; implementation can live in `App.Inference` if it needs process probing or DLL loading.

`App.Inference`

- Local process clients and runtime detection.
- Implement:
  - `OcrWorkerClient`
  - `OcrWorkerProtocol`
  - `RuntimeDetectionService`
  - worker crash handling
  - stdout JSON-lines reader
  - stderr log capture
  - cancellation by JSON command first, process kill second

`App.Desktop`

- WinUI 3 shell and pages.
- No direct model download, OCR, export, or storage logic beyond calling services.
- Pages:
  - New OCR
  - Models
  - History
  - Settings
  - Diagnostics/About

`workers/ocr-worker`

- Owns heavy OCR and document processing.
- Renders PDFs with PDFium.
- Preprocesses images.
- Invokes local llama runtime.
- Streams text deltas and page progress.
- Writes or returns outputs according to the app/worker contract.
- Never calls cloud OCR and never talks to model APIs.

### Service Map

`StoragePathService`

- Detect portable vs installed mode.
- Portable triggers:
  - `portable-data` directory beside executable
  - `visitexta-portable.txt` marker beside executable
  - legacy sidecar data from existing VisiTexta where migration is safe
- Installed root:
  - `%LOCALAPPDATA%\VisiTexta`
- Expose exact paths for Settings and Diagnostics pages.
- Ensure folders exist.
- Clean temp folder on startup.

`SettingsService`

- Load and save `settings.json`.
- Defaults:
  - runtime profile: CPU compatible
  - default OCR mode: Exact OCR
  - default export: Markdown
  - DPI: 300
  - max OCR dimension: 1600
  - idle prewarm enabled if persistent runtime is present
  - theme: system
- Store:
  - storage preference
  - selected model profile/file
  - runtime profile
  - default workflow mode
  - default export format
  - preview/performance options

`ModelRegistryService`

- Own curated model definitions.
- Validate registry consistency:
  - one recommended profile
  - each curated profile has id, label, repo, default file
  - mmproj requirement is explicit
  - file markers are non-empty for curated profiles
- Discover local GGUF files and mmproj readiness.
- Mark support tier:
  - Recommended
  - Tested
  - Legacy
  - Experimental
- Keep custom models best-effort and not auto-selected unless compatible.

`ModelDownloadService`

- Download specific GGUF files and required companion mmproj files.
- Resume partial downloads with Range.
- Use `.part` files until verified.
- Verify SHA-256 for curated downloads.
- Update local model manifest.
- Emit download progress to UI.
- Support delete model and open model folder.

`RuntimeDetectionService`

- Search app-local runtime folders.
- Detect:
  - `llama-server.exe`
  - `llama-mtmd-cli.exe`
  - `llama-cli.exe`
  - `pdfium.dll`
- Classify CPU and accelerated runtimes.
- Check likely compatibility for DirectML, CUDA, Vulkan where practical.
- Return a user-readable runtime status and a worker-readable runtime plan.

`ExportService`

- Primary Markdown write next to source file.
- Mode-aware exports:
  - Exact OCR: Markdown, TXT
  - Notes: Markdown, TXT, searchable text PDF, study CSV
  - Extract: Markdown, JSON, CSV, TXT
- Duplicate-safe names for automatic writes.
- Save-as support through WinUI file pickers.
- Atomic file writes.

`HistoryService`

- Store each job as its own JSON file, matching QuietScribe style.
- Fields:
  - id
  - source path
  - source file name
  - mode
  - created/updated dates
  - model profile/file
  - runtime profile/effective runtime
  - pages
  - status
  - progress
  - output path
  - error
  - warnings
  - retry options snapshot
- Actions:
  - open result
  - open folder
  - retry
  - delete history item
- Recover in-progress jobs as failed after app restart.

`DiagnosticsService`

- Build a copyable diagnostic report.
- Include:
  - app version
  - Windows version
  - .NET version
  - process architecture
  - storage mode and all paths
  - model folder contents summary
  - selected model readiness
  - runtime files detected/missing
  - PDFium detected/missing
  - last worker errors
  - recent log file paths

## OCR Workflow

Recommended native flow:

1. UI creates an `OcrJob` from file picker, drag/drop, paste, or retry.
2. `StoragePathService` creates job temp/log locations.
3. `ModelRegistryService` resolves the active model and mmproj.
4. `RuntimeDetectionService` resolves the runtime plan.
5. `ExportService` reserves the duplicate-safe primary Markdown output path.
6. `OcrWorkerClient` starts `ocr-worker.exe` with redirected stdin/stdout/stderr.
7. App sends one JSON-lines `ocr_job` command.
8. Worker renders/preprocesses pages locally.
9. Worker invokes local llama runtime.
10. Worker emits progressive page and text events.
11. UI appends deltas live and updates per-page progress.
12. Worker writes primary Markdown or returns final Markdown for app-side write.
13. App records final history and enables export actions.

Recommended ownership:

- Worker owns PDF rendering, preprocessing, OCR runtime invocation, and streaming page text.
- App owns settings, model download, runtime plan selection, history persistence, export commands, and user-facing errors.
- App should compute output paths. Worker may write the primary Markdown to the provided path, but the app should verify existence after `done` and record the result.

This keeps heavy work isolated while preserving user-visible output behavior.

## Worker Protocol

Use JSON Lines over stdin/stdout. One JSON object per line. Stderr is treated as logs only and captured by `OcrWorkerClient`.

### Commands From App to Worker

Start job:

```json
{
  "command": "ocr_job",
  "protocol_version": 1,
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "source_path": "C:\\Docs\\scan.pdf",
  "output_markdown_path": "C:\\Docs\\scan.ocr.md",
  "temp_dir": "C:\\Users\\me\\AppData\\Local\\VisiTexta\\temp\\job-8f95",
  "log_dir": "C:\\Users\\me\\AppData\\Local\\VisiTexta\\logs",
  "mode": "exact_ocr",
  "prompt_override": null,
  "study_boost": false,
  "extract_template_id": "invoice_receipt",
  "dpi": 300,
  "max_ocr_dimension": 1600,
  "model": {
    "profile_id": "glm-ocr",
    "model_path": "C:\\Users\\me\\AppData\\Local\\VisiTexta\\models\\GLM-OCR.Q4_K_M.gguf",
    "mmproj_path": "C:\\Users\\me\\AppData\\Local\\VisiTexta\\models\\mmproj-GLM-OCR.gguf"
  },
  "runtime": {
    "profile": "cpu_compatible",
    "preferred_server_paths": ["C:\\App\\bin\\cpu\\llama-server.exe"],
    "fallback_cli_paths": ["C:\\App\\bin\\cpu\\llama-mtmd-cli.exe"]
  }
}
```

Cancel job:

```json
{
  "command": "cancel",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c"
}
```

Shutdown worker:

```json
{
  "command": "shutdown"
}
```

### Events From Worker to App

Required event names:

- `job_started`
- `page_started`
- `text_delta`
- `page_done`
- `progress`
- `warning`
- `error`
- `done`

Job started:

```json
{
  "event": "job_started",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "source_path": "C:\\Docs\\scan.pdf",
  "message": "Preparing document"
}
```

Page started:

```json
{
  "event": "page_started",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "page_number": 1,
  "total_pages": 4,
  "preview_image_path": "C:\\...\\page-1.png",
  "message": "Reading page 1 of 4"
}
```

Text delta:

```json
{
  "event": "text_delta",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "page_number": 1,
  "delta": "Recognized text chunk"
}
```

Page done:

```json
{
  "event": "page_done",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "page_number": 1,
  "total_pages": 4,
  "page_markdown": "## Page 1\n\nRecognized text..."
}
```

Progress:

```json
{
  "event": "progress",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "stage": "ocr",
  "percent": 42.5,
  "page_number": 2,
  "total_pages": 4,
  "rendered_pages": 4,
  "recognized_pages": 1,
  "message": "Extracting text from page 2 of 4"
}
```

Warning:

```json
{
  "event": "warning",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "code": "runtime_fallback",
  "message": "Accelerated runtime failed. Falling back to CPU compatible runtime."
}
```

Error:

```json
{
  "event": "error",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "code": "worker_runtime_failed",
  "message": "The local OCR runtime exited before returning text.",
  "recoverable": false
}
```

Done:

```json
{
  "event": "done",
  "job_id": "8f95e2d1-7c3f-44d5-96bb-5ec0e9d9719c",
  "status": "done",
  "pages": 4,
  "output_markdown_path": "C:\\Docs\\scan.ocr.md",
  "elapsed_ms": 38240,
  "warnings": []
}
```

### Worker Crash Handling

`OcrWorkerClient` must:

- Read stdout asynchronously.
- Capture stderr to a capped log buffer and log file.
- Treat invalid JSON as a worker warning or protocol error depending on severity.
- Detect process exit before `done`.
- Mark the active job failed without crashing the UI.
- Include stderr/log excerpt in the user-facing error and diagnostic report.
- Kill the entire process tree on cancellation timeout.

## UI Plan

### Shell

Use the QuietScribe shell pattern:

- Mica backdrop
- app icon and native title bar
- `NavigationView` sidebar
- `Frame` navigation
- shared resources in `App.xaml`
- card-like panels using Fluent resources

Navigation:

- New OCR
- Models
- History
- Settings
- Diagnostics/About

### New OCR Page

Purpose: The main work surface for new OCR jobs.

Expected controls:

- Drag/drop zone for PNG, JPG, JPEG, and PDF
- Browse button
- paste image action
- job queue
- workflow mode segmented control:
  - Exact OCR
  - Notes
  - Extract
- extract preset selector when Extract mode is active
- OCR profile preset selector:
  - Starter
  - Recommended
  - Higher quality
  - Faster
- model selector/status
- runtime selector/status
- progressive result editor/preview
- source preview with per-page navigation
- per-page progress
- cancel/retry actions
- copy result
- export action menu
- open result
- reveal in Explorer

Design should feel like QuietScribe: utilitarian, low-drama, clear controls, and local-first status language.

### Models Page

Purpose: Manage local OCR models.

Expected features:

- Curated OCR model registry
- Recommended GLM-OCR callout
- installed/not installed/runtime-ready status
- mmproj companion status
- download/resume progress
- checksum validation state
- delete model
- open model folder
- advanced custom `owner/repo/file.gguf` download field
- clear support labels:
  - Recommended
  - Tested
  - Legacy
  - Experimental

### History Page

Purpose: Local job record browser.

Expected fields:

- file name
- source path
- mode
- date
- model
- runtime
- pages
- status
- output path
- warnings/errors

Expected actions:

- open result
- open folder
- retry
- delete history item

### Settings Page

Purpose: App-level preferences and path visibility.

Expected controls:

- theme
- storage mode: portable vs installed
- exact settings/history/models/temp/logs/diagnostics paths
- runtime profile:
  - CPU compatible
  - Auto
  - Accelerated if available
- default OCR mode
- default export format
- default DPI/profile
- idle model prewarm
- privacy/local-first explanation

### Diagnostics/About Page

Purpose: Supportability.

Expected controls:

- app version
- Windows version
- .NET version
- runtime files detected/missing
- PDFium detected/missing
- model folder
- last errors
- open logs folder
- copy diagnostic report

## Migration Map

| Current VisiTexta Area | Native Replacement | Notes |
| --- | --- | --- |
| React/Tauri shell | `App.Desktop` WinUI 3 shell | Replace completely. Preserve product concepts, not code. |
| Tauri commands in `lib.rs` | WinUI pages plus services | Commands become service calls and worker client calls. |
| `pipeline.rs` | `OcrWorkerClient`, `ocr-worker`, `ExportService`, `HistoryService` | Move heavy pipeline to worker and keep app responsive. |
| `pdf.rs` | Worker `PdfiumPageRenderer` | Still PDFium-based. Validate `pdfium.dll` in diagnostics. |
| `llm.rs` | Worker `LlamaRuntimeInvoker` plus `RuntimeDetectionService` | Preserve persistent server, CLI fallback, streaming, sanitization, and runtime profile semantics. |
| `models.rs` | `ModelRegistryService` and `ModelDownloadService` | Port curated registry, mmproj pairing, resume, SHA-256 validation, and manifest logic. |
| `storage.rs` | `StoragePathService` | Preserve portable/install behavior and duplicate-safe output naming. |
| `settings.rs` | `SettingsService` | Port defaults and add native settings fields. |
| `history.rs` | `HistoryService` | Expand history schema and store per-job JSON files. |
| `modes.rs` | `App.Models` enums plus `App.Core` post-processing | Preserve Exact OCR, Notes, Extract definitions and export options. |
| `study.rs` | `App.Core` or `App.Services` Notes processor | Port deterministic logic and tests. |
| `extract.rs` | `App.Core` or `App.Services` Extract processor | Port structured metadata, CSV, and verification logic. |
| `formatting.rs` | `App.Core` formatting helpers | Port cleanup behavior and tests. |
| TypeScript export helpers | `ExportService` | Move Markdown/TXT/PDF/JSON/CSV logic to C#. |
| Benchmark CLI | Native test/QA fixtures | Keep benchmark intent; first build can start with unit tests and fixture OCR smoke tests. |

## Migration Risks

1. llama.cpp multimodal CLI/server compatibility

   Current behavior depends on `llama-server`, `llama-mtmd-cli`, `--mmproj`, streaming JSON/SSE formats, and output sanitization. The worker should be built behind a narrow `LlamaRuntimeInvoker` so runtime quirks do not leak into UI code.

2. Progressive output parity

   Users expect text to appear live. The worker protocol must emit `text_delta` as soon as llama produces text and must preserve page ordering.

3. PDFium packaging

   `pdfium.dll` path resolution, architecture match, and redistribution must be verified in Diagnostics and release packaging.

4. Large PDFs and memory use

   Keep lazy rendering, temp files, max image dimensions, and rich preview suppression for large jobs. Avoid loading every rendered page image into UI memory.

5. Model registry and checksum behavior

   The current curated download path is stricter than QuietScribe's whole-repo downloader. Preserve file-level GGUF/mmproj downloads and checksum validation for curated profiles.

6. Portable mode

   Portable path detection must be deterministic. A portable zip should never accidentally write settings/models to `%LOCALAPPDATA%` when `portable-data` is present.

7. Export parity

   Notes PDF, Notes CSV, Extract JSON, and Extract CSV currently rely on frontend helpers plus Rust PDF export. Native `ExportService` needs coverage before UI polish.

8. History migration

   Existing `history.json` and settings may need one-time migration. If migration is deferred, v1 must at least avoid corrupting old files.

9. Tesseract artifacts

   The current repo carries `resources/tessdata`, but the audited OCR path uses local GGUF vision models, not Tesseract. Do not make Tesseract a new v1 dependency unless a follow-up audit finds a real user-facing path that needs it.

10. UI freezing

   The app must never block the UI on model downloads, PDF rendering, OCR, checksum validation, or worker startup. All such operations need async service APIs and cancellation.

## Milestones

### Milestone 0 - Planning Gate

- Add this report.
- Confirm scope: native WinUI 3, local worker, no cloud OCR, no chat.

Exit criteria:

- `docs/native-rebuild-plan.md` exists and explains inventory, architecture, worker protocol, risks, milestones, and v1 exclusions.

### Milestone 1 - Native Solution Skeleton

- Create `VisiTexta.slnx`.
- Add projects:
  - `App.Desktop`
  - `App.Core`
  - `App.Inference`
  - `App.Models`
  - `App.Services`
  - `App.Tests`
  - `workers/ocr-worker`
- Add WinUI shell with NavigationView and pages.
- Add shared Fluent resources in `App.xaml`.

Exit criteria:

- `dotnet build VisiTexta.slnx` passes.
- App launches to New OCR page.

### Milestone 2 - Domain Models and Core Services

- Add settings, paths, history, model registry, export, diagnostics, runtime status contracts.
- Implement `StoragePathService`, `SettingsService`, `HistoryService`, `ModelRegistryService`.
- Port duplicate-safe output naming.

Exit criteria:

- Unit tests pass for paths, settings round-trip, history persistence, output naming, model registry validation.

### Milestone 3 - Worker Protocol and Mock Worker

- Implement worker protocol parser in `App.Inference`.
- Implement worker process client.
- Add a mock mode or test worker that emits deterministic events.
- Wire New OCR UI to stream mock progress/text.

Exit criteria:

- Worker event parsing tests pass.
- UI remains responsive during mock OCR.
- Worker crash is shown as a failed job, not an app crash.

### Milestone 4 - PDF/Image Local Processing

- Implement worker PDFium renderer.
- Implement image preprocessing and max dimension handling.
- Support PNG, JPG, JPEG, and PDF.
- Emit per-page progress and preview paths.

Exit criteria:

- Fixture image and PDF jobs produce page events.
- Missing PDFium yields useful Diagnostics and UI errors.

### Milestone 5 - llama Runtime Integration

- Implement runtime detection.
- Implement `llama-server` persistent path.
- Implement CLI fallback path.
- Implement output sanitization and first-token/text-delta events.
- Resolve model/mmproj paths from registry.

Exit criteria:

- GLM-OCR can OCR a local image.
- PDF pages stream text progressively.
- Runtime fallback warning works.
- Cancellation terminates worker/runtime process tree.

### Milestone 6 - Workflow Modes and Exports

- Port Exact OCR cleanup.
- Port Notes mode.
- Port Extract mode and templates.
- Implement Markdown/TXT/PDF/JSON/CSV exports.

Exit criteria:

- Mode tests pass.
- Export tests pass.
- UI export menu only shows formats valid for the active mode.

### Milestone 7 - Models Page

- Implement curated registry UI.
- Implement model downloads with resume and checksum validation.
- Implement mmproj companion downloads.
- Implement delete and open folder.

Exit criteria:

- GLM-OCR download resumes and validates.
- Missing mmproj is clearly shown.
- Model status updates without restarting the app.

### Milestone 8 - History, Settings, Diagnostics

- Complete History page actions.
- Complete Settings page path/runtime/defaults controls.
- Complete Diagnostics/About page and copy report.
- Add local logs.

Exit criteria:

- User can retry a job from history.
- Diagnostics report contains enough info for support.
- Portable and installed path modes are visibly correct.

### Milestone 9 - Portable Release

- Add `scripts/build-release.ps1` similar to QuietScribe.
- Publish self-contained WinUI app.
- Copy worker, runtimes, PDFium, docs, and `portable-data`.
- Produce portable zip.
- Do not ship installer by default until signing strategy is decided.

Exit criteria:

- Clean machine portable zip launches.
- Portable app writes data under `portable-data`.
- Worker and PDFium are detected from the packaged layout.

### Milestone 10 - QA and Regression

- Add focused unit tests.
- Add worker protocol tests.
- Add model registry/download validation tests with mocked HTTP where practical.
- Add fixture-based OCR smoke tests when local runtimes/models are available.
- Add release QA script.

Exit criteria:

- Build passes.
- Unit tests pass.
- App does not freeze during OCR.
- Portable release works.

## Test Plan

Required unit tests:

- `StoragePathService` portable vs installed mode
- duplicate-safe output naming
- settings load/save defaults
- history persistence and delete
- interrupted job recovery
- model registry validation
- model/mmproj readiness detection
- model download planner for curated profiles
- checksum validation behavior
- runtime detection classification
- worker event parsing
- worker crash/error handling
- Exact OCR cleanup
- Notes post-processing
- Extract structured output
- Markdown/TXT/PDF/JSON/CSV export formatting

Recommended integration tests:

- mock worker streams image OCR events
- mock worker streams multi-page PDF events
- cancellation kills worker process
- missing worker executable gives useful error
- missing model gives useful error
- missing PDFium gives useful error

Manual QA:

- Fresh installed mode launch
- Fresh portable zip launch
- First-run recommended model download
- Resume interrupted model download
- OCR single image
- OCR multi-page PDF
- Exact OCR, Notes, and Extract modes
- all applicable exports
- history retry/open/delete
- runtime profile switching
- Diagnostics copy/open logs

## Intentionally Not Included in v1

- Cloud OCR
- AI chat
- account system or sync
- installer-first distribution
- unsigned installer promotion before signing strategy
- remote document storage
- telemetry
- training or fine-tuning models
- guaranteed support for arbitrary GGUF vision models beyond best-effort custom downloads
- a Tauri or React shell
- Python as a required OCR runtime dependency
- Tesseract as a new OCR path unless a follow-up product decision explicitly adds it
- mobile/macOS/Linux targets
- automatic GPU driver installation

## Open Decisions Before Implementation

1. Confirm the worker language as C#/.NET console. This is the recommended path for consistency with the requested stack.
2. Choose the PDFium .NET binding and packaging layout after a small spike.
3. Decide whether v1 migrates existing Tauri `settings.json` and `history.json`, or simply preserves them untouched and starts a new native schema.
4. Decide whether model files stay flat under `models` as today or move to per-profile folders. Recommendation: keep flat files for v1 to reduce migration risk.
5. Decide whether the worker writes primary Markdown or returns final content for app-side writing. Recommendation: app computes output path, worker writes primary Markdown, app verifies and records it.
