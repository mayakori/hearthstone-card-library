---
name: sdd
description: Use when the user types /sdd, asks to hand the current task or slice to a separate session, or needs a fresh worktree session to continue the work.
---

# `/sdd` Worktree Handoff

현재 작업을 별도 세션이 바로 착수할 수 있는 자족적 핸드오프 프롬프트로 변환한다. 핸드오프만 만들고 현재 세션에서 작업을 시작하지 않는다.

## 대상

- `/sdd`: 직전까지 논의한 작업을 변환한다.
- `/sdd <작업>`: 인자로 받은 작업을 변환한다.

## 출력 계약

- 아래 구조를 채운 코드블록 하나만 출력한다. 코드블록 밖에는 아무것도 쓰지 않는다.
- 사용자가 준 HCL ID는 그대로 보존한다. 현재 작업 ID가 명확하면 그것을 사용하고, 확정할 근거가 없으면 `<HCL-###>`로 둔다.
- 작업명, 배경, 범위, 완료 조건을 현재 대화에서 채운다. 받는 세션이 이 대화를 보지 않아도 착수할 수 있어야 한다.
- 구현하거나 파일을 수정하거나 새 ID를 발급하지 않는다.

```text
<HCL-###> · <한글 작업명>

이 작업의 Codex 제목을 `<HCL-###> · <한글 작업명>`으로 지정한다.

**정본과 격리**
- 저장소 `AGENTS.md`와 main의 `docs/TODO.md`, `docs/DONE.md`, `docs/kanban.html`을 먼저 읽는다.
- 기존 HCL 작업이면 정본에 기록된 `codex/hcl-###-slug` branch와 worktree만 사용한다.
- 신규 작업이면 main에서 다음 HCL ID를 발급하고 tracking·소유권을 같은 관리 커밋에 기록한 뒤, 설계 문서 작성 전 전용 worktree를 만든다.
- 기능 worktree에서 main 전용 tracking 파일을 수정하지 않는다.

**작업**
<무엇을 왜 수행하는지, 관련 파일·정본·제약을 포함한 자족적 설명>

**완료 조건**
- <관찰하거나 명령으로 검증할 수 있는 조건>

**검증과 병합**
1. 작업에 비례한 테스트를 실행하고, 사용자 live smoke가 필요한 변경이면 확인을 받은 뒤 계속한다.
2. 최신 main을 반영하고 같은 검증을 다시 실행한다.
3. `/va <HCL-###>`와 `npm run merge:check -- <HCL-###>`를 통과시킨다.
4. main에서 `git merge --squash`하고 같은 staged 변경에 tracking 최종 상태를 반영한다.
5. main에서 `npm run check` 후 `type(<HCL-###>): summary` 단일 커밋을 만든다.
6. 최종 검증 뒤 기능 worktree와 branch를 정리하며, push는 사용자 승인 후에만 한다.
```

## 확인

출력 전에 코드블록이 하나뿐인지, 작업을 실제로 시작하지 않았는지, HCL ID를 임의로 바꾸지 않았는지 확인한다.
