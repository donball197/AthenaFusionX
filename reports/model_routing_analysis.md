# Model Routing Analysis

## Introduction
This report analyzes the routing logic in `crates/runtime_core/src/router.rs` and searches the codebase for model configuration, environment variables, cascade definitions, and hardcoded model identifiers.

## Router Implementation
The `router.rs` file defines the core routing mechanism for the agent system. It includes:
- **DirectiveClass Enum**: Defines the types of commands the system can handle (e.g., `Analysis`, `Execution`, `Calibration`).
- **ContextBudget Struct**: Manages the computational resources allocated to each task.
- **AgentProfile Struct**: Represents the configuration of the agent, including its name, model, and budget.
- **IntelligentRouter Struct**: The main router that classifies directives and assigns them to appropriate agents.

### Key Functions
- `classify(d: &str) -> DirectiveClass`: Determines the type of a given directive string.
- `route(directive: &str) -> AgentProfile`: Assigns a directive to a specific agent profile based on its class.

## Model Configuration and Environment Variables
While the router itself is agnostic to specific model implementations (it uses generic `AgentProfile` structs), the actual model configuration is typically loaded from environment variables or configuration files. The codebase does not contain explicit hardcoded model identifiers within the router logic, suggesting that the model selection is dynamic and configured externally.

## Cascade Definitions
There are no explicit "cascade" definitions (like a chain of models) visible in the provided router code. The routing appears to be a direct mapping from directive class to a single agent profile. However, the existence of multiple budget levels (minimal, standard, doctor, security) suggests that the system is prepared to handle different levels of complexity or security requirements, which could be interpreted as a form of cascading capability, though not strictly defined as a chain in this specific file.

## Conclusion
The `router.rs` file provides a robust framework for classifying and routing agent tasks. The separation between the router logic and the specific model implementations (via `AgentProfile`) allows for flexibility in the system's configuration. Further analysis of the environment loading and specific agent implementations would be required to fully characterize the model routing behavior in a live deployment.
