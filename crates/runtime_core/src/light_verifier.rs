use anyhow::Result;

pub struct LightVerifier;

impl LightVerifier {
    pub async fn quick_check() -> Result<String> {
        let out = tokio::process::Command::new("cargo")
            .args(["check", "--quiet", "--message-format=short"])
            .output().await?;
        Ok(if out.status.success() { "[VERIFY_OK]".into() } else { "[VERIFY_FAIL]".into() })
    }
}
