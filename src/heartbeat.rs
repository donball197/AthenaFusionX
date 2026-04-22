use crate::cluster::{ClusterMesh, ClusterNode};
use local_ip_address::local_ip;
use std::sync::Arc;
use sysinfo::System;
use tokio::time::{sleep, Duration};

pub async fn start(mesh: Arc<ClusterMesh>, node_id: String, port: u16) {
    let mut sys = System::new_all();
    let host = local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".to_string());
    let client = reqwest::Client::new();
    let leader_url = std::env::var("NEXUS_LEADER_URL").ok();

    loop {
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let cpu = sys.global_cpu_usage();
        let ram = if sys.total_memory() > 0 {
            ((sys.used_memory() as f32) / (sys.total_memory() as f32)) * 100.0
        } else {
            30.0
        };

        let node =
            ClusterNode { id: node_id.clone(), host: host.clone(), port, cpu, ram, last_seen: 0 };

        // Register to local mesh
        mesh.register(node.clone());

        // Phone home to leader dashboard
        if let Some(url) = &leader_url {
            let _ = client.post(format!("{}/api/cluster/register", url)).json(&node).send().await;
        }

        sleep(Duration::from_secs(5)).await;
    }
}
