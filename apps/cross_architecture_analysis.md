# Architectural Report: Warp Terminal & Runtime Core

## Overview

This report analyzes seven source files from two distinct subsystems within the Warp codebase:

1. **Computer Use Crate** (`apps/warp/crates/computer_use/`) – A cross-platform library for screen capture, input simulation, and video recording, used for AI-driven computer interaction.
2. **Warp Terminal Shell & Escape Sequences** (`apps/warp/crates/warp_terminal/`) – Shell abstraction and terminal input encoding for the Warp terminal emulator.
3. **Runtime Core** (`crates/runtime_core/`) – An AI agent runtime with a cognitive loop, security gate, and executor for file/command operations.

These components are largely independent but share a common theme: they provide low-level primitives for AI-driven automation and terminal interaction. The report examines each module's architecture, key abstractions, and design decisions.

---

## 1. Computer Use Crate

### 1.1 `lib.rs` – Core Library

**Purpose:** Provides a platform-agnostic API for controlling a computer (mouse, keyboard, screenshots) and recording the screen. Supports macOS, Linux (X11/Wayland), and Windows, with a no-op fallback for unsupported platforms.

**Key Abstractions:**

- **`Actor` trait** – Asynchronous interface for performing actions (`perform_actions`). Implementations are platform-specific (macOS, Linux, Windows, noop).
- **`Recorder` trait** – Long-lived capability for video capture (`start`/`stop`). Returns a `RecordingHandle` that owns the capture process.
- **`Target` enum** – Specifies whether an action targets the whole screen or a specific background window (with `window_id` and `pid`). This enables per-window control without disturbing the user.
- **`Action` enum** – Serializable actions: mouse down/up/move/wheel, text typing, key down/up, wait.
- **`ScreenshotParams`** – Configuration for post-action screenshots, including region and target.
- **`PointerSession`** – Tracks pointer state across action batches to correctly record drags that span multiple `UseComputer` calls.

**Platform Handling:**

- Uses `cfg_attr` to select platform-specific modules (`mac/mod.rs`, `linux/mod.rs`, `windows/mod.rs`).
- A `noop` module provides stubs for unsupported platforms and test builds.
- `create_actor()` and `create_recorder()` return boxed trait objects, with `test-util` feature forcing noop implementations.
- `background_supported()` indicates whether per-window control is available (macOS and Linux X11 only).

**Recording Lifecycle:**

- `RecordingConfig` includes frame rate, max duration/size, playback speed multiplier, and target.
- `RecordingHandle` owns the ffmpeg child process and cleans up partial output on drop unless explicitly stopped.
- `RecordingOutput` carries the final file path and metadata.
- `post_process_recording()` (Linux only) trims inactive gaps and burns action overlays.
- `generate_video_thumbnail()` extracts a representative frame and composites a play button.

**Notable Design Decisions:**

- **Per-action targeting** – `TargetedAction` pairs an action with a target, allowing a single batch to drive multiple windows.
- **Background session ownership** – `end_background_session(owner)` tears down only the state owned by a specific conversation, enabling concurrent sessions.
- **Pointer session** – Solves the problem of drags split across separate tool calls by persisting the last point and active button.
- **Error handling** – `RecordingError` distinguishes environment, start, and finalize failures.

### 1.2 `bin/use_computer.rs` – CLI Tool

**Purpose:** A developer-facing CLI to manually test computer use actions.

**Features:**

- Subcommands: `click`, `text`, `screenshot`, `keypress`, `windows`.
- Global flags `--pid` and `--window-id` for background window targeting.
- Validates that both flags are provided together to avoid silent downgrade to screen targeting.
- Uses `experimental_list_windows()` for window enumeration (macOS/Linux only).
- Exits via `ExitCode` to ensure `Drop` runs (important for Linux X11 input device cleanup).

**Design Notes:**

