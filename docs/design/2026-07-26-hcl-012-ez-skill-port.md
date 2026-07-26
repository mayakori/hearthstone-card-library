# HCL-012 `/ez` Local Skill Port

## Goal

Restore the preserved `/ez` explanation skill as a tracked project-local skill for both Codex-compatible and Claude-compatible discovery paths.

## Contract

- `.agents/skills/ez/SKILL.md` and `.claude/skills/ez/SKILL.md` must be text-identical.
- `/ez` without an argument must clarify the immediately preceding explanation.
- `/ez <target>` must clarify the supplied concept, term, or code behavior.
- The explanation stays concise and accurate for a general programmer.
- It favors the big picture and why it matters over variable-by-variable narration.
- It does not introduce GStack, Superpowers, Hearthstone-specific behavior, or new technical depth.

## Source

The semantic source of truth is the preserved pre-repair project-local skill in the workflow-repair backup. Line-ending normalization is allowed; wording and behavior are not rewritten.

## Non-goals

- Do not install `/ez` globally.
- Do not restore the obsolete Superpowers implementation plan.
- Do not modify application runtime code.

## Acceptance

- Both skill folders pass the Codex skill validator.
- Repository tests enforce the two-copy equality and trigger contract.
- `npm run check`, `/va HCL-012`, and `npm run merge:check -- HCL-012` pass before squash merge.
