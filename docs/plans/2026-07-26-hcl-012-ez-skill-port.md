# HCL-012 `/ez` Local Skill Port Plan

1. Add a failing repository test for the two required skill paths, identical content, and `/ez` trigger.
2. Port the preserved skill wording to `.agents/skills/ez/SKILL.md` and `.claude/skills/ez/SKILL.md`.
3. Validate both skill folders with `quick_validate.py`.
4. Run tracking tests, the full repository check, and a normalized source comparison.
5. Commit the feature branch, run `/va HCL-012` and the merge gate, then squash merge on main with final tracking state.
