# HCL-004 Repository Baseline Audit Report

## Scope

This report records the ownership and ignore policy for the local repository baseline before any application, fixture, design, or preview content is staged. The audit preserves the pre-existing `.gitignore` changes and adds only local orchestration state and generated Tauri schema output to the ignore policy.

## Pre-change inventory

- `git ls-files --others --exclude-standard` returned `129` files.
- `git status --short` showed the pre-existing `.gitignore` modification plus untracked project and local/generated paths.
- The initial inventory divides into `123` Track candidates and `6` Ignore candidates: two `.superpowers/brainstorm/**` state files and four `src-tauri/gen/schemas/*.json` generated files.
- After applying the ignore policy, the remaining `123` untracked files divide by top-level path as follows: `.codex` 5, `assets` 1, `data` 3, `docs` 5, `index.html` 1, `package-lock.json` 1, `preview` 40, `src` 6, `src-tauri` 59, `tsconfig.json` 1, and `vite.config.ts` 1.

## Ownership policy

| Classification | Paths | Decision |
| --- | --- | --- |
| Agent contracts | `.codex/README.md`, `.codex/agents/*.toml` | Track |
| App entry/config | `index.html`, `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `assets/app-icon.svg` | Track |
| Frontend | `src/**` | Track |
| Tauri backend/package assets | `src-tauri/**` except ignored `target/` and `gen/schemas/` | Track |
| Official-data fixtures | `data/fixtures/**` | Track |
| Approved design records | `docs/design/**`, `docs/handoff-ui-design.md` | Track |
| Independent feasibility preview | `preview/**` | Track |
| Local agent execution state | `.superpowers/**` | Ignore |
| Generated/dependency/build state | `.worktrees/`, `node_modules/`, `dist/`, `target/`, `src-tauri/target/`, `src-tauri/gen/schemas/`, `.vite/`, `.gstack/` | Ignore |
| Secrets and machine state | `.env`, `.env.*` except `.env.example`, `*.log`, `.DS_Store` | Ignore |

`src-tauri/capabilities/default.json` remains a Track candidate because it is the source capability configuration. Only the generated editor/capability schemas under `src-tauri/gen/schemas/` are ignored.

## Secret and focused-test evidence

- The plan's exact `rg` secret-pattern scan found one source-text match at `docs/design/card-data-schema-explorer.html:731`: a callback identifier named `token`. It is syntax-highlighting source code, not a credential, token value, private key, password, API key, or secret. There were no secret-pattern hits outside this false positive.
- `npx vitest run docs/design/card-data-contract-mock.test.ts` exited `0`: 1 test file passed and `5/5` tests passed. The plan expected `4/4`; the current untracked test file contains a fifth direct-detail-hash test added concurrently, so this report records the current higher actual count without changing application or test content.

## Ignore and inventory verification

- `git check-ignore -v .superpowers/sdd/progress.md` returned `.gitignore:13:.superpowers/`.
- `git check-ignore -v src-tauri/gen/schemas/desktop-schema.json` returned `.gitignore:14:src-tauri/gen/schemas/`.
- `git ls-files --others --exclude-standard -- src-tauri/gen/schemas .superpowers` returned no paths.
- `git ls-files --others --exclude-standard` returned `123` paths, all covered by the Track rows above; no local execution state or generated schema path remained.
- `git diff --check 0dfa7f3^ -- docs/TODO.md docs/kanban.html docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md` exited `0` with no whitespace-error output after removing the plan's extra EOF blank line.

## Final Task 2 verification

- `git diff --cached --name-only` returned exactly three paths: `.gitignore`, `docs/superpowers/plans/2026-07-18-hcl-004-repository-baseline-audit.md`, and this report.
- `git diff --cached --check` exited `0` with no whitespace-error output.
- `git status --short` showed only those three allowed tracked changes staged; the approved `123` Track candidates remained untracked for Task 3.
- `git check-ignore -v .superpowers/sdd/task-1-report.md` confirmed that the Task 1 correction is ignored by `.gitignore:13:.superpowers/` and is not committed.
- No application, fixture, preview, design source, or generated schema content is staged by Task 2.
