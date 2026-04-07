# MemPalace Rustification Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current Python-first core path with a native Rust implementation that preserves the existing MemPalace CLI and searchable local-memory workflow.

**Architecture:** Start from the currently tested contract surface instead of the largest modules. Build a Rust crate that owns config loading, transcript normalization, local storage, project mining, conversation mining, and search. Keep the higher-risk AAAK, MCP, knowledge graph, and wake-up layers as phase-two ports after the core storage path is proven.

**Tech Stack:** Rust 2024, `clap`, `serde`, `serde_json`, `serde_yaml`, `rusqlite`, `walkdir`, `sha2`, `tempfile`

---

### Task 1: Freeze the Migration Boundary

**Files:**
- Create: `docs/plans/2026-04-07-mempalace-rustification.md`
- Modify: `README.md`

**Step 1: Document the target behavior**

- Keep the existing command surface centered on `init`, `mine`, `search`, and `status`.
- Preserve local-only storage and zero cloud dependencies.
- Preserve transcript normalization for plain text and simple JSON exports.

**Step 2: Call out deferred surfaces**

- Phase two: `compress`, `wake-up`, `layers`, `AAAK`, `knowledge_graph`, `mcp_server`.
- Make the README explicit about what is already Rust-native versus what remains queued.

**Step 3: Verify the document is actionable**

Run: `sed -n '1,220p' docs/plans/2026-04-07-mempalace-rustification.md`
Expected: clear scope, architecture, and task breakdown.

### Task 2: Create a Rust Contract Harness

**Files:**
- Create: `Cargo.toml`
- Create: `tests/rust_config.rs`
- Create: `tests/rust_normalize.rs`
- Create: `tests/rust_mining.rs`

**Step 1: Write failing Rust tests**

- Config tests for defaults, env overrides, and init.
- Normalize tests for plain text, empty files, and Claude-style JSON arrays.
- Mining tests for project ingest, conversation ingest, and search recall.

**Step 2: Run tests to prove the harness is red**

Run: `cargo test`
Expected: compile failure because the Rust library does not exist yet.

### Task 3: Implement the Rust Core

**Files:**
- Create: `src/lib.rs`
- Create: `src/main.rs`
- Create: `src/error.rs`
- Create: `src/config.rs`
- Create: `src/normalize.rs`
- Create: `src/storage.rs`
- Create: `src/room_detector.rs`
- Create: `src/miner.rs`
- Create: `src/convo.rs`
- Create: `src/search.rs`

**Step 1: Implement configuration and file normalization**

- Match the Python config priority: env > `config.json` > defaults.
- Normalize plain text and basic JSON chat exports into transcript text.

**Step 2: Implement local storage**

- Use SQLite under the palace path instead of ChromaDB.
- Store verbatim drawers plus searchable metadata.
- Expose count and search helpers for tests and CLI commands.

**Step 3: Implement mining flows**

- Project mining: scan readable files, route to rooms, chunk content, persist drawers.
- Conversation mining: normalize transcripts, chunk exchanges, detect rooms, persist drawers.

**Step 4: Implement CLI parity for the core path**

- `init` writes `mempalace.yaml`.
- `mine` supports `projects` and `convos`.
- `search` prints verbatim hits.
- `status` prints counts by wing and room.

### Task 4: Re-verify and Mark Remaining Gaps

**Files:**
- Modify: `README.md`

**Step 1: Run Rust verification**

Run: `cargo test`
Expected: all Rust tests pass.

**Step 2: Run a smoke command**

Run: `cargo run -- status --palace /tmp/mempalace-smoke`
Expected: a friendly empty-palace message or zero-count summary.

**Step 3: Update user-facing docs**

- Add a Rust section to the README.
- Call out the still-unported Python modules so follow-up work is explicit.
