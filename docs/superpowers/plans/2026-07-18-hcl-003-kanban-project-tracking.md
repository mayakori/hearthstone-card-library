# HCL-003 Kanban Project Tracking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the dc_browser-style TODO/DONE/worktree workflow with a Git-tracked, main-only local HTML Kanban board replacing node views.

**Architecture:** `docs/TODO.md` is the detailed active-work authority, `docs/DONE.md` is the archive, and an embedded JSON block in `docs/kanban.html` mirrors every TODO task for local visualization. A dependency-free Node validator parses all three artifacts, rejects drift, and joins the existing `npm run check` gate; only the main worktree may change these tracking files.

**Tech Stack:** Markdown, standalone HTML/CSS/JavaScript, Node.js ESM and built-in test runner, npm scripts, Git worktrees

## Global Constraints

- Work only under `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library`.
- Treat HCL-003 as the bootstrap exception: execute it in the main worktree because it creates and updates the three main-only tracking files.
- Git-track `docs/TODO.md`, `docs/DONE.md`, and `docs/kanban.html`, but never edit them from a feature worktree.
- Preserve the user's existing `.gitignore` change and all unrelated untracked files.
- Stage only the exact files named by each task.
- Do not create node/edge views, browser drag-and-drop, `localStorage` state, external assets, or network requests.
- Use statuses `backlog`, `ready`, `in_progress`, `verify`, and `done` only.
- Use priorities `P0`, `P1`, `P2`, and `P3` only.
- Use types `feature`, `bug`, `design`, `research`, and `chore` only.
- Keep HCL-001 and HCL-002 in DONE; mirror HCL-003 through HCL-010 one-to-one between TODO and the Kanban data.
- Complete each code change with RED→GREEN evidence before committing.
- Before claiming completion, run `npm run check` and `git diff --check` and inspect the final `git status --short`.

---

## File Map

- `scripts/validate-project-tracking.mjs`: parse TODO, DONE, and embedded Kanban JSON; report deterministic validation errors; provide the CLI used by npm.
- `tests/project-tracking.test.mjs`: unit tests for the parser and validation rules using in-memory fixtures.
- `tests/project-tracking-files.test.mjs`: repository-level contract tests for the real tracking files, board shell, documentation, and npm wiring.
- `docs/TODO.md`: active work and resume notes for HCL-003 through HCL-010.
- `docs/DONE.md`: archived HCL-001 and HCL-002 records.
- `docs/kanban.html`: offline five-column Kanban board and embedded card data.
- `AGENTS.md`: authoritative agent rules for IDs, main-only tracking edits, lifecycle, and pre-merge gate.
- `CLAUDE.md`: short entry-point reminder that delegates to AGENTS.
- `README.md`: user-facing links to Kanban, TODO, and DONE instead of a duplicate roadmap.
- `package.json`: expose `tracking:test` and `tracking:check` and include them in `check`.
- `vitest.config.ts`: keep Node built-in runner suites out of Vitest discovery.

---

### Task 1: Project tracking parser and validator

**Files:**
- Create: `scripts/validate-project-tracking.mjs`
- Create: `tests/project-tracking.test.mjs`

**Interfaces:**
- Consumes: TODO Markdown task headings and metadata, DONE Markdown headings, and `<script id="kanban-data" type="application/json">` from the board.
- Produces: `parseTodoTasks(markdown)`, `parseDoneTasks(markdown)`, `parseKanbanCards(html)`, `validateTracking(input)`, and `validateTrackingFiles(repoRoot)`.
- CLI success: `Project tracking OK: <active> active cards, <archived> archived tasks.`
- CLI failure: one `- <message>` line per validation error and exit code 1.

- [ ] **Step 1: Write validator unit tests**

Create `tests/project-tracking.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";

import {
  parseDoneTasks,
  parseKanbanCards,
  parseTodoTasks,
  validateTracking,
} from "../scripts/validate-project-tracking.mjs";

const todo = `# Active Work

## HCL-003 — 칸반 기반 프로젝트 관리 도입

- Status: \`in_progress\`
- Priority: \`P0\`
- Type: \`chore\`
- Updated: \`2026-07-18\`
- Codex: \`HCL-003 · 칸반 기반 프로젝트 관리 도입\`
- Branch: \`main\`
- Worktree: \`main\`
- Depends on: \`—\`
- Spec: \`docs/spec.md\`
- Plan: \`docs/plan.md\`
- Next gate: 구현
- Blocked: \`—\`

### Goal

관리 체계를 만든다.
`;

const done = `# Done

## HCL-001 — 프로젝트 부트스트랩 설계

- Completed: \`2026-07-16\`
`;

const card = {
  id: "HCL-003",
  title: "칸반 기반 프로젝트 관리 도입",
  status: "in_progress",
  priority: "P0",
  type: "chore",
  updated: "2026-07-18",
  codex: "HCL-003 · 칸반 기반 프로젝트 관리 도입",
  branch: "main",
  worktree: "main",
  dependsOn: [],
  spec: "docs/spec.md",
  plan: "docs/plan.md",
  nextGate: "구현",
  blocked: "",
};

function board(cards = [card]) {
  return `<!doctype html><script id="kanban-data" type="application/json">${JSON.stringify({ cards })}</script>`;
}

test("parses TODO metadata and strips code ticks and em-dash nulls", () => {
  assert.deepEqual(parseTodoTasks(todo), [
    {
      id: "HCL-003",
      title: "칸반 기반 프로젝트 관리 도입",
      status: "in_progress",
      priority: "P0",
      type: "chore",
      updated: "2026-07-18",
      codex: "HCL-003 · 칸반 기반 프로젝트 관리 도입",
      branch: "main",
      worktree: "main",
      dependsOn: "",
      spec: "docs/spec.md",
      plan: "docs/plan.md",
      nextGate: "구현",
      blocked: "",
    },
  ]);
  assert.deepEqual(parseDoneTasks(done), [
    { id: "HCL-001", title: "프로젝트 부트스트랩 설계", spec: "", plan: "" },
  ]);
  assert.deepEqual(parseKanbanCards(board()), [card]);
});

test("accepts matching active, archived, and board data", () => {
  assert.deepEqual(
    validateTracking({
      todoMarkdown: todo,
      doneMarkdown: done,
      kanbanHtml: board(),
      repoRoot: "C:/repo",
      fileExists: () => true,
    }),
    [],
  );
});

