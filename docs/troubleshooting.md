# Troubleshooting

## No OCR Runtime Found

Put local llama runtime files under one of these folders:

- `bin`
- `resources/bin`
- packaged app `bin`
- packaged app `resources/bin`

Useful files:

- `llama-server.exe`
- `llama-mtmd-cli.exe`
- `llama-cli.exe`

## PDFium Missing

PDF OCR needs `pdfium.dll` beside the runtime files. Open Diagnostics to see the exact locations searched.

## Model Is Downloaded But Not Ready

Most curated OCR models need a companion `mmproj` GGUF. Open Models and redownload the profile so the companion file is fetched.

## Portable Mode Not Active

Create `portable-data` or `visitexta-portable.txt` beside `VisiTexta.exe` before launch. Settings, history, models, temp files, logs, and diagnostics will then stay beside the portable app.

## Worker Failed

Open Diagnostics and copy the report. It includes app paths, runtime detection, PDFium status, and recent errors.
