# HCL-004 Repository Baseline Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing local scaffold, fixtures, design artifacts, previews, and Tauri application into a verified Git baseline without absorbing transient agent state or generated output.

**Architecture:** This is a main-worktree baseline exception because the untracked source files being audited do not exist in a separate Git worktree. The audit first records ownership and ignore policy, then stages explicit path groups, verifies the staged inventory and full project gate, and finally advances the canonical TODO/Kanban state.

**Tech Stack:** Git, PowerShell, Node.js 24+, Vitest, TypeScript, Vite, Rust/Cargo, Tauri 2

## Global Constraints

- `docs/TODO.md`, `docs/DONE.md`, and `docs/kanban.html` are edited and committed only from `main`.
- Preserve every pre-existing user file; do not delete, rewrite, or reformat application content during this audit.
- Do not stage `.superpowers/`, `.worktrees/`, dependency trees, build output, environment files, logs, or generated Tauri schema caches.
- Stage baseline files with explicit paths. Never use `git add .` or an equivalent broad add.
- Treat the current local card mocks and previews as project artifacts only after their focused tests and the full `npm run check` gate pass.
- Do not modify `src/`, `src-tauri/src/`, fixture payloads, or preview behavior merely to make the baseline look cleaner. A genuine failure becomes a separate task unless it is a direct baseline configuration defect.
- Completion requires zero unexpected untracked project files, `npm run check` exit 0, `git diff --check` exit 0, and a read-only review with no Critical or Important findings.

---

### Task 1: Start HCL-004 on the main authority

**Files:**
- Create: `docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md`
- Modify: `docs/TODO.md`
- Modify: `docs/kanban.html`

**Interfaces:**
- Consumes: HCL-003 main-only tracking workflow and the existing HCL-004 Ready card.
- Produces: An `in_progress` HCL-004 authority with Codex title, main-worktree exception, plan path, and an executable next gate.

- [ ] **Step 1: Record the main-worktree exception and plan**

Set the HCL-004 fields in both authority files to:

```text
Status: in_progress
Codex: HCL-004 · 현재 저장소 기준선 감사·정리
Branch: main
Worktree: main
Plan: docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md
Next gate: classify all 129 currently untracked files as project, generated, or local-only
```

- [ ] **Step 2: Verify authority synchronization**

Run:

```powershell
npm run tracking:check
npm run tracking:test
```

Expected: `Project tracking OK: 8 active cards, 2 archived tasks.` and all tracking tests pass.

- [ ] **Step 3: Commit the start state**

```powershell
git add -- docs/TODO.md docs/kanban.html docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md
git commit -m "docs(HCL-004): plan repository baseline audit"
```

### Task 2: Record ownership and ignore policy

**Files:**
- Modify: `.gitignore`
- Create: `docs/superpowers/reports/2026-07-18-hcl-004-repository-baseline-audit.md`

**Interfaces:**
- Consumes: `git ls-files --others --exclude-standard`, directory inventory, secret scan, and verification output.
- Produces: A durable classification that later reviewers can compare with the staged baseline.

- [ ] **Step 1: Capture the pre-change RED inventory**

Run:

```powershell
$files = @(git ls-files --others --exclude-standard)
$files.Count
git status --short
```

Expected before ignore policy: 129 untracked files plus the pre-existing `.gitignore` modification.

- [ ] **Step 2: Extend ignore policy only for local/generated artifacts**

Append these exact entries while preserving the existing ignore rules:

```gitignore
.superpowers/
src-tauri/gen/schemas/
```

`.superpowers/` contains local orchestration state and review reports. `src-tauri/gen/schemas/` is generated Tauri editor/capability schema output; `src-tauri/capabilities/default.json` remains tracked as the source configuration.

- [ ] **Step 3: Write the audit report with the approved classification**

The report must include this exact ownership table:

| Classification | Paths | Decision |
| --- | --- | --- |
| Agent contracts | `.codex/README.md`, `.codex/agents/*.toml` | Track |
| App entry/config | `index.html`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `assets/app-icon.svg` | Track |
| Frontend | `src/**` | Track |
| Tauri backend/package assets | `src-tauri/**` except ignored `target/` and `gen/schemas/` | Track |
| Official-data fixtures | `data/fixtures/**` | Track |
| Approved design records | `docs/design/**`, `docs/handoff-ui-design.md` | Track |
| Independent feasibility preview | `preview/**` | Track |
| Local agent execution state | `.superpowers/**` | Ignore |
| Generated/dependency/build state | `.worktrees/`, `node_modules/`, `dist/`, `target/`, `src-tauri/target/`, `src-tauri/gen/schemas/`, `.vite/`, `.gstack/` | Ignore |
| Secrets and machine state | `.env`, `.env.*` except `.env.example`, `*.log`, `.DS_Store` | Ignore |

Also record: pre-change count `129`, no secret-pattern hits outside false-positive source text, focused card mock test `4/4`, and the exact final verification results.

- [ ] **Step 4: Verify ignore behavior**

Run:

```powershell
git check-ignore -v .superpowers/sdd/progress.md
git check-ignore -v src-tauri/gen/schemas/desktop-schema.json
git ls-files --others --exclude-standard
```

Expected: the first two paths are ignored by `.gitignore`; remaining output contains only the project artifacts listed as Track.

