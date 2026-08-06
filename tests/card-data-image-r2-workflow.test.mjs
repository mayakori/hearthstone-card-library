import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const workflowPath = join(root, ".github/workflows/card-data-image-r2-candidate.yml");
const scriptPath = join(root, "scripts/card-data-image-r2-candidate.mjs");

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function helperModule() {
  assert.equal(existsSync(scriptPath), true, "image R2 helper must exist");
  return import(pathToFileURL(scriptPath).href);
}

function createCandidate() {
  const candidateRoot = join(tmpdir(), `hcl-015-${crypto.randomUUID()}`);
  mkdirSync(join(candidateRoot, "packs"), { recursive: true });
  mkdirSync(join(candidateRoot, "maps"), { recursive: true });
  const pack = Buffer.from("pack fixture");
  const koMap = Buffer.from("ko map fixture");
  const enMap = Buffer.from("en map fixture");
  writeFileSync(join(candidateRoot, "packs/ko_KR-normal-000.tar.zst"), pack);
  writeFileSync(join(candidateRoot, "maps/ko_KR.json.zst"), koMap);
  writeFileSync(join(candidateRoot, "maps/en_US.json.zst"), enMap);
  const receipt = {
    schemaVersion: 1,
    dataVersion: "36.0.3-build247416-r1",
    runId: "12345",
    runAttempt: 2,
    candidatePrefix:
      "candidates/images/36.0.3-build247416-r1/runs/12345-2",
    packs: [
      {
        path: "packs/ko_KR-normal-000.tar.zst",
        bytes: pack.length,
        sha256: sha256(pack),
        memberCount: 1,
        unpackedBytes: 4,
      },
    ],
    maps: [
      { path: "maps/ko_KR.json.zst", bytes: koMap.length, sha256: sha256(koMap) },
      { path: "maps/en_US.json.zst", bytes: enMap.length, sha256: sha256(enMap) },
    ],
  };
  writeFileSync(join(candidateRoot, "receipt.json"), JSON.stringify(receipt));
  return { candidateRoot, receipt };
}

test("image helper returns only verified pack/map entries before receipt", async () => {
  const { candidateEntries, receiptEntry } = await helperModule();
  const { candidateRoot, receipt } = createCandidate();

  const entries = candidateEntries({ candidateRoot });

  assert.deepEqual(
    entries.map(({ relativePath, objectKey }) => ({ relativePath, objectKey })),
    [...receipt.packs, ...receipt.maps].map(({ path }) => ({
      relativePath: path,
      objectKey: `${receipt.candidatePrefix}/${path}`,
    })),
  );
  assert.equal(entries.some(entry => entry.relativePath === "receipt.json"), false);
  assert.equal(
    receiptEntry({ candidateRoot }).objectKey,
    `${receipt.candidatePrefix}/receipt.json`,
  );
});

test("image helper rejects bytes or paths outside the candidate contract", async () => {
  const { candidateEntries } = await helperModule();
  const { candidateRoot } = createCandidate();
  writeFileSync(join(candidateRoot, "maps/en_US.json.zst"), "tampered");
  assert.throws(() => candidateEntries({ candidateRoot }), /size|SHA-256/);

  const second = createCandidate();
  const receiptPath = join(second.candidateRoot, "receipt.json");
  const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
  receipt.maps[0].path = "stable.json";
  writeFileSync(receiptPath, JSON.stringify(receipt));
  assert.throws(() => candidateEntries({ candidateRoot: second.candidateRoot }), /path/);
});

test("image workflow is manual, bounded, candidate-only, and receipt-last", () => {
  assert.equal(existsSync(workflowPath), true, "image workflow must exist");
  const workflow = readFileSync(workflowPath, "utf8");
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /\bschedule:|\bpush:/);
  assert.match(workflow, /permissions:\s*\r?\n\s+contents: read/);
  assert.match(workflow, /timeout-minutes: 90/);
  assert.match(workflow, /image-baseline-build/);
  assert.match(workflow, /image-baseline-verify/);
  assert.match(workflow, /card-data-image-r2-candidate\.mjs entries/);
  assert.match(workflow, /card-data-image-r2-candidate\.mjs receipt-entry/);
  assert.ok(
    workflow.indexOf("Download and verify packs and maps") <
      workflow.indexOf("Upload receipt last"),
  );
  assert.match(workflow, /retention-days: 7/);
  assert.doesNotMatch(workflow, /stable\.json|versions\.json|current\.json/);
});