test("reports active/card drift with the affected ID and field", () => {
  const changed = { ...card, status: "verify", title: "다른 제목" };
  const errors = validateTracking({
    todoMarkdown: todo,
    doneMarkdown: done,
    kanbanHtml: board([changed]),
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes("HCL-003 title mismatch: TODO=칸반 기반 프로젝트 관리 도입, Kanban=다른 제목"));
  assert.ok(errors.includes("HCL-003 status mismatch: TODO=in_progress, Kanban=verify"));
});

test("rejects missing cards, duplicate IDs, and active/archive overlap", () => {
  const errors = validateTracking({
    todoMarkdown: `${todo}\n${todo}`,
    doneMarkdown: `${done}\n## HCL-003 — 잘못된 완료 중복\n`,
    kanbanHtml: board([]),
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes("Duplicate TODO ID: HCL-003"));
  assert.ok(errors.includes("ID exists in both TODO and DONE: HCL-003"));
  assert.ok(errors.includes("TODO task missing from Kanban: HCL-003"));
});

test("rejects invalid enums, missing required fields, and missing done links", () => {
  const invalidTodo = todo
    .replace("- Status: `in_progress`", "- Status: `started`")
    .replace("- Priority: `P0`", "- Priority: `urgent`");
  const invalidCard = { ...card, status: "started", priority: "urgent", type: "unknown" };
  delete invalidCard.updated;
  const errors = validateTracking({
    todoMarkdown: invalidTodo,
    doneMarkdown: done,
    kanbanHtml: board([invalidCard]),
    repoRoot: "C:/repo",
    fileExists: () => false,
  });

  assert.ok(errors.includes("HCL-003 invalid TODO status: started"));
  assert.ok(errors.includes("HCL-003 invalid TODO priority: urgent"));
  assert.ok(errors.includes("HCL-003 invalid Kanban type: unknown"));
  assert.ok(errors.includes("HCL-003 missing Kanban field: updated"));

  const doneTodo = todo.replace("`in_progress`", "`done`");
  const doneCard = { ...card, status: "done" };
  const missingLinks = validateTracking({
    todoMarkdown: doneTodo,
    doneMarkdown: done,
    kanbanHtml: board([doneCard]),
    repoRoot: "C:/repo",
    fileExists: () => false,
  });
  assert.ok(missingLinks.includes("HCL-003 completed spec does not exist: docs/spec.md"));
  assert.ok(missingLinks.includes("HCL-003 completed plan does not exist: docs/plan.md"));
});

test("fails closed when Kanban JSON cannot be parsed", () => {
  assert.throws(
    () => parseKanbanCards('<script id="kanban-data" type="application/json">{broken}</script>'),
    /Kanban JSON is invalid/,
  );
  assert.throws(() => parseKanbanCards("<html></html>"), /Kanban data block is missing/);
});

test("rejects malformed task and card IDs instead of skipping them", () => {
  const malformedTodo = todo.replaceAll("HCL-003", "HCL-X");
  const malformedCard = { ...card, id: "HCL-X", codex: "HCL-X · 칸반 기반 프로젝트 관리 도입" };
  const errors = validateTracking({
    todoMarkdown: malformedTodo,
    doneMarkdown: done,
    kanbanHtml: board([malformedCard]),
    repoRoot: "C:/repo",
    fileExists: () => true,
  });
  assert.ok(errors.includes("Invalid TODO ID: HCL-X"));
  assert.ok(errors.includes("Invalid Kanban ID: HCL-X"));
});
```

- [ ] **Step 2: Run the tests to verify RED**

Run:

```powershell
node --test tests/project-tracking.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `scripts/validate-project-tracking.mjs`.

- [ ] **Step 3: Implement the dependency-free validator**

Create `scripts/validate-project-tracking.mjs`:

```js
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const defaultRepoRoot = path.resolve(path.dirname(modulePath), "..");
const taskHeadingPattern = /^## (HCL-[A-Za-z0-9-]+) — (.+)$/gm;

const allowedStatuses = new Set(["backlog", "ready", "in_progress", "verify", "done"]);
const allowedPriorities = new Set(["P0", "P1", "P2", "P3"]);
const allowedTypes = new Set(["feature", "bug", "design", "research", "chore"]);
const requiredTodoFields = ["status", "priority", "type", "updated"];
const requiredCardFields = [
  "id",
  "title",
  "status",
  "priority",
  "type",
  "updated",
  "codex",
  "branch",
  "worktree",
  "dependsOn",
  "spec",
  "plan",
  "nextGate",
  "blocked",
];

const fieldNames = new Map([
  ["Status", "status"],
  ["Priority", "priority"],
  ["Type", "type"],
  ["Updated", "updated"],
  ["Codex", "codex"],
  ["Branch", "branch"],
  ["Worktree", "worktree"],
  ["Depends on", "dependsOn"],
  ["Spec", "spec"],
  ["Plan", "plan"],
  ["Next gate", "nextGate"],
  ["Blocked", "blocked"],
]);

function unwrap(value) {
  const trimmed = value.trim();
  const unwrapped = trimmed.startsWith("`") && trimmed.endsWith("`")
    ? trimmed.slice(1, -1)
    : trimmed;
  return unwrapped === "—" ? "" : unwrapped;
}

function headingMatches(markdown) {
  return [...String(markdown).matchAll(taskHeadingPattern)];
}

function metadataFrom(source, matches, index) {
  const start = matches[index].index + matches[index][0].length;
  const end = matches[index + 1]?.index ?? source.length;
  const fields = {};
  for (const line of source.slice(start, end).split(/\r?\n/)) {
    const field = line.match(/^- ([A-Za-z][A-Za-z ]+):\s*(.+)$/);
    const key = field && fieldNames.get(field[1]);
    if (key) fields[key] = unwrap(field[2]);
  }
  return fields;
}

export function parseTodoTasks(markdown) {
  const source = String(markdown);
  const matches = headingMatches(source);
  return matches.map((match, index) => ({
    id: match[1],
    title: match[2].trim(),
    ...metadataFrom(source, matches, index),
  }));
}

export function parseDoneTasks(markdown) {
  const source = String(markdown);
  const matches = headingMatches(source);
  return matches.map((match, index) => {
    const metadata = metadataFrom(source, matches, index);
    return {
      id: match[1],
      title: match[2].trim(),
      spec: metadata.spec ?? "",
      plan: metadata.plan ?? "",
    };
  });
}

export function parseKanbanCards(html) {
  const block = String(html).match(
    /<script\s+id=["']kanban-data["']\s+type=["']application\/json["']\s*>([\s\S]*?)<\/script>/i,
  );
  if (!block) throw new Error("Kanban data block is missing");
  let data;
  try {
    data = JSON.parse(block[1]);
  } catch (error) {
    throw new Error(`Kanban JSON is invalid: ${error.message}`);
  }
  if (!data || !Array.isArray(data.cards)) throw new Error("Kanban data must contain a cards array");
  return data.cards;
}

function duplicateIds(items) {
  const seen = new Set();
  const duplicates = new Set();
  for (const { id } of items) {
    if (seen.has(id)) duplicates.add(id);
    seen.add(id);
  }
  return [...duplicates].sort();
}

function compareField(todoTask, card, field, errors) {
  if (todoTask[field] !== card[field]) {
    errors.push(`${todoTask.id} ${field} mismatch: TODO=${todoTask[field]}, Kanban=${card[field]}`);
  }
}

function resolveTrackedPath(repoRoot, trackedPath) {
  const candidate = path.resolve(repoRoot, trackedPath.replaceAll("/", path.sep));
  const rootPrefix = `${path.resolve(repoRoot)}${path.sep}`;
  return candidate.startsWith(rootPrefix) ? candidate : null;
}

export function validateTracking({
  todoMarkdown,
  doneMarkdown,
  kanbanHtml,
  repoRoot = defaultRepoRoot,
  fileExists = existsSync,
}) {
  const errors = [];
  const todoTasks = parseTodoTasks(todoMarkdown);
  const doneTasks = parseDoneTasks(doneMarkdown);
  let cards;
  try {
    cards = parseKanbanCards(kanbanHtml);
  } catch (error) {
    return [error.message];
  }

  for (const id of duplicateIds(todoTasks)) errors.push(`Duplicate TODO ID: ${id}`);
  for (const id of duplicateIds(doneTasks)) errors.push(`Duplicate DONE ID: ${id}`);
  for (const id of duplicateIds(cards)) errors.push(`Duplicate Kanban ID: ${id}`);

  const doneIds = new Set(doneTasks.map(({ id }) => id));
  for (const task of doneTasks) {
    if (!/^HCL-\d{3,}$/.test(task.id)) errors.push(`Invalid DONE ID: ${task.id}`);
    for (const field of ["spec", "plan"]) {
      if (!task[field]) continue;
      const resolved = resolveTrackedPath(repoRoot, task[field]);
      if (!resolved || !fileExists(resolved)) {
        errors.push(`${task.id} archived ${field} does not exist: ${task[field]}`);
      }
    }
  }
  for (const { id } of todoTasks) {
    if (doneIds.has(id)) errors.push(`ID exists in both TODO and DONE: ${id}`);
  }

  const todoById = new Map(todoTasks.map((task) => [task.id, task]));
  const cardById = new Map(cards.map((cardItem) => [cardItem.id, cardItem]));

  for (const task of todoTasks) {
    if (!/^HCL-\d{3,}$/.test(task.id)) errors.push(`Invalid TODO ID: ${task.id}`);
    for (const field of requiredTodoFields) {
      if (!task[field]) errors.push(`${task.id} missing TODO field: ${field}`);
    }
    if (task.status && !allowedStatuses.has(task.status)) {
      errors.push(`${task.id} invalid TODO status: ${task.status}`);
    }
    if (task.priority && !allowedPriorities.has(task.priority)) {
      errors.push(`${task.id} invalid TODO priority: ${task.priority}`);
    }
    if (task.type && !allowedTypes.has(task.type)) {
      errors.push(`${task.id} invalid TODO type: ${task.type}`);
    }
    if (!cardById.has(task.id)) errors.push(`TODO task missing from Kanban: ${task.id}`);

    if (task.status === "done") {
      for (const field of ["spec", "plan"]) {
        if (!task[field]) continue;
        const resolved = resolveTrackedPath(repoRoot, task[field]);
        if (!resolved || !fileExists(resolved)) {
          errors.push(`${task.id} completed ${field} does not exist: ${task[field]}`);
        }
      }
    }
  }

  for (const cardItem of cards) {
    if (!/^HCL-\d{3,}$/.test(cardItem.id ?? "")) errors.push(`Invalid Kanban ID: ${cardItem.id}`);
    for (const field of requiredCardFields) {
      const value = cardItem[field];
      if (value === undefined || value === null) errors.push(`${cardItem.id ?? "Unknown card"} missing Kanban field: ${field}`);
    }
    if (cardItem.status && !allowedStatuses.has(cardItem.status)) {
      errors.push(`${cardItem.id} invalid Kanban status: ${cardItem.status}`);
    }
    if (cardItem.priority && !allowedPriorities.has(cardItem.priority)) {
      errors.push(`${cardItem.id} invalid Kanban priority: ${cardItem.priority}`);
    }
    if (cardItem.type && !allowedTypes.has(cardItem.type)) {
      errors.push(`${cardItem.id} invalid Kanban type: ${cardItem.type}`);
    }
    if (!Array.isArray(cardItem.dependsOn)) errors.push(`${cardItem.id} Kanban dependsOn must be an array`);

    const task = todoById.get(cardItem.id);
    if (!task) {
      errors.push(`Kanban card missing from TODO: ${cardItem.id}`);
      continue;
    }
    for (const field of ["title", "status", "priority", "type"]) {
      compareField(task, cardItem, field, errors);
    }
  }

  return errors;
}

export function validateTrackingFiles(repoRoot = defaultRepoRoot) {
  const todoMarkdown = readFileSync(path.join(repoRoot, "docs", "TODO.md"), "utf8");
  const doneMarkdown = readFileSync(path.join(repoRoot, "docs", "DONE.md"), "utf8");
  const kanbanHtml = readFileSync(path.join(repoRoot, "docs", "kanban.html"), "utf8");
  const errors = validateTracking({ todoMarkdown, doneMarkdown, kanbanHtml, repoRoot });
  return {
    errors,
    activeCount: parseTodoTasks(todoMarkdown).length,
    archivedCount: parseDoneTasks(doneMarkdown).length,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  try {
    const result = validateTrackingFiles(defaultRepoRoot);
    if (result.errors.length) {
      console.error("Project tracking validation failed:");
      for (const error of result.errors) console.error(`- ${error}`);
      process.exitCode = 1;
    } else {
      console.log(`Project tracking OK: ${result.activeCount} active cards, ${result.archivedCount} archived tasks.`);
    }
  } catch (error) {
    console.error(`Project tracking validation failed:\n- ${error.message}`);
    process.exitCode = 1;
  }
}
```

- [ ] **Step 4: Run the unit tests to verify GREEN**

Run:

```powershell
node --test tests/project-tracking.test.mjs
```

Expected: 7 tests pass, 0 fail.

- [ ] **Step 5: Commit the validator**

```powershell
git add -- scripts/validate-project-tracking.mjs tests/project-tracking.test.mjs
git commit -m "test(HCL-003): add project tracking validator"
```

---

### Task 2: Canonical TODO/DONE data and offline Kanban board

**Files:**
- Create: `docs/TODO.md`
- Create: `docs/DONE.md`
- Create: `docs/kanban.html`
- Create: `tests/project-tracking-files.test.mjs`

**Interfaces:**
- Consumes: the validator from Task 1 and the approved initial HCL-001 through HCL-010 mapping.
- Produces: eight active TODO entries mirrored by eight Kanban cards, two archived DONE entries, and an offline board contract.

- [ ] **Step 1: Write the real-file contract test**

Create `tests/project-tracking-files.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseKanbanCards, validateTrackingFiles } from "../scripts/validate-project-tracking.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("real tracking files stay synchronized", () => {
  const result = validateTrackingFiles(repoRoot);
  assert.deepEqual(result.errors, []);
  assert.equal(result.activeCount, 8);
  assert.equal(result.archivedCount, 2);
});

test("offline Kanban exposes the five columns and approved controls", () => {
  const html = readFileSync(path.join(repoRoot, "docs", "kanban.html"), "utf8");
  for (const status of ["backlog", "ready", "in_progress", "verify", "done"]) {
    assert.match(html, new RegExp(`data-column="${status}"`));
  }
  for (const id of ["board-search", "type-filter", "priority-filter", "active-only"]) {
    assert.match(html, new RegExp(`id="${id}"`));
  }
  assert.doesNotMatch(html, /<(?:script|link|img)[^>]+https?:\/\//i);
  assert.doesNotMatch(html, /localStorage|dragstart|draggable=/i);

  const cards = parseKanbanCards(html);
  assert.deepEqual(cards.map(({ id }) => id), [
    "HCL-003",
    "HCL-004",
    "HCL-005",
    "HCL-006",
    "HCL-007",
    "HCL-008",
    "HCL-009",
    "HCL-010",
  ]);
});
```

- [ ] **Step 2: Run the real-file test to verify RED**

Run:

```powershell
node --test tests/project-tracking-files.test.mjs
```

Expected: FAIL with `ENOENT` for `docs/TODO.md`.

- [ ] **Step 3: Create the active TODO authority**

Create `docs/TODO.md` with this structure and exact metadata. Keep the Goal, Progress, and Verification text concise but include the listed facts.

```markdown
# Hearthstone Card Library — Active Backlog & Resume Notes

> `docs/TODO.md` is the active-work authority. `docs/kanban.html` mirrors every
> task below, while archived work lives in `docs/DONE.md`. These three files are
> Git-tracked and may be edited only in the main worktree.

## Status Guide

- `backlog`: not yet scoped or waiting for priority
- `ready`: scope, done criteria, and dependencies are clear
- `in_progress`: active implementation or design work
- `verify`: implementation complete; validation, review, or merge remains
- `done`: verified and present on main; retained until Done exceeds 10 cards
- Blocked work stays in its current status and records the unblock condition.

## HCL-003 — 칸반 기반 프로젝트 관리 도입

- Status: `in_progress`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-18`
- Codex: `HCL-003 · 칸반 기반 프로젝트 관리 도입`
- Branch: `main`
- Worktree: `main`
- Depends on: `—`
- Spec: `docs/superpowers/specs/2026-07-17-hcl-003-kanban-project-tracking-design.md`
- Plan: `docs/superpowers/plans/2026-07-18-hcl-003-kanban-project-tracking.md`
- Next gate: tracking validator, canonical files, and repository workflow integration
- Blocked: `—`

### Goal

Adopt TODO/DONE/worktree tracking and replace dc_browser-style node views with a local HTML Kanban board.

### Progress

The user approved Git-tracked main-only tracking files, HCL sequential IDs, the five-column state model, and validator-enforced TODO/Kanban synchronization.

### Verification

Design committed as `8530985`; implementation verification remains.

## HCL-004 — 현재 저장소 기준선 감사·정리

- Status: `ready`
- Priority: `P0`
- Type: `chore`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-003`
- Spec: `—`
- Plan: `—`
- Next gate: audit ownership and verification state of every untracked project file
- Blocked: `—`

### Goal

Establish a verified Git baseline without absorbing unrelated user changes.

### Progress

The app scaffold, fixtures, previews, and project instructions exist locally but most are not tracked.

### Verification

Not started.

## HCL-005 — 기본 UI와 사용자 흐름 설계

- Status: `backlog`
- Priority: `P1`
- Type: `design`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-004`
- Spec: `—`
- Plan: `—`
- Next gate: run the approved GStack product and design workflow
- Blocked: `HCL-004 baseline must be established first`

### Goal

Approve the desktop information architecture, core user flow, advanced filters, card details, and deck editing flow before runtime implementation.

### Progress

The handoff prompt exists at `docs/handoff-ui-design.md`.

### Verification

Not started.

## HCL-006 — 공식 카드 엔드포인트 어댑터

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-004`
- Spec: `—`
- Plan: `—`
- Next gate: research the current official endpoint and freeze fixture contracts
- Blocked: `HCL-004 baseline must be established first`

### Goal

Collect official Korean card data behind a Rust adapter while preserving official IDs, slugs, locale, and source metadata.

### Progress

Only temporary website-proxy fixtures and preview experiments exist.

### Verification

Not started.

## HCL-007 — 원본 캐시와 정규화 로컬 DB

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-006`
- Spec: `—`
- Plan: `—`
- Next gate: define raw-cache and normalized-schema boundaries after the adapter contract
- Blocked: `HCL-006 adapter contract is required`

### Goal

Persist raw official responses separately from the normalized local card model.

### Progress

No production database contract has been approved.

### Verification

Not started.

## HCL-008 — 기본 검색과 AND/OR 필터

- Status: `backlog`
- Priority: `P1`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-007`
- Spec: `—`
- Plan: `—`
- Next gate: design the filter expression model against normalized card data
- Blocked: `HCL-007 normalized schema is required`

### Goal

Search cards with nested AND/OR expressions beyond the in-game search syntax.

### Progress

The existing offline preview demonstrates only a small fixed AND filter sample.

### Verification

Not started.

## HCL-009 — 카드 효과 의미 태깅과 시너지 모델

- Status: `backlog`
- Priority: `P2`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-008`
- Spec: `—`
- Plan: `—`
- Next gate: define inferred semantic fields and provenance after base search works
- Blocked: `HCL-008 query model is required`

### Goal

Store inferred effect semantics separately from official text and expose synergy, card-pool, and bucket relationships.

### Progress

The semantic sample fixture contains only exploratory predicates.

### Verification

Not started.

## HCL-010 — 덱 편집기와 덱 코드 가져오기·내보내기

- Status: `backlog`
- Priority: `P2`
- Type: `feature`
- Updated: `2026-07-18`
- Codex: `—`
- Branch: `—`
- Worktree: `—`
- Depends on: `HCL-008, HCL-009`
- Spec: `—`
- Plan: `—`
- Next gate: approve deck rules, validation, statistics, and deck-code flow
- Blocked: `HCL-008 search and HCL-009 semantic relationships are required`

### Goal

Build and validate decks from search results and import/export Hearthstone deck codes.

### Progress

No production deck model exists.

### Verification

Not started.
```

- [ ] **Step 4: Create the DONE archive**

Create `docs/DONE.md`:

```markdown
# Hearthstone Card Library — Completed Work

> Completed tasks retain their HCL IDs permanently. IDs in this file are never
> reused and must not also appear in `docs/TODO.md` or `docs/kanban.html`.

## HCL-001 — 프로젝트 부트스트랩 설계

- Completed: `2026-07-16`
- Priority: `P1`
- Type: `design`
- Spec: `docs/superpowers/specs/2026-07-16-hearthstone-card-library-bootstrap-design.md`
- Plan: `—`
- Commit: `6a3f0d8`

### Result

Defined the project goal, Tauri/SolidJS/Rust boundaries, fixture policy, AI role contracts, and expected bootstrap files. This record covers the committed design only; HCL-004 will audit the untracked scaffold.

### Verification

The design document is present in Git history at `6a3f0d8`.

## HCL-002 — 오프라인 카드 프리뷰 설계·구현 계획

- Completed: `2026-07-16`
- Priority: `P1`
- Type: `design`
- Spec: `docs/superpowers/specs/2026-07-16-offline-card-preview-design.md`
- Plan: `docs/superpowers/plans/2026-07-16-offline-card-preview.md`
- Commit: `c9307cf, d490e49`

### Result

Defined and planned a twelve-card Korean offline preview. This record does not claim that the untracked preview implementation completed the unchecked implementation plan.

### Verification

The design and plan are present in Git history at `c9307cf` and `d490e49`.
```

- [ ] **Step 5: Create the offline Kanban shell and embedded cards**

Create `docs/kanban.html`. Use one card object for every TODO entry, preserving the exact TODO title, status, priority, and type. The complete document is:

```html
<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Hearthstone Card Library Kanban</title>
  <style>
    :root { color-scheme: light dark; --bg:#11141a; --panel:#1a2029; --card:#222a35; --line:#364151; --text:#f3f6fb; --muted:#9ca9ba; --accent:#67a5ff; --blocked:#f2a65a; --done:#72c69a; }
    * { box-sizing:border-box; }
    body { margin:0; min-width:320px; background:var(--bg); color:var(--text); font:14px/1.5 "Segoe UI","Malgun Gothic",sans-serif; }
    header { position:sticky; top:0; z-index:10; padding:20px clamp(16px,3vw,36px); background:rgba(17,20,26,.96); border-bottom:1px solid var(--line); }
    .heading { display:flex; flex-wrap:wrap; align-items:baseline; gap:10px 18px; }
    h1 { margin:0; font-size:clamp(22px,3vw,34px); }
    #summary { color:var(--muted); }
    .controls { display:grid; grid-template-columns:minmax(200px,2fr) repeat(2,minmax(130px,1fr)) auto; gap:10px; margin-top:16px; }
    input, select, label.toggle { min-height:40px; border:1px solid var(--line); border-radius:8px; background:var(--panel); color:var(--text); padding:8px 11px; }
    label.toggle { display:flex; align-items:center; gap:8px; white-space:nowrap; }
    main { padding:20px clamp(16px,3vw,36px) 40px; overflow-x:auto; }
    .board { display:grid; grid-template-columns:repeat(5,minmax(260px,1fr)); gap:14px; min-width:1356px; }
    .column { align-self:start; min-height:180px; padding:12px; border:1px solid var(--line); border-radius:12px; background:var(--panel); }
    .column-header { display:flex; justify-content:space-between; align-items:center; gap:8px; margin-bottom:10px; }
    .column h2 { margin:0; font-size:15px; text-transform:uppercase; letter-spacing:.05em; }
    .count { min-width:26px; padding:2px 7px; border-radius:999px; background:var(--card); color:var(--muted); text-align:center; }
    .cards { display:grid; gap:10px; }
    article { padding:12px; border:1px solid var(--line); border-radius:10px; background:var(--card); box-shadow:0 8px 18px rgba(0,0,0,.14); }
    article.is-blocked { border-color:var(--blocked); }
    .card-top, .badges { display:flex; flex-wrap:wrap; align-items:center; gap:6px; }
    .card-top { justify-content:space-between; }
    .id { color:var(--accent); font-weight:700; }
    .badge { padding:2px 6px; border:1px solid var(--line); border-radius:999px; color:var(--muted); font-size:11px; }
    .badge.blocked { border-color:var(--blocked); color:var(--blocked); }
    article h3 { margin:9px 0 7px; font-size:15px; }
    .next { margin:0; color:var(--muted); }
    details { margin-top:10px; border-top:1px solid var(--line); padding-top:8px; }
    summary { cursor:pointer; color:var(--accent); }
    dl { display:grid; grid-template-columns:72px 1fr; gap:5px 8px; margin:9px 0 0; }
    dt { color:var(--muted); }
    dd { margin:0; overflow-wrap:anywhere; }
    a { color:var(--accent); }
    .empty { margin:8px 0; color:var(--muted); text-align:center; }
    @media (max-width:760px) { .controls { grid-template-columns:1fr; } main { overflow:visible; } .board { min-width:0; grid-template-columns:1fr; } }
  </style>
</head>
<body>
  <header>
    <div class="heading"><h1>Hearthstone Card Library</h1><span id="summary"></span></div>
    <div class="controls" aria-label="칸반 필터">
      <input id="board-search" type="search" placeholder="ID 또는 작업명 검색" autocomplete="off">
      <select id="type-filter" aria-label="유형"><option value="">모든 유형</option><option>feature</option><option>bug</option><option>design</option><option>research</option><option>chore</option></select>
      <select id="priority-filter" aria-label="우선순위"><option value="">모든 우선순위</option><option>P0</option><option>P1</option><option>P2</option><option>P3</option></select>
      <label class="toggle"><input id="active-only" type="checkbox"> 진행 중만</label>
    </div>
  </header>
  <main>
    <div class="board">
      <section class="column" data-column="backlog"><div class="column-header"><h2>Backlog</h2><span class="count">0</span></div><div class="cards"></div></section>
      <section class="column" data-column="ready"><div class="column-header"><h2>Ready</h2><span class="count">0</span></div><div class="cards"></div></section>
      <section class="column" data-column="in_progress"><div class="column-header"><h2>In Progress</h2><span class="count">0</span></div><div class="cards"></div></section>
      <section class="column" data-column="verify"><div class="column-header"><h2>Verify</h2><span class="count">0</span></div><div class="cards"></div></section>
      <section class="column" data-column="done"><div class="column-header"><h2>Done</h2><span class="count">0</span></div><div class="cards"></div></section>
    </div>
  </main>
  <script id="kanban-data" type="application/json">
  {
    "cards": [
      {"id":"HCL-003","title":"칸반 기반 프로젝트 관리 도입","status":"in_progress","priority":"P0","type":"chore","updated":"2026-07-18","codex":"HCL-003 · 칸반 기반 프로젝트 관리 도입","branch":"main","worktree":"main","dependsOn":[],"spec":"superpowers/specs/2026-07-17-hcl-003-kanban-project-tracking-design.md","plan":"superpowers/plans/2026-07-18-hcl-003-kanban-project-tracking.md","nextGate":"tracking validator, canonical files, and repository workflow integration","blocked":""},
      {"id":"HCL-004","title":"현재 저장소 기준선 감사·정리","status":"ready","priority":"P0","type":"chore","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-003"],"spec":"","plan":"","nextGate":"audit ownership and verification state of every untracked project file","blocked":""},
      {"id":"HCL-005","title":"기본 UI와 사용자 흐름 설계","status":"backlog","priority":"P1","type":"design","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-004"],"spec":"","plan":"","nextGate":"run the approved GStack product and design workflow","blocked":"HCL-004 baseline must be established first"},
      {"id":"HCL-006","title":"공식 카드 엔드포인트 어댑터","status":"backlog","priority":"P1","type":"feature","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-004"],"spec":"","plan":"","nextGate":"research the current official endpoint and freeze fixture contracts","blocked":"HCL-004 baseline must be established first"},
      {"id":"HCL-007","title":"원본 캐시와 정규화 로컬 DB","status":"backlog","priority":"P1","type":"feature","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-006"],"spec":"","plan":"","nextGate":"define raw-cache and normalized-schema boundaries after the adapter contract","blocked":"HCL-006 adapter contract is required"},
      {"id":"HCL-008","title":"기본 검색과 AND/OR 필터","status":"backlog","priority":"P1","type":"feature","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-007"],"spec":"","plan":"","nextGate":"design the filter expression model against normalized card data","blocked":"HCL-007 normalized schema is required"},
      {"id":"HCL-009","title":"카드 효과 의미 태깅과 시너지 모델","status":"backlog","priority":"P2","type":"feature","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-008"],"spec":"","plan":"","nextGate":"define inferred semantic fields and provenance after base search works","blocked":"HCL-008 query model is required"},
      {"id":"HCL-010","title":"덱 편집기와 덱 코드 가져오기·내보내기","status":"backlog","priority":"P2","type":"feature","updated":"2026-07-18","codex":"","branch":"","worktree":"","dependsOn":["HCL-008","HCL-009"],"spec":"","plan":"","nextGate":"approve deck rules, validation, statistics, and deck-code flow","blocked":"HCL-008 search and HCL-009 semantic relationships are required"}
    ]
  }
  </script>
  <script>
    (() => {
      "use strict";
      const statuses = ["backlog", "ready", "in_progress", "verify", "done"];
      const priorityRank = { P0:0, P1:1, P2:2, P3:3 };
      const KANBAN_CARDS = Object.freeze(JSON.parse(document.querySelector("#kanban-data").textContent).cards);
      const controls = {
        search: document.querySelector("#board-search"),
        type: document.querySelector("#type-filter"),
        priority: document.querySelector("#priority-filter"),
        activeOnly: document.querySelector("#active-only"),
      };
      const summary = document.querySelector("#summary");

      function linkOrText(value) {
        if (!value) return document.createTextNode("—");
        const link = document.createElement("a");
        link.href = value;
        link.textContent = value.split("/").at(-1);
        return link;
      }

      function detailRow(list, label, value, isLink = false) {
        const term = document.createElement("dt");
        term.textContent = label;
        const detail = document.createElement("dd");
        detail.append(isLink ? linkOrText(value) : document.createTextNode(value || "—"));
        list.append(term, detail);
      }

      function renderCard(card) {
        const article = document.createElement("article");
        if (card.blocked) article.classList.add("is-blocked");
        const top = document.createElement("div");
        top.className = "card-top";
        top.innerHTML = `<span class="id"></span><span class="badges"></span>`;
        top.querySelector(".id").textContent = card.id;
        const badges = top.querySelector(".badges");
        for (const value of [card.priority, card.type]) {
          const badge = document.createElement("span"); badge.className = "badge"; badge.textContent = value; badges.append(badge);
        }
        if (card.blocked) {
          const badge = document.createElement("span"); badge.className = "badge blocked"; badge.textContent = "BLOCKED"; badges.append(badge);
        }
        const title = document.createElement("h3"); title.textContent = card.title;
        const next = document.createElement("p"); next.className = "next"; next.textContent = card.nextGate;
        const details = document.createElement("details");
        details.innerHTML = "<summary>상세</summary><dl></dl>";
        const list = details.querySelector("dl");
        detailRow(list, "Updated", card.updated);
        detailRow(list, "Codex", card.codex);
        detailRow(list, "Branch", card.branch);
        detailRow(list, "Worktree", card.worktree);
        detailRow(list, "Depends", card.dependsOn.join(", "));
        detailRow(list, "Blocked", card.blocked);
        detailRow(list, "Spec", card.spec, true);
        detailRow(list, "Plan", card.plan, true);
        article.append(top, title, next, details);
        return article;
      }

      function visibleCards() {
        const query = controls.search.value.normalize("NFKC").toLocaleLowerCase("ko-KR").trim();
        return KANBAN_CARDS
          .filter((card) => !controls.activeOnly.checked || ["in_progress", "verify"].includes(card.status))
          .filter((card) => !controls.type.value || card.type === controls.type.value)
          .filter((card) => !controls.priority.value || card.priority === controls.priority.value)
          .filter((card) => !query || `${card.id} ${card.title}`.normalize("NFKC").toLocaleLowerCase("ko-KR").includes(query))
          .sort((a, b) => priorityRank[a.priority] - priorityRank[b.priority] || a.id.localeCompare(b.id));
      }

      function render() {
        const visible = visibleCards();
        for (const status of statuses) {
          const column = document.querySelector(`[data-column="${status}"]`);
          const statusCards = visible.filter((card) => card.status === status);
          column.querySelector(".count").textContent = statusCards.length;
          const container = column.querySelector(".cards");
          if (statusCards.length) container.replaceChildren(...statusCards.map(renderCard));
          else {
            const empty = document.createElement("p"); empty.className = "empty"; empty.textContent = "표시할 작업 없음"; container.replaceChildren(empty);
          }
        }
        summary.textContent = `${visible.length} / ${KANBAN_CARDS.length} active tasks`;
      }

      for (const control of Object.values(controls)) control.addEventListener("input", render);
      render();
    })();
  </script>
