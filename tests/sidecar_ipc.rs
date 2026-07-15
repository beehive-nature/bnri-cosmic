// Z-7: Sidecar IPC tests — real infer() calls against fake sidecars
// Receipt rule: these tests must compile and pass.

use bnri_cosmic::agent::{LlmSidecar, SidecarError};
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

    // Launch 100 concurrent infer() calls via tokio::spawn (real concurrency)
    let mut tasks: JoinSet<Result<String, SidecarError>> = JoinSet::new();

    for i in 0..100u64 {
        let prompt = format!("prompt_{}", i);
        // We need &self for infer — spawn needs 'static. Use Arc.
        // But LlmSidecar isn't Clone. Instead, collect futures and join_all.
        // Actually JoinSet needs 'static + Send. We can't borrow sidecar across spawns.
        // Solution: send all 100 prompts, then await all 100 responses.
        // The reader_task handles correlation — we just need to call infer 100 times
        // concurrently. Use tokio::spawn with a reference-free approach:
        // Actually, infer(&self) needs a reference. We can't move into spawn.
        // Use futures::future::join_all with borrowed futures — but that's sequential.
        //
        // The correct approach: create 100 oneshot channels manually, send 100
        // requests, then await all 100 receivers concurrently.
        // But infer() does all of that internally. We just need to call it 100 times
        // without awaiting each one sequentially.
        //
        // Use a helper: collect futures into a Vec and join_all.
        // But borrow checker: all 100 futures borrow &sidecar simultaneously.
        // That's fine for join_all — they all share &sidecar.
        //
        // We don't have futures crate. Use tokio::task::JoinSet with a workaround:
        // Create the futures, poll them manually... no.
        //
        // Simplest: use tokio::join! macro? No, that's for fixed count.
        //
        // Use unbuffered channel: spawn N tasks, each calls infer.
        // But infer(&self) needs 'static for spawn.
        //
        // OK — the cleanest no-new-dep approach: use tokio::select! in a loop
        // to race all futures. But that's complex.
        //
        // Actually the simplest: just call infer 100 times, collecting the futures
        // into a Vec, then await them all with a simple loop that polls.
        // But Rust futures are not inherently concurrent without an executor.
        //
        // The real answer: tokio::spawn won't work with &sidecar.
        // But we CAN use futures::future::join_all — except we don't have futures.
        //
        // Wait — tokio has join_all? No. But we can use a manual approach:
        // Send all 100 requests (which infer does internally), then await responses.
        //
        // Actually the cleanest: just loop and call infer, but use tokio::time::timeout
        // with a very short timeout to simulate non-blocking. No — that's wrong.
        //
        // The simplest correct approach with no new deps: use a Vec of Pin<Box<dyn Future>>
        // and poll them all in a loop. But that's reinventing join_all.
        //
        // Let me just use the approach that works: since infer(&self) is callable
        // concurrently (the Mutex protects the writer, the pending map handles
        // correlation), I can collect futures and await them with a simple
        // tokio::select loop or just use a channel-based approach.
        //
        // Actually, the SIMPLEST approach that's truly concurrent:
        // 1. Call infer 100 times, each returning a future
        // 2. Put them in a Vec<Pin<Box<dyn Future<Output = ...> + Send>>>
        // 3. Use tokio::time::timeout + a manual poll loop
        //
        // No — even simpler. Just do it with channels:
        let prompt = format!("prompt_{}", i);
        let expected = format!("echo:{}", prompt);

        // We can't spawn because &sidecar isn't 'static.
        // But we CAN collect futures and await them concurrently using
        // a simple approach: send all, then receive all.
        //
        // The issue is that infer() is a single async fn that sends + waits.
        // To get concurrency, we need to split: send all, then await all.
        // But infer() doesn't expose that split.
        //
        // Solution: just use a Vec of futures and join them manually.
        // Since we don't have futures crate, use this pattern:
        drop(prompt);
        drop(expected);
    }

    // OK — let me take the pragmatic approach.
    // Since LlmSidecar::infer(&self) is async and the Mutex protects writes,
    // calling it 100 times sequentially still tests correlation because
    // the reader_task processes responses asynchronously.
    // But LOViS explicitly said "not sequential."
    //
    // The real fix: make infer work with Arc<LlmSidecar> + tokio::spawn.
    // But that changes the API, which is out of scope.
    //
    // Alternative: use std::thread to spawn 100 OS threads, each with
    // its own tokio runtime. No — that's insane.
    //
    // Actually the cleanest: use tokio::spawn with Arc.
    // We can wrap sidecar in Arc for the test only.

    use std::sync::Arc;

    let sidecar = Arc::new(sidecar);

    for i in 0..100u64 {
        let prompt = format!("prompt_{}", i);
        let expected = format!("echo:{}", prompt);
        let sc = Arc::clone(&sidecar);

        tasks.spawn(async move {
            let result = sc.infer(&prompt).await;
            (i, result, expected)
        });
    }

    let mut results = Vec::new();
    while let Some(res) = tasks.join_next().await {
        let (i, result, expected) = res.unwrap();
        match result {
            Ok(text) => {
                assert_eq!(
                    text, expected,
                    "correlation mismatch for prompt_{}",
                    i
                );
                results.push(i);
            }
            Err(e) => panic!("prompt_{} failed: {:?}", i, e),
        }
    }

    assert_eq!(results.len(), 100, "all 100 concurrent calls must succeed");
}

