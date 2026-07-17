# Claude Code Project Instructions

@AGENTS.md

이 프로젝트의 공통 개발 규칙과 아키텍처 경계는 `AGENTS.md`를 따른다.

빠른 시작:

```powershell
npm install
npm run tauri:dev
```

완료 전 검증:

```powershell
npm run check
```

## Skill routing

When the user's request matches an available skill, invoke it via the Skill tool. When in doubt, invoke the skill.

Key routing rules:

- 제품 아이디어나 핵심 사용자 문제 구체화 → `/office-hours`
- 제품 전략, 범위와 우선순위 검토 → `/plan-ceo-review`
- UI 구조와 UX 계획 검토 → `/plan-design-review`
- 디자인 시스템과 시각 방향 → `/design-consultation`
- 여러 디자인 대안 생성과 비교 → 사용자 동의 후 `/design-shotgun`
- 아키텍처 검토 → `/plan-eng-review`
- 버그와 예상하지 못한 동작 → `/investigate`
- QA와 실제 화면 검증 → `/qa` 또는 `/qa-only`
- 코드 리뷰 → `/review`
- 배포와 PR → `/ship`

UI 구체화 단계에서는 GStack 설계 스킬만 사용하며, 사용자가 설계를 승인하기 전에는 구현을 시작하지 않는다.

## Project Tracking

- 활성 작업 정본: `docs/TODO.md`
- 완료 작업: `docs/DONE.md`
- 로컬 칸반: `docs/kanban.html`
- 세 파일은 Git 추적하되 main 작업 트리에서만 수정한다.
- 기능 worktree 병합 전 AGENTS의 관리 문서 diff 게이트를 실행한다.