</body>
</html>
```

- [ ] **Step 6: Run focused tracking tests and the validator**

Run:

```powershell
node --test tests/project-tracking.test.mjs tests/project-tracking-files.test.mjs
node scripts/validate-project-tracking.mjs
```

Expected: 9 tests pass, 0 fail; CLI prints `Project tracking OK: 8 active cards, 2 archived tasks.`

- [ ] **Step 7: Commit the canonical tracking files and board**

```powershell
git add -- docs/TODO.md docs/DONE.md docs/kanban.html tests/project-tracking-files.test.mjs
git commit -m "feat(HCL-003): add canonical Kanban tracking"
```

---

### Task 3: Repository workflow and npm gate integration

**Files:**
- Modify: `tests/project-tracking-files.test.mjs`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `README.md`
- Modify: `package.json`
- Modify: `vitest.config.ts`

**Interfaces:**
- Consumes: Task 1 validator, Task 2 files, existing `npm run check` sequence, and current project instructions.
- Produces: explicit main-only rules, user-facing tracking links, Vitest/Node runner separation, `tracking:test`, `tracking:check`, and a composite verification gate.

- [ ] **Step 1: Add failing repository workflow contract tests**

Append to `tests/project-tracking-files.test.mjs`:

```js
test("repository instructions enforce main-only tracking and the merge gate", () => {
  const agents = readFileSync(path.join(repoRoot, "AGENTS.md"), "utf8");
  const claude = readFileSync(path.join(repoRoot, "CLAUDE.md"), "utf8");
  const readme = readFileSync(path.join(repoRoot, "README.md"), "utf8");
  const vitestConfig = readFileSync(path.join(repoRoot, "vitest.config.ts"), "utf8");
  const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));

  for (const file of ["docs/TODO.md", "docs/DONE.md", "docs/kanban.html"]) {
    assert.match(agents, new RegExp(file.replace("/", "\\/")));
  }
  assert.match(agents, /main\.\.\.HEAD -- docs\/TODO\.md docs\/DONE\.md docs\/kanban\.html/);
  assert.match(claude, /Project Tracking/);
  assert.match(readme, /docs\/kanban\.html/);
  assert.match(readme, /docs\/TODO\.md/);
  assert.match(readme, /docs\/DONE\.md/);
  assert.equal(
    packageJson.scripts["tracking:test"],
    "node --test tests/project-tracking.test.mjs tests/project-tracking-files.test.mjs",
  );
  assert.equal(packageJson.scripts["tracking:check"], "node scripts/validate-project-tracking.mjs");
  assert.match(packageJson.scripts.check, /^npm run tracking:test && npm run tracking:check && /);
  assert.match(vitestConfig, /configDefaults\.exclude/);
  assert.match(vitestConfig, /tests\/project-tracking\*\.test\.mjs/);
});
```

- [ ] **Step 2: Run the contract test to verify RED**

Run:

```powershell
node --test tests/project-tracking-files.test.mjs
```

Expected: FAIL because AGENTS does not yet contain the main-only merge gate and Vitest does not exclude Node-runner tracking suites.

- [ ] **Step 3: Add the authoritative tracking workflow to AGENTS**

Insert this section after `### 4. Goal-Driven Execution` and before `## Product Design Workflow` in `AGENTS.md`:

