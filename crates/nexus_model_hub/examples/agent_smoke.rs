use nexus_model_hub::neural_agent::{ContextPacket, MicroAgent};
use tokio::sync::broadcast;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let key = std::env::var("NVIDIA_API_KEY").unwrap_or_default();
    let client = reqwest::Client::new();
    for model in ["nvidia/nemotron-3-ultra-550b-a55b", "z-ai/glm-5.3", "z-ai/glm-5.3-flash"] {
        let (tx, mut rx) = broadcast::channel::<String>(4096);
        let agent = MicroAgent::init(tx)?;
        let ctx = ContextPacket {
            system_prompt: Some("You are terse.".into()),
            model_override: Some(model.into()),
            ..Default::default()
        };
        let res = agent
            .prepare_and_run(&client, &key, "Write a Rust fn that reverses a string.", ctx)
            .await;
        let (mut s, mut t) = (0, 0);
        while let Ok(m) = rx.try_recv() {
            if m.contains("NEURAL_STREAM") {
                s += 1;
            }
            if m.contains("NEURAL_THOUGHT") {
                t += 1;
            }
        }
        match res {
            Ok((text, ms)) => {
                println!("{model}: {} chars, {ms}ms, stream={s}, thought={t}", text.len())
            }
            Err(e) => println!("{model}: ERROR {e:#}"),
        }
    }
    Ok(())
}
