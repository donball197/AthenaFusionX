//! Mirror-agent bridge: delegated synthesis for missions whose
//! cortical-loop write path fails (truncation, turns exhausted,
//! model confusion).
//!
//! Dormant unless `NEXUS_MIRROR_DELEGATION=1` is set. In that case,
//! a failing write is handed to a running mirror-agent sidecar at
//! `http://127.0.0.1:8081` with the accumulated evidence corpus as
//! context. On success, a synthetic `[WRITE_OK] path` marker is
//! pushed into `local_evidence` so downstream verification behaves
//! identically to a normal write.

use anyhow::Result;
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
    /// Returns `Ok(())` iff the file exists with non-trivial content
    /// after the call completes.
    pub async fn delegate_write(
        &self,
        target_path: &str,
        mission: &str,
        evidence_corpus: &str,
    ) -> Result<()> {
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
             - Finish every sentence. Close every section.\n\
             - After the write succeeds, respond with exactly: DONE\n"
        );

        let client = reqwest::Client::builder().timeout(self.timeout).build()?;

        let resp = client
            .post(format!("{}/message", self.endpoint))
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(prompt)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("mirror-agent HTTP: {e}"))?;

        let body = resp.text().await?;
        if !body.contains("DONE") {
            let tail: String = body.chars().rev().take(200).collect::<String>().chars().rev().collect();
            return Err(anyhow::anyhow!(
                "mirror-agent did not signal DONE; response tail: {tail}"
            ));
        }

        // Trust but verify: file must exist and be non-empty.
        let meta = tokio::fs::metadata(target_path).await
            .map_err(|e| anyhow::anyhow!("mirror reported DONE but {target_path} unreadable: {e}"))?;
        if meta.len() < 200 {
            return Err(anyhow::anyhow!(
                "mirror reported DONE but {target_path} is {} bytes (< 200)",
                meta.len()
            ));
        }
        Ok(())
    }
}
