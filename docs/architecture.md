# Architecture

VisiTexta Native follows the same native Windows shape as QuietScribe while keeping VisiTexta's OCR product scope.

```text
/src
  /App.Desktop    WinUI 3 shell and pages
  /App.Core       contracts, output naming, formatting, workflow processors
  /App.Inference  runtime detection and OCR worker client
  /App.Models     domain records and enums
  /App.Services   settings, storage, history, models, exports, diagnostics
  /App.Tests      focused unit tests
/workers
  /ocr-worker     separate local OCR worker process
```

The desktop app owns user state, paths, model downloads, history, exports, and diagnostics. Heavy OCR work is isolated behind `IOcrWorkerClient` and a JSON-lines worker protocol so worker crashes do not crash the UI.

The current implementation includes the native shell, service layer, worker protocol, buildable worker process, model registry/download plumbing, and test coverage for core persistence/protocol behavior. The worker renders PDF pages locally through PDFium, sends PNG/JPG/PDF page images through the bundled llama multimodal CLI, streams JSON-lines progress/text events, and fails with recoverable local setup errors when a runtime, model, mmproj, or PDFium file is missing.
