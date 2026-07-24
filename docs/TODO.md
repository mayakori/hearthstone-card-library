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

- Status: `done`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-18`
- Codex: `HCL-004 · 현재 저장소 기준선 감사·정리`
- Branch: `main`
- Worktree: `main`
- Depends on: `HCL-003`
- Spec: `—`
- Plan: `docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md`
- Next gate: start HCL-005 approved product/design workflow
- Blocked: `—`

### Goal

Establish a verified Git baseline without absorbing unrelated user changes.

### Progress

The audit classified 129 original untracked files, committed all 123 intended project files in two scoped commits (75 application/config and 48 fixture/design/preview), and kept all 6 local/generated candidates ignored. Forbidden tracked paths remain at 0. A later preview edit owned by the active UI task is explicitly excluded and remains unstaged.

### Verification

Focused design tests passed 5/5. The Task 3 full gate passed with tracking tests 15/15, Vitest 6/6, TypeScript/Vite build success, and Rust check success. Read-only review returned Spec Approved and Quality Approved with one Minor follow-up for the machine-local path in `docs/handoff-ui-design.md:7`; there were no Critical or Important findings.

## HCL-005 — 기본 UI와 사용자 흐름 설계

- Status: `in_progress`
- Priority: `P1`
- Type: `design`
- Updated: `2026-07-24`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-004`
- Spec: `docs/design/hearthstone-workbench-main-screen.md`
- Plan: `—`
- Next gate: start the separate component-contract design session with `docs/handoff-ui-design.md`
- Blocked: `—`

### Goal

Approve the desktop information architecture, core user flow, advanced filters, card details, and deck editing flow before runtime implementation.

### Progress

The main workbench is approved and documented in `docs/design/hearthstone-workbench-main-screen.md`. It uses a full-height, two-pane layout without a global top command bar and falls back to a single-panel switch when the library cannot preserve three card columns together with an untruncated deck list.

Approved layout scope:

- [x] Record the approved main-screen reference in `docs/design/hearthstone-workbench-main-screen.md`.
- [x] Freeze the `WorkbenchLayout` split between `CardLibraryPane` and `DeckPane` at 720p, 1080p, and 1440p.
- [x] Define independent `ScrollArea` behavior for the card library and deck list, including stable scrollbar gutters.
- [x] Confirm the three-level component boundary: screen containers, domain components, and shared primitives.
- [x] Save the separate-session prompt in `docs/handoff-ui-design.md`.

Next component-contract scope:

- [ ] Audit `preview/hearthstone-component-map.html` against the approved reference and current responsive mockup.
- [ ] Define each component's responsibilities, props, events, state ownership, and derived state.
- [ ] Define default, empty, disabled, focus, hover, edit, drag, loading, and error states where applicable.
- [ ] Define DOM, keyboard, accessibility, and responsive invariants.
- [ ] Define component contract tests and the shared-primitive conformance suite.
- [ ] Define Workbench composition tests and the minimum visual-regression matrix.
- [ ] Save only user-approved component contracts under `docs/design/`.

Deferred follow-up:

- [ ] Design and componentize `CardDetailDialog` after the base workbench layout is approved; include right-click opening, focus return, background locking, and small-viewport internal scrolling.

### Verification

The preview was checked at the 16px-font split boundary: 1280px renders a 960px library and 320px deck, 824px preserves three library columns and a 243px deck, and 823px switches to a single panel without page overflow. The approved document captures the 12–20px font-dependent thresholds and the component-test invariants. The current handoff prompt points the next session to the approved reference and explicitly prevents runtime implementation before component-contract approval.

## HCL-006 — 공식 Hearthstone API 어댑터

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-23`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-005`
- Spec: `—`
- Plan: `—`
- Next gate: review the official API guide and freeze auth, pagination, rate-limit, retry, and fixture contracts
- Blocked: `HCL-005 design approval is required before runtime implementation`

### Goal

Collect official Korean card data through the documented Hearthstone API behind a Rust adapter while preserving official IDs, slugs, locale, and source metadata.

### Progress

The earlier website-internal fetch approach is discarded. The official API guide is the source of truth, and fetch details are intentionally deferred until its authentication, pagination, rate-limit, retry, and response contracts are reviewed. Existing website-proxy fixtures and preview experiments remain exploratory only.

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
- Updated: `2026-07-23`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-007`
- Spec: `—`
- Plan: `—`
- Next gate: design the filter expression model and snippet/autocomplete interaction against normalized card data
- Blocked: `HCL-007 normalized schema is required`

### Goal

Search cards with nested AND/OR expressions beyond the in-game search syntax.

### Progress

The existing offline preview demonstrates keyboard condition input, but production search snippets and autocomplete are not implemented.

Planned follow-up search UX:

- [ ] Offer reusable snippets for common card-property and semantic-effect conditions.
- [ ] Provide keyboard-first autocomplete for fields, operators, values, and known card-pool or synergy names.
- [ ] Scope suggestions to the active AND/OR/NOT input mode without replacing manual text entry.

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

## HCL-011 — 서버리스 한 번 클릭 앱 업데이트

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-23`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-005`
- Spec: `—`
- Plan: `—`
- Next gate: freeze the public release endpoint and signing-key ownership, then specify the GitHub Actions and Tauri updater flow
- Blocked: `HCL-005 design approval and the public release repository decision are required`

### Goal

Ship serverless application updates through GitHub Actions and GitHub Releases. The app checks for a new signed version at launch; after the user clicks `업데이트` once, it saves in-progress state, downloads and verifies the artifact, installs it in Windows passive mode, and relaunches the updated app without further input.

### Progress

The one-click update UX is approved. Fully unattended mid-session installation is rejected because it could interrupt deck editing or search work. Update failures must preserve the currently installed version and expose a retry action.

### Verification

Not started.
