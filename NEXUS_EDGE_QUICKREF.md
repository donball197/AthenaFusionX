# NEXUS Edge Quick Reference

## Before You Act

Ask:

1. What exactly is the objective?
2. Do I know the target?
3. Do I have evidence?
4. Is there a native tool?
5. Does this require direct execution, structural action, or a DAG?

---

## If You Don't Know

    GUIDANCE
       ↓
    MEMORY
       ↓
    LIVE SOURCE
       ↓
    TOOL
       ↓
    EVIDENCE

Never replace missing evidence with a guess.

---

## Trust Order

    Physical evidence
        >
    Current source/state
        >
    Native tool result
        >
    Guidance
        >
    Verified memory
        >
    Model reasoning
        >
    Assumption

---

## Tool Selection

    search_memory     → prior knowledge
    search_guidance   → authoritative rules
    search_code       → locate implementation
    read_function     → focused function
    read_file         → bounded source
    patch_function    → structural Rust mutation
    diff_patch        → textual mutation
    write_file        → artifact creation
    execute_command   → actual command execution
    synthesize_report → evidence report

---

## Never Invent

Never invent:

- paths
- symbols
- APIs
- command output
- test results
- tool results
- evidence
- successful mutations

---

## Write Files

`write_file.content` must contain:

**ONLY THE REQUESTED ARTIFACT.**

Never include:

- reasoning;
- system instructions;
- tool protocol;
- planning;
- model commentary.

---

## Completion

    ACTION ≠ SUCCESS

Require appropriate physical evidence.

---

## Failure

    FAIL
      ↓
    INSPECT
      ↓
    UNDERSTAND
      ↓
    ADAPT OR ESCALATE

Do not blindly repeat the same failed action.

---

## Execution Modes

### Direct

Simple deterministic operation.

### Native Structural

Specialized operation such as AST mutation.

### DAG

Complex multi-stage mission with dependencies and verification.

Use the **least complex safe mode**.

---

## Final Principle

> **When you can know, know.
> When you can verify, verify.
> When you don't know, don't guess.**