```markdown
## Project Tracking Workflow

작업 상태의 상세 정본은 `docs/TODO.md`, 완료 보관은 `docs/DONE.md`, 시각화는 `docs/kanban.html`이다. 세 파일은 Git으로 추적하지만 오직 이 저장소의 `main` 작업 트리에서만 수정·커밋한다.

main 전용 정본의 절대경로는 다음과 같다.

- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\TODO.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\DONE.md`
- `C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\kanban.html`

### Task IDs and titles

- 새 작업은 TODO와 DONE의 최대 번호 다음 `HCL-###` ID를 `main`에서 발급한다.
- Codex 작업 제목은 `HCL-### · 작업명`, branch는 `codex/hcl-###-slug`, worktree는 `hcl-###-slug`를 사용한다.
- 독립 worktree와 병합이 필요한 작업만 새 ID를 받고, 작은 구현 단계는 같은 작업의 체크리스트로 둔다.
- 완료된 ID는 DONE 이관 후에도 유지하며 재사용하지 않는다.

### Status and ownership

- 상태는 `backlog → ready → in_progress → verify → done` 순서다.
- 막힌 작업은 현재 상태를 유지하고 `Blocked`에 원인과 해제 조건을 기록한다.
- 기능 worktree에서는 spec, plan, 코드와 테스트를 수정할 수 있지만 `docs/TODO.md`, `docs/DONE.md`, `docs/kanban.html`은 수정하지 않는다.
- 기능 진행 중 상태가 바뀌면 main 절대경로의 TODO와 칸반을 같은 관리 커밋에서 갱신한다.
- TODO와 칸반을 수정한 뒤 `npm run tracking:check`를 실행한다.

