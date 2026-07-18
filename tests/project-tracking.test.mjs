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
  const columns = ["backlog", "ready", "in_progress", "verify", "done"]
    .map((status) => `<section data-column="${status}"></section>`)
    .join("");
  return `<!doctype html>${columns}<script id="kanban-data" type="application/json">${JSON.stringify({ cards })}</script>`;
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

test("rejects malformed HCL task headings in TODO instead of skipping them", () => {
  const malformedHeading = "## HCL-011 - 잘못된 구분자";
  const errors = validateTracking({
    todoMarkdown: `${todo}\n${malformedHeading}\n`,
    doneMarkdown: done,
    kanbanHtml: board(),
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes(`Malformed TODO task heading: ${malformedHeading}`));
});

test("rejects malformed HCL task headings in DONE instead of skipping them", () => {
  const malformedHeading = "## HCL-011 —";
  const errors = validateTracking({
    todoMarkdown: todo,
    doneMarkdown: `${done}\n${malformedHeading}\n`,
    kanbanHtml: board(),
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes(`Malformed DONE task heading: ${malformedHeading}`));
});

test("rejects a missing required Kanban column", () => {
  const withoutVerify = board().replace('<section data-column="verify"></section>', "");
  const errors = validateTracking({
    todoMarkdown: todo,
    doneMarkdown: done,
    kanbanHtml: withoutVerify,
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes("Kanban column must appear exactly once: verify (found 0)"));
});

test("rejects a duplicate required Kanban column", () => {
  const duplicateDone = board().replace(
    '<section data-column="done"></section>',
    '<section data-column="done"></section><section data-column="done"></section>',
  );
  const errors = validateTracking({
    todoMarkdown: todo,
    doneMarkdown: done,
    kanbanHtml: duplicateDone,
    repoRoot: "C:/repo",
    fileExists: () => true,
  });

  assert.ok(errors.includes("Kanban column must appear exactly once: done (found 2)"));
});

test("ignores Kanban column lookalikes outside real HTML start tags", () => {
  const decoys = {
    comment: '<!-- data-column="done" -->',
    script: '<script>const decoy = \'data-column="done"\';</script>',
    style: '<style>[data-column="done"] { display: block; }</style>',
  };

  for (const [source, decoy] of Object.entries(decoys)) {
    const withoutRealDone = board().replace('<section data-column="done"></section>', decoy);
    const errors = validateTracking({
      todoMarkdown: todo,
      doneMarkdown: done,
      kanbanHtml: withoutRealDone,
      repoRoot: "C:/repo",
      fileExists: () => true,
    });

    assert.ok(
      errors.includes("Kanban column must appear exactly once: done (found 0)"),
      `${source} content must not count as a real Kanban column`,
    );
  }
});
