# Hearthstone Card Library — Active Backlog & Resume Notes

> `docs/TODO.md` is the active-work authority. `docs/kanban.html` mirrors every
> task below, while archived work lives in `docs/DONE.md`. These three files are
> Git-tracked and may be edited only in the main worktree.

## Status Guide

- `backlog`: not yet scoped or waiting for priority
- `ready`: scope, done criteria, and dependencies are clear
- `in_progress`: active implementation or design work
- `verify`: implementation complete; validation, review, or merge remains
- `done`: verified and present on main; retained until Done exceeds 10 cards
- Blocked work stays in its current status and records the unblock condition.

## HCL-003 — 칸반 기반 프로젝트 관리 도입

- Status: `done`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-18`
- Codex: `HCL-003 · 칸반 기반 프로젝트 관리 도입`
- Branch: `main`
- Worktree: `main`
- Depends on: `—`
- Spec: `docs/superpowers/specs/2026-07-17-hcl-003-kanban-project-tracking-design.md`
- Plan: `docs/superpowers/plans/2026-07-18-hcl-003-kanban-project-tracking.md`
- Next gate: start HCL-004 repository baseline audit
- Blocked: `—`

### Goal

Adopt TODO/DONE/worktree tracking and replace dc_browser-style node views with a local HTML Kanban board.

### Progress

The user approved Git-tracked main-only tracking files, HCL sequential IDs, the five-column state model, and validator-enforced TODO/Kanban synchronization.

### Verification

`npm run check` and `git diff --check` passed. The local Kanban passed search, type, priority, active-only, detail-link, 390px responsive, and console smoke checks.

## HCL-004 — 현재 저장소 기준선 감사·정리

- Status: `ready`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-003`
- Spec: `—`
- Plan: `—`
- Next gate: audit ownership and verification state of every untracked project file
- Blocked: `—`

### Goal

Establish a verified Git baseline without absorbing unrelated user changes.

### Progress

The app scaffold, fixtures, previews, and project instructions exist locally but most are not tracked.

### Verification

Not started.

## HCL-005 — 기본 UI와 사용자 흐름 설계

- Status: `backlog`
- Priority: `P1`
- Type: `design`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-004`
- Spec: `—`
- Plan: `—`
- Next gate: run the approved GStack product and design workflow
- Blocked: `HCL-004 baseline must be established first`

### Goal

Approve the desktop information architecture, core user flow, advanced filters, card details, and deck editing flow before runtime implementation.

### Progress

The handoff prompt exists at `docs/handoff-ui-design.md`.

### Verification

Not started.

## HCL-006 — 공식 카드 엔드포인트 어댑터

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-004`
- Spec: `—`
- Plan: `—`
- Next gate: research the current official endpoint and freeze fixture contracts
- Blocked: `HCL-004 baseline must be established first`

### Goal

Collect official Korean card data behind a Rust adapter while preserving official IDs, slugs, locale, and source metadata.

### Progress

Only temporary website-proxy fixtures and preview experiments exist.

### Verification

Not started.

## HCL-007 — 원본 캐시와 정규화 로컬 DB

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-006`
- Spec: `—`
- Plan: `—`
- Next gate: define raw-cache and normalized-schema boundaries after the adapter contract
- Blocked: `HCL-006 adapter contract is required`

### Goal

Persist raw official responses separately from the normalized local card model.

### Progress

No production database contract has been approved.

### Verification

Not started.

## HCL-008 — 기본 검색과 AND/OR 필터

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-007`
- Spec: `—`
- Plan: `—`
- Next gate: design the filter expression model against normalized card data
- Blocked: `HCL-007 normalized schema is required`

### Goal

Search cards with nested AND/OR expressions beyond the in-game search syntax.

### Progress

The existing offline preview demonstrates only a small fixed AND filter sample.

### Verification

Not started.

## HCL-009 — 카드 효과 의미 태깅과 시너지 모델

- Status: `backlog`
- Priority: `P2`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-008`
- Spec: `—`
- Plan: `—`
- Next gate: define inferred semantic fields and provenance after base search works
- Blocked: `HCL-008 query model is required`

### Goal

Store inferred effect semantics separately from official text and expose synergy, card-pool, and bucket relationships.

### Progress

The semantic sample fixture contains only exploratory predicates.

### Verification

Not started.

## HCL-010 — 덱 편집기와 덱 코드 가져오기·내보내기

- Status: `backlog`
- Priority: `P2`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-008, HCL-009`
- Spec: `—`
- Plan: `—`
- Next gate: approve deck rules, validation, statistics, and deck-code flow
- Blocked: `HCL-008 search and HCL-009 semantic relationships are required`

### Goal

Build and validate decks from search results and import/export Hearthstone deck codes.

### Progress

No production deck model exists.

### Verification

Not started.
