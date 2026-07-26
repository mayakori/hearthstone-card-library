# HCL-013 `/sdd` Local Skill Port

## Goal

Restore the `dc_browser` `/sdd` handoff skill as a tracked project-local skill for both Codex-compatible and Claude-compatible discovery paths.

## Contract

- `.agents/skills/sdd/SKILL.md` and `.claude/skills/sdd/SKILL.md` are text-identical.
- `/sdd` converts the immediately preceding task into a self-contained prompt for a separate worktree session.
- `/sdd <task>` converts the supplied task instead.
- The response is exactly one fenced code block with no surrounding explanation.
- The skill produces the handoff prompt only; it does not start or implement the handed-off task.
- A supplied HCL ID is preserved. Without a reliable ID, the prompt uses the `<HCL-###>` placeholder instead of allocating an ID.
- The prompt carries the task, verifiable acceptance criteria, HCL branch/worktree isolation, main-only tracking ownership, validation, `/va`, merge-gate, squash-merge, and push-approval contracts.
- It does not refer to GStack, Superpowers, `dc_browser`, node-view HTML, or the old `scripts/dev.mjs` command.

## Source and adaptation

The semantic source is the existing `dc_browser` project-local `/sdd` skill. Its trigger, single-copy-block output, self-contained handoff purpose, and no-execution rule are preserved. Its repository-specific workflow template is replaced with the current `AGENTS.md` HCL workflow.

## Non-goals

- Do not install `/sdd` globally.
- Do not port `grill-me` or `guarding-todo-node-view`.
- Do not modify application runtime code.
- Do not create a task or worktree when `/sdd` is invoked; the receiving session owns execution.

## Acceptance

- The missing-skill consumer baseline is recorded.
- Both skill folders pass `quick_validate.py`.
- A fresh consumer with the ported skill produces one self-contained code block, preserves the supplied task ID, and does not execute the task.
- Repository tests protect discovery, two-copy equality, and forbidden legacy dependencies.
- `npm run check`, `/va HCL-013`, and `npm run merge:check -- HCL-013` pass before squash merge.

## Consumer evidence

- RED without the skill: the consumer confused `/sdd` with an implementation procedure, emitted no handoff block, changed the requested ID decision, and asked a follow-up question.
- GREEN with the skill: fresh consumers produced exactly one fenced handoff block for both `/sdd HCL-014 ...` and argument-free `/sdd`, preserved `HCL-014` and `HCL-015`, included the HCL validation/merge gates, and performed no task work or file writes.
