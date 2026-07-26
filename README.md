# Hearthstone Card Lab

하스스톤 공식 카드 데이터를 로컬에 수집하고, 고급 조건 검색과 덱 편집을 거쳐 덱 코드를 만드는 개인 학습용 데스크톱 프로젝트이다.

## 기술 구성

- Desktop shell: Tauri v2
- Backend: Rust
- Frontend: SolidJS + TypeScript + Vite
- Test: Vitest + Rust unit test
- 기본 locale: `ko_KR`

## 빠른 시작

필요 도구는 Node.js, npm, Rust toolchain과 Windows WebView2이다.

```powershell
cd C:\Users\main\Desktop\claude_project\hearthstone-card-library
npm install
npm run tauri:dev
```

브라우저에서 프론트엔드만 확인하려면 다음을 실행한다.

```powershell
npm run dev
```

## 검증 명령

```powershell
npm run test        # 프론트엔드 테스트
npm run build       # TypeScript 검사 + Vite production build
npm run rust:check  # Rust/Tauri 컴파일 검사
npm run check       # 작업 추적 테스트·동기화 검사 후 위 세 검사를 실행
```

데스크톱 실행 파일과 설치 번들을 만들 때는 다음을 사용한다.

```powershell
npm run tauri:build
```

## 디렉터리

```text
.
├─ src/                 SolidJS 프론트엔드
├─ src-tauri/           Rust/Tauri 백엔드
├─ data/fixtures/       공식 응답 개발 샘플
├─ preview/             독립 HTML 수집 검증본
├─ docs/                설계, 계획과 작업 관리
│  ├─ TODO.md             활성 작업과 재개 메모
│  ├─ DONE.md             완료 작업 보관
│  └─ kanban.html         로컬 칸반 보드
├─ .codex/agents/       AI 역할 계약
├─ AGENTS.md            공통 AI 개발 규칙
└─ CLAUDE.md            Claude Code 진입 문서
```

## 프로젝트 관리

- 로컬 칸반: [`docs/kanban.html`](docs/kanban.html)
- 활성 작업과 재개 메모: [`docs/TODO.md`](docs/TODO.md)
- 완료 작업: [`docs/DONE.md`](docs/DONE.md)

작업은 `HCL-###` ID로 관리한다. 상세 진행 상태는 TODO가 정본이며 칸반은 같은 상태를 5열로 시각화한다. 세 관리 파일은 Git으로 추적하지만 기능 worktree에서는 수정하지 않는다.

기능 작업은 `codex/hcl-###-slug` branch의 독립 worktree에서 설계부터 구현까지 진행한다. 병합 전 `/va HCL-###`와 `npm run merge:check -- HCL-###`를 통과시킨 뒤 main에서 squash merge해 작업별 최종 커밋 하나만 남긴다. 이 저장소의 이후 워크플로에서는 GStack을 사용하지 않으며, 과거 설계 문서는 기록으로 보존한다.

실험용 카드 HTML은 [`preview/cards.html`](preview/cards.html)에서 열 수 있다.
