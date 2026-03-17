use crate::errors::{PipelineError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct LlmOcrEngine {
    runner_path: PathBuf,
    mmproj_path: Option<PathBuf>,
    model_path: PathBuf,
    threads: u16,
}

impl LlmOcrEngine {
    pub fn new(model_path: PathBuf, threads: u16) -> Result<Self> {
        if !model_path.exists() {
            return Err(PipelineError::Llm(format!(
                "model file not found: {}",
                model_path.to_string_lossy()
            )));
        }

        let model_name = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let use_qwen2vl = model_name.contains("qwen") && model_name.contains("-vl");
        let runner_path = if use_qwen2vl {
            resolve_runner_exe(&["llama-mtmd-cli.exe", "llama-cli.exe"]).ok_or_else(|| {
                PipelineError::Llm("llama-mtmd-cli executable not found".into())
            })?
        } else {
            resolve_runner_exe(&["llama-cli.exe"]) 
                .ok_or_else(|| PipelineError::Llm("llama-cli executable not found".into()))?
        };

        let mmproj_path = if use_qwen2vl {
            Some(
                resolve_mmproj_path(&model_path)
                    .ok_or_else(|| PipelineError::Llm("mmproj model not found for Qwen2-VL".into()))?,
            )
        } else {
            None
        };

        Ok(Self {
            runner_path,
            mmproj_path,
            model_path,
            threads,
        })
    }

    pub fn recognize(&self, image_path: &Path, prompt: &str) -> Result<String> {
        let effective_prompt = format!(
            "You are an OCR engine. Read all visible text from the provided image. {} Return only markdown output and no extra explanation.",
            prompt.trim()
        );

        let mut cmd = Command::new(&self.runner_path);
        cmd
            .arg("-m")
            .arg(&self.model_path)
            .arg("--image")
            .arg(image_path)
            .arg("-p")
            .arg(effective_prompt)
            .arg("-n")
            .arg("2048")
            .arg("--temp")
            .arg("0")
            .arg("--ctx-size")
            .arg("8192")
            .arg("--threads")
            .arg(self.threads.max(1).to_string());

        if let Some(mmproj) = &self.mmproj_path {
            cmd.arg("--mmproj").arg(mmproj);
        }

        let output = cmd
            .output()
            .map_err(|e| PipelineError::Llm(format!("spawn failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "no output captured".to_string()
            };
            return Err(PipelineError::Llm(format!(
                "llama-cli exited with {}: {}",
                output.status,
                detail
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| PipelineError::Llm(format!("utf8 decode failed: {e}")))
    }
}

pub fn runtime_has_llama_cli() -> bool {
    resolve_runner_exe(&["llama-mtmd-cli.exe", "llama-cli.exe"]).is_some()
}

fn resolve_runner_exe(names: &[&str]) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    for name in names {
        let candidates = [
            exe_dir.as_ref().map(|p| p.join("bin").join(name)),
            exe_dir
                .as_ref()
                .map(|p| p.join("resources").join("bin").join(name)),
            exe_dir.as_ref().map(|p| p.join(name)),
            Some(PathBuf::from("src-tauri").join("bin").join(name)),
            Some(PathBuf::from(name)),
        ];

        for candidate in candidates.iter().flatten() {
            if candidate.exists() {
                return Some(candidate.to_path_buf());
            }
        }
    }

    None
}

fn resolve_mmproj_path(model_path: &Path) -> Option<PathBuf> {
    let parent = model_path.parent()?;
    let mut found = Vec::new();
    for entry in std::fs::read_dir(parent).ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if name.starts_with("mmproj") && name.ends_with(".gguf") {
            found.push(path);
        }
    }

    found.sort_by_key(|path| {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name.contains("f16") {
            0
        } else if name.contains("bf16") {
            1
        } else if name.contains("f32") {
            2
        } else {
            3
        }
    });

    found.into_iter().next()
}