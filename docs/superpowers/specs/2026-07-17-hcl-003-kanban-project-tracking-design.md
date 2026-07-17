# HCL-003 칸반 기반 프로젝트 관리 설계

**상태:** 승인됨

**작성일:** 2026-07-17

**작업 ID:** HCL-003

**Codex 작업 제목:** `HCL-003 · 칸반 기반 프로젝트 관리 도입`

## 목표

`dc_browser`의 작업 ID, 활성 백로그, 완료 보관, 기능별 worktree, spec→plan→구현→검증 흐름을 이 프로젝트에 맞게 도입한다. 기존 노드 그래프 HTML은 가져오지 않고, 활성 작업을 로컬 HTML 칸반으로 시각화한다.

관리 문서는 Git으로 추적하되 진행 상태를 기록하는 세 파일은 `main` 작업 트리에서만 수정하여 기능 worktree 병합 충돌을 방지한다.

## 설계 원칙

- `docs/TODO.md`가 활성 작업과 재개 메모의 상세 정본이다.
- `docs/DONE.md`가 완료 후 정리된 작업의 장기 기록이다.
- `docs/kanban.html`은 `TODO.md` 상태를 보여 주는 로컬 시각화다.
- TODO, DONE, 칸반은 모두 Git으로 추적한다.
- 위 세 파일은 오직 `main` 작업 트리에서만 수정하고 커밋한다.
- 기능 worktree는 spec, plan, 코드와 테스트만 소유하며 진행 관리 파일을 수정하지 않는다.
- 관리 상태는 자동 추론하지 않는다. 중요한 게이트를 통과할 때 명시적으로 갱신한다.
- 칸반과 TODO의 중복 데이터는 검증 스크립트로 일치 여부를 강제한다.

## 파일별 책임

### `docs/TODO.md`

활성 작업, 최근 완료되어 아직 정리되지 않은 작업, 상세 진행 기록과 다음 재개 지점을 보관한다. 작업마다 아래 형식을 사용한다.

```markdown
## HCL-003 — 칸반 기반 프로젝트 관리 도입

- Status: `in_progress`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-17`
- Codex: `HCL-003 · 칸반 기반 프로젝트 관리 도입`
- Branch: `main`
- Worktree: `main`
- Depends on: `—`
- Spec: `docs/superpowers/specs/2026-07-17-hcl-003-kanban-project-tracking-design.md`
- Plan: `—`
- Next gate: 승인된 구현 계획 작성
- Blocked: `—`

### Goal

작업이 달성해야 하는 결과.

### Progress

중요 결정, 현재 상태와 재개 지점.

### Verification

실행한 검증과 결과.
```

`Status`, `Priority`, `Type`, `Updated`는 필수다. 아직 없는 branch, worktree, spec, plan은 `—`로 기록한다.

### `docs/DONE.md`

정리된 완료 작업을 동일한 ID로 보관한다. ID는 이관 후에도 유지하며 재사용하지 않는다. 완료 일자, 결과, 주요 검증, 관련 spec·plan과 최종 커밋을 남긴다.

완료 직후에는 작업을 `done` 상태로 TODO와 칸반에 유지한다. 칸반의 Done 열에는 가장 최근에 완료된 작업을 최대 10개까지 유지한다. 11번째 완료 카드가 생기면 완료일이 가장 오래된 항목을 TODO에서 제거하고 DONE으로 이관하며 칸반에서도 제거한다.

### `docs/kanban.html`

서버 없이 `file://`로 열리는 단일 HTML이다. 외부 스크립트, 웹 폰트, 스타일시트와 네트워크 요청을 사용하지 않는다.

칸반은 다음 기능을 제공한다.

- Backlog, Ready, In Progress, Verify, Done의 다섯 열
- 열별 카드 수
- 제목과 ID 검색
- 유형과 우선순위 필터
- 전체 작업과 진행 중 작업 보기 전환
- 카드 상세 펼치기
- `main`에 존재하는 spec·plan의 상대 링크
- 좁은 화면에서도 사용할 수 있는 반응형 레이아웃

열 안의 카드는 우선순위 `P0`→`P3`, 작업 ID 오름차순으로 정렬한다. 브라우저 드래그와 `localStorage` 상태는 제공하지 않는다. 카드 데이터는 HTML 안의 명확한 `KANBAN_CARDS` 데이터 블록에 두고 일반 상태 갱신에서는 이 블록만 수정한다.

### 기존 문서

- `docs/superpowers/specs/`: 승인된 설계
- `docs/superpowers/plans/`: 승인된 구현 계획
- `README.md`: 중복 로드맵 대신 칸반과 관리 문서 링크 제공
- `AGENTS.md`, `CLAUDE.md`: ID, worktree, main 전용 수정, 상태 전이와 검증 규칙 명시

