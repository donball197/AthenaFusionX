# Warp Terminal Emulator Architecture Report

## Overview

Warp is a high-performance, cross-platform terminal emulator built in Rust. It utilizes a custom immediate/retained hybrid UI framework called **WarpUI**, and supports two distinct front-ends sharing a common core:
1. **GUI Desktop App** (`app/`): A GPU-accelerated desktop application rendering via pathfinder geometry and WGSL shaders, packaged as a native `.app`/binary.
2. **Headless TUI App** (`crates/warp_tui`): A console-based terminal interface rendering into a cell grid (`TuiBuffer`) via crossterm input bindings, compiled behind the `tui` feature flag.

---

## Workspace Structure & Core Modules

The Cargo workspace contains over 60 member crates. The major architectural layers comprise:


┌────────────────────────────────────────────────────────┐
│                        Front-Ends                      │
│         app/ (GUI)                 warp_tui/ (TUI)     │
├────────────────────────────────────────────────────────┤
│                      Shared Core                       │
│     warp_core/      warpui/      warpui_core/          │
├────────────────────────────────────────────────────────┤
│                    Feature Surfaces                    │
│   terminal/   ai/   drive/   auth/   settings/   ipc/  │
├────────────────────────────────────────────────────────┤
│                    Persistence & API                   │
│         persistence/ (Diesel/SQLite)   graphql/        │
└────────────────────────────────────────────────────────┘


### Key Crates & Packages
- **`app/`**: Main GUI desktop application binary.
- **`crates/warpui/` & `crates/warpui_core/`**: Custom UI framework. Houses the Entity-Component-Handle system, layout elements, GPU scene graphing, and the TUI cell-grid element library (`src/elements/tui`).
- **`crates/warp_core/`**: Shared foundational utilities, platform abstractions, telemetry, and feature flags.
- **`crates/warp_tui/`**: Headless TUI runner front-end.
- **`crates/editor/`**: Rich text editing component models.
- **`crates/ipc/`**: Inter-process communication primitives.
- **`crates/graphql/`**: GraphQL client implementation and schema synchronization bindings.
- **`crates/persistence/`**: Diesel ORM integration layer interacting with a local SQLite database, managing migrations and schema (`crates/persistence/src/schema.rs`).
- **`crates/integration/`**: GUI-specific end-to-end integration and display test framework.

---

## Key Architectural Patterns

### 1. Entity-Handle System
Views and models are decoupled from direct pointer ownership through an Entity-Component-Handle architecture:
- A global `App` instance owns all entities (views and models).
- Views reference peer or child components using `ViewHandle<T>` wrappers.
- `AppContext` grants scoped, temporary mutable access to handles strictly during render or event-dispatch cycles.

### 2. Scene Graph and Rendering Pipelines
WarpUI abstracts drawing primitives through a unified `Scene` description layer (`crates/warpui_core/src/scene.rs`), which records primitives (rectangles, glyphs, images, icons) across layered z-indices (`ZIndex::Normal` and `ZIndex::Overlay`). 
- **GUI Render Path**: The scene is processed into GPU geometry via `pathfinder_geometry`, clipping boundaries, and WGSL shaders. Hit-testing is accelerated via spatial indexing using `rstar` R-Trees (`Layer::hit_map`).
- **TUI Render Path**: Cell-grid elements (`TuiElement` trait) paint directly into a virtual `TuiBuffer` cell matrix.

### 3. Text Layout and Styling Subsystem
Text processing (`crates/warpui_core/src/text_layout.rs`) supports runs, multi-font fallbacks, ligature tracking, character caret mapping, and custom baselines:
- **`TextFrame` & `Line`**: Handle multiline text frames, alignment (`TextAlignment`), and truncation clipping (`ClipConfig`, `ClipStyle::Fade` or `ClipStyle::Ellipsis`).
- **Caching**: Layout and metrics are cached per-frame using `TextCache` with swap-buffer invalidation between frames (`LayoutCache`, `Line`, `TextFrame`).
- **Decorations**: Run backgrounds, borders, soft-wrapped hyperlink underlines, strikethroughs, and error marker waves are calculated relative to baseline positions and glyph runs.

### 4. Feature Flag System
Compile-time configuration combined with runtime plumbing controls experimental features:
- Variants are declared inside the `FeatureFlag` enum (`warp_core/src/features.rs`).
- Flags are organized into rollout cohorts (`DOGFOOD_FLAGS`, `PREVIEW_FLAGS`, `RELEASE_FLAGS`).
- Dynamic runtime checks via `FeatureFlag::YourFlag.is_enabled()` are preferred over compile-time `#[cfg(...)]` directives wherever possible to allow toggling without rebuilding.

---

## Development Guidelines & Safety Rules

### Terminal Model Locking
- **Deadlock Prevention**: Terminal models (`TerminalModel`) must be handled with extreme care regarding `.lock()` calls.
- Avoid acquiring multiple locks on the same model from disparate call stacks within the same thread scope.
- Prefer passing already-locked references down call hierarchies, keeping lock scopes strictly minimized.

### Code Style & Quality Rules
- **Imports**: Place import statements at the top of files. Use absolute paths or scope-embedded imports only inside `cfg`-guarded branches.
- **Context Parameters**: Functions taking context parameters (`AppContext`, `ViewContext`, `ModelContext`) must name the parameter `ctx` and place it last in the signature (unless a closure parameter is present, which takes final position).
- **Unused Parameters**: Completely remove unused parameters rather than prefixing them with `_`.
- **Formatting**: Inline format arguments in macros (`println!`, `eprintln!`, `format!`) must satisfy Clippy's `uninlined_format_args` lint.
- **Logging**: Do not pass `Itertools::format` formatters directly into logging macros (`log::*`, `safe_*`), as formatters are single-use. Use `.join(", ")` to construct a reusable `String`.
- **Comments**: Adhere strictly to a "Why Only" comment policy. Never describe *what* or *how* self-documenting code executes. Avoid line-by-line narrations, edit transformations, or function call site listings in docstrings. Line-wrap comments up to the formatter limit of 100 columns.

### Testing & Verification
- Parallel unit and integration execution relies on `cargo nextest`.
- **GUI Tests**: Uses the custom integration harness in `crates/integration/`.
- **TUI Tests**: Validated through render-to-lines unit tests.