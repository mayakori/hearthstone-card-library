# Tool-Independent Project Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove every future GStack invocation from the active project workflow while preserving historical documents and the existing design approval gates.

**Architecture:** Replace tool-specific routing in the two active agent instruction files with repository-native workflow rules, then replace the current UI handoff's GStack section with a direct review checklist. Historical plans, designs, local tool state, runtime code, and project tracking files remain untouched.

**Tech Stack:** Markdown, PowerShell, ripgrep, Git, existing npm verification scripts

## Global Constraints

- Do not use GStack commands or skills for this project.
- Preserve all existing design, plan, specification, and audit records.
- Preserve `.gstack/` and the `.gitignore` entry for `.gstack/`.
- Do not modify runtime code, project tracking files, or unrelated user changes.
- Preserve the requirement that users approve product and UI designs before runtime implementation begins.

---

### Task 1: Replace the active GStack workflow

**Files:**
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `docs/handoff-ui-design.md`
- Reference: `docs/design/2026-07-25-tool-independent-project-workflow.md`

**Interfaces:**
- Consumes: The approved scope and acceptance criteria in `docs/design/2026-07-25-tool-independent-project-workflow.md`.
- Produces: Active project instructions and the HCL-005 handoff that can be followed without invoking any GStack command or skill.

- [ ] **Step 1: Confirm the current active instructions still contain GStack routes**

Run:

```powershell
rg -n "/(office-hours|plan-ceo-review|plan-design-review|design-consultation|design-shotgun|plan-eng-review|investigate|qa|qa-only|review|ship)\b" AGENTS.md CLAUDE.md docs/handoff-ui-design.md
```

Expected: matches in all three files, demonstrating that the future workflow still routes work through GStack.

- [ ] **Step 2: Replace `AGENTS.md` product design routing**

Replace the introductory sentence and GStack-specific bullets under `## Product Design Workflow` with:

```markdown
제품 방향, 사용자 흐름과 UI를 구체화할 때는 특정 도구나 스킬에 의존하지 않고 다음 절차를 따른다.

- 이 프로젝트에서는 GStack 명령과 스킬을 사용하지 않는다.
- 제품 전제나 핵심 사용자 문제가 불명확하면 사용자에게 확인하고 성공 기준을 명시한다.
- 제품 범위와 우선순위를 정할 때는 결과에 영향을 주는 2~3개 대안, 장단점과 추천안을 제시한다.
- 화면 구조, 정보 위계, 상태 소유권, 상호작용, 반응형과 접근성 계약을 검토한다.
- 시각 언어, 타이포그래피와 색상을 변경해야 할 때는 먼저 사용자 승인을 받고 대안을 비교한다.
- 설계 검토와 구현을 같은 단계에서 병행하지 않는다.
- 사용자 승인을 받기 전에는 `src/`, `src-tauri/`, `package.json`과 런타임 코드를 수정하지 않는다.
- 설계 결과는 `docs/design/` 아래의 간결한 문서에 기록하고 승인된 문서를 구현 기준으로 사용한다.
```

Keep the existing eight-step basic design sequence unchanged.

- [ ] **Step 3: Replace `CLAUDE.md` skill routing**

Replace the complete `## Skill routing` section with:

```markdown
## Project Workflow

- 이 프로젝트에서는 GStack 명령과 스킬을 사용하지 않는다.
- 제품 설계, 구현, 조사, 버그 분석, QA, 코드 리뷰와 배포는 `AGENTS.md`의 절차와 저장소 명령을 직접 따른다.
- 결과에 영향을 주는 선택은 대안과 장단점을 제시하고 사용자 확인을 받는다.
- UI 설계는 사용자가 승인하기 전까지 구현으로 전환하지 않는다.
- 완료를 주장하기 전에 요청에 맞는 테스트와 `npm run check`를 실행한다.
```

Keep `## Project Tracking` and the quick-start and verification commands unchanged.

- [ ] **Step 4: Replace the UI handoff's future GStack instructions**

Replace the `GStack 사용:` block in `docs/handoff-ui-design.md` with:

```markdown
후속 설계 검토:

- 컴포넌트 정보 구조, 상태 소유권, 상호작용, 반응형과 접근성 계약을 설계 문서에서 직접 검토한다.
- 시각 시스템은 이미 승인되었으므로 변경하지 않는다.
- 새로운 시각 언어가 필요하면 사용자 승인을 받은 뒤 대안을 비교한다.
- 같은 단계에서 구현을 병행하지 않는다.
```

Keep the existing UI decisions, test inventory, and `중요한 제한` section unchanged.

- [ ] **Step 5: Verify the active workflow**

Run:

```powershell
rg -n "/(office-hours|plan-ceo-review|plan-design-review|design-consultation|design-shotgun|plan-eng-review|investigate|qa|qa-only|review|ship)\b" AGENTS.md CLAUDE.md docs/handoff-ui-design.md
```

Expected: no matches.

Run:

```powershell
rg -n "GStack 명령과 스킬을 사용하지 않는다" AGENTS.md CLAUDE.md
```

Expected: one match in each file.

Run:

```powershell
git diff --name-only -- src src-tauri package.json docs/TODO.md docs/DONE.md docs/kanban.html docs/superpowers/specs docs/superpowers/reports
```

Expected: no output.

Run:

```powershell
git diff --check
npm run check
```

Expected: both commands exit successfully.

- [ ] **Step 6: Review and commit only the workflow change**

Run:

```powershell
git diff -- AGENTS.md CLAUDE.md docs/handoff-ui-design.md
git status --short
git add -- AGENTS.md CLAUDE.md docs/handoff-ui-design.md docs/superpowers/plans/2026-07-26-tool-independent-project-workflow.md
git diff --cached --check
git commit -m "docs: remove gstack from active workflow"
```

Expected: the diff contains only the approved active-workflow replacements and the implementation plan; the commit succeeds without staging unrelated user changes.
