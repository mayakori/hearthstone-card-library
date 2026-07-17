# Hearthstone Card Library Agent Guide

이 문서는 이 저장소에서 작업하는 AI 개발 에이전트의 공통 규칙이다.

## Project Goal

공식 하스스톤 카드 데이터를 로컬에 수집하고, 인게임 검색보다 세밀한 AND/OR 조건과 카드 효과 의미를 이용해 덱을 구성한 뒤 덱 코드로 내보내는 개인용 Tauri 데스크톱 앱을 만든다.

## Working Principles

### 1. Think Before Coding

- 모호한 요구사항을 숨기지 않는다.
- 결과에 영향을 주는 가정은 구현 전에 밝힌다.
- 여러 해석이 가능하고 선택 결과가 크게 달라지면 질문한다.

### 2. Simplicity First

- 요청을 만족하는 최소 코드부터 작성한다.
- 한 번만 쓰는 기능을 위해 추상화 계층을 만들지 않는다.
- 현재 필요하지 않은 확장성이나 설정 기능을 미리 추가하지 않는다.

### 3. Surgical Changes

- 요청과 직접 관련된 파일만 수정한다.
- 기존 샘플과 사용자 변경사항을 보존한다.
- 무관한 리팩터링이나 포맷 변경을 섞지 않는다.

### 4. Goal-Driven Execution

- 작업을 검증 가능한 완료 조건으로 바꾼다.
- 코드 변경 후 관련 테스트, 타입 검사, Rust 검사를 실행한다.
- 실제 명령 결과 없이 완료를 주장하지 않는다.

## Project Tracking Workflow

작업 상태의 상세 정본은 `docs/TODO.md`, 완료 보관은 `docs/DONE.md`, 시각화는 `docs/kanban.html`이다. 세 파일은 Git으로 추적하지만 오직 이 저장소의 `main` 작업 트리에서만 수정·커밋한다.

main 전용 정본의 절대경로는 다음과 같다.

- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\TODO.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\DONE.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\kanban.html`

### Task IDs and titles

- 새 작업은 TODO와 DONE의 최대 번호 다음 `HCL-###` ID를 `main`에서 발급한다.
- Codex 작업 제목은 `HCL-### · 작업명`, branch는 `codex/hcl-###-slug`, worktree는 `hcl-###-slug`를 사용한다.
- 독립 worktree와 병합이 필요한 작업만 새 ID를 받고, 작은 구현 단계는 같은 작업의 체크리스트로 둔다.
- 완료된 ID는 DONE 이관 후에도 유지하며 재사용하지 않는다.

### Status and ownership

- 상태는 `backlog → ready → in_progress → verify → done` 순서다.
- 막힌 작업은 현재 상태를 유지하고 `Blocked`에 원인과 해제 조건을 기록한다.
- 기능 worktree에서는 spec, plan, 코드와 테스트를 수정할 수 있지만 `docs/TODO.md`, `docs/DONE.md`, `docs/kanban.html`은 수정하지 않는다.
- 기능 진행 중 상태가 바뀌면 main 절대경로의 TODO와 칸반을 같은 관리 커밋에서 갱신한다.
- TODO와 칸반을 수정한 뒤 `npm run tracking:check`를 실행한다.

### Worktree and merge gate

기능 작업은 승인된 설계 이후 별도 worktree에서 시작한다. 병합 전 기능 브랜치에서 다음 명령의 출력이 없어야 한다.

```powershell
git diff --name-only main...HEAD -- docs/TODO.md docs/DONE.md docs/kanban.html
```

출력이 있으면 병합을 중단하고 관리 상태를 main 파일에 다시 반영한다. 기능 브랜치의 관리 문서 변경을 병합하지 않는다. 검증과 사용자 확인 후 squash merge하고, main 최종 검증 뒤 카드를 `done`으로 변경한다.

## Product Design Workflow

제품 방향, 사용자 흐름과 UI 구체화 단계에서는 GStack을 사용한다.

- 제품 전제나 사용자 문제가 불명확하면 `/office-hours`로 먼저 구체화한다.
- 제품 범위와 우선순위를 재검토할 때는 `/plan-ceo-review`를 사용한다.
- 화면 구조, 정보 위계, 상호작용과 UX 계획은 `/plan-design-review`로 검토한다.
- 시각 언어, 타이포그래피, 색상과 디자인 시스템은 `/design-consultation`으로 구체화한다.
- 여러 시각적 대안을 비교할 필요가 있고 사용자가 동의하면 `/design-shotgun`을 사용한다.
- GStack 설계 단계와 구현 워크플로를 동시에 진행하지 않는다.
- 사용자 승인을 받기 전에는 `src/`, `src-tauri/`, `package.json`과 런타임 코드를 수정하지 않는다.
- 설계 결과는 `docs/design/` 아래의 간결한 문서에 기록하고 승인된 문서를 구현 기준으로 사용한다.

기본 설계 순서는 다음과 같다.

1. 제품 이해와 핵심 사용자 작업 확인
2. 핵심 사용자 흐름 정의
3. 데스크톱 화면 구조 비교와 선택
4. 고급 필터 UX 구체화
5. 카드 상세와 관련 카드 탐색 구체화
6. 덱 편집과 덱 코드 흐름 구체화
7. 시각 시스템과 와이어프레임 승인
8. 승인 후 별도 구현 작업 시작

## Architecture Boundaries

- `src/`: SolidJS 프론트엔드. 화면, 필터 편집기, 덱 편집기와 사용자 상호작용을 담당한다.
- `src-tauri/src/`: Rust 백엔드. 공식 데이터 수집, 캐시, 로컬 DB, 카드 의미 분석과 Tauri IPC를 담당한다.
- `data/fixtures/`: 테스트와 개발용 공식 응답 샘플이다. 런타임 DB로 사용하지 않는다.
- `preview/`: 데이터 수집 가능성을 검증한 임시 독립 HTML이다. 제품 UI와 결합하지 않는다.
- `docs/`: 설계와 구현 계획을 보관한다.

프론트엔드에서 공식 사이트를 직접 호출하는 구조를 기본값으로 만들지 않는다. 네트워크 요청과 원본 응답 처리는 Rust 어댑터 뒤에 격리한다.

## Card Data Rules

- 카드의 공식 `id`, `slug`, locale과 원본 출처를 보존한다.
- 원본 응답과 앱에서 사용하는 정규화 모델을 구분한다.
- 공식 카드 라이브러리의 내부 엔드포인트는 공개 안정 API가 아니므로 변경 가능성을 전제로 어댑터와 fixture 테스트를 둔다.
- 카드 텍스트에서 추출한 의미 조건은 원문과 별도 필드에 저장하고, 추론 결과임을 구분할 수 있게 한다.
- 기본 locale은 `ko_KR`이다.

## Commands

```powershell
npm install
npm run dev
npm run tauri:dev
npm run test
npm run build
npm run rust:check
npm run check
```

## Agent Roles

역할 계약은 `.codex/agents/*.toml`에 있다. 현재 런타임에서 역할 파일이 자동 선택되지 않으면 에이전트 작업 메시지에 다음을 포함한다.

```text
Role contract: .codex/agents/{role}.toml
이 파일의 developer_instructions를 읽고 지정된 범위에서 작업하라.
```

- `card-data-researcher`: 공식 카드 데이터와 메타데이터 조사
- `frontend-engineer`: SolidJS 화면과 프론트엔드 테스트
- `tauri-engineer`: Rust 수집·저장·IPC 계층
- `reviewer`: 변경사항과 검증 결과를 읽기 전용으로 검토

동일 파일을 여러 에이전트가 동시에 수정하지 않는다.

## Definition of Done

- 요청된 동작이 구현되어 있다.
- `npm run check`가 통과한다.
- 데이터 형식 변경 시 관련 fixture 또는 테스트가 갱신되어 있다.
- 사용자 변경사항과 무관한 파일이 수정되지 않았다.
- 실행 방법 또는 중요한 제약이 바뀌면 `README.md`가 갱신되어 있다.