- The CLI always enables `background_enabled: true` in `Options`.
- Screenshot region parsing supports `"x1,y1,x2,y2"` format.
- Key parsing accepts hex keycodes (`0x24`) or single characters.

---

## 2. Warp Terminal

### 2.1 `shell/mod.rs` – Shell Abstraction

**Purpose:** Models different shells (Zsh, Bash, Fish, PowerShell) and provides utilities for history parsing, alias/abbreviation extraction, and shell-specific command generation.

**Key Structures:**

- **`Shell`** – Holds shell type, version, options (e.g., `autocd`), detected plugins, and the shell binary path.
- **`ShellType`** – Enum with methods for:
  - History file locations.
  - RC file paths (target-OS aware using `TypedPathBuf`).
  - Command combiners (`&&`, `; and`, etc.).
  - Alias/abbreviation parsing from shell output.
  - History file parsing (handles zsh metafication, bash timestamps, fish YAML, PowerShell plain lines).
  - Input reporting sequences (for shell integration).
  - Kill buffer and execute command byte sequences.
  - Executable enumeration commands and output parsing.
- **`ShellLaunchData`** – Describes how to launch a shell: executable, WSL distro, MSYS2, or Docker sandbox. Provides path conversion methods between host and target OS encodings.

**Notable Features:**

- **Zsh history parsing** – Handles `extended_history` format and metafication (XOR 32 after meta byte).
- **Fish history** – Unescapes `\n` and `\\` sequences.
- **Path conversion** – `ShellLaunchData` converts between Unix/Windows paths for WSL and MSYS2, and handles Docker sandbox paths (always Unix).
- **Executable enumeration** – Shell-specific commands to list executables, with post-processing for MSYS2 (strips `.dll`, adds extension-less variants) and PowerShell (UTF-8 output).
- **Input reporting** – Version-gated for Bash (≥4.0), uses `ESC i` for Zsh/Fish, `ESC 1` for PowerShell (avoids letter "i" due to layout issues).

### 2.2 `model/escape_sequences.rs` – Terminal Input Encoding

**Purpose:** Converts keystrokes and mouse events into ANSI escape sequences to send to the PTY.

**Key Components:**

- **C0/C1 control character constants** – Standard ANSI control codes.
- **`ToEscapeSequence` trait** – Implemented for `KeystrokeWithDetails` and `MouseState`.
- **`KeystrokeWithDetails`** – Wraps a keystroke with platform-provided key details (e.g., `chars` for text input).
- **`ModeProvider` trait** – Provides terminal mode state (e.g., application cursor mode) to select CSI vs SS3.
- **Kitty keyboard protocol** – `maybe_convert_keystroke_to_csi_u` attempts to encode using CSI u when the protocol is active.
- **Legacy fallbacks** – Function keys, C0 control codes (Ctrl+letter), cursor movement, delete, meta/alt, backspace.

**Encoding Logic:**

- `to_escape_sequence()` tries kitty protocol first, then falls back to legacy encodings.
- `to_pty_bytes()` (for headless TUI) layers additional fallbacks: Ctrl+letter → C0 byte, printable text → UTF-8, named control keys → C0.
- Mouse events encode button, position, and action (press/release/scroll) using SGR extended coordinates.

**Notable Details:**

- **Modifier parameter** – `modifier_param()` computes xterm/Kitty numeric modifier (shift=1, alt=2, ctrl=4, super=8).
- **Modified cursor keys** – Always use CSI `1;<mods> <letter>` form, never SS3.
- **Delete key** – Modified delete uses `CSI 3;<mods> ~`.
- **Backspace** – Guards against Windows reporting `\x08` for Shift+Backspace; returns DEL (0x7f) for unmodified.
- **Alt-screen scroll** – Converts wheel events to mouse reports or SS3 arrow keys depending on mode.

---

## 3. Runtime Core (AI Agent)

### 3.1 `action_gate.rs` – Security Gate

