// Agent module — bLOVErAi + bQueenBee interfaces
// bLOVErAi: private companion, 1:1 bonded, never leaves the machine
// bQueenBee: public agent, RBI seat, ATProto voice
// SPDX-License-Identifier: AGPL-3.0-only
//
// Z-3: framed sidecar IPC (newline-delimited JSON, id-correlated, fail-closed)
// Z-4: TransactionQuote moved to crate::quote
// Z-6: Mutex<ChildStdin> for &self async writes + SidecarError::Sidecar
// Z-7: start_with_command for injectable sidecar command

use crate::quote::TransactionQuote;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex};

// ──────────────────────────────────────────────────────────
// Sidecar error type
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarError {
    NotRunning,
    Timeout,
    WriteFailed(String),
    /// The sidecar itself reported an error in its JSON response.
    Sidecar(String),
}

impl std::fmt::Display for SidecarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SidecarError::NotRunning => write!(f, "LLM sidecar is not running"),
            SidecarError::Timeout => write!(f, "LLM sidecar request timed out"),
            SidecarError::WriteFailed(msg) => write!(f, "Failed to write to sidecar: {}", msg),
            SidecarError::Sidecar(msg) => write!(f, "Sidecar error: {}", msg),
        }
    }
}

impl std::error::Error for SidecarError {}