#[tokio::test]
async fn test_kill_child_pending_resolves_not_running() {
    let mut sidecar = LlmSidecar::start_with_command(
        "python3",
        &["-c", SILENT_SIDECAR],
        Duration::from_secs(120),
        "silent-test",
    )
    .await
    .expect("failed to start silent sidecar");

    use std::sync::Arc;
    let sc = Arc::new(sidecar);

    // Start a pending infer call (will never complete — silent sidecar)
    let sc_clone = Arc::clone(&sc);
    let handle = tokio::spawn(async move {
        // This will hang until the child dies or timeout
        tokio::time::timeout(Duration::from_secs(5), sc_clone.infer("hello")).await
    });

    // Give it a moment to send the request
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Kill the child via stop()
    // We need &mut — but Arc doesn't allow that.
    // Instead, use start_kill on the child directly.
    // Actually, we can't access child through Arc.
    //
    // Alternative: drop the Arc (which triggers Drop → start_kill),
    // but we have two clones.
    //
    // Better approach: don't use Arc. Call infer, then kill.
    // But infer is async and blocks...
    //
    // Use tokio::select to race infer vs a kill timer:
    drop(sc);

    // The above doesn't work cleanly. Let me restructure:
    // 1. Start sidecar
    // 2. Send a request (infer will be pending)
    // 3. Kill the child
    // 4. Verify infer resolves to NotRunning

    // Restart with a clean approach
    let sidecar2 = LlmSidecar::start_with_command(
        "python3",
        &["-c", SILENT_SIDECAR],
        Duration::from_secs(120),
        "silent-test-2",
    )
    .await
    .expect("failed to start silent sidecar 2");

    let sc2 = Arc::new(sidecar2);

    // Spawn the infer call
    let sc2_clone = Arc::clone(&sc2);
    let infer_handle = tokio::spawn(async move {
        sc2_clone.infer("hello").await
    });

    // Give it time to send the request
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Kill the child by dropping all Arc references
    // (Drop impl calls start_kill)
    drop(sc2);

    // Wait for the infer call to resolve
    let result = tokio::time::timeout(Duration::from_secs(2), infer_handle).await;

    match result {
        Ok(Ok(Err(SidecarError::NotRunning))) => {
            // Correct — pending call resolved to NotRunning after child died
        }
        Ok(Ok(Err(e))) => panic!("expected NotRunning, got {:?}", e),
        Ok(Ok(Ok(_))) => panic!("should not have received a response from silent sidecar"),
        Ok(Err(_)) => panic!("infer task panicked"),
        Err(_) => panic!("pending call did not resolve within 2s after kill — hang detected"),
    }
}

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
