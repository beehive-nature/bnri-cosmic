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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    ///
    /// The `Option` is inside the Mutex, not outside, so `stop(&self)` can take
    /// the writer and drop it (signalling EOF) without `&mut self`.
    writer: Mutex<Option<ChildStdin>>,
    /// G-2: `Mutex<Option<Child>>` so the child can be killed and reaped through
    /// a shared reference. The app holds the sidecar in an `Arc` — that is the
    /// point of `infer(&self)` being concurrent — and a `&mut self` stop could
    /// never be called through one.
    child: Mutex<Option<Child>>,
    next_id: AtomicU64,
    pending: PendingMap,
    /// Truthful liveness. Cleared by `stop()` AND by `reader_task` when the
    /// child's stdout hits EOF or errors — i.e. when the sidecar dies on its
    /// own, which nothing else observes.
    ///
    /// This exists because the obvious implementation lies. Reading
    /// `writer.is_some()` reports liveness of *our handle*, not of the child:
    /// only `stop()` ever takes the writer, so a child that crashes or is killed
    /// externally leaves the writer `Some` and `is_running()` answering `true`
    /// about a dead process. An `AtomicBool` is also readable without `.await`,
    /// which keeps `is_running()` synchronous and non-blocking.
    alive: Arc<AtomicBool>,
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

        // The reader owns the only view of the child's death that arrives
        // without us asking, so it carries the flag.
        let alive = Arc::new(AtomicBool::new(true));
        let alive_clone = Arc::clone(&alive);

        tokio::spawn(async move {
            reader_task(stdout, pending_clone, alive_clone).await;
        });

        Ok(LlmSidecar {
            writer: Mutex::new(Some(stdin)),
            child: Mutex::new(Some(child)),
            next_id: AtomicU64::new(1),
            pending,
            alive,
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
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        // Register before writing, so a response arriving between the write and
        // the await below still finds its sender.
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        // From here on, `id` is in the pending map. Every early return must
        // remove it first — a `?` that skips the removal strands this request's
        // oneshot::Sender in the map for the life of the process.
        if let Err(e) = self.send_request(id, prompt).await {
            self.pending.lock().await.remove(&id);
            return Err(e);
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

    /// Serialize and write one request. Split out of `infer` so that every
    /// failure path returns an error rather than a `?` that would bypass the
    /// pending-map cleanup at the call site.
    async fn send_request(&self, id: u64, prompt: &str) -> Result<(), SidecarError> {
        let request = SidecarRequest {
            id,
            prompt: prompt.to_string(),
        };
        let json_line = serde_json::to_string(&request)
            .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;

        // Lock ONLY around the write — never held across the oneshot await.
        let mut guard = self.writer.lock().await;
        let w = guard.as_mut().ok_or(SidecarError::NotRunning)?;
        w.write_all(format!("{}\n", json_line).as_bytes())
            .await
            .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
        w.flush()
            .await
            .map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
        Ok(())
        // Lock dropped here — concurrent callers can proceed
    }

    /// Whether the sidecar is actually running.
    ///
    /// Truthful in both directions: `false` after `stop()`, and `false` once the
    /// child dies on its own (the reader clears the flag on EOF or read error).
    /// Synchronous and non-blocking — reading an atomic needs no lock and no
    /// `.await`, so a UI thread can call this freely.
    pub fn is_running(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Stop the sidecar — kills and reaps the child process.
    ///
    /// G-2: takes `&self` so it is callable through an `Arc`. The app holds the
    /// sidecar in an `Arc` (that is what makes `infer(&self)` concurrent), so a
    /// `&mut self` stop could never be reached — the production API could not
    /// stop its own sidecar.
    ///
    /// Order matters: the writer is dropped first to signal EOF, then the child
    /// is killed and reaped, and only then is the pending map cleared. Clearing
    /// the map drops every `oneshot::Sender`, so each waiting `infer()` observes
    /// a closed channel and resolves to `NotRunning` rather than hanging.
    pub async fn stop(&self) {
        // Flag first: a concurrent is_running() must never report a sidecar we
        // have already begun tearing down.
        self.alive.store(false, Ordering::SeqCst);

        self.writer.lock().await.take(); // Drop writer → signal EOF

        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
            let _ = child.wait().await; // reap — no zombie
        }

        // Drop all pending senders → receivers get RecvError → NotRunning
        let mut map = self.pending.lock().await;
        map.clear();
    }
}

impl Drop for LlmSidecar {
    fn drop(&mut self) {
        // Best-effort only. `get_mut()` needs no lock — `&mut self` in Drop
        // proves no other holder exists — so this stays sync and cannot block.
        // It does not reap; a caller that needs the child reaped calls `stop()`.
        if let Some(child) = self.child.get_mut().as_mut() {
            let _ = child.start_kill(); // fail-closed: try to kill on the way out
        }
    }
}

/// Reader task — continuously reads lines from sidecar stdout and dispatches
/// responses to waiting callers via oneshot channels.
///
/// When stdout closes (child died), the task exits. All pending senders in the
/// map are dropped, causing waiting receivers to get a RecvError which maps
/// to NotRunning.
async fn reader_task(stdout: ChildStdout, pending: PendingMap, alive: Arc<AtomicBool>) {
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

    // Reader exiting means stdout closed or errored: the child is gone. This is
    // the only place that observes a death we did not cause — a crash, an
    // external kill, the process exiting on its own — so clearing the flag here
    // is what makes is_running() truthful rather than merely post-stop correct.
    // Idempotent with stop(), which also clears it.
    alive.store(false, Ordering::SeqCst);

    // Clean up all pending requests
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
