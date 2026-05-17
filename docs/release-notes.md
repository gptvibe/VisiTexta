# Release Notes

## 3.0.7

VisiTexta 3.0.7 reshapes the native Windows app into a transcript-first workspace instead of splitting OCR and history into separate destinations.

Highlights:

- The native shell sidebar now keeps recent transcripts visible while you work, so reopening a past OCR run no longer requires a separate History page.
- The main OCR workspace now loads selected transcript history back into the editor/output pane on the right, closer to a ChatGPT or Codex-style layout.
- Run OCR actions move to the top of the workspace so starting a new job is visible immediately instead of being buried below the configuration stack.
- First launch now auto-downloads the recommended local OCR model when no runtime-ready model is installed, using the same native curated downloader and progress reporting as the Models page.

## 3.0.6

VisiTexta 3.0.6 improves the native app shell, model setup, and live OCR responsiveness after the 3.0.5 startup fix.

Highlights:

- The native shell now uses a darker Codex-style sidebar, neutral window chrome, tighter cards, and clearer active navigation.
- GLM-OCR downloads now prefer the smaller `GLM-OCR.mmproj-Q8_0.gguf` companion file, so the recommended setup reaches a ready state faster.
- The Models page now distinguishes installed, incomplete, and ready model states, and shows `Finish setup` when only the companion file is missing.
- Model download progress is throttled to keep the UI responsive during large GGUF downloads.
- The OCR worker now forwards llama stdout deltas while the runtime is still running instead of waiting for the whole process to exit.
- OCR output appends text without rebuilding the whole text box content for every chunk.

## 3.0.5

VisiTexta 3.0.5 fixes the New OCR page crash introduced while wiring native streaming output.

Highlights:

- The New OCR page no longer uses unsupported `Grid.Padding` XAML, so WinUI can instantiate the page at startup.
- Other native pages now avoid the same runtime-only XAML parser failure when opened from the sidebar.
- Portable release builds now fail fast if required WinUI `.xbf` or `.pri` resources are not staged.
- Streaming OCR deltas still append into the output box while the worker reports progress.
- Native package metadata and model-download user agent are aligned with the 3.0.5 patch release.

## 3.0.4

VisiTexta 3.0.4 fixes a native WinUI startup failure that left the process running without ever showing a window on some Windows systems.

Highlights:

- The main window shell is now constructed in code instead of depending on startup-time XAML parsing for the first window.
- Startup failure details are now written to `portable-data\diagnostics\startup-errors.log` so native launch problems do not fail silently.
- The portable release path is validated against the actual packaged app, not just the raw publish output.

## 3.0.3

VisiTexta 3.0.3 makes the native WinUI desktop app the sole documented release path.

Highlights:

- README and release documentation now point packagers to `./scripts/build-release.ps1` for Windows-native shipping builds.
- The legacy Tauri app under `app/` is now explicitly marked as reference/dev-only and is no longer presented as the active release target.
- Native version metadata is aligned at 3.0.3 so the packaged app and native download user agent identify the same release.

## 2.0.2

VisiTexta 2.0.2 finishes the desktop release path with deterministic release QA on the release workstation.

Highlights:

- Release QA now captures benchmark command output robustly and stops stale OCR worker processes before build and gate steps, avoiding Windows file-lock failures during validation.
- Cold and warm OCR regression baselines are documented and maintained as release-workstation envelope baselines, so the gate stays meaningful without flapping on ordinary local variance.
- Packaging and publishing metadata are updated for the 2.0.2 release.

## 2.0.1

VisiTexta 2.0.1 is a focused patch release for the Tauri desktop app.

Highlights:

- New OCR runs now switch the workspace to the live OCR pane immediately, so streaming text is visible while extraction is still in progress.
- Starting a new job now clears the previous selection first, allowing the first incoming stream event to claim the preview instead of waiting for the final invoke result.
- Packaging metadata is updated for the 2.0.1 release.

## 2.0.0

VisiTexta 2.0.0 is a local-first Windows OCR desktop app for images and PDFs. It ships with a bundled local runtime path, supports first-run model download when no supported model is already installed, and keeps OCR output next to the original source file instead of hiding it inside app storage.

Highlights:

- First-run setup now auto-downloads the recommended GLM-OCR profile when no curated OCR model is available yet.
- Storage behavior is explicit. Installer builds use `%LOCALAPPDATA%\VisiTexta\`, while portable builds keep settings, history, models, temp files, and pasted inputs in `portable-data\` beside the executable.
- Runtime profiles are now clearer:
  `CPU compatible` is the widest-compatibility default.
  `Auto` prefers a compatible accelerated runtime when one is bundled.
  `Accelerated if available` tries the accelerated runtime first, then falls back cleanly when needed.
- The preview workspace is calmer on laptop screens. Jobs now use segmented views for Original, OCR, Notes / Extract, and Export, with a compact status bar for model, runtime, storage mode, and progress.
- Notes workflow adds source-linked page references, plus Markdown, plain text, and searchable text-based PDF export.
- Extract workflow adds invoice / receipt, table-to-CSV, meeting photo / whiteboard, and contract key-point presets. Presets return readable Markdown plus structured JSON, and CSV where it applies. Extract output also includes an uncertainty / verification section for fields that may need manual review.
- Release QA now includes benchmark gating. Warm and cold OCR benchmark baselines live under `app/benchmarks/baselines/`, and `npm --prefix app run release:qa` runs build checks plus regression gates for time-to-first-text, total runtime, and peak memory.

Packaging notes:

- `npm --prefix app run tauri:build` now uses the gated release path.
- `npm --prefix app run tauri:build:installer` and `npm --prefix app run tauri:build:portable` also run release QA before packaging.
- `unsafe` script variants remain available for local packaging work when you explicitly want to bypass the release gate.
