# HCL-013 `/sdd` Local Skill Port Plan

1. Record a consumer baseline without `/sdd` and confirm it fails to produce the required handoff.
2. Add a failing repository test for both discovery paths, identical content, the `/sdd` trigger, and exclusion of legacy workflow dependencies.
3. Port the reusable `dc_browser` behavior and replace its old workflow template with the repository's current HCL contracts.
4. Validate both skill folders and re-run the consumer scenario with the skill available.
5. Run the full repository check, commit the feature branch, and pass `/va HCL-013` plus the merge gate.
6. Squash merge on main with the final tracking state, then remove the feature worktree and branch.
