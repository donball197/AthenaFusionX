# JCode Architecture Analysis: Client-Server Loop

## Overview
The JCode architecture utilizes an Axum-based server (`bins/ide_backend/src/server.rs`) to orchestrate interactions between the UI, the neural agent, and the runtime core. The protocol layer (`crates/protocol/src/lib.rs`) currently serves as a foundational initialization point, though the primary request-response logic is encapsulated within the `AppState` and the defined `Intent` structures in the backend.

## Data Flow Mapping
1. **Request Entry**: The server utilizes Axum routing to handle incoming requests, specifically targeting `Intent` structures which encapsulate directives.
2. **State Management**: The `AppState` struct acts as the central hub, holding references to:
   - `MicroAgent` (Neural processing)
   - `Orchestrator` (Task management)
   - `MemoryPalace` (Context persistence)
   - `ArbiterAgent` (Decision logic)
   - `VmoState` (Virtual memory state)
3. **Execution Loop**:
   - Incoming JSON payloads are deserialized into `Intent`.
   - The backend routes these intents to internal handlers (e.g., `ws_bridge`, `vitals_handler`, `cluster_api`).
   - The `Orchestrator` and `ArbiterAgent` process the directive, interacting with the `MemoryPalace` to maintain state consistency.
   - Results are broadcast via the `tokio::sync::broadcast::Sender` or returned directly through the Axum response cycle.

## Protocol Integration
The `crates/protocol` crate is currently decoupled from the primary request-response loop, suggesting a modular design where protocol definitions are separated from the transport-layer implementation in `ide_backend`. Future adaptations should leverage this crate to standardize the `Intent` schema across both the client and server.