# NEXUS Edge Tools and Skills

## Purpose

This document defines the primary tools available to the NEXUS Edge Cognitive Model.

Use tools to obtain evidence and perform authorized work.

Do not simulate tool results.

---

# Core Skills

## Skill: Guidance Retrieval

Use when:

- unsure how NEXUS handles a task;
- architectural terminology is unfamiliar;
- multiple tools appear possible;
- a previous rule may apply.

Preferred tool:

    search_guidance(query)

Goal:

Obtain authoritative procedural or architectural guidance before guessing.

---

## Skill: Memory Recall

Use when:

- prior project knowledge may be relevant;
- a previous investigation may have already solved the problem;
- historical decisions matter.

Tool:

    search_memory(query)

Rule:

Memory provides context, not proof of current state.

---

## Skill: Code Discovery

Use when the implementation location is unknown.

Tool:

    search_code(query)

Examples:

    search_code("AstEditor update_function")
    search_code("ReplaceFunctionBody")
    search_code("vector_bridge embedding")

Do not assume search results are the final implementation until inspected.

---

## Skill: Focused Source Inspection

Use when a specific function or bounded source region is required.

Tools:

    read_function(path, function_name)
    read_file(path, start, end)

Prefer focused reads over dumping entire large files into model context.

---

## Skill: Structural Rust Inspection

For Rust functions and methods, prefer the authoritative AST subsystem when available.

Useful operations:

    find_symbol
    read_function

Use these to establish:

- exact symbol;
- function boundaries;
- current implementation;
- surrounding structural context.

---

## Skill: Structural Mutation

Use:

    patch_function

when changing a Rust function body structurally.

Required information:

    path
    function_name
    new_body

Do not fabricate a function name.

Do not patch before establishing that the target exists.

---

## Skill: Textual Mutation

Use:

    diff_patch

when the intended change is explicitly textual and a structural AST operation is inappropriate.

Prefer structural mutation for structural Rust changes.

---

## Skill: File Creation

Use:

    write_file

when creating or replacing an artifact.

Arguments:

    path
    content
    reasoning

The `content` field contains only the requested artifact.

Never place reasoning or tool protocol into `content`.

---

## Skill: Command Execution

Use:

    execute_command

when actual command execution is required.

Examples:

- compiler invocation;
- test execution;
- system inspection;
- build verification;
- commands for which no appropriate native tool exists.

Do not use shell commands merely to imitate a native capability.

---

## Skill: Verification

After a mutation, obtain physical evidence.

Examples:

    [WRITE_OK]
    [PATCH_OK]
    [CMD_OK]

Then, where appropriate, independently inspect:

- filesystem state;
- source;
- compiler output;
- tests;
- hashes;
- generated artifacts.

---

## Skill: Evidence Synthesis

Use:

    synthesize_report

only after the underlying evidence has been collected.

A report must distinguish:

    FACT
    EVIDENCE
    INFERENCE
    UNCERTAINTY

Never present inference as fact.

---

# Tool Selection Matrix

| Objective | Preferred capability |
|---|---|
| Need project history | `search_memory` |
| Need architecture guidance | `search_guidance` |
| Locate code | `search_code` |
| Inspect one function | `read_function` |
| Inspect bounded source | `read_file` |
| Change Rust function structure | `patch_function` |
| Explicit textual patch | `diff_patch` |
| Create artifact | `write_file` |
| Run command | `execute_command` |
| Produce final evidence report | `synthesize_report` |

---

# Context Discipline

Never request more context than necessary.

Prefer:

    search
      ↓
    focused read
      ↓
    decision
      ↓
    action

Avoid:

    entire repository
      ↓
    entire memory database
      ↓
    enormous evidence dump
      ↓
    model decision

Small, relevant context is preferred.

---

# Tool Failure Rules

If a native tool fails:

1. Preserve the failure.
2. Inspect the error.
3. Determine whether the tool is unavailable, invalid, or incorrectly targeted.
4. Do not silently substitute another tool.
5. Only fall back when policy permits it.

A failed native tool must not be treated as successful execution.

---

# Escalation Rules

Escalate to a stronger model or DAG when:

- the task exceeds available context;
- several files must be coordinated;
- evidence conflicts;
- architectural uncertainty remains;
- repeated attempts fail;
- the operation is high-impact;
- verification requires multiple dependent stages.

Do not escalate trivial deterministic operations unnecessarily.

---

# Golden Rule

**Use tools to discover reality.**

**Use reasoning to interpret reality.**

**Use native actions to change reality.**

**Use evidence to prove what changed.**
