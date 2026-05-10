# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> For full architecture, data flow, DB schema, and build order see **README.md** — that is the single source of truth for how this project works.

## Commands

```bash
# Full app (Vite frontend + Tauri/Rust backend)
pnpm tauri dev
pnpm tauri build

# Frontend only
pnpm dev        # Vite dev server on port 1420
pnpm build      # tsc + vite build → ../dist

# Rust
cargo build     # from src-tauri/ or workspace root
cargo test      # run all Rust tests
cargo test --package audio_engine   # test a single crate

# Integration test (runs the full audio pipeline as a binary)
cargo run --bin integration --manifest-path src-tauri/Cargo.toml
```

No JavaScript test framework is configured yet.

## Code conventions

**Modularity over monoliths.** Every component — Rust struct or React component — exposes explicit lifecycle methods (`start`/`stop`, `connect`/`disconnect`, etc.) rather than one large function that does everything. Logic is split at natural boundaries so each piece can be tested and replaced independently.

Rust example — prefer:
```rust
impl AudioManager {
    pub fn start(&mut self) -> Result<()> { ... }
    pub fn stop(&mut self) -> Result<()> { ... }
}
```
over a single `run_audio_pipeline()` that captures, resamples, and feeds VAD in one body.

## Key constraints

- **macOS only** — requires `com.apple.security.device.audio-input` entitlement for mic access
- Recommended hardware: Apple M-series with ≥ 48 GB RAM (Qwen3-Omni-30B)
- `pnpm` is the required package manager (enforced in `tauri.conf.json`)
- TypeScript is configured with `strict: true` and `noUnusedLocals: true`

<!-- scout -->
## Project knowledge base (scout)

This project has a scout knowledge base in `.scout/`. At the start of every
conversation, read `.scout/architecture.md` to understand the codebase structure
before answering questions or making changes.

For semantic search over the codebase:

```bash
kb query nihongo "your question here"
kb query nihongo "your question here" --k 8   # more results
```

Prefer `kb query` over guessing when asked about specific implementation details,
data flow, or where something is defined.
