import test from "node:test";
import assert from "node:assert/strict";

import { validateMergeGate } from "../scripts/validate-merge-gate.mjs";

const validInput = {
  taskId: "HCL-006",
  branch: "codex/hcl-006-api-pipeline",
  mainSha: "1".repeat(40),
  headSha: "2".repeat(40),
  statusPorcelain: "",
  trackingDiff: "",
  vaResult: {
    taskId: "HCL-006",
    branch: "codex/hcl-006-api-pipeline",
    mainSha: "1".repeat(40),
    headSha: "2".repeat(40),
    status: "clean",
    checkedAt: "2026-07-26T00:00:00.000Z",
  },
};

test("accepts a clean HCL feature branch with a current VA result", () => {
  assert.deepEqual(validateMergeGate(validInput), []);
});

test("rejects main and malformed or mismatched task branches", () => {
  assert.match(
    validateMergeGate({ ...validInput, branch: "main" }).join("\n"),
    /branch must match codex\/hcl-###-slug/,
  );
  assert.match(
    validateMergeGate({ ...validInput, branch: "codex/hcl-005-frontend-contract" }).join("\n"),
    /branch task does not match HCL-006/,
  );
});

test("rejects a dirty worktree and tracking-file changes", () => {
  const errors = validateMergeGate({
    ...validInput,
    statusPorcelain: " M src/App.tsx",
    trackingDiff: "docs/TODO.md\ndocs/kanban.html\n",
  });

  assert.ok(errors.includes("worktree must be clean before merge"));
  assert.ok(errors.includes("feature branch changes main-only tracking files: docs/TODO.md, docs/kanban.html"));
});

test("rejects a missing, invalid, or blocked VA result", () => {
  assert.ok(validateMergeGate({ ...validInput, vaResult: null }).includes(".va-result.json is missing or invalid"));
  assert.ok(
    validateMergeGate({ ...validInput, vaResult: { ...validInput.vaResult, status: "blocked" } })
      .includes("VA status must be clean"),
  );
  assert.ok(
    validateMergeGate({ ...validInput, vaResult: { ...validInput.vaResult, checkedAt: "not-a-date" } })
      .includes("VA checkedAt must be an ISO-8601 timestamp"),
  );
});

test("rejects stale VA task, branch, main, and head identities", () => {
  const errors = validateMergeGate({
    ...validInput,
    vaResult: {
      ...validInput.vaResult,
      taskId: "HCL-005",
      branch: "codex/hcl-006-old-name",
      mainSha: "3".repeat(40),
      headSha: "4".repeat(40),
    },
  });

  assert.ok(errors.includes("VA taskId does not match HCL-006"));
  assert.ok(errors.includes("VA branch does not match codex/hcl-006-api-pipeline"));
  assert.ok(errors.includes("VA mainSha is stale"));
  assert.ok(errors.includes("VA headSha is stale"));
});
