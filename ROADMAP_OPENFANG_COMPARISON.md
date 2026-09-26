# Architectural Comparison: OpenFang vs. AthenaFusionX

## 1. Workspace Structure and Kernel Design
OpenFang employs a modular, 14-crate workspace structure centered around the `openfang-kernel`. This design emphasizes composability and security, with distinct crates for hands (capabilities), metering, and storage. In contrast, AthenaFusionX utilizes a monolithic `nexus_runtime_core` that integrates all critical functions into a single binary, prioritizing speed and simplicity over modularity.

## 2. WASM Sandboxing and Metering
Both systems implement WASM sandboxing, but their approaches differ significantly. OpenFang uses a dual-metered system (fuel + epoch interruption) that allows for fine-grained control over execution time and resource consumption. AthenaFusionX, while capable of metering, relies heavily on the raw performance of its ARM64 architecture and the `VmoHandler` for memory management, suggesting a trade-off between strict isolation and execution speed.

## 3. Memory and Persistence
OpenFang leverages SQLite with a WAL (Write-Ahead Logging) mechanism for persistent storage, ensuring data integrity and efficient writes. AthenaFusionX's `MemoryPalace` appears to be a more abstract, in-memory data structure that may not offer the same level of persistent reliability as a database-backed solution. The recent addition of `SqliteCheckpointer` in AthenaFusionX suggests an attempt to bridge this gap, but the core architecture remains distinct.

## 4. Capability Management
OpenFang introduces 