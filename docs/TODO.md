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
- Updated: `2026-07-26`
- Codex: `HCL-005 · 기본 UI와 사용자 흐름 설계`
- Branch: `codex/hcl-005-frontend-contract`
- Worktree: `hcl-005-frontend-contract`
- Depends on: `HCL-004`
- Spec: `docs/design/hearthstone-workbench-main-screen.md`
- Plan: `—`
- Next gate: continue component-contract design in the assigned worktree from `docs/handoff-ui-design.md`
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

The preview was checked at the 16px-font split boundary: 1280px renders a 960px library and 320px deck, 824px preserves three library columns and a 243px deck, and 823px switches to a single panel without page overflow. The approved document now defines a 12–24px UI root-font range in 2px steps while retaining actual rendered text measurement as the source of deck minimum width. The current handoff prompt points the next session to the approved reference and explicitly prevents runtime implementation before component-contract approval.

## HCL-006 — 공식 Hearthstone API 수집·정규화 파이프라인

- Status: `done`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-08-06`
- Codex: `HCL-006 · 공식 Hearthstone API 수집·정규화 파이프라인`
- Branch: `main`
- Worktree: `main`
- Depends on: `HCL-004`
- Spec: `docs/superpowers/specs/2026-07-25-hcl-006-official-card-data-pipeline-design.md`
- Plan: `docs/superpowers/plans/2026-08-05-hcl-006-official-card-data-pipeline.md`
- Next gate: start HCL-014 Raw R2 candidate publication automation
- Blocked: `—`

### Goal

Build a trusted Rust data-pipeline CLI that collects official Korean and English card data, preserves locale-specific Raw snapshots, normalizes the data, and produces verified SQLite packages outside the Tauri runtime.

### Progress

The earlier website-internal fetch approach is discarded. The official Game Data API and official patch notes are the sources of truth. The approved R2 release design places collection and normalization in a trusted GitHub Actions pipeline while the packaged Tauri app downloads only verified normalized data and images.

Approved design progress:

- [x] Separate GitHub Actions collection and normalization from the packaged Tauri runtime.
- [x] Keep `card-data-contract` and `card-data-pipeline` as isolated Rust workspace crates in the same repository.
- [x] Preserve Raw and normalized data separately and support `ko_KR` and `en_US` from the first release.
- [x] Consolidate schema, query-pool, version, image, R2 and updater decisions in `docs/design/card-data-architecture-decisions.md`.
- [x] Record the separate-session resume context and first unresolved collection question in `docs/handoff-hcl-006-official-api-pipeline.md`.
- [x] Freeze OAuth, endpoints, metadata, pagination, timeout, retry and Raw envelope contracts.
- [x] Freeze deterministic normalized SQLite, zstd and manifest output contracts.
- [x] Approve the implementation specification and write the implementation plan.

### Verification

The main branch now contains the approved two-locale official API collector, Raw preservation, normalized 13-table `STRICT` SQLite output, deterministic zstd package and manifest, JSONL CLI, offline end-to-end coverage, and a credentialed live smoke. The final live dataset excluded all 69 alternate hero skins while preserving their official relation IDs and retained non-skin gameplay references.

## HCL-007 — 카드 데이터 패키지 설치·로컬 활성화

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-08-05`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-006`
- Spec: `—`
- Plan: `—`
- Next gate: define verified package installation, locale DB activation, rollback and local lifecycle after HCL-006
- Blocked: `—`

### Goal

Consume a verified HCL-006 package in the Tauri backend, install the selected locale SQLite into the local cache, atomically activate it, and preserve the previously active database when verification or activation fails.

### Progress

HCL-006 owns upstream API collection, Raw preservation, normalization and package production. HCL-007 starts at the package-consumer boundary and does not duplicate those responsibilities. R2 scheduling and publication remain outside this local installation slice.

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
- Blocked: `HCL-007 must expose an activated locale SQLite to the Rust search backend`

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

## HCL-012 — ez 로컬 스킬 포팅

- Status: `done`
- Priority: `P2`
- Type: `chore`
- Updated: `2026-07-26`
- Codex: `HCL-012 · ez 로컬 스킬 포팅`
- Branch: `main`
- Worktree: `main`
- Depends on: `—`
- Spec: `docs/design/2026-07-26-hcl-012-ez-skill-port.md`
- Plan: `docs/plans/2026-07-26-hcl-012-ez-skill-port.md`
- Next gate: `—`
- Blocked: `—`

### Goal

Restore the preserved `/ez` explanation skill in both project-local agent discovery paths without reintroducing GStack or Superpowers workflow dependencies.

### Progress

The preserved `/ez` skill is tracked in both `.agents/skills/ez/` and `.claude/skills/ez/`. The copies remain text-identical to each other and semantically identical to the workflow-repair backup, without a global installation or GStack/Superpowers dependency.

### Verification

Both skill directories passed `quick_validate.py`. The red/green repository contract test passed as part of `npm run check` (tracking 20/20, Vitest 6/6, build, and Rust check), `/va HCL-012` returned `clean`, and `npm run merge:check -- HCL-012` passed before squash merge.

## HCL-013 — sdd 로컬 스킬 포팅

- Status: `done`
- Priority: `P2`
- Type: `chore`
- Updated: `2026-07-26`
- Codex: `HCL-013 · sdd 로컬 스킬 포팅`
- Branch: `main`
- Worktree: `main`
- Depends on: `—`
- Spec: `docs/design/2026-07-26-hcl-013-sdd-skill-port.md`
- Plan: `docs/plans/2026-07-26-hcl-013-sdd-skill-port.md`
- Next gate: `—`
- Blocked: `—`

### Goal

Restore the `dc_browser` `/sdd` handoff skill in both project-local agent discovery paths, adapted to this repository's HCL worktree and squash-merge workflow without GStack, Superpowers, or node-view dependencies.

### Progress

The `/sdd` skill is tracked in both project-local discovery paths with identical HCL-native definitions. It preserves the original single-copy-block handoff behavior while excluding the old repository's node-view, development-command, and process dependencies.

### Verification

The RED consumer omitted the handoff; fresh GREEN consumers produced exactly one self-contained code block for both argument and no-argument forms while preserving supplied task IDs. Both skill validators passed, tracking tests passed 21/21, Vitest passed 6/6, build and Rust check passed, independent review reported no remaining Critical or Important findings, `/va HCL-013` returned `clean`, and `npm run merge:check -- HCL-013` passed.

## HCL-014 — R2 Raw 후보 업로드 자동화

- Status: `done`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-08-06`
- Codex: `HCL-014 · R2 Raw 후보 업로드 자동화`
- Branch: `main`
- Worktree: `main`
- Depends on: `HCL-006`
- Spec: `docs/design/2026-08-06-hcl-014-r2-raw-candidate-workflow.md`
- Plan: `docs/plans/2026-08-06-hcl-014-r2-raw-candidate-workflow.md`
- Next gate: define candidate retention and promotion separately from HCL-007 application package consumption
- Blocked: `—`

### Goal

Run the production HCL-006 collector from a manually dispatched GitHub Actions workflow, retain the complete verified package as a short-lived Actions artifact, upload only the two locale Raw zstd assets to an immutable Cloudflare R2 candidate prefix, and verify the uploaded bytes without changing any production pointer.

### Progress

The merged implementation adds a manual GitHub Actions workflow that runs the single HCL-006 production path, retains the complete package as a seven-day Actions artifact, derives a two-object Raw-only receipt, uploads those objects to a run-unique R2 candidate prefix, and downloads them again for byte-size and SHA-256 verification. It does not introduce a Raw-only collector, scheduled publishing, signatures, production pointers, or app download behavior.

### Verification

The Node workflow contract passed 6/6, including tamper rejection, exact Raw-only object planning, remote-byte verification semantics and forbidden pointer checks. The planner accepted the credentialed HCL-006 live package and selected exactly the 264,521-byte Korean Raw asset and 250,162-byte English Raw asset. `npm run check`, `/va HCL-014`, and `npm run merge:check -- HCL-014` passed after the branch was updated with current main. GitHub Actions run `31060455325`, attempt 2, then collected the live two-locale package, uploaded two Raw objects under `candidates/raw/36.0.3-build247416-r1/runs/31060455325-2`, downloaded both objects from R2, and verified all 514,695 bytes successfully without changing a production pointer.

## HCL-015 — 카드 이미지 기준팩 후보 파이프라인

