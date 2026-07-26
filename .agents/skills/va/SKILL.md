---
name: va
description: Use when the user types /va or before an HCL feature branch is squash-merged. Performs a read-only architecture-drift audit against this repository's task, design, architecture, fixture, test, and diff contracts, then records a merge-gate result.
---

# HCL Architecture Drift Audit

`/va HCL-###`는 현재 HCL 기능 worktree가 승인된 설계와 프로젝트 경계에서 벗어났는지 머지 직전에 진단한다. 기존 프로젝트의 문서 이름이나 ADR 번호를 추정해 재사용하지 않는다.

## Scope

- 현재 worktree 하나만 점검한다. 다른 worktree를 자동 선택하거나 수정하지 않는다.
- 진단은 읽기 전용이다. 코드, 설계 문서, `docs/TODO.md`, `docs/DONE.md`, `docs/kanban.html`을 수정하지 않는다.
- 유일한 출력 파일은 저장소 루트의 ignored 파일 `.va-result.json`이다.
- 스타일 선호나 무관한 개선 제안은 부식으로 보고하지 않는다.

## Preconditions

1. 작업 ID 인자를 `HCL-###` 형식으로 받는다.
2. 현재 브랜치가 같은 ID의 `codex/hcl-###-slug` 형식인지 확인한다.
3. `git status --porcelain`이 비어 있는지 확인한다.
4. 다음 명령의 출력이 비어 있는지 확인한다.

```powershell
git diff --name-only main...HEAD -- docs/TODO.md docs/DONE.md docs/kanban.html
```

하나라도 실패하면 진단 결과를 `blocked`로 기록하고 머지 금지 사유를 보고한다.

## Canonical comparison order

아래 순서로 실제 파일을 직접 읽고 대조한다.

1. `git show main:docs/TODO.md`의 해당 작업 카드에서 Spec, Plan, 목표와 다음 게이트를 확인한다.
2. 현재 worktree의 해당 Spec과 Plan을 읽는다.
3. 작업에 관련된 `docs/design/` 문서와 handoff 문서를 읽는다.
4. `AGENTS.md`의 Architecture Boundaries와 Card Data Rules를 읽는다.
5. 변경된 데이터 계약에 대응하는 fixture와 테스트를 읽는다.
6. `git diff --stat main...HEAD`, `git diff --name-status main...HEAD`, `git diff main...HEAD`와 커밋되지 않은 변경을 대조한다.

Spec 또는 Plan 경로가 없거나 현재 branch에서 파일을 찾을 수 없으면 추정하지 말고 `blocked`로 판정한다.

## Audit questions

- 구현이 승인된 Spec과 Plan의 범위와 완료 조건을 충족하는가?
- 프론트엔드, Rust 백엔드, fixture, preview, docs 경계가 `AGENTS.md`와 일치하는가?
- 공식 원본 모델, 정규화 모델, 의미 추론 필드와 출처·locale 구분이 유지되는가?
- 새 필드나 IPC 동작에 fixture 또는 테스트 보호가 있는가?
- 문서가 주장하지만 코드에 없는 기능, 소비되지 않는 scaffold, 코드와 어긋난 주석이 있는가?
- 기능 branch가 main 전용 tracking 파일을 변경하지 않았는가?
- 범위 밖 리팩터링, 생성물, 비밀값 또는 임시 파일이 diff에 섞이지 않았는가?

## Result

각 발견은 `무엇 / 위반한 계약 / file:line 증거 / 머지 전 처방`으로 보고한다. 발견을 억지로 만들지 않는다.

현재 main과 HEAD의 SHA를 각각 아래 명령으로 얻는다.

```powershell
git rev-parse main
git rev-parse HEAD
git branch --show-current
```

저장소 루트에 다음 JSON을 기록한다.

```json
{
  "taskId": "HCL-006",
  "branch": "codex/hcl-006-api-pipeline",
  "mainSha": "<40-character SHA>",
  "headSha": "<40-character SHA>",
  "status": "clean",
  "checkedAt": "<UTC ISO-8601 timestamp>"
}
```

- 모든 precondition과 감사 항목이 통과한 경우에만 `status`를 `clean`으로 쓴다.
- 하나라도 막힘이 있으면 `status`를 `blocked`로 쓰고 발견을 보고한다.
- 결과를 쓴 뒤 `npm run merge:check -- HCL-###`를 실행한다.
- main 또는 HEAD가 바뀌면 기존 결과는 stale이므로 `/va`를 다시 실행한다.
