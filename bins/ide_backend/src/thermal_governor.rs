//! ThermalGovernor — watches CPU temperature and reduces agent turn-rate
//! when the substrate is running hot (>70°C → half-speed, >80°C → pause).
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::time::{interval, Duration};

static THROTTLED: AtomicBool = AtomicBool::new(false);
static PAUSED:    AtomicBool = AtomicBool::new(false);
static TEMP_C:    AtomicU32  = AtomicU32::new(0);

pub fn init() {
    tracing::info!(">> [THERMAL] Governor initialised");
}

pub fn is_throttled() -> bool { THROTTLED.load(Ordering::Relaxed) }
pub fn is_paused()    -> bool { PAUSED.load(Ordering::Relaxed) }
pub fn current_temp() -> f32  { TEMP_C.load(Ordering::Relaxed) as f32 }

pub fn spawn() {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));

        loop {
            ticker.tick().await;

            // Read ARM64 thermal zone directly (more reliable on Duet)
            let temp = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
                .unwrap_or_else(|_| "45000".to_string())
                .trim()
                .parse::<f32>()
                .unwrap_or(45000.0) / 1000.0;

            TEMP_C.store(temp as u32, Ordering::Relaxed);
            THROTTLED.store(temp > 70.0, Ordering::Relaxed);
            PAUSED.store(temp > 80.0, Ordering::Relaxed);

            if temp > 75.0 {
                tracing::warn!("[THERMAL] Temperature {:.1}°C — throttling agent pipeline", temp);
            }
        }
    });
}
