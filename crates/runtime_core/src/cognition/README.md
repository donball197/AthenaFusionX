# Cognition Module (Phase 8A v3)

Architecture version: `5.0.0-alpha`  
Feature flag: `cognition_v5`

Pure scaffolding. Zero runtime impact until later phases are implemented.

| Component            | Future Role                                   | Phase |
|----------------------|-----------------------------------------------|-------|
| ExecutiveCortex      | Intent / Mission / Policy / Decision          | 8B    |
| PolicyEngine         | Safety & constraint policy                    | 8B    |
| GoalManager          | Durable multi-mission goals                   | 8B    |
| EventBus             | Decoupled pub/sub with rich metadata          | 8B/8D |
| Blackboard           | Shared CognitiveContext                       | 8B    |
| AttentionSystem      | Focus & salience                              | 8B    |
| WorkingMemory        | Transient scratchpad                          | 8B    |
| BeliefEngine         | Long-lived confidence                         | 8C    |
| PredictionEngine     | Forward models                                | 8C    |
| UncertaintyEngine    | Epistemic / aleatoric uncertainty             | 8C    |
| CausalReasoner       | Causal links                                  | 8C    |
| ReflectionEngine     | Post-mission lessons                          | 8C    |
| StrategyManager      | Approach selection                            | 8B/8C |
| CognitiveScheduler   | Event-driven cycle (replaces fixed turns)     | 8D    |

Existing CorticalLoop remains the sole execution engine until Phase 8D.
