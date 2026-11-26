use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};

static THROTTLED: AtomicBool = AtomicBool::new(false);
static PAUSED: AtomicBool = AtomicBool::new(false);
static TEMP_C: AtomicU32 = AtomicU32::new(0);

pub async fn run(mut shutdown: broadcast::Receiver<()>) -> anyhow::Result<()> {
    let mut ticker = interval(Duration::from_secs(10));
    let mut components = sysinfo::Components::new();

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("[THERMAL] Shutdown received");
                return Ok(());
            }
            _ = ticker.tick() => {}
        }

        components.refresh();

        let max_temp = components
            .iter()
            .map(|c| c.temperature())
            .filter(|t| t.is_finite())
            .fold(0f32, f32::max);

        TEMP_C.store(max_temp as u32, Ordering::Relaxed);
        THROTTLED.store(max_temp > 70.0, Ordering::Relaxed);
        PAUSED.store(max_temp > 80.0, Ordering::Relaxed);

        if max_temp > 75.0 {
            tracing::warn!("[THERMAL] Temperature {}°C — throttling agent pipeline", max_temp);
        }
    }
}
