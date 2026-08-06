import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const workflowPath = join(root, ".github/workflows/card-data-raw-r2-candidate.yml");
const scriptPath = join(root, "scripts/card-data-raw-r2-candidate.mjs");

async function candidateModule() {
  assert.equal(existsSync(scriptPath), true, "candidate planner must exist");
  return import(pathToFileURL(scriptPath).href);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function createPackage() {
  const packageRoot = join(tmpdir(), `hcl-014-${crypto.randomUUID()}`);
  const versionRoot = join(packageRoot, "36.0.3-build247416-r1");
  mkdirSync(join(versionRoot, "raw"), { recursive: true });
  mkdirSync(join(versionRoot, "normalized"), { recursive: true });
  const ko = Buffer.from("ko raw zstd fixture");
  const en = Buffer.from("en raw zstd fixture");
  const sqlite = Buffer.from("normalized fixture");
  writeFileSync(join(versionRoot, "raw/ko_KR.json.zst"), ko);
  writeFileSync(join(versionRoot, "raw/en_US.json.zst"), en);
  writeFileSync(join(versionRoot, "normalized/ko_KR.sqlite.zst"), sqlite);
  writeFileSync(
    join(versionRoot, "manifest.json"),
    JSON.stringify({
      schemaVersion: 1,
      dataVersion: "36.0.3-build247416-r1",
      supportedLocales: ["ko_KR", "en_US"],
      locales: {
        ko_KR: {
          raw: { path: "raw/ko_KR.json.zst", bytes: ko.length, sha256: sha256(ko) },
          normalized: {
            path: "normalized/ko_KR.sqlite.zst",
            bytes: sqlite.length,
            sha256: sha256(sqlite),
          },
        },
        en_US: {
          raw: { path: "raw/en_US.json.zst", bytes: en.length, sha256: sha256(en) },
          normalized: {
            path: "normalized/en_US.sqlite.zst",
            bytes: sqlite.length,
            sha256: sha256(sqlite),
          },
        },
      },
    }),
  );
  return { versionRoot, ko, en };
}

test("HCL-014 workflow and candidate planner exist", () => {
  assert.equal(existsSync(workflowPath), true);
  assert.equal(existsSync(scriptPath), true);
});

test("candidate receipt contains only the two verified Raw assets", async () => {
  const { createCandidateReceipt } = await candidateModule();
  const { versionRoot } = createPackage();
  const receipt = createCandidateReceipt({
    versionRoot,
    runId: "12345",
    runAttempt: "2",
  });

  assert.equal(receipt.dataVersion, "36.0.3-build247416-r1");
  assert.equal(
    receipt.prefix,
    "candidates/raw/36.0.3-build247416-r1/runs/12345-2",
  );
  assert.deepEqual(
    receipt.objects.map(({ locale, sourcePath, objectKey }) => ({
      locale,
      sourcePath: sourcePath.replaceAll("\\", "/").split("/").slice(-2).join("/"),
      objectKey,
    })),
    [
      {
        locale: "ko_KR",
        sourcePath: "raw/ko_KR.json.zst",
        objectKey:
          "candidates/raw/36.0.3-build247416-r1/runs/12345-2/raw/ko_KR.json.zst",
      },
      {
        locale: "en_US",
        sourcePath: "raw/en_US.json.zst",
        objectKey:
          "candidates/raw/36.0.3-build247416-r1/runs/12345-2/raw/en_US.json.zst",
      },
    ],
  );
  assert.equal(receipt.objects.some(object => object.objectKey.includes("normalized")), false);
});

test("candidate planning rejects tampered Raw bytes", async () => {
  const { createCandidateReceipt } = await candidateModule();
  const { versionRoot } = createPackage();
  writeFileSync(join(versionRoot, "raw/ko_KR.json.zst"), "tampered");
  assert.throws(
    () => createCandidateReceipt({ versionRoot, runId: "1", runAttempt: "1" }),
    /Raw asset (size|SHA-256) mismatch/,
  );
});

test("download verification checks every receipt object byte-for-byte", async () => {
  const { createCandidateReceipt, verifyDownloadedCandidates } = await candidateModule();
  const { versionRoot, ko, en } = createPackage();
  const receipt = createCandidateReceipt({ versionRoot, runId: "9", runAttempt: "1" });
  const downloadRoot = join(tmpdir(), `hcl-014-download-${crypto.randomUUID()}`);
  mkdirSync(join(downloadRoot, "raw"), { recursive: true });
  writeFileSync(join(downloadRoot, "raw/ko_KR.json.zst"), ko);
  writeFileSync(join(downloadRoot, "raw/en_US.json.zst"), en);

  assert.deepEqual(verifyDownloadedCandidates({ receipt, downloadRoot }), {
    verifiedObjects: 2,
    verifiedBytes: ko.length + en.length,
  });
  writeFileSync(join(downloadRoot, "raw/en_US.json.zst"), "tampered");
  assert.throws(
    () => verifyDownloadedCandidates({ receipt, downloadRoot }),
    /Downloaded Raw asset (size|SHA-256) mismatch/,
  );
});

test("workflow is manual, least-privilege, candidate-only, and verifies R2 downloads", () => {
  assert.equal(existsSync(workflowPath), true, "workflow must exist");
  const workflow = readFileSync(workflowPath, "utf8");
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /\bschedule:/);
  assert.doesNotMatch(workflow, /\bpush:/);
  assert.match(workflow, /permissions:\s*\r?\n\s+contents: read/);
  assert.match(
    workflow,
    /cargo run --locked -p card-data-pipeline --bin card-data-pipeline --release -- build/,
  );
  assert.match(workflow, /BLIZZARD_CLIENT_ID: \$\{\{ secrets\.BLIZZARD_CLIENT_ID \}\}/);
  assert.match(workflow, /R2_ACCESS_KEY_ID: \$\{\{ secrets\.R2_ACCESS_KEY_ID \}\}/);
  assert.match(workflow, /R2_ACCOUNT_ID: \$\{\{ vars\.R2_ACCOUNT_ID \}\}/);
  assert.match(workflow, /R2_BUCKET: \$\{\{ vars\.R2_BUCKET \}\}/);
  assert.match(workflow, /actions\/upload-artifact@v4/);
  assert.match(workflow, /rustup toolchain install stable --profile minimal/);
  assert.doesNotMatch(workflow, /rustup (?:toolchain install|override set) 1\.85\.0/);
  assert.match(workflow, /card-data-raw-r2-candidate\.mjs plan/);
  assert.match(workflow, /aws s3 cp/);
  assert.match(workflow, /card-data-raw-r2-candidate\.mjs verify/);
  assert.doesNotMatch(workflow, /stable\.json|versions\.json|current\.json/);
  assert.doesNotMatch(workflow, /normalized\/.*aws s3 cp/);
});

test("Vitest leaves the Node workflow contract to the Node test runner", () => {
  const vitestConfig = readFileSync(join(root, "vitest.config.ts"), "utf8");
  assert.match(vitestConfig, /tests\/card-data-raw-r2-workflow\.test\.mjs/);
});
