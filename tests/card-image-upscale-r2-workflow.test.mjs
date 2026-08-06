import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const workflowPath = join(root, ".github/workflows/card-image-upscale-r2-candidate.yml");
const scriptPath = join(root, "scripts/card_image_upscale.py");
const requirementsPath = join(root, "requirements/card-image-upscale.txt");
const vitestConfigPath = join(root, "vitest.config.ts");
const workflowPaths = [
  join(root, ".github/workflows/card-data-raw-r2-candidate.yml"),
  join(root, ".github/workflows/card-data-image-r2-candidate.yml"),
  workflowPath,
];

test("GPU workflow is manual-only and locked to the trusted Windows runner", () => {
  assert.equal(existsSync(workflowPath), true, "GPU workflow must exist");
  const workflow = readFileSync(workflowPath, "utf8");
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /source_candidate_prefix:/);
  assert.match(workflow, /max_images:/);
  assert.doesNotMatch(workflow, /\bpull_request(?:_target)?:|\bpush:|\bschedule:|\bworkflow_run:/);
  assert.match(workflow, /permissions:\s*\r?\n\s+contents: read/);
  assert.match(workflow, /runs-on: \[self-hosted, Windows, X64, gpu, rtx4090\]/);
  assert.match(workflow, /timeout-minutes: 120/);
  assert.match(workflow, /cancel-in-progress: false/);
  assert.doesNotMatch(workflow, /BLIZZARD_CLIENT_ID|BLIZZARD_CLIENT_SECRET/);
});

test("GPU workflow verifies, uploads derived objects, verifies remote bytes, then uploads receipt", () => {
  const workflow = readFileSync(workflowPath, "utf8");
  for (const marker of [
    "Validate runner and install pinned dependencies",
    "Build locally verified upscale candidate",
    "Upload packs and map",
    "Download and verify derived objects",
    "Upload receipt last",
    "Clean GPU intermediates",
  ]) {
    assert.match(workflow, new RegExp(marker));
  }
  assert.ok(workflow.indexOf("Upload packs and map") < workflow.indexOf("Download and verify derived objects"));
  assert.ok(workflow.indexOf("Download and verify derived objects") < workflow.indexOf("Upload receipt last"));
  assert.match(workflow, /if: \$\{\{ always\(\) \}\}/);
  assert.match(workflow, /retention-days: 7/);
  assert.doesNotMatch(workflow, /stable\.json|versions\.json|current\.json/);
});

test("upscale implementation and pinned runtime requirements are present", () => {
  assert.equal(existsSync(scriptPath), true);
  assert.equal(existsSync(requirementsPath), true);
  const workflow = readFileSync(workflowPath, "utf8");
  const requirements = readFileSync(requirementsPath, "utf8");
  assert.match(requirements, /^boto3==\d+\.\d+\.\d+ \\$/m);
  assert.match(requirements, /^Pillow==\d+\.\d+\.\d+ \\$/m);
  assert.match(requirements, /^zstandard==\d+\.\d+\.\d+ \\$/m);
  assert.match(requirements, /^--only-binary=:all:$/m);
  assert.match(requirements, /--hash=sha256:[0-9a-f]{64}/);
  assert.match(workflow, /--require-hashes/);
  assert.doesNotMatch(requirements, />=|~=|\*/);
});

test("every third-party workflow action is pinned to a full commit SHA", () => {
  for (const path of workflowPaths) {
    const workflow = readFileSync(path, "utf8");
    const uses = [...workflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)].map(match => match[1]);
    assert.ok(uses.length > 0, `${path} must use at least one action`);
    for (const value of uses) {
      assert.match(value, /^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/, `${value} must be SHA pinned`);
    }
  }
});

test("Vitest leaves the GPU workflow contract to the Node test runner", () => {
  const config = readFileSync(vitestConfigPath, "utf8");
  assert.match(config, /tests\/card-image-upscale-r2-workflow\.test\.mjs/);
});