- [ ] **Step 5: Commit policy and report**

```powershell
git add -- .gitignore docs/superpowers/reports/2026-07-18-hcl-004-repository-baseline-audit.md
git commit -m "docs(HCL-004): record repository baseline policy"
```

### Task 3: Track the verified project baseline

**Files:**
- Add: `.codex/README.md`
- Add: `.codex/agents/*.toml`
- Add: `assets/app-icon.svg`
- Add: `data/fixtures/*.json`
- Add: `docs/design/*`
- Add: `docs/handoff-ui-design.md`
- Add: `index.html`
- Add: `package-lock.json`
- Add: `preview/**`
- Add: `src/**`
- Add: `src-tauri/**` except ignored generated/build paths
- Add: `tsconfig.json`
- Add: `vite.config.ts`

**Interfaces:**
- Consumes: the Task 2 classification and current passing local files.
- Produces: a cloneable application/design baseline that `npm install` and the documented commands can reproduce.

- [ ] **Step 1: Scan candidate text files for secret-shaped content**

Run:

```powershell
rg -n -uu -g '!node_modules/**' -g '!target/**' -g '!src-tauri/target/**' -g '!dist/**' -g '!package-lock.json' -g '!*.png' -g '!*.jpg' -g '!*.ico' -g '!*.icns' "(BEGIN (RSA|OPENSSH|EC) PRIVATE KEY|api[_-]?key\s*[:=]|secret\s*[:=]|password\s*[:=]|token\s*[:=])" .codex assets data docs index.html preview src src-tauri tsconfig.json vite.config.ts
```

Expected: no credential or private-key findings. Source-code matches that merely mention a token variable must be documented as false positives.

- [ ] **Step 2: Stage explicit application/config paths**

```powershell
git add -- .codex/README.md .codex/agents assets/app-icon.svg index.html package-lock.json src src-tauri tsconfig.json vite.config.ts
git diff --cached --name-only
git diff --cached --check
```

Expected: no ignored `src-tauri/gen/schemas/` or `src-tauri/target/` path and no whitespace error.

- [ ] **Step 3: Run the full gate and commit the application baseline**

```powershell
npm run check
git commit -m "chore(HCL-004): track application baseline"
```

Expected: tracking tests 15/15, Vitest 5/5, TypeScript/Vite build success, and Rust `cargo check` success.

- [ ] **Step 4: Stage explicit fixture/design/preview paths**

```powershell
git add -- data/fixtures docs/design docs/handoff-ui-design.md preview
git diff --cached --name-only
git diff --cached --check
```

Expected: only the approved fixture, design, and independent preview artifacts.

- [ ] **Step 5: Verify and commit project artifacts**

```powershell
npx vitest run docs/design/card-data-contract-mock.test.ts
npm run check
git commit -m "docs(HCL-004): track data and design artifacts"
```

Expected: card mock tests 4/4 and the full gate pass.

### Task 4: Review and close the baseline audit

**Files:**
- Modify: `docs/superpowers/reports/2026-07-18-hcl-004-repository-baseline-audit.md`
- Modify: `docs/TODO.md`
- Modify: `docs/kanban.html`

**Interfaces:**
- Consumes: all HCL-004 commits, full test output, status inventory, and read-only reviewer findings.
- Produces: HCL-004 `done`, HCL-005 `ready`, and a precise next design gate.

- [ ] **Step 1: Verify the final inventory**

Run:

```powershell
git ls-files --others --exclude-standard
git status --short
npm run check
npm run tracking:check
git diff --check
```

Expected: no unexpected untracked project file; only ignored local/generated state may remain; all commands exit 0.

- [ ] **Step 2: Request a read-only review**

The reviewer must compare the implementation range with this plan and confirm:

- no secret, generated schema, dependency tree, build output, or agent-local state is tracked;
- every classified project source, fixture, design, preview, lockfile, icon, and role contract is tracked;
- tests and README commands are reproducible;
- no existing application content was rewritten merely for baseline cleanup.

Critical and Important findings must be fixed and re-reviewed before proceeding.

- [ ] **Step 3: Advance the canonical queue**

Apply these exact state changes in TODO and Kanban:

```text
HCL-004: status done; next gate start HCL-005 approved product/design workflow
HCL-005: status ready; Blocked empty
HCL-006: status backlog; Depends on HCL-005; Blocked HCL-005 design approval is required before runtime implementation
```

Record the final test counts and review verdict in HCL-004 Verification.

- [ ] **Step 4: Verify and commit the management closeout**

```powershell
npm run tracking:check
npm run check
git add -- docs/TODO.md docs/kanban.html docs/superpowers/reports/2026-07-18-hcl-004-repository-baseline-audit.md
git commit -m "chore(HCL-004): complete repository baseline audit"
```

Expected: HCL-004 appears in Done, HCL-005 in Ready, HCL-006 in Backlog, and every full gate passes.

## Self-Review

- Spec coverage: The plan covers all current untracked top-level groups, local/generated exclusions, secret scanning, explicit staging, reproducibility, review, and queue advancement.
- Placeholder scan: The plan contains no TBD, deferred implementation, or unspecified test step.
- Type consistency: The exact task IDs, statuses, commands, file paths, and required column values match the HCL tracking contract.

