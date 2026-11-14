//! Contract tests for MirrorBridge — verify HTTP shape without mirror-agent.
//!
//! Uses raw tokio TcpListener + hand-written HTTP responses instead of
//! an axum test server, to avoid adding axum as a dev-dependency.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Mock that returns the given status and (optionally) writes new content
/// to `post_write` before responding. The write simulates what a real
/// mirror-agent turn does — the bridge's "unchanged from pre-delegation"
/// check requires the file content to actually differ after the turn.
async fn spawn_http_mock_impl(
    status_line: &'static str,
    post_write: Option<(PathBuf, String)>,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let post_write = Arc::new(post_write);

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let post_write = post_write.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let mut header_end = 0;
                let mut content_length = 0usize;
                loop {
                    let n = socket.read(&mut buf[header_end..]).await.unwrap_or(0);
                    if n == 0 { return; }
                    header_end += n;
                    let s = String::from_utf8_lossy(&buf[..header_end]);
                    if let Some(pos) = s.find("\r\n\r\n") {
                        for line in s[..pos].lines() {
                            let lower = line.to_ascii_lowercase();
                            if let Some(v) = lower.strip_prefix("content-length: ") {
                                content_length = v.trim().parse().unwrap_or(0);
                            }
                        }
                        let already_read = header_end - (pos + 4);
                        if already_read >= content_length { break; }
                    }
                }

                if let Some((path, content)) = post_write.as_ref() {
                    let _ = tokio::fs::write(path, content).await;
                }

                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

async fn spawn_http_mock(status_line: &'static str) -> SocketAddr {
    spawn_http_mock_impl(status_line, None).await
}

async fn spawn_http_mock_with_file(
    status_line: &'static str,
    path: PathBuf,
    content: String,
) -> SocketAddr {
    spawn_http_mock_impl(status_line, Some((path, content))).await
}

fn write_target(dir: &std::path::Path, name: &str, bytes: usize) -> String {
    let path = dir.join(name);
    std::fs::write(&path, "x".repeat(bytes)).unwrap();
    path.to_string_lossy().to_string()
}

fn temp_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "mirror_bridge_test_{}_{}",
        std::process::id(),
        n
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn success_204_plus_file_passes() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    // Pre-delegation content: small stub.
    let target = write_target(&dir, "deliverable.md", 50);
    // Mock writes new content — this is what a real mirror-agent turn does.
    let new_content = "y".repeat(500);
    let addr = spawn_http_mock_with_file(
        "204 No Content",
        PathBuf::from(&target),
        new_content,
    )
    .await;

    let bridge = MirrorBridge::new(&format!("http://{}", addr));
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn success_204_but_missing_file_fails() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    let target = dir.join("missing.md").to_string_lossy().to_string();
    let addr = spawn_http_mock("204 No Content").await;

    let bridge = MirrorBridge::new(&format!("http://{}", addr));
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unreadable") || msg.contains("No such file"),
        "got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn success_204_but_tiny_file_fails() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    // Tiny file: size gate fires before the unchanged check.
    let target = write_target(&dir, "stub.md", 50);
    let addr = spawn_http_mock("204 No Content").await;

    let bridge = MirrorBridge::new(&format!("http://{}", addr));
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("50") || msg.contains("< 200"),
        "got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn http_409_is_reported_as_busy() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    let target = write_target(&dir, "deliverable.md", 500);
    let addr = spawn_http_mock("409 Conflict").await;

    let bridge = MirrorBridge::new(&format!("http://{}", addr));
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("409"), "got: {msg}");
    assert!(
        msg.contains("busy") || msg.contains("another turn already running"),
        "got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn http_502_is_reported_as_model_failure() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    let target = write_target(&dir, "deliverable.md", 500);
    let addr = spawn_http_mock("502 Bad Gateway").await;

    let bridge = MirrorBridge::new(&format!("http://{}", addr));
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("502"), "got: {msg}");
    assert!(
        msg.contains("model request failed") || msg.contains("API key"),
        "got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn connection_refused_is_reported_as_http_error() {
    use nexus_runtime_core::mirror_bridge::MirrorBridge;
    let dir = temp_dir();
    let target = write_target(&dir, "deliverable.md", 500);
    // Port 1 is almost never listening.
    let bridge = MirrorBridge::new("http://127.0.0.1:1");
    let result = bridge.delegate_write(&target, "test", "ev").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("HTTP") || msg.contains("connect"),
        "got: {msg}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
