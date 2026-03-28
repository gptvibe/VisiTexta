use crate::errors::{PipelineError, Result};
use crate::events::RunnerMode;
use base64::Engine;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const OCR_MAX_TOKENS: &str = "2048";
const OCR_CONTEXT_SIZE: &str = "8192";
const PROCESS_LOG_LIMIT: usize = 64 * 1024;
const SERVER_STARTUP_TIMEOUT: Duration = Duration::from_secs(90);
const SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const SERVER_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const SERVER_PROBE_INTERVAL: Duration = Duration::from_millis(250);
const OCR_SERVER_HOST: &str = "127.0.0.1";

static ANSI_ESCAPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").expect("ansi regex"));
static PERSISTENT_WORKERS: Lazy<Mutex<PersistentWorkerManager>> =
    Lazy::new(|| Mutex::new(PersistentWorkerManager::default()));

const FILTERED_STDOUT_PREFIXES: &[&str] = &[
    "build ",
    "model ",
    "modalities ",
    "available commands",
    "loaded media from",
    "you are an ocr engine",
    "warn:",
    "main: loading model",
    "encoding image",
    "decoding image",
    "llama_perf_context_print",
];

#[derive(Debug, Clone)]
pub enum OcrStreamEvent {
    WorkerStarting {
        mode: RunnerMode,
        message: String,
    },
    ModelReady {
        mode: RunnerMode,
        message: String,
    },
    FirstToken {
        mode: RunnerMode,
    },
    TextChunk {
        mode: RunnerMode,
        chunk: String,
    },
    RunnerError {
        mode: RunnerMode,
        message: String,
        will_fallback: bool,
    },
}

#[derive(Clone)]
pub struct LlmOcrEngine {
    runner_paths: Vec<PathBuf>,
    server_paths: Vec<PathBuf>,
    mmproj_path: Option<PathBuf>,
    model_path: PathBuf,
    threads: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerConfig {
    runner_path: PathBuf,
    model_path: PathBuf,
    mmproj_path: Option<PathBuf>,
    threads: u16,
}

#[derive(Default)]
struct PersistentWorkerManager {
    current: Option<PersistentWorker>,
    disabled_config: Option<WorkerConfig>,
}

struct PersistentWorker {
    config: WorkerConfig,
    port: u16,
    child: Child,
    client: Client,
    stdout_log: Arc<Mutex<String>>,
    stderr_log: Arc<Mutex<String>>,
    stdout_handle: Option<JoinHandle<()>>,
    stderr_handle: Option<JoinHandle<()>>,
}

enum StreamPipeMessage {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    StdoutClosed,
    StderrClosed,
}

#[derive(Default)]
struct StreamingSanitizer {
    current_line: String,
    emitted_bytes_in_line: usize,
    pending_blank_lines: usize,
    seen_content: bool,
}

impl LlmOcrEngine {
    pub fn new(model_path: PathBuf, threads: u16) -> Result<Self> {
        if !model_path.exists() {
            return Err(PipelineError::Llm(format!(
                "model file not found: {}",
                model_path.to_string_lossy()
            )));
        }

        let runtime_requirements = crate::models::resolve_runtime_model_requirements(&model_path)
            .map_err(|err| PipelineError::Llm(err.to_string()))?;

        let runner_paths = resolve_runner_exes(cli_runner_names());
        let server_paths = resolve_runner_exes(server_runner_names());
        if runner_paths.is_empty() && server_paths.is_empty() {
            return Err(PipelineError::Llm(
                "no multimodal-compatible llama runtime found".into(),
            ));
        }

        Ok(Self {
            runner_paths,
            server_paths,
            mmproj_path: runtime_requirements.mmproj_path,
            model_path,
            threads,
        })
    }

    pub fn recognize_streaming<F>(
        &self,
        image_path: &Path,
        prompt: &str,
        mut on_event: F,
    ) -> Result<String>
    where
        F: FnMut(OcrStreamEvent),
    {
        let effective_prompt = format!(
            "You are an OCR engine. Read all visible text from the provided image. {} Return only markdown output and no extra explanation.",
            prompt.trim()
        );

        if let Some(server_path) = self.server_paths.first() {
            let config = WorkerConfig {
                runner_path: server_path.clone(),
                model_path: self.model_path.clone(),
                mmproj_path: self.mmproj_path.clone(),
                threads: self.threads.max(1),
            };

            let should_try_persistent = {
                let manager = PERSISTENT_WORKERS.lock();
                manager.can_attempt(&config)
            };

            if should_try_persistent {
                let mut manager = PERSISTENT_WORKERS.lock();
                match manager.recognize(config, image_path, &effective_prompt, &mut on_event) {
                    Ok(output) => return Ok(output),
                    Err(err) => on_event(OcrStreamEvent::RunnerError {
                        mode: RunnerMode::Persistent,
                        message: err.to_string(),
                        will_fallback: !self.runner_paths.is_empty(),
                    }),
                }
            }
        }

        let mut last_error: Option<PipelineError> = None;

        for (index, runner) in self.runner_paths.iter().enumerate() {
            match self.try_recognize_with_runner(
                runner,
                image_path,
                &effective_prompt,
                &mut on_event,
            ) {
                Ok(output) => return Ok(output),
                Err(err) => {
                    let will_fallback = index + 1 < self.runner_paths.len();
                    on_event(OcrStreamEvent::RunnerError {
                        mode: RunnerMode::Transient,
                        message: err.to_string(),
                        will_fallback,
                    });
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            PipelineError::Llm("no OCR runner was able to produce output".into())
        }))
    }

    fn try_recognize_with_runner<F>(
        &self,
        runner: &Path,
        image_path: &Path,
        effective_prompt: &str,
        on_event: &mut F,
    ) -> Result<String>
    where
        F: FnMut(OcrStreamEvent),
    {
        on_event(OcrStreamEvent::WorkerStarting {
            mode: RunnerMode::Transient,
            message: format!("Launching {}", runner.to_string_lossy()),
        });

        let mut cmd = Command::new(runner);
        cmd.arg("-m")
            .arg(&self.model_path)
            .arg("--image")
            .arg(image_path)
            .arg("-p")
            .arg(effective_prompt)
            .arg("-n")
            .arg(OCR_MAX_TOKENS)
            .arg("--temp")
            .arg("0")
            .arg("--ctx-size")
            .arg(OCR_CONTEXT_SIZE)
            .arg("--no-warmup")
            .arg("--threads")
            .arg(self.threads.max(1).to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(mmproj) = &self.mmproj_path {
            cmd.arg("--mmproj").arg(mmproj);
        }

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            PipelineError::Llm(format!(
                "spawn failed for {}: {e}",
                runner.to_string_lossy()
            ))
        })?;

        on_event(OcrStreamEvent::ModelReady {
            mode: RunnerMode::Transient,
            message: format!("Runner started: {}", runner.to_string_lossy()),
        });

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PipelineError::Llm("failed to capture llama runner stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| PipelineError::Llm("failed to capture llama runner stderr".into()))?;

        let (tx, rx) = mpsc::channel::<StreamPipeMessage>();
        let stdout_handle = spawn_child_pipe_reader(stdout, tx.clone(), true);
        let stderr_handle = spawn_child_pipe_reader(stderr, tx, false);

        let mut raw_stdout = String::new();
        let mut stderr_bytes = Vec::new();
        let mut sanitizer = StreamingSanitizer::default();
        let mut first_token_emitted = false;
        let mut stdout_closed = false;
        let mut stderr_closed = false;

        while !(stdout_closed && stderr_closed) {
            match rx.recv() {
                Ok(StreamPipeMessage::Stdout(bytes)) => {
                    let chunk = String::from_utf8_lossy(&bytes).into_owned();
                    raw_stdout.push_str(&chunk);
                    for delta in sanitizer.push_chunk(&chunk) {
                        if !delta.is_empty() {
                            if !first_token_emitted {
                                on_event(OcrStreamEvent::FirstToken {
                                    mode: RunnerMode::Transient,
                                });
                                first_token_emitted = true;
                            }
                            on_event(OcrStreamEvent::TextChunk {
                                mode: RunnerMode::Transient,
                                chunk: delta,
                            });
                        }
                    }
                }
                Ok(StreamPipeMessage::Stderr(bytes)) => stderr_bytes.extend_from_slice(&bytes),
                Ok(StreamPipeMessage::StdoutClosed) => stdout_closed = true,
                Ok(StreamPipeMessage::StderrClosed) => stderr_closed = true,
                Err(_) => break,
            }
        }

        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        for delta in sanitizer.finish() {
            if !delta.is_empty() {
                if !first_token_emitted {
                    on_event(OcrStreamEvent::FirstToken {
                        mode: RunnerMode::Transient,
                    });
                    first_token_emitted = true;
                }
                on_event(OcrStreamEvent::TextChunk {
                    mode: RunnerMode::Transient,
                    chunk: delta,
                });
            }
        }

        let status = child
            .wait()
            .map_err(|e| PipelineError::Llm(format!("wait failed: {e}")))?;
        let stderr = sanitize_stderr(&String::from_utf8_lossy(&stderr_bytes));

        if !status.success() {
            let stdout = raw_stdout.trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "no output captured".to_string()
            };
            return Err(PipelineError::Llm(format!(
                "{} exited with {}: {}",
                runner.to_string_lossy(),
                status,
                detail
            )));
        }