**Purpose:** Enforces deterministic security policies on motor commands before execution.

**Key Structure:**

- **`ActionGate`** – Holds an `ExecutionSandbox` with a list of active capabilities.
- **`approve(&MotorCommand)`** – Validates each action type:
  - **`ExecuteCommand`** – Blocks destructive commands (`rm -rf /`, `mkfs`) and secret inspection (`cat .env`, `printenv`).
  - **`WriteFile`** – Blocks path traversal (`..`) and plaintext API key leaks.
  - **`PatchFile`** – Blocks path traversal.
  - All actions call `sandbox.validate_intent()` with a policy name.

**Design Notes:**

- Uses `anyhow` for error messages with prefixes like `ACTION_GATE_SECURITY_VIOLATION` and `ACTION_GATE_POLICY_FAULT`.
- The sandbox capabilities are hardcoded in `new()` (read/write substrate, execute system call, halt cognition).
- The gate is stateless; it does not track history or context.

### 3.2 `executor.rs` – Motor Action Executor

**Purpose:** Executes `MotorAction` commands (file operations, command execution, search, etc.) with path sanitization and caching.

**Key Components:**

- **`RoboticExecutor`** – Holds an FSM state (`Idle`, `Executing`, `Complete`, `Fault`), API key, broadcast sender, and an action cache.
- **`sanitize(path)`** – Resolves paths relative to workspace root, prevents traversal, and falls back to crate-relative or filename search.
- **`execute_motor_command()`** – Dispatches to `execute_single_action()`.

**Action Implementations:**

- **`ExecuteCommand`** – Runs via `PodmanShield::execute()` (containerized execution), truncates output, caches results.
- **`WriteFile`** – Checks for identical content (no-op), creates parent dirs, writes.
- **`ReadFile`** – Reads file, stores embedding in memory, truncates large content.
- **`ReadFileChunk`** – Reads a line range.
- **`SearchCode`** – Uses `rg`/`grep` with fallback to `crates/runtime_core/src/`.
- **`ReplaceLines`** – Replaces a line range with new content.
- **`RunTests`** – Runs `cargo test` with output truncation.
- **`DiffPatch`** – Applies a search/replace patch via `DiffEngine`.
- **`PatchFunction`** – Updates a function body via `AstEditor`.
- **`SearchMemory`** – Semantic search using embeddings.
- **`PatchFile`** – Simple string replacement.
- **`SynthesizeReport`** – Calls an LLM to generate a markdown report from source files.
- **`Halt`** – Returns a halt message.

**Path Sanitization:**

- Allows `/tmp/` paths.
- Rewrites common prefixes (`src/` → `crates/runtime_core/src/`, etc.).
- Falls back to filename search in workspace root.
- Ensures canonical path stays within workspace root.

### 3.3 `cortical_loop.rs` – Cognitive Loop

**Purpose:** Implements the main agent reasoning loop: strategize → critique → translate → execute → evaluate, with belief propagation and contract enforcement.

**Key Structures:**

- **`CortexOutcome`** – Either `CompletionProposed` or `TurnsExhausted`.
- **`CortexState`** – State machine enum: `Strategize`, `Critique`, `Translate`, `Execute`, `Evaluate`, `Complete`, `Fail`.
- **`CorticalLoop`** – Holds HTTP client, API key, world model, memory, VMO state, distributed scheduler, broadcast sender, and `ActionGate`.

**Flow:**

1. **Ignite** – Gathers context from world model, workspace facts, and ancestor evidence. Generates hypotheses via `EpistemicEngine::generate_hypotheses()`.
2. **Strategize** – Selects best hypothesis, checks contract satisfaction, proposes next action via LLM. Detects token resonance (mode collapse) and forces pivot.
3. **Critique** – Red-team critic evaluates proposal; may veto (negative confidence modifier). Circuit breaker overrides repeated vetoes on mutation tasks.
4. **Translate** – Converts proposal to concrete `AgentAction`s via JSON parser or LLM translator. Anti-loop detection prevents repeating the same action >3 times.
5. **Execute** – Creates `RoboticExecutor`, runs actions through `ActionGate`, collects evidence. If Rust files mutated, runs `cargo check`; on failure, reverts mutations and adds veto evidence.
6. **Evaluate** – LLM evaluates evidence, updates belief scores, may revert negative mutations. If halt action with high belief, completes.

