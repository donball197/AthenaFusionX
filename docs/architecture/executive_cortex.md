# Executive Cortex

**Phase 8A – skeleton only.**

Owns exclusively:
- Intent
- Mission
- Goals (via GoalManager)
- Policy (via PolicyEngine)
- ExecutiveDecision (+ DecisionReason)

Does **not** own:
- Attention → AttentionSystem
- Resource Budget → CognitiveScheduler
- Interruptions → EventBus / Scheduler
- Focus → WorkingMemory / AttentionSystem