        let cleaned_output = sanitize_model_stdout(&raw_stdout);

        if cleaned_output.trim().is_empty() {
            let detail = if !stderr.is_empty() {
                format!(" stderr: {}", stderr)
            } else {
                String::new()
            };
            return Err(PipelineError::Llm(format!(
                "{} produced empty output. This usually means the runtime invocation is incompatible.{}",
                runner.to_string_lossy(),
                detail
            )));
        }

        Ok(cleaned_output)
    }
}

impl PersistentWorkerManager {
    fn can_attempt(&self, config: &WorkerConfig) -> bool {
        self.disabled_config
            .as_ref()
            .map(|disabled| disabled != config)
            .unwrap_or(true)
    }

    fn recognize<F>(
        &mut self,
        config: WorkerConfig,
        image_path: &Path,
        prompt: &str,
        on_event: &mut F,
    ) -> Result<String>
    where
        F: FnMut(OcrStreamEvent),
    {
        if !self.can_attempt(&config) {
            return Err(PipelineError::Llm(
                "persistent OCR worker is disabled for the active model".into(),
            ));
        }

        let started_now = self.ensure_worker(&config, on_event)?;
        let worker = self
            .current
            .as_mut()
            .ok_or_else(|| PipelineError::Llm("persistent OCR worker is unavailable".into()))?;

        let ready_message = if started_now {
            "Persistent OCR worker is ready".to_string()
        } else {
            "Reusing warm OCR worker".to_string()
        };
        on_event(OcrStreamEvent::ModelReady {
            mode: RunnerMode::Persistent,
            message: ready_message,
        });

        let result = worker.stream_request(image_path, prompt, on_event);
        if result.is_err() {
            self.disabled_config = Some(config);
            self.shutdown_current_worker();
        } else {
            self.disabled_config = None;
        }
        result
    }

    fn ensure_worker<F>(&mut self, config: &WorkerConfig, on_event: &mut F) -> Result<bool>
    where
        F: FnMut(OcrStreamEvent),
    {
        if self
            .current
            .as_ref()
            .map(|worker| worker.config != *config)
            .unwrap_or(false)
        {
            self.shutdown_current_worker();
        }

        if let Some(worker) = self.current.as_mut() {
            if worker.is_alive()? {
                return Ok(false);
            }
            self.shutdown_current_worker();
        }

        let worker = PersistentWorker::spawn(config.clone(), on_event)?;
        self.current = Some(worker);
        Ok(true)
    }

    fn shutdown_current_worker(&mut self) {
        if let Some(mut worker) = self.current.take() {
            worker.shutdown();
        }
    }

    fn reset(&mut self) {
        self.shutdown_current_worker();
        self.disabled_config = None;
    }
}

