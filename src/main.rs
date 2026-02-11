mod alloc; mod resonator; mod lazarus; mod brain; mod agents; mod server;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub brain: Mutex<brain::NeuralNet>,
    pub swarm: Mutex<Vec<agents::SwarmUnit>>, 
}

#[tokio::main]
async fn main() {
    alloc::initialize_mnemnos_lean();

    // 1. Initialize the 68-Agent Swarm (0MB initial footprint)
    let swarm = agents::spawn_swarm(68);

    // 2. Initialize Unified State
    let state = Arc::new(AppState {
        brain: Mutex::new(brain::NeuralNet::new()),
        swarm: Mutex::new(swarm),
    });

    // 3. Ignite Lazarus Sentinel with access to the Swarm
    // It will now automatically sleep agents after 5 mins of inactivity
    tokio::spawn(lazarus::pulse_monitor(Arc::clone(&state)));

    // 4. Ignite the Modular IDE Server
    let app = server::router(Arc::clone(&state));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    println!(">> [NEXUS] COMMAND CENTER LIVE ON: {}", addr);
    println!(">> [SENTINEL] HIBERNATION ACTIVE (5m TIMEOUT)");

    axum::serve(listener, app.into_make_service()).await.unwrap();
}
