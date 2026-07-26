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
  assert.equal(result.activeCount, 10);
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
    "HCL-011",
    "HCL-012",
  ]);
});

test("repository instructions enforce the project workflow and merge gate", () => {
  const agents = readFileSync(path.join(repoRoot, "AGENTS.md"), "utf8");
  const claude = readFileSync(path.join(repoRoot, "CLAUDE.md"), "utf8");
  const readme = readFileSync(path.join(repoRoot, "README.md"), "utf8");
  const vitestConfig = readFileSync(path.join(repoRoot, "vitest.config.ts"), "utf8");
  const packageJson = JSON.parse(readFileSync(path.join(repoRoot, "package.json"), "utf8"));

  for (const file of ["docs/TODO.md", "docs/DONE.md", "docs/kanban.html"]) {
    assert.match(agents, new RegExp(file.replace("/", "\\/")));
  }
  assert.match(agents, /npm run merge:check -- HCL-###/);
  assert.match(agents, /\/va/);
  assert.match(agents, /GStack.*사용하지 않는다/);
  assert.match(agents, /명시적으로 요청/);
  assert.match(claude, /Project Tracking/);
  assert.match(claude, /@AGENTS\.md/);
  assert.match(readme, /docs\/kanban\.html/);
  assert.match(readme, /docs\/TODO\.md/);
  assert.match(readme, /docs\/DONE\.md/);
  assert.match(readme, /npm run merge:check -- HCL-###/);
  assert.match(readme, /squash merge/);
  assert.equal(packageJson.scripts["merge:check"], "node scripts/validate-merge-gate.mjs");
  assert.equal(
    packageJson.scripts["tracking:test"],
    "node --test tests/project-tracking.test.mjs tests/project-tracking-files.test.mjs tests/merge-gate.test.mjs",
  );
  assert.equal(packageJson.scripts["tracking:check"], "node scripts/validate-project-tracking.mjs");
  assert.match(packageJson.scripts.check, /^npm run tracking:test && npm run tracking:check && /);
  assert.match(vitestConfig, /configDefaults\.exclude/);
  assert.match(vitestConfig, /\.worktrees\/\*\*/);
  assert.match(vitestConfig, /tests\/project-tracking\*\.test\.mjs/);
  assert.match(vitestConfig, /tests\/merge-gate\.test\.mjs/);
  assert.equal(
    readFileSync(path.join(repoRoot, ".agents", "skills", "va", "SKILL.md"), "utf8"),
    readFileSync(path.join(repoRoot, ".claude", "skills", "va", "SKILL.md"), "utf8"),
  );
});
