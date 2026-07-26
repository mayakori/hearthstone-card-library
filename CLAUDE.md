# Claude Code Project Instructions

@AGENTS.md

이 프로젝트의 공통 개발 규칙, 스킬 정책, 아키텍처 경계와 병합 절차는 `AGENTS.md`를 단일 정본으로 따른다.

빠른 시작:

```powershell
npm install
npm run tauri:dev
```

## Project Tracking

- 활성 작업 정본: `docs/TODO.md`
- 완료 작업: `docs/DONE.md`
- 로컬 칸반: `docs/kanban.html`
- 세 파일은 main worktree에서만 수정한다.
- 기능 worktree 병합 전 `/va HCL-###`와 `npm run merge:check -- HCL-###`를 실행한다.
