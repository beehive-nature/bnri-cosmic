// Z-7: Sidecar IPC tests — real infer() calls against fake sidecars.
// Receipt rule: these tests must compile and pass.
// SPDX-License-Identifier: AGPL-3.0-only

use bnri_cosmic::agent::{LlmSidecar, SidecarError};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;

/// Fake sidecar that echoes each request's prompt back as "echo:{prompt}".
const ECHO_SIDECAR: &str = r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
        resp = {"id": req["id"], "text": f"echo:{req['prompt']}"}
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
    except Exception as e:
        sys.stderr.write(f"error: {e}\n")
        sys.stderr.flush()
"#;

/// Silent sidecar that reads but never responds.
const SILENT_SIDECAR: &str = r#"
import sys, time
for line in sys.stdin:
    time.sleep(999)
"#;

/// 100 concurrent `infer()` calls against an echo sidecar. Each response must
/// match its OWN prompt — that is what pins id-correlation, rather than
/// first-response-wins.
#[tokio::test]
async fn test_concurrent_correlation_100_calls() {
    let sidecar = LlmSidecar::start_with_command(
        "python3",
        &["-c", ECHO_SIDECAR],
        Duration::from_secs(30),
        "echo-test",
    )
    .await
    .expect("failed to start echo sidecar");

    // Arc so each spawned task can hold the sidecar: `infer(&self)` is
    // concurrent by design, and tokio::spawn needs 'static.
    let sidecar = Arc::new(sidecar);

    // Declared with the type the spawned futures actually return — the tuple,
    // not the bare inner Result.
    let mut tasks: JoinSet<(u64, Result<String, SidecarError>, String)> = JoinSet::new();

    for i in 0..100u64 {
        let prompt = format!("prompt_{}", i);
        let expected = format!("echo:{}", prompt);
        let sc = Arc::clone(&sidecar);

        tasks.spawn(async move {
            let result = sc.infer(&prompt).await;
            (i, result, expected)
        });
    }

    let mut completed = Vec::new();
    while let Some(res) = tasks.join_next().await {
        let (i, result, expected) = res.expect("infer task panicked");
        match result {
            Ok(text) => {
                assert_eq!(text, expected, "correlation mismatch for prompt_{}", i);
                completed.push(i);
            }
            Err(e) => panic!("prompt_{} failed: {:?}", i, e),
        }
    }

    assert_eq!(completed.len(), 100, "all 100 concurrent calls must succeed");
}

/// `stop()` through the `Arc` mid-flight must resolve a pending `infer()` to
/// `NotRunning` promptly, rather than leaving it to the inference timeout.
///
/// The assertion window (1s) is far below the sidecar's configured 120s
/// timeout, so a pass here cannot be the timeout firing instead.
///
/// `sc_clone` is still held by the spawned task while `stop()` runs, so the
/// Arc refcount is > 1 and `Drop` does not run: this pins `stop()` itself
/// doing the work, which is the whole point of G-2.
#[tokio::test]
async fn test_stop_through_arc_resolves_pending_to_not_running() {
    let sidecar = LlmSidecar::start_with_command(
        "python3",
        &["-c", SILENT_SIDECAR],
        Duration::from_secs(120),
        "silent-test",
    )
    .await
    .expect("failed to start silent sidecar");

    let sc = Arc::new(sidecar);

    // Start an infer() the silent sidecar will never answer.
    let sc_clone = Arc::clone(&sc);
    let infer_handle = tokio::spawn(async move { sc_clone.infer("hello").await });

    // Let the request reach the child before stopping it.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Stop through the Arc — the call `stop(&mut self)` made impossible.
    sc.stop().await;

    let result = tokio::time::timeout(Duration::from_secs(1), infer_handle).await;

    match result {
        Ok(Ok(Err(SidecarError::NotRunning))) => {
            // Correct — the pending call resolved once stop() dropped its sender.
        }
        Ok(Ok(Err(e))) => panic!("expected NotRunning, got {:?}", e),
        Ok(Ok(Ok(text))) => panic!("silent sidecar cannot have answered: {:?}", text),
        Ok(Err(e)) => panic!("infer task panicked: {:?}", e),
        Err(_) => panic!("pending call did not resolve within 1s of stop() — hang detected"),
    }
}

/// `is_running()` must tell the truth after `stop()`.
///
/// Built around the liveness-before-death rule: the probe runs BEFORE the
/// assertion under test. If the host has no working interpreter the child is
/// already dead, the reader has already cleared the flag, and this fails at the
/// probe — loudly, naming the reason — instead of the post-stop assertion
/// passing because a corpse and a stopped sidecar report identically.
///
/// The defect this pins: reading `writer.is_some()` reports our handle, not the
/// child. Only `stop()` takes the writer, so a child that dies on its own leaves
/// `is_running()` answering `true` about a dead process.
#[tokio::test]
async fn test_is_running_truthful_after_stop() {
    let sidecar = LlmSidecar::start_with_command(
        "python3",
        &["-c", SILENT_SIDECAR],
        Duration::from_secs(120),
        "liveness-test",
    )
    .await
    .expect("failed to start silent sidecar");

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        sidecar.is_running(),
        "sidecar must be alive before stop() — if this fails, the host has no \
         working interpreter and every later assertion here would pass vacuously"
    );

    sidecar.stop().await;

    assert!(
        !sidecar.is_running(),
        "is_running() must report false after stop()"
    );
}

/// A silent sidecar must produce `Timeout` at the configured duration.
#[tokio::test]
async fn test_timeout_fires() {
    let sidecar = LlmSidecar::start_with_command(
        "python3",
        &["-c", SILENT_SIDECAR],
        Duration::from_secs(2), // 2 second timeout for test
        "silent-timeout-test",
    )
    .await
    .expect("failed to start silent sidecar for timeout test");

    let result = sidecar.infer("hello").await;

    assert!(
        matches!(result, Err(SidecarError::Timeout)),
        "expected Timeout, got {:?}",
        result
    );
}