// ──────────────────────────────────────────────────────────
// Internal protocol types (newline-delimited JSON)
// ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
struct SidecarRequest {
    id: u64,
    prompt: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SidecarResponse {
    pub id: u64,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

// ──────────────────────────────────────────────────────────
// LLM sidecar — supervised process, async IPC
// ──────────────────────────────────────────────────────────

pub type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>;

pub struct LlmSidecar {
    /// Writer behind Mutex so &self can lock and write without &mut self.
    /// Locked only around the write — never held across the oneshot await.
    writer: Option<Mutex<ChildStdin>>,
    child: Option<Child>,
    next_id: AtomicU64,
    pending: PendingMap,
    model_name: String,
    timeout_duration: Duration,
}

impl LlmSidecar {
    /// Start the sidecar with an injectable command (for testing).
    ///
    /// The sidecar reads newline-delimited JSON from stdin and writes
    /// newline-delimited JSON to stdout. Each request has a monotonic `id`;
    /// the sidecar MUST echo the same `id` in its response.
    pub async fn start_with_command(
        program: &str,
        args: &[&str],
        timeout: Duration,
        label: &str,
    ) -> Result<Self, String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start sidecar: {}", e))?;

        let stdin = child.stdin.take().ok_or("Failed to capture sidecar stdin")?;
        let stdout = child.stdout.take().ok_or("Failed to capture sidecar stdout")?;

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        tokio::spawn(async move {
            reader_task(stdout, pending_clone).await;
        });

        Ok(LlmSidecar {
            writer: Some(Mutex::new(stdin)),
            child: Some(child),
            next_id: AtomicU64::new(1),
            pending,
            model_name: label.to_string(),
            timeout_duration: timeout,
        })
    }

    /// Start the local LLM sidecar (GLM-5.2 via Colibri).
    /// Thin wrapper around start_with_command.
    pub async fn start(
        model_path: &str,
        ram_limit_gb: u32,
        timeout_secs: u64,
    ) -> Result<Self, String> {
        Self::start_with_command(
            "python3",
            &[
                "-m",
                "colibri",
                "--model",
                model_path,
                "--ram-limit",
                &ram_limit_gb.to_string(),
            ],
            Duration::from_secs(timeout_secs),
            model_path,
        )
        .await
    }

    /// Send a prompt to the LLM and get a response.
    ///
    /// Async and correlated — multiple concurrent calls each get their own
    /// correct response via id matching. If the child dies, all pending and
    /// future calls return NotRunning. If no response within timeout, returns
    /// Timeout.
    pub async fn infer(&self, prompt: &str) -> Result<String, SidecarError> {
        let writer = self.writer.as_ref().ok_or(SidecarError::NotRunning)?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        // Send request as newline-delimited JSON
        let request = SidecarRequest {
            id,
            prompt: prompt.to_string(),
        };
        let json_line = serde_json::to_string(&request)
            .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;

        // Lock ONLY around the write — never hold across the oneshot await
        {
            let mut w = writer.lock().await;
            w.write_all(format!("{}\n", json_line).as_bytes())
                .await
                .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
            w.flush()
                .await
                .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
            // Lock dropped here — concurrent callers can proceed
        }

        // Wait for response with timeout
        match tokio::time::timeout(self.timeout_duration, rx).await {
            Ok(Ok(Ok(text))) => Ok(text),
            Ok(Ok(Err(msg))) => Err(SidecarError::Sidecar(msg)), // sidecar reported error
            Ok(Err(_)) => Err(SidecarError::NotRunning), // sender dropped — child died
            Err(_) => {
                // Timeout — remove from pending map
                let mut map = self.pending.lock().await;
                map.remove(&id);
                Err(SidecarError::Timeout)
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.writer.is_some()
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Stop the sidecar — kills the child process.
    pub async fn stop(&mut self) {
        self.writer.take(); // Drop writer → signal EOF

        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        // Drop all pending senders → receivers get error → NotRunning
        let mut map = self.pending.lock().await;
        map.clear();
    }
}

impl Drop for LlmSidecar {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill(); // Best-effort kill (fail-closed)
        }
    }
}

/// Reader task — continuously reads lines from sidecar stdout and dispatches
/// responses to waiting callers via oneshot channels.
///
/// When stdout closes (child died), the task exits. All pending senders in the
/// map are dropped, causing waiting receivers to get a RecvError which maps
/// to NotRunning.
async fn reader_task(stdout: ChildStdout, pending: PendingMap) {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF — child stdout closed
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<SidecarResponse>(trimmed) {
                    Ok(resp) => {
                        let mut map = pending.lock().await;
                        if let Some(sender) = map.remove(&resp.id) {
                            let result = if let Some(text) = resp.text {
                                Ok(text)
                            } else if let Some(err) = resp.error {
                                Err(err)
                            } else {
                                Err("sidecar response has neither text nor error".to_string())
                            };
                            let _ = sender.send(result);
                        }
                        // If id not in map, the request already timed out — ignore
                    }
                    Err(_) => continue, // Malformed JSON — skip, don't crash reader
                }
            }
            Err(_) => break, // Read error — treat as child death
        }
    }

    // Reader exiting — clean up all pending requests
    let mut map = pending.lock().await;
    map.clear(); // Drops all senders → receivers get RecvError → NotRunning
}

// ──────────────────────────────────────────────────────────
// bLOVErAi — private companion (1:1 bonded to one human)
// ──────────────────────────────────────────────────────────

pub struct Bloverai {
    llm: Option<LlmSidecar>,
    context_path: String,
}

impl Bloverai {
    pub fn new(context_path: &str) -> Self {
        Bloverai {
            llm: None,
            context_path: context_path.to_string(),
        }
    }

    pub async fn start_llm(
        &mut self,
        model_path: &str,
        ram_limit_gb: u32,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let sidecar = LlmSidecar::start(model_path, ram_limit_gb, timeout_secs).await?;
        self.llm = Some(sidecar);
        Ok(())
    }

    pub async fn chat(&mut self, message: &str) -> Result<String, SidecarError> {
        if let Some(ref llm) = self.llm {
            llm.infer(message).await
        } else {
            Err(SidecarError::NotRunning)
        }
    }

    /// Simulate a transaction and generate a quote.
    /// bLOVErAi never signs — the human signs after the quote.
    /// Simulation stays unimplemented in this task.
    pub fn simulate_transaction(&self, _action: &str) -> Result<TransactionQuote, String> {
        Err("Transaction simulation not yet implemented".to_string())
    }
}