impl PersistentWorker {
    fn spawn<F>(config: WorkerConfig, on_event: &mut F) -> Result<Self>
    where
        F: FnMut(OcrStreamEvent),
    {
        on_event(OcrStreamEvent::WorkerStarting {
            mode: RunnerMode::Persistent,
            message: format!(
                "Starting warm OCR worker via {}",
                config.runner_path.to_string_lossy()
            ),
        });

        let port = reserve_local_port()?;
        let mut cmd = Command::new(&config.runner_path);
        cmd.arg("-m")
            .arg(&config.model_path)
            .arg("--host")
            .arg(OCR_SERVER_HOST)
            .arg("--port")
            .arg(port.to_string())
            .arg("-n")
            .arg(OCR_MAX_TOKENS)
            .arg("--temp")
            .arg("0")
            .arg("--ctx-size")
            .arg(OCR_CONTEXT_SIZE)
            .arg("--no-warmup")
            .arg("--threads")
            .arg(config.threads.max(1).to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(mmproj) = &config.mmproj_path {
            cmd.arg("--mmproj").arg(mmproj);
        }

        #[cfg(target_os = "windows")]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            PipelineError::Llm(format!(
                "failed to start persistent OCR worker {}: {e}",
                config.runner_path.to_string_lossy()
            ))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            PipelineError::Llm("failed to capture persistent worker stdout".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            PipelineError::Llm("failed to capture persistent worker stderr".into())
        })?;

        let stdout_log = Arc::new(Mutex::new(String::new()));
        let stderr_log = Arc::new(Mutex::new(String::new()));
        let stdout_handle = spawn_log_drain_thread(stdout, Arc::clone(&stdout_log));
        let stderr_handle = spawn_log_drain_thread(stderr, Arc::clone(&stderr_log));

        let client = Client::builder()
            .connect_timeout(SERVER_PROBE_TIMEOUT)
            .build()
            .map_err(|e| PipelineError::Llm(format!("failed to create OCR client: {e}")))?;

        let mut worker = Self {
            config,
            port,
            child,
            client,
            stdout_log,
            stderr_log,
            stdout_handle: Some(stdout_handle),
            stderr_handle: Some(stderr_handle),
        };

        if let Err(err) = worker.wait_until_ready() {
            worker.shutdown();
            return Err(err);
        }

        Ok(worker)
    }

    fn wait_until_ready(&mut self) -> Result<()> {
        let deadline = Instant::now() + SERVER_STARTUP_TIMEOUT;
        let props_url = format!("http://{}:{}/props", OCR_SERVER_HOST, self.port);

        while Instant::now() < deadline {
            if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| PipelineError::Llm(format!("failed to probe OCR worker: {e}")))?
            {
                let detail = self.log_excerpt();
                return Err(PipelineError::Llm(format!(
                    "persistent OCR worker exited with {status}: {detail}"
                )));
            }

            match self
                .client
                .get(&props_url)
                .timeout(SERVER_PROBE_TIMEOUT)
                .send()
            {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(_) | Err(_) => thread::sleep(SERVER_PROBE_INTERVAL),
            }
        }

        Err(PipelineError::Llm(format!(
            "persistent OCR worker did not become ready within {} seconds: {}",
            SERVER_STARTUP_TIMEOUT.as_secs(),
            self.log_excerpt()
        )))
    }

    fn stream_request<F>(
        &mut self,
        image_path: &Path,
        prompt: &str,
        on_event: &mut F,
    ) -> Result<String>
    where
        F: FnMut(OcrStreamEvent),
    {
        let image_data = base64::engine::general_purpose::STANDARD.encode(
            fs::read(image_path)
                .map_err(|e| PipelineError::Llm(format!("failed to read OCR input image: {e}")))?,
        );

        let url = format!("http://{}:{}/completion", OCR_SERVER_HOST, self.port);
        let payload = json!({
            "prompt": format!("Image: [img-1]\n{prompt}"),
            "image_data": [
                {
                    "id": 1,
                    "data": image_data,
                }
            ],
            "n_predict": 2048,
            "temperature": 0.0,
            "stream": true,
            "cache_prompt": false,
        });

        let response = self
            .client
            .post(url)
            .timeout(SERVER_REQUEST_TIMEOUT)
            .json(&payload)
            .send()
            .map_err(|e| PipelineError::Llm(format!("persistent OCR request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let detail = body.trim();
            let suffix = if detail.is_empty() {
                self.log_excerpt()
            } else {
                detail.to_string()
            };
            return Err(PipelineError::Llm(format!(
                "persistent OCR request returned {status}: {suffix}"
            )));
        }

        let mut output = String::new();
        let mut first_token_emitted = false;
        let mut reader = BufReader::new(response);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .map_err(|e| PipelineError::Llm(format!("failed to read OCR stream: {e}")))?;
            if bytes_read == 0 {
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let payload = trimmed
                .strip_prefix("data:")
                .map(str::trim)
                .unwrap_or(trimmed);
            if payload == "[DONE]" {
                break;
            }

            let value: Value = match serde_json::from_str(payload) {
                Ok(value) => value,
                Err(_) => continue,
            };

            if let Some(chunk) = extract_stream_chunk(&value) {
                if !chunk.is_empty() {
                    if !first_token_emitted {
                        on_event(OcrStreamEvent::FirstToken {
                            mode: RunnerMode::Persistent,
                        });
                        first_token_emitted = true;
                    }

                    output.push_str(&chunk);
                    on_event(OcrStreamEvent::TextChunk {
                        mode: RunnerMode::Persistent,
                        chunk,
                    });
                }
            }

            if value.get("stop").and_then(Value::as_bool).unwrap_or(false) {
                break;
            }
        }

        let cleaned_output = sanitize_model_stdout(&output);
        if cleaned_output.trim().is_empty() {
            return Err(PipelineError::Llm(
                "persistent OCR worker returned empty output".into(),
            ));
        }

        Ok(cleaned_output)
    }

    fn is_alive(&mut self) -> Result<bool> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|e| PipelineError::Llm(format!("failed to check OCR worker status: {e}")))
    }

    fn log_excerpt(&self) -> String {
        let stderr_log = self.stderr_log.lock();
        let stderr = sanitize_stderr(stderr_log.as_str());
        if !stderr.is_empty() {
            return stderr;
        }
        drop(stderr_log);

        let stdout_log = self.stdout_log.lock();
        let stdout = sanitize_stderr(stdout_log.as_str());
        if !stdout.is_empty() {
            return stdout;
        }

        "no worker logs captured".to_string()
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();

        if let Some(handle) = self.stdout_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.stderr_handle.take() {
            let _ = handle.join();
        }
    }
}