- Status: `done`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-08-06`
- Codex: `HCL-015 · 카드 이미지 기준팩 후보 파이프라인`
- Branch: `main`
- Worktree: `main`
- Depends on: `HCL-006`, `HCL-014`
- Spec: `docs/design/2026-08-06-hcl-015-image-baseline-candidate-pipeline.md`
- Plan: `docs/plans/2026-08-06-hcl-015-image-baseline-candidate-pipeline.md`
- Next gate: define image candidate retention and promotion separately from HCL-007 application package consumption
- Blocked: `—`

### Goal

Build a trusted GitHub Actions image pipeline that derives the current `ko_KR` and `en_US` normal/crop image set from an HCL-006 package, verifies and content-addresses the official bytes, creates deterministic baseline packs and maps, uploads them to an immutable R2 candidate prefix, and verifies the downloaded candidate without publishing an application pointer.

### Progress

The merged implementation validates the complete HCL-006 package before extracting both locale image requests, downloads normal/crop bytes with bounded concurrency, HTTPS/redirect/timeout/retry and media checks, globally deduplicates by SHA-256, and writes deterministic 480 MiB-sharded tar.zst packs, locale maps and a canonical receipt. A separate manual workflow builds the package, uploads only image packs/maps to the run-unique R2 candidate prefix, downloads and verifies every object and archive member, then uploads and rechecks the receipt last. Gold, hero skins, pointers, delta/bootstrap packs and application consumption remain excluded.

### Verification

Fixture-first coverage includes source parity and absent URLs, PNG/JPEG/WebP validation, timeout, retry-after, redirect and byte caps, bounded concurrency, cross-locale deduplication, deterministic output, multi-shard splitting, unsafe run identities and tamper rejection. Fresh `npm run check`, `/va HCL-015` and `npm run merge:check -- HCL-015` passed on the clean implementation head. GitHub Actions run `31066060504`, attempt 1, then collected and normalized 1,645 live cards for each locale, processed 6,580 image slots, preserved 1,180 absent and 2 unavailable slots, uploaded 4,314 unique verified images plus two locale maps under `candidates/images/36.0.3-build247416-r1/runs/31066060504-1`, downloaded and verified all packs and maps from R2, and uploaded and byte-compared the receipt last without changing a production pointer.

## HCL-016 — 로컬 GPU 이미지 업스케일 후보 파이프라인

- Status: `done`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-08-06`
- Codex: `HCL-016 · 로컬 GPU 이미지 업스케일 후보 파이프라인`
- Branch: `main`
- Worktree: `main`
- Depends on: `HCL-015`
- Spec: `docs/design/2026-08-06-hcl-016-gpu-upscale-candidate-pipeline.md`
- Plan: `docs/plans/2026-08-06-hcl-016-gpu-upscale-candidate-pipeline.md`
- Next gate: define derived image candidate retention and application-package consumption with HCL-007
- Blocked: `—`

### Goal

Consume a verified immutable HCL-015 R2 image candidate, upscale its normal card images on the trusted local RTX 4090 self-hosted runner, restore original alpha after deterministic x2 output processing, publish verified derived packs and maps to a separate immutable R2 candidate prefix, and upload the derived receipt last without changing the official source candidate or a production pointer.

### Progress

The merged implementation consumes the verified HCL-015 receipt and maps from R2, derives the exact pack set from normal references, validates every source archive member, runs the pinned Real-ESRGAN tool on the trusted Windows GPU runner, restores alpha after x2 postprocessing, builds verified derived packs/map/receipt, re-downloads all remote objects and uploads the receipt last. Workflow actions, the Real-ESRGAN archive/executable/models and every Python wheel are SHA-256 pinned. The runner is registered and online; repository Actions permissions are read-only, restrict external actions, require full Action SHA pins and require approval for every external contributor workflow.

### Verification

Node/Python contract tests pass, including unsafe identity/path rejection, HCL-015 crop-owner pack compatibility for normal references, locale-bounded sampling, alpha restoration, deterministic derived pack/map/receipt verification, receipt-last ordering, trusted runner labels and full Action SHA pins. A local 20-image smoke read the actual HCL-015 candidate and verified all 20 output members. Fresh `npm run check`, `/va HCL-016` and `npm run merge:check -- HCL-016` passed. GitHub Actions smoke run `31076153311` generated, uploaded and remotely reverified 20 images. Complete run `31076252673` generated and remotely reverified all 3,284 unique normal images, publishing 1,643 `ko_KR` and 1,641 `en_US` members in two immutable R2 packs plus the map, then uploaded receipt SHA-256 `5f297735b6f2db51bb95be181d56819190933688aeebe2d1cf732a8b130b310a` last without changing a production pointer.
