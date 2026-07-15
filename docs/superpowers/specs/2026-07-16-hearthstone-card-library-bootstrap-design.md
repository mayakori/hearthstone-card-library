# Hearthstone Card Library Bootstrap Design

**Date:** 2026-07-16
**Status:** Approved for implementation planning
**Project path:** `C:\Users\main\Desktop\claude_project\hearthstone-card-library`

## Goal

Create an AI-development-ready Tauri v2 project skeleton for a future Hearthstone card library. The bootstrap must start cleanly, build on Windows, keep domain code independent from Tauri, and give Codex and Claude consistent project instructions.

The first implementation slice establishes the development environment only. It does not implement card fetching, persistence, image downloads, filtering, or a finished UI.

## Decisions

- Desktop shell: Tauri v2.
- Frontend: SolidJS, Vite, and TypeScript.
- Native language: Rust using the stable MSVC toolchain.
- Package manager: npm.
- Repository: a new Git repository on the `main` branch.
- Code intelligence: the locally installed CodeGraph MCP server.
- Reference project: `C:\Users\main\Desktop\VSCode_Project\dc_browser`.
- Reuse strategy: transplant the reference project's clean architectural and AI-workflow patterns instead of cloning and pruning the entire application.

## Repository Structure

```text
hearthstone-card-library/
├─ crates/
│  ├─ hs-core/                 # Pure Rust domain/data library; no Tauri dependency
│  └─ hs-tauri/                # Thin Tauri application and IPC boundary
├─ frontend/                   # SolidJS + Vite + TypeScript
├─ docs/
│  └─ superpowers/
│     ├─ specs/
│     └─ plans/
├─ .agents/
│  └─ skills/                  # Only portable project-local skills
├─ .claude/
├─ .codex/
├─ .mcp.json
├─ AGENTS.md
├─ CLAUDE.md
├─ Cargo.toml
├─ README.md
└─ .gitignore
```

The root `Cargo.toml` is a workspace containing `hs-core` and `hs-tauri`. The Tauri configuration lives in `crates/hs-tauri`; its development URL and production asset directory point to the separate `frontend` project.

## Component Boundaries

### `hs-core`

This crate will eventually own Hearthstone card types, official-site API access, pagination, normalization, caching, and query logic. It must remain free of Tauri types so its behavior can be unit tested without launching a desktop shell.

For the bootstrap, it contains only a minimal library target and a smoke test proving the workspace is wired correctly.

### `hs-tauri`

This crate owns application startup, Tauri configuration, capabilities, and future IPC commands. IPC handlers should remain thin: validate input, call `hs-core`, and translate results into serializable responses.

For the bootstrap, it starts a window, serves the frontend, and exposes one read-only `runtime_info` command that returns the app name and version. This proves the frontend-to-Rust IPC boundary works without introducing card behavior.

### `frontend`

The frontend owns presentation and local UI state. It will eventually render card grids and implement search, filters, sorting, and details. Data acquisition and durable caching stay behind Tauri commands rather than being duplicated in browser code.

For the bootstrap, it renders a small project-ready screen and displays the result of `runtime_info`. A direct browser launch shows an explicit "Tauri runtime required" state instead of a generic JavaScript failure.

## Intended Future Data Flow

```text
SolidJS UI
  → typed Tauri invoke
  → thin command in hs-tauri
  → hs-core fetch/cache/query service
  → official Hearthstone site API
  → normalized result back to the UI
```

The official card endpoint and cache schema will be specified in a later feature slice. API credentials, scraping fallbacks, and bulk image storage are explicitly outside this bootstrap.

## AI Development Setup

### Shared instructions

`AGENTS.md` will be adapted from the current working copy of the reference project's `AGENTS.md`, not copied blindly. It will retain:

- think-before-coding guidance;
- simplicity and surgical-change rules;
- goal-driven, verifiable execution;
- worktree-based feature development;
- explicit skill routing and verification-before-completion rules.

It will remove DCInside-specific absolute paths, F/D task codes, TODO node-view requirements, and assumptions about scripts that do not exist in the new repository. New absolute paths and commands will reference this project only.

`CLAUDE.md` will express the same core behavioral contract for Claude-compatible agents. Project facts and commands must agree between the two files.

### Local agent tooling

- `.codex/config.toml` and `.mcp.json` configure the installed `codegraph serve --mcp` command.
- `.claude/settings.json` permits the CodeGraph read tools used by the reference project.
- `.claude/CLAUDE.md` contains the portable CodeGraph usage guidance.
- The portable `ez` and `grill-me` local skills are copied into `.agents/skills` and `.claude/skills`.
- The reference project's `sdd`, `va`, and `guarding-todo-node-view` skills are excluded because they depend on DCInside-specific documents, paths, and workflow conventions.
- Generated CodeGraph indexes, Claude local overrides, logs, worktrees, build outputs, and secrets are ignored by Git.

## Error Handling

Bootstrap commands must fail visibly with their original tool output. Configuration must not silently fall back to a browser-only mode because that would hide missing Tauri IPC behavior.

The starter UI invokes `runtime_info`, distinguishes a normal Tauri response from an unsupported direct-browser launch, and displays a concise failure state. Rust command failures are returned as serializable errors rather than causing a panic.

## Verification

Implementation is complete only when every following check passes from the new repository:

1. `npm --prefix frontend install` completes and creates a lockfile.
2. `npm --prefix frontend run build` passes TypeScript checking and Vite production build.
3. `npm --prefix frontend run test` passes the starter Vitest tests, including the runtime success and unsupported-browser states.
4. `cargo fmt --all -- --check` passes.
5. `cargo test --workspace` passes.
6. `cargo check --workspace` passes.
7. `codegraph init -i` completes and CodeGraph reports a usable index.
8. `cargo tauri dev` launches the desktop application without a configuration or compilation error, then the smoke process is stopped cleanly.
9. `git status --short` contains no generated build products or local-only configuration.

## Success Criteria

- A developer or coding agent can clone/open the repository, install frontend dependencies, and run the Tauri application using documented commands.
- Rust domain code and the Tauri boundary are separate workspace crates.
- Codex and Claude receive consistent project-specific instructions without stale `dc_browser` paths.
- CodeGraph is configured and its generated state is excluded from version control.
- The initial commit history contains the reviewed design and a separate, verified bootstrap implementation commit.

## Deferred Work

- Final contract for `https://hearthstone.blizzard.com/api/cards`.
- Pagination, locale, metadata, and rate-limit behavior.
- SQLite or file-cache schema.
- Card image caching policy.
- Search/filter/query model.
- Finished card-library UI and visual design.
- Packaging, signing, auto-update, and release automation.