### Worktree and merge gate

기능 작업은 승인된 설계 이후 별도 worktree에서 시작한다. 병합 전 기능 브랜치에서 다음 명령의 출력이 없어야 한다.

```powershell
git diff --name-only main...HEAD -- docs/TODO.md docs/DONE.md docs/kanban.html
```

출력이 있으면 병합을 중단하고 관리 상태를 main 파일에 다시 반영한다. 기능 브랜치의 관리 문서 변경을 병합하지 않는다. 검증과 사용자 확인 후 squash merge하고, main 최종 검증 뒤 카드를 `done`으로 변경한다.
```

- [ ] **Step 4: Add the concise entry-point reminder to CLAUDE**

Append to `CLAUDE.md`:

```markdown
## Project Tracking

- 활성 작업 정본: `docs/TODO.md`
- 완료 작업: `docs/DONE.md`
- 로컬 칸반: `docs/kanban.html`
- 세 파일은 Git 추적하되 main 작업 트리에서만 수정한다.
- 기능 worktree 병합 전 AGENTS의 관리 문서 diff 게이트를 실행한다.
```

- [ ] **Step 5: Replace the duplicate README roadmap with tracking links**

In `README.md`, replace `## 현재 상태` and `## 다음 구현 순서` through the end of that numbered list with:

