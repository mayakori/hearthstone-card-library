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
