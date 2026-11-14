//! Mirror-agent bridge: delegated synthesis for missions whose
//! cortical-loop write path fails (truncation, turns exhausted,
//! model confusion).
//!
//! Dormant unless `NEXUS_MIRROR_DELEGATION=1` is set. In that case,
//! a failing write is handed to a running mirror-agent sidecar at
//! `http://127.0.0.1:8081` with the accumulated evidence corpus as
//! context. A successful delegation is verified against the target
//! file before the caller may record a synthetic [WRITE_OK] marker.
//!
//! # Approval model
//!
//! `mirror-agent serve` hardcodes `AgentApprovalMode::Auto` (see
//! `mirror/mirror-agent/src/http/cli.rs:289`). In Auto mode,
//! `ToolPermissions { allow_outside_workspace: true }`, so the sidecar
//! will honor writes to paths outside its `--workspace` root if given
//! an absolute path.
//!
//! Delegations pass workspace-relative `target_path` values from
//! `session.contract.required_artifact_paths`, so the outside-workspace
//! capability is not exercised today. If the delegation surface ever
//! widens to allow absolute paths from user contracts, revisit this:
//! the sidecar would honor writes the cortical loop's
//! `artifact_denied_by_policy` gate would have refused.

use anyhow::{anyhow, bail, Result};
use std::time::Duration;

pub struct MirrorBridge {
    endpoint: String,
    timeout: Duration,
}

impl MirrorBridge {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            timeout: Duration::from_secs(900),
        }
    }

    /// Ask the sidecar to produce the deliverable at `target_path`.
    ///
    /// The sidecar contract signals a completed turn with HTTP 204.
    /// Success additionally requires the target file to exist, contain
    /// at least 200 bytes, and differ from its pre-delegation contents.
    pub async fn delegate_write(
        &self,
        target_path: &str,
        mission: &str,
        evidence_corpus: &str,
    ) -> Result<()> {
        // Capture the original contents so a stale file cannot be
        // mistaken for a successful delegated write.
        let before = match tokio::fs::read(target_path).await {
            Ok(contents) => Some(contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(anyhow!(
                    "cannot inspect target before Mirror delegation ({target_path}): {error}"
                ));
            }
        };

        let prompt = format!(
            "You are a subagent in the AthenaFusionX audit pipeline.\n\
             Your ONLY task: write the deliverable file.\n\n\
             MISSION:\n{mission}\n\n\
             TARGET FILE: {target_path}\n\
             (relative to workspace root)\n\n\
             EVIDENCE GATHERED BY THE PARENT AGENT:\n\
             ---\n{evidence_corpus}\n---\n\n\
             INSTRUCTIONS:\n\
             - Use the `write` tool to create {target_path}.\n\
             - Content must be complete prose. No code blocks.\n\
             - Reference functions by name only (backticks ok).\n\
             - Do NOT cite line numbers.\n\
             - Finish every sentence. Close every section.\n"
        );

        let client = reqwest::Client::builder().timeout(self.timeout).build()?;

        let response = client
            .post(format!("{}/message", self.endpoint))
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(prompt)
            .send()
            .await
            .map_err(|error| anyhow!("mirror-agent HTTP request failed: {error}"))?;

        let status = response.status();
        if status != reqwest::StatusCode::NO_CONTENT {
            let body = response.text().await.unwrap_or_default();
            let hint = match status.as_u16() {
                415 => " (Content-Type must be text/plain)",
                409 => " (mirror-agent busy: another turn already running)",
                400 => " (empty prompt)",
                504 => " (turn timed out — increase --turn-timeout-seconds)",
                502 => " (model request failed — check API key/quota)",
                503 => " (service unavailable)",
                _   => "",
            };
            bail!(
                "mirror-agent returned HTTP {status}{hint}; response: {}",
                body.chars().take(500).collect::<String>()
            );
        }

        // HTTP 204 means the agent turn completed, not that the requested
        // file was correctly written. Verify the actual target independently.
        let after = tokio::fs::read(target_path).await.map_err(|error| {
            anyhow!("Mirror turn completed but target {target_path} is unreadable: {error}")
        })?;

        if after.len() < 200 {
            bail!(
                "Mirror turn completed but target {target_path} is only {} bytes (< 200)",
                after.len()
            );
        }

        if before.as_deref() == Some(after.as_slice()) {
            bail!(
                "Mirror turn completed but target {target_path} is unchanged from its pre-delegation contents"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::MirrorBridge;
    use std::path::PathBuf;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn mock_sidecar(status: u16, write_target: Option<PathBuf>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock sidecar");
        let address = listener.local_addr().expect("mock sidecar address");

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = [0u8; 8192];
            let _ = stream.read(&mut request).await;

            if let Some(path) = write_target {
                tokio::fs::write(path, vec![b'x'; 256]).await.expect("write mock deliverable");
            }

            let reason = if status == 204 { "No Content" } else { "Bad Gateway" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).await.expect("send mock response");
        });

        format!("http://{address}")
    }

    #[tokio::test]
    async fn accepts_204_when_target_is_created() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("deliverable.md");
        let endpoint = mock_sidecar(204, Some(target.clone())).await;

        MirrorBridge::new(&endpoint)
            .delegate_write(
                target.to_str().expect("UTF-8 target path"),
                "write a report",
                "test evidence",
            )
            .await
            .expect("changed deliverable should be accepted");

        assert_eq!(tokio::fs::metadata(target).await.expect("deliverable exists").len(), 256);
    }

    #[tokio::test]
    async fn rejects_204_when_target_is_missing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("missing.md");
        let endpoint = mock_sidecar(204, None).await;

        let error = MirrorBridge::new(&endpoint)
            .delegate_write(
                target.to_str().expect("UTF-8 target path"),
                "write a report",
                "test evidence",
            )
            .await
            .expect_err("missing deliverable must fail");

        assert!(error.to_string().contains("unreadable"));
    }

    #[tokio::test]
    async fn rejects_204_when_existing_target_is_unchanged() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("stale.md");
        let original = vec![b's'; 256];
        tokio::fs::write(&target, &original).await.expect("create stale target");
        let endpoint = mock_sidecar(204, None).await;

        let error = MirrorBridge::new(&endpoint)
            .delegate_write(
                target.to_str().expect("UTF-8 target path"),
                "write a report",
                "test evidence",
            )
            .await
            .expect_err("unchanged target must fail");

        assert!(error.to_string().contains("unchanged"));
    }

    #[tokio::test]
    async fn rejects_non_204_http_response() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let target = directory.path().join("deliverable.md");
        let endpoint = mock_sidecar(502, None).await;

        let error = MirrorBridge::new(&endpoint)
            .delegate_write(
                target.to_str().expect("UTF-8 target path"),
                "write a report",
                "test evidence",
            )
            .await
            .expect_err("non-204 response must fail");

        assert!(error.to_string().contains("HTTP 502"));
    }
}