impl StreamingSanitizer {
    fn push_chunk(&mut self, raw: &str) -> Vec<String> {
        let normalized = strip_ansi(raw).replace("\r\n", "\n").replace('\r', "\n");
        let mut deltas = Vec::new();

        for ch in normalized.chars() {
            if ch == '\n' {
                self.finish_line(&mut deltas);
            } else {
                self.current_line.push(ch);
                self.emit_partial_line(&mut deltas);
            }
        }

        deltas
    }

    fn finish(&mut self) -> Vec<String> {
        let mut deltas = Vec::new();
        if !self.current_line.is_empty() {
            self.emit_completed_line(false, &mut deltas);
        }
        self.pending_blank_lines = 0;
        deltas
    }

    fn finish_line(&mut self, deltas: &mut Vec<String>) {
        self.emit_completed_line(true, deltas);
        self.current_line.clear();
        self.emitted_bytes_in_line = 0;
    }

    fn emit_completed_line(&mut self, append_newline: bool, deltas: &mut Vec<String>) {
        let line = self.current_line.clone();
        if should_drop_completed_output_line(&line) {
            return;
        }

        if line.trim().is_empty() {
            if self.seen_content {
                self.pending_blank_lines += 1;
            }
            return;
        }

        self.flush_pending_blank_lines(deltas);

        if self.emitted_bytes_in_line < line.len() {
            deltas.push(line[self.emitted_bytes_in_line..].to_string());
        }
        if append_newline {
            deltas.push("\n".to_string());
        }

        self.seen_content = true;
        self.emitted_bytes_in_line = line.len();
    }

    fn emit_partial_line(&mut self, deltas: &mut Vec<String>) {
        if should_hold_partial_output_line(&self.current_line) {
            return;
        }
        if self.current_line.trim().is_empty() {
            return;
        }

        self.flush_pending_blank_lines(deltas);

        if self.emitted_bytes_in_line < self.current_line.len() {
            deltas.push(self.current_line[self.emitted_bytes_in_line..].to_string());
            self.emitted_bytes_in_line = self.current_line.len();
            self.seen_content = true;
        }
    }

