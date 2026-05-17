# Model Support

VisiTexta Native keeps a curated GGUF vision-model registry.

Recommended:

- GLM-OCR
- Repo: `mradermacher/GLM-OCR-GGUF`
- Default file: `GLM-OCR.Q4_K_M.gguf`
- Requires companion `mmproj`

Tested alternatives:

- Qwen2-VL OCR 2B: `mradermacher/Qwen2-VL-OCR-2B-Instruct-GGUF`
- Qwen2.5-VL 3B: `mradermacher/Qwen2.5-VL-3B-Instruct-GGUF`

Downloads are stored under the active app-data `models` folder. Curated downloads fetch the main GGUF and a companion `mmproj` when required, resume `.part` files when possible, and verify SHA-256 when Hugging Face LFS metadata provides a hash.

Advanced custom downloads must use `owner/repo/file.gguf`. Custom models are treated as best-effort and are not promoted over curated profiles.