**Key Mechanisms:**

- **Belief propagation** – Graph of hypotheses/observations with edges (supports/contradicts) and belief scores.
- **Contract evaluation** – `ContractEvaluator` checks if objective is satisfied (e.g., mutation marker present).
- **Anti-loop** – Tracks action attempts globally; vetoes repeated actions.
- **Compiler critic** – Validates Rust mutations with `cargo check` and reverts on failure.
- **Debug dumps** – Writes LLM inputs/outputs to `.nexus_debug/` for post-mortem.

**Notable Design Decisions:**

- **Deterministic guards** – Override LLM authority when contract is satisfied or halt is proposed without evidence.
- **Budget extension** – If belief is high but calls exhausted, extends budget dynamically.
- **Search relevance filtering** – Suppresses positive belief updates for low-relevance search results.

---

## 4. Cross-Cutting Concerns

### 4.1 Platform Abstraction

- **Computer Use** uses `cfg_attr` to select platform modules, with a `noop` fallback.
- **Shell** uses `cfg(windows)` for PowerShell history paths and `TargetOS` for RC file paths.
- **Runtime Core** is platform-agnostic but uses Unix-specific `ExitStatusExt` in tests.

### 4.2 Error Handling

- **Computer Use** uses `thiserror` for `RecordingError` with distinct variants.
- **Runtime Core** uses `anyhow` with descriptive error messages (e.g., `PATH_TRAVERSAL_DENIED`).
- **Shell** uses `anyhow::Result` and `report_error!` macro for alias parsing.

### 4.3 Async Patterns

- **Computer Use** uses `async_trait` for `Actor` and `Recorder`.
- **Runtime Core** uses `tokio` extensively (broadcast channels, mutexes, process spawning).
- **Cortical Loop** is fully async with a state machine loop.

### 4.4 Serialization

- **Computer Use** uses `serde` with custom `Vector2IDef` remote derive for `Vector2I`.
- **Shell** uses `serde` for `ShellLaunchData`.
- **Runtime Core** uses `serde_json` for LLM payloads and debug dumps.

---

## 5. Design Patterns and Architectural Style

- **Trait-based polymorphism** – `Actor`, `Recorder`, `ToEscapeSequence`, `ModeProvider`.
- **State machine** – `CortexState` in the cognitive loop; FSM in `RoboticExecutor`.
- **Strategy pattern** – Shell-specific methods in `ShellType`.
- **Facade** – `computer_use::lib.rs` exposes a unified API over platform-specific implementations.
- **Security gate** – `ActionGate` acts as a policy enforcement point.
- **Caching** – `ActionCache` in executor for command results.
- **Observer** – Broadcast channel for logging (`NEURAL_STREAM`).

---

## 6. Potential Issues and Observations

### Computer Use

- **Windows support** – `background_supported()` returns false; only whole-screen control. The `imp` module for Windows is not shown but likely has limited functionality.
- **Recording** – Only Linux and macOS have real recorders; Windows gets a no-op.
- **Pointer session** – The `PointerSession` is only populated by the Linux x11 actor; other platforms ignore it.

### Shell

- **PowerShell input reporting** – Uses `ESC 1` to avoid the letter "i" due to layout issues; this is a workaround.
- **Fish history** – Does not support multi-line alias values (noted in code).
- **Executable enumeration** – For MSYS2, `.dll` files are filtered out, but the logic is duplicated for Bash/Zsh and