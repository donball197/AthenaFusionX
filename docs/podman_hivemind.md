markdown
# PodmanEngine & HiveMind Integration

## Overview

The PodmanEngine serves as the execution environment for HiveMind agents, providing a secure and isolated space for running containerized tasks. This document details the connection mechanism, data flow, and security considerations for this integration.

## Connection Mechanism

The PodmanEngine connects to the HiveMind via a gRPC stream. This stream is initiated by the PodmanEngine upon startup and remains active throughout its lifecycle.

1.  **Authentication:** The PodmanEngine authenticates with HiveMind using a pre-shared secret key. This key is configured during the PodmanEngine's deployment and is used to establish a secure connection.
2.  **gRPC Stream:** Once authenticated, a bidirectional gRPC stream is established between the PodmanEngine and HiveMind. This stream is used for:
    *   **Task Distribution:** HiveMind sends task definitions (container images, commands, environment variables, etc.) to the PodmanEngine.
    *   **Status Updates:** The PodmanEngine sends status updates (running, completed, failed, etc.) back to HiveMind.
    *   **Log Streaming:** The PodmanEngine streams logs from running containers to HiveMind for centralized logging and monitoring.
    *   **Result Reporting:** The PodmanEngine reports the results of completed tasks back to HiveMind.
3.  **Heartbeat:** The PodmanEngine sends periodic heartbeat messages to HiveMind to indicate its availability.  If HiveMind does not receive a heartbeat within a configured timeout, it assumes the PodmanEngine is unavailable and will redistribute its tasks.

## Data Flow

The following diagram illustrates the data flow between the PodmanEngine and HiveMind: