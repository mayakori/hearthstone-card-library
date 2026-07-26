# Workbench 컴포넌트 설계 핸드오프

아래 프롬프트를 프로젝트를 연 별도 작업에 전달한다.

```text
작업 루트:
C:\Users\main\Desktop\Claude_Project\hearthstone-card-library

HCL-005의 다음 설계 단계로, 승인된 메인 화면을 구성할 프론트엔드 컴포넌트 계약을 구체화하라.

이번 세션은 구현 세션이 아니다. 메인 화면의 구조와 시각 방향은 이미 승인되었으므로 이를 다시 설계하지 말고 컴포넌트 경계, 상태, 상호작용과 무결성 검사 방법을 확정하는 데 집중한다.

먼저 반드시 다음 파일을 읽는다.

- AGENTS.md
- CLAUDE.md
- README.md
- docs/design/hearthstone-workbench-main-screen.md
- docs/TODO.md
- preview/hearthstone-workbench-foundation.html
- preview/responsive-workbench-lab.html
- preview/hearthstone-component-map.html

정본 우선순위:

1. docs/design/hearthstone-workbench-main-screen.md
2. preview/hearthstone-workbench-foundation.html
3. preview/responsive-workbench-lab.html
4. preview/hearthstone-component-map.html

`preview/hearthstone-component-map.html`은 초기 경계 탐색용 자료다. 현재 정본과 달라진 부분이 있을 수 있으므로 승인 문서와 최신 메인 목업을 기준으로 감사를 먼저 수행한다.

목표:

승인된 3단계 구조를 기준으로 각 컴포넌트의 명확한 계약과 테스트 가능한 불변조건을 정의한다.

1. 화면 컨테이너
2. 도메인 컴포넌트
3. 공통 프리미티브

기준 컴포넌트 구조:

- WorkbenchScreen
- ResponsivePaneSwitch
- WorkbenchLayout
- CardLibraryPane
  - LibraryToolbar
  - ManaMultiSelect
  - CardTypeFilter
  - CardLibraryScrollArea
  - CardGrid
  - CardTile
  - FilterDock
  - LogicBucket
  - FilterClause
  - FilterCommandBar
  - LogicModeIndicator
  - FilterCommandInput
  - SaveQueryTemplateAction
- DeckPane
  - DeckSummary
  - EditableDeckTitle
  - DeckCount
  - DeckMetrics
  - ManaCurve
  - DeckListScrollArea
  - DeckList
  - DeckRow
  - DeckHoverPreview
  - DeckActionBar
  - AnalyzeDeckAction
  - CopyDeckCodeAction
- 공통 프리미티브 후보
  - ScrollArea
  - CommandButton
  - Icon
  - TextInput
  - 선택 컨트롤
  - FocusRing
  - Tooltip

각 컴포넌트마다 다음을 정의한다.

- 책임과 명시적인 비책임
- 입력 props
- 출력 events 또는 commands
- 상태의 소유자와 파생 상태
- DOM 구조상 필요한 최소 계약
- 키보드와 포커스 계약
- 접근성 이름, role과 aria 상태
- 반응형 동작과 축약 조건
- 기본, 빈 상태, 비활성, 포커스, 호버, 편집과 드래그 상태
- 필요한 경우 오류와 로딩 상태
- 컴포넌트 계약 테스트에서 검증할 불변조건
- Workbench 조합 테스트에서만 검증할 항목

컴포넌트 무결성 검사도 설계에 포함한다.

1. 컴포넌트 계약 테스트
   - props와 events
   - 상태 전이
   - 키보드 조작
   - 접근성 속성
   - DOM 불변조건
2. 공통 프리미티브 적합성 검사
   - ScrollArea 두께, stable gutter와 키보드 포커스
   - 버튼과 입력의 포커스 표시
   - 아이콘 버튼의 접근 가능한 이름
   - 비활성 상태와 색상 대비
   - 축약 상태에서도 의미가 보존되는지
3. Workbench 조합 및 최소 시각 회귀
   - 카드 라이브러리와 덱 목록의 독립 스크롤
   - 고정 필터 명령행과 덱 액션 바
   - 3열 카드 라이브러리와 잘리지 않는 덱 카드명
   - 단일 패널 전환 임계값
   - 12px, 16px와 20px UI 글꼴
   - 720p, 1080p와 1440p
   - 페이지 가로 오버플로와 하단 잘림 방지

모든 테스트 항목은 `docs/design/hearthstone-workbench-main-screen.md`의 “10. 구현 시 보존할 무결성 조건” 중 하나 이상에 추적 가능해야 한다.

후속 설계 검토:

- 컴포넌트 정보 구조, 상태 소유권, 상호작용, 반응형과 접근성 계약을 설계 문서에서 직접 검토한다.
- 시각 시스템은 이미 승인되었으므로 변경하지 않는다.
- 새로운 시각 언어가 필요하면 사용자 승인을 받은 뒤 대안을 비교한다.
- 같은 단계에서 구현을 병행하지 않는다.

중요한 제한:

- 아직 기능을 구현하지 않는다.
- src/, src-tauri/, package.json을 수정하지 않는다.
- DB, API, Tauri IPC와 카드 동기화를 다루지 않는다.
- 메인 화면 레이아웃과 승인된 디자인 토큰을 임의로 변경하지 않는다.
- CardDetailDialog와 카드 상세 팝업은 현재 범위 밖이다.
- 스니펫, 자동완성과 템플릿 관리 UI는 후속 범위다.
- 작은 시각 조각을 무조건 별도 컴포넌트로 분리하지 않는다.
- 상태 소유권이나 테스트 전략처럼 중요한 선택에는 2~3개 대안과 추천안을 제시하고 사용자 확인을 받는다.
- 사용자 승인 전에는 설계 문서를 저장하거나 구현으로 넘어가지 않는다.

권장 진행 순서:

1. 최신 정본과 기존 component map의 차이 감사
2. 화면 컨테이너와 도메인 컴포넌트 경계 확인
3. 공통 프리미티브 승격 기준 확인
4. props와 events 계약
5. 상태 소유권과 파생 상태
6. 컴포넌트별 상태 매트릭스
7. 접근성 및 반응형 불변조건
8. 무결성 검사와 테스트 계층
9. Workbench 조합 테스트 및 최소 시각 회귀 범위
10. 승인된 결과를 docs/design 아래의 간결한 컴포넌트 설계 문서로 저장

첫 응답에서는 다음만 수행한다.

- 지정된 파일을 읽고 현재 정본을 확인한다.
- 기존 component map에서 최신 설계와 어긋난 부분을 찾는다.
- 제안하는 3단계 컴포넌트 분류를 표로 보여준다.
- 상태 소유권과 테스트 계약을 문서화하는 방식 2~3개를 비교하고 추천안을 제시한다.
- 잘못 이해한 부분이 없는지 확인한다.

첫 응답에서는 파일 수정, 코드 작성, 상세 props 확정과 구현 계획 작성을 하지 않는다.
```