별도의 노드 뷰 HTML은 만들지 않는다.

## 작업 ID와 이름 규칙

작업 ID는 `HCL-001`, `HCL-002`처럼 프로젝트 접두사와 단일 증가 순번으로 구성한다. 새 ID는 TODO와 DONE에 존재하는 최대 번호의 다음 번호다. ID에 작업 유형을 넣지 않으며 한 번 발급한 ID는 재분류되거나 DONE으로 이관되어도 바꾸지 않는다.

허용 유형은 다음과 같다.

- `feature`
- `bug`
- `design`
- `research`
- `chore`

우선순위는 다음과 같다.

- `P0`: 다른 작업을 막는 선행 작업
- `P1`: 다음 핵심 구현
- `P2`: 계획된 후속 작업
- `P3`: 아이디어 또는 장기 보류

연결되는 이름은 아래 규칙을 따른다.

- Codex 작업: `HCL-006 · 공식 카드 엔드포인트 어댑터`
- branch: `codex/hcl-006-card-adapter`
- worktree: `hcl-006-card-adapter`
- 커밋: `feat(HCL-006): add official card adapter`
- spec: `YYYY-MM-DD-hcl-006-card-adapter-design.md`
- plan: `YYYY-MM-DD-hcl-006-card-adapter.md`

같은 ID를 여러 Codex 작업으로 나눌 때만 제목 끝에 `설계`, `구현`, `검증` 단계를 붙인다. 별도 worktree와 독립 병합이 필요한 작업은 새 ID를 발급한다. 한 작업 안의 작은 단계는 체크리스트와 진행 메모로 관리한다.

## 칸반 상태 모델

### `backlog`

아직 구체화되지 않았거나 우선순위 대기 중인 작업이다.

### `ready`

범위, 완료 조건과 선행 의존성이 정리되어 바로 착수할 수 있다.

### `in_progress`

worktree 또는 허용된 `main` 작업에서 실제 진행 중이다.

### `verify`

구현이나 문서 작업은 끝났으며 테스트, 리뷰, 사용자 확인, 부식 진단 또는 병합이 남았다.

### `done`

필수 검증을 통과했고 `main`에 반영되었다.

막힘은 별도 열로 만들지 않는다. 카드를 현재 열에 유지하고 `Blocked` 필드와 카드 배지에 원인과 해제 조건을 기록한다. 검증 실패 시 카드를 `in_progress`로 되돌리고 실패 내용을 TODO에 남긴다.

## worktree와 상태 갱신 흐름

1. `main`의 TODO와 칸반에서 새 ID를 발급하고 `ready`로 커밋한다.
2. `codex/hcl-###-slug` branch와 `hcl-###-slug` worktree를 만든다.
3. Codex 작업 제목을 `HCL-### · 작업명`으로 설정한다.
4. `main`의 TODO와 칸반을 `in_progress`로 갱신한다.
5. 기능 worktree에서 spec→plan→TDD→검증을 진행한다.
6. 중요한 게이트마다 작업자는 진행 상태를 보고하고, main 조정자가 TODO와 칸반을 함께 갱신한다.
7. 구현과 작업별 테스트가 끝나면 `verify`로 갱신한다.
8. 기능 브랜치에 관리 문서 diff가 없는지 확인한다.
9. 검토, 필수 테스트와 사용자 확인을 통과하면 `main`에 squash merge한다.
10. `main`에서 최종 검증 후 `done`으로 갱신한다.
11. Done 열이 10개를 넘으면 가장 오래된 done 항목을 DONE으로 옮기고 칸반에서 제거한다.

main 전용 관리 파일의 절대경로는 다음과 같다.

- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\TODO.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\DONE.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\kanban.html`

다른 worktree에서 작업 중이어도 관리 상태는 위 main 파일에만 반영한다. 관리 커밋은 정확히 필요한 관리 파일만 stage하여 다른 사용자 변경을 포함하지 않는다.

spec과 plan이 아직 기능 worktree에만 있으면 칸반에는 예정 경로를 텍스트로 표시한다. 파일이 `main`에 존재할 때만 클릭 가능한 링크를 만든다.

## 충돌 방지 게이트

기능 브랜치를 병합하기 전에 아래 범위에 diff가 없어야 한다.

```powershell
git diff --name-only main...HEAD -- docs/TODO.md docs/DONE.md docs/kanban.html
```

출력이 있으면 병합을 중단한다. 상태 변경을 main 전용 파일에 다시 반영하고 기능 브랜치에서는 관리 문서를 병합 대상으로 남기지 않는다.

Git 추적 자체는 충돌을 만들지 않는다. 기능 브랜치가 관리 파일을 수정하지 않으면 main에서만 바뀐 파일은 squash merge와 충돌하지 않는다.

## 동기화 검증

`scripts/validate-project-tracking.mjs`는 자동 수정 없이 불일치를 보고하고 실패한다.

검증 범위는 다음과 같다.

- TODO의 모든 항목이 칸반에 정확히 한 번 존재한다.
- 칸반의 모든 카드가 TODO에 존재한다.
- 같은 ID의 상태, 제목, 유형과 우선순위가 일치한다.
- ID가 TODO와 DONE에 동시에 존재하지 않는다.
- 전체 ID가 중복되지 않고 허용 형식을 따른다.
- 상태, 유형과 우선순위가 허용 값에 속한다.
- `done` 작업의 spec·plan 경로가 기록된 경우 `main`에 실제 파일이 존재한다.
- 칸반 HTML에 필수 열과 카드 데이터 블록이 존재한다.

오류 메시지는 문제가 된 ID, 필드, TODO 값과 칸반 값을 함께 표시한다. 파서가 일부 데이터만 읽은 상태에서 성공하지 않도록 예상하지 못한 형식을 오류로 취급한다.

`npm run tracking:check`가 이 검증을 실행하며 `npm run check`에도 포함한다. 검증기에는 정상 데이터, ID 불일치, 상태 불일치, 중복 ID와 잘못된 값에 대한 Node 테스트를 둔다.

## 초기 작업 등록

도입 시 다음 ID와 상태를 사용한다.

| ID | 상태 | 우선순위 | 유형 | 작업 |
|---|---|---:|---|---|
| HCL-001 | Done/DONE 이관 | P1 | design | 프로젝트 부트스트랩 설계 |
| HCL-002 | Done/DONE 이관 | P1 | design | 오프라인 카드 프리뷰 설계·구현 계획 |
| HCL-003 | In Progress | P0 | chore | 칸반 기반 프로젝트 관리 도입 |
| HCL-004 | Ready | P0 | chore | 현재 저장소 기준선 감사·정리 |
| HCL-005 | Backlog | P1 | design | 기본 UI와 사용자 흐름 설계 |
| HCL-006 | Backlog | P1 | feature | 공식 카드 엔드포인트 어댑터 |
| HCL-007 | Backlog | P1 | feature | 원본 캐시와 정규화 로컬 DB |
| HCL-008 | Backlog | P1 | feature | 기본 검색과 AND/OR 필터 |
| HCL-009 | Backlog | P2 | feature | 카드 효과 의미 태깅과 시너지 모델 |
| HCL-010 | Backlog | P2 | feature | 덱 편집기와 덱 코드 가져오기·내보내기 |

HCL-001과 HCL-002는 DONE에 기록하고 초기 칸반에서는 제외한다. HCL-002는 설계와 계획만 완료된 것으로 기록하며 프리뷰 구현 완료를 의미하지 않는다.

현재 앱 파일 대부분이 Git에 추적되지 않았으므로 HCL-004에서 파일 소유권, 실제 구현 상태와 `npm run check` 결과를 감사한 뒤 기준선을 별도 확정한다. HCL-004 완료 전에는 미추적 앱 산출물을 완료 구현으로 간주하지 않는다.

## 완료 조건

- TODO, DONE과 칸반이 Git에 추가되어 있다.
- TODO와 칸반에 HCL-003부터 HCL-010까지 승인된 초기 상태가 기록되어 있다.
- DONE에 HCL-001과 HCL-002가 사실 범위에 맞게 기록되어 있다.
- 칸반이 `file://`에서 외부 요청 없이 렌더링된다.
- 검색, 필터, 진행 중 보기와 카드 펼치기가 동작한다.
- 동기화 검증기가 정상 상태를 통과하고 의도적인 불일치를 실패시킨다.
- `npm run check`가 프로젝트 관리 검증을 포함해 통과한다.
- AGENTS와 CLAUDE에 main 전용 관리 규칙과 병합 게이트가 반영되어 있다.
- README가 중복 로드맵 대신 칸반과 TODO/DONE을 안내한다.
- 기능 브랜치가 세 관리 파일을 수정하지 않는다는 규칙이 명시되어 있다.

## 비범위

- GitHub Projects 또는 외부 이슈 서비스 연동
- 브라우저 드래그 앤 드롭
- `localStorage` 기반 상태 저장
- 노드/엣지 그래프 뷰
- 상태 자동 추론 또는 자동 카드 이동
- 작업별 별도 관리 문서 디렉터리
- HCL-004의 실제 기준선 정리와 기존 미추적 파일 일괄 커밋