```markdown
## 프로젝트 관리

- 로컬 칸반: [`docs/kanban.html`](docs/kanban.html)
- 활성 작업과 재개 메모: [`docs/TODO.md`](docs/TODO.md)
- 완료 작업: [`docs/DONE.md`](docs/DONE.md)

작업은 `HCL-###` ID로 관리한다. 상세 진행 상태는 TODO가 정본이며 칸반은 같은 상태를 5열로 시각화한다. 세 관리 파일은 Git으로 추적하지만 기능 worktree에서는 수정하지 않는다.

실험용 카드 HTML은 [`preview/cards.html`](preview/cards.html)에서 열 수 있다.
```

Replace the README directory tree with this exact block:

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

- [ ] **Step 6: Exclude Node-runner tracking suites from Vitest**

Replace `vitest.config.ts` with:

```ts
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "happy-dom",
    exclude: [...configDefaults.exclude, "tests/project-tracking*.test.mjs"],
  },
});
```

- [ ] **Step 7: Wire the validator into npm**

Update the `scripts` object in `package.json` so the relevant tail is exactly:

```json
"rust:check": "cargo check --manifest-path src-tauri/Cargo.toml",
"tracking:test": "node --test tests/project-tracking.test.mjs tests/project-tracking-files.test.mjs",
"tracking:check": "node scripts/validate-project-tracking.mjs",
"check": "npm run tracking:test && npm run tracking:check && npm run test && npm run build && npm run rust:check"
```

- [ ] **Step 8: Run the workflow contract and npm tracking gates**

Run:

```powershell
node --test tests/project-tracking-files.test.mjs
npm run tracking:test
npm run tracking:check
npm run test
```

Expected: file contract has 3 tests pass; tracking suite has 10 tests pass; CLI reports 8 active cards and 2 archived tasks; Vitest runs only the existing `src/app-meta.test.ts` suite and passes 1 test.

- [ ] **Step 9: Commit the workflow integration**

```powershell
git add -- AGENTS.md CLAUDE.md README.md package.json vitest.config.ts tests/project-tracking-files.test.mjs
git commit -m "docs(HCL-003): enforce Kanban tracking workflow"
```

---

### Task 4: Live board verification and HCL-003 completion

**Files:**
- Modify: `docs/TODO.md`
- Modify: `docs/kanban.html`

**Interfaces:**
- Consumes: complete tracking system and repository verification commands.
- Produces: browser-verified local board, final GREEN suite, and HCL-003 in `done` with recorded evidence.

- [ ] **Step 1: Run the complete automated gate**

Run:

```powershell
npm run check
git diff --check
```

Expected:

- tracking Node suite: 10 tests pass, 0 fail
- project tracking CLI: 8 active cards, 2 archived tasks
- Vitest: existing project tests pass
- TypeScript and Vite build: exit 0
- Rust `cargo check`: exit 0
- `git diff --check`: no output

- [ ] **Step 2: Verify the board in the local browser**

Open the absolute local file:

```text
C:\Users\main\Desktop\Claude_Project\hearthstone-card-library\docs\kanban.html
```

Verify all of these behaviors:

1. Five columns render with HCL-003 in In Progress, HCL-004 in Ready, and HCL-005 through HCL-010 in Backlog.
2. Searching `HCL-006` leaves exactly the adapter card visible.
3. Type `feature` leaves HCL-006 through HCL-010 visible.
4. Priority `P0` leaves HCL-003 and HCL-004 visible.
5. `진행 중만` leaves only HCL-003 visible.
6. Expanding HCL-003 shows branch, worktree, spec, and plan.
7. Spec and plan links open local files.
8. At 390px viewport width the page has one column and no root horizontal overflow.
9. The browser console contains no errors or warnings.

- [ ] **Step 3: Record final verification and move HCL-003 to Done**

In `docs/TODO.md`, change HCL-003 metadata and verification to:

```markdown
- Status: `done`
- Updated: `2026-07-18`
- Next gate: start HCL-004 repository baseline audit

### Verification

`npm run check` and `git diff --check` passed. The local Kanban passed search, type, priority, active-only, detail-link, 390px responsive, and console smoke checks.
```

In the HCL-003 object inside `docs/kanban.html`, change only these fields:

```json
"status":"done",
"updated":"2026-07-18",
"nextGate":"start HCL-004 repository baseline audit"
```

- [ ] **Step 4: Re-run synchronization and full verification**

Run:

```powershell
npm run tracking:check
npm run check
git diff --check
git status --short
```

Expected: all commands pass; status shows only the intended HCL-003 tracking changes plus pre-existing unrelated user changes.

- [ ] **Step 5: Commit the completed tracking state**

```powershell
git add -- docs/TODO.md docs/kanban.html
git commit -m "chore(HCL-003): complete project tracking rollout"
```

- [ ] **Step 6: Report the handoff**

Report:

- absolute links to `docs/kanban.html`, `docs/TODO.md`, and `docs/DONE.md`
- the HCL-003 completion commit
- automated test counts and browser checks
- the unchanged pre-existing `.gitignore` and untracked-file status
- HCL-004 as the next Ready task