    fn flush_pending_blank_lines(&mut self, deltas: &mut Vec<String>) {
        if self.pending_blank_lines > 0 {
            deltas.push("\n".repeat(self.pending_blank_lines));
            self.pending_blank_lines = 0;
        }
    }
}

pub fn runtime_has_ocr_runner() -> bool {
    !resolve_runner_exes(cli_runner_names()).is_empty()
        || !resolve_runner_exes(server_runner_names()).is_empty()
}

pub fn shutdown_persistent_worker() {
    let mut manager = PERSISTENT_WORKERS.lock();
    manager.reset();
}

fn cli_runner_names() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["llama-mtmd-cli.exe", "llama-cli.exe"]
    }
    #[cfg(not(target_os = "windows"))]
    {
        &["llama-mtmd-cli", "llama-cli"]
    }
}

fn server_runner_names() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["llama-server.exe"]
    }
    #[cfg(not(target_os = "windows"))]
    {
        &["llama-server"]
    }
}

fn spawn_child_pipe_reader<R>(
    mut reader: R,
    tx: mpsc::Sender<StreamPipeMessage>,
    is_stdout: bool,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = [0_u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => {
                    let message = if is_stdout {
                        StreamPipeMessage::Stdout(buf[..read].to_vec())
                    } else {
                        StreamPipeMessage::Stderr(buf[..read].to_vec())
                    };
                    if tx.send(message).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }

        let _ = tx.send(if is_stdout {
            StreamPipeMessage::StdoutClosed
        } else {
            StreamPipeMessage::StderrClosed
        });
    })
}

fn spawn_log_drain_thread<R>(mut reader: R, log: Arc<Mutex<String>>) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = [0_u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => append_capped_log(log.as_ref(), &String::from_utf8_lossy(&buf[..read])),
                Err(_) => break,
            }
        }
    })
}

fn append_capped_log(log: &Mutex<String>, chunk: &str) {
    let mut buffer = log.lock();
    buffer.push_str(chunk);

    if buffer.len() <= PROCESS_LOG_LIMIT {
        return;
    }

    let overflow = buffer.len().saturating_sub(PROCESS_LOG_LIMIT);
    let drain_to = buffer
        .char_indices()
        .map(|(idx, _)| idx)
        .find(|idx| *idx >= overflow)
        .unwrap_or(buffer.len());
    buffer.drain(..drain_to);
}

fn reserve_local_port() -> Result<u16> {
    TcpListener::bind((OCR_SERVER_HOST, 0))
        .map_err(|e| PipelineError::Llm(format!("failed to reserve OCR worker port: {e}")))?
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| PipelineError::Llm(format!("failed to read OCR worker port: {e}")))
}

fn extract_stream_chunk(value: &Value) -> Option<String> {
    value
        .get("content")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .or_else(|| {
            value
                .pointer("/choices/0/text")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .or_else(|| {
            value
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
        .or_else(|| {
            value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
        })
}

fn sanitize_stderr(raw: &str) -> String {
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn sanitize_model_stdout(raw: &str) -> String {
    let stripped = strip_ansi(raw);

    let mut lines: Vec<String> = stripped
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| line.to_string())
        .collect();

    lines.retain(|line| !should_drop_completed_output_line(line));

    while lines.first().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }

    lines.join("\n")
}

fn strip_ansi(raw: &str) -> String {
    ANSI_ESCAPE_RE.replace_all(raw, "").into_owned()
}

fn should_drop_completed_output_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if FILTERED_STDOUT_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }

    if trimmed.starts_with('/') {
        return true;
    }

    trimmed.chars().all(|c| matches!(c, '▄' | '▀' | '█' | ' '))
}

fn should_hold_partial_output_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    if FILTERED_STDOUT_PREFIXES
        .iter()
        .any(|prefix| prefix.starts_with(&lower))
    {
        return true;
    }

    if trimmed.starts_with('/') {
        return true;
    }

    trimmed.chars().all(|c| matches!(c, '▄' | '▀' | '█' | ' '))
}

fn resolve_runner_exes(names: &[&str]) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

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
            if candidate.exists() && seen.insert(candidate.clone()) {
                found.push(candidate.to_path_buf());
            }
        }
    }

    found
}
