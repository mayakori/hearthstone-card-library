# Project Agents

`.codex/agents`의 TOML 파일은 역할 계약이다. 에이전트를 사용할 때 작업 메시지에 역할 파일 경로, 수정 범위와 완료 조건을 함께 전달한다.

예시:

```text
Role contract: .codex/agents/frontend-engineer.toml
developer_instructions를 읽고 src/ 범위에서 카드 그리드 컴포넌트를 구현하라.
완료 조건: 관련 Vitest와 npm run build 통과.
```
