use tokio::time::{sleep, Duration};
use std::sync::Arc;
use crate::AppState;

pub async fn pulse_monitor(state: Arc<AppState>) {
    // TEST MODE: 10-second threshold for immediate feedback
    let hibernation_threshold = Duration::from_secs(10);

    loop {
        {
            let mut swarm = state.swarm.lock().await;
            let mut purged_count = 0;

            for agent in swarm.iter_mut() {
                if agent.is_active && agent.last_active.elapsed() > hibernation_threshold {
                    agent.clear_scratchpad();
                    purged_count += 1;
                }
            }

            if purged_count > 0 {
                println!(">> [LAZARUS] FAST-PURGE: {} agents deactivated after 10s idle.", purged_count);
            }
        }

        // Check every 5 seconds for the test
        sleep(Duration::from_secs(5)).await;
    }
}
