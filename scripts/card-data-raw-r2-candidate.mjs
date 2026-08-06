import { createHash } from "node:crypto";
import {
  appendFileSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const LOCALES = ["ko_KR", "en_US"];
const HASH_PATTERN = /^[0-9a-f]{64}$/;
const DATA_VERSION_PATTERN = /^\d+\.\d+\.\d+-build\d+-r[1-9]\d*$/;
const POSITIVE_INTEGER_PATTERN = /^[1-9]\d*$/;

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function requirePositiveInteger(value, name) {
  const text = String(value);
  if (!POSITIVE_INTEGER_PATTERN.test(text)) {
    throw new Error(`${name} must be a positive integer`);
  }
  return text;
}

function readManifest(versionRoot) {
  const manifestPath = join(versionRoot, "manifest.json");
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    throw new Error(`Unable to read package manifest: ${manifestPath}`);
  }
  if (!DATA_VERSION_PATTERN.test(manifest.dataVersion ?? "")) {
    throw new Error("Manifest dataVersion is invalid");
  }
  if (basename(versionRoot) !== manifest.dataVersion) {
    throw new Error("Version directory does not match manifest dataVersion");
  }
  if (
    !Array.isArray(manifest.supportedLocales) ||
    manifest.supportedLocales.length !== LOCALES.length ||
    !LOCALES.every(locale => manifest.supportedLocales.includes(locale))
  ) {
    throw new Error("Manifest must contain exactly ko_KR and en_US");
  }
  return manifest;
}

function checkedRawObject({ versionRoot, manifest, locale, prefix }) {
  const raw = manifest.locales?.[locale]?.raw;
  const expectedPath = `raw/${locale}.json.zst`;
  if (
    raw?.path !== expectedPath ||
    !Number.isSafeInteger(raw.bytes) ||
    raw.bytes < 0 ||
    !HASH_PATTERN.test(raw.sha256 ?? "")
  ) {
    throw new Error(`Manifest Raw contract is invalid for ${locale}`);
  }
  const sourcePath = resolve(versionRoot, ...expectedPath.split("/"));
  if (statSync(sourcePath).size !== raw.bytes) {
    throw new Error(`Raw asset size mismatch for ${locale}`);
  }
  if (sha256(sourcePath) !== raw.sha256) {
    throw new Error(`Raw asset SHA-256 mismatch for ${locale}`);
  }
  return {
    locale,
    sourcePath,
    objectKey: `${prefix}/${expectedPath}`,
    bytes: raw.bytes,
    sha256: raw.sha256,
  };
}

export function createCandidateReceipt({ versionRoot, runId, runAttempt }) {
  const absoluteVersionRoot = resolve(versionRoot);
  const safeRunId = requirePositiveInteger(runId, "runId");
  const safeRunAttempt = requirePositiveInteger(runAttempt, "runAttempt");
  const manifest = readManifest(absoluteVersionRoot);
  const prefix = `candidates/raw/${manifest.dataVersion}/runs/${safeRunId}-${safeRunAttempt}`;
  return {
    schemaVersion: 1,
    dataVersion: manifest.dataVersion,
    sourceRun: { id: safeRunId, attempt: safeRunAttempt },
    prefix,
    objects: LOCALES.map(locale =>
      checkedRawObject({
        versionRoot: absoluteVersionRoot,
        manifest,
        locale,
        prefix,
      }),
    ),
  };
}

export function verifyDownloadedCandidates({ receipt, downloadRoot }) {
  if (
    receipt?.schemaVersion !== 1 ||
    !Array.isArray(receipt.objects) ||
    receipt.objects.length !== LOCALES.length
  ) {
    throw new Error("Candidate receipt is invalid");
  }
  let verifiedBytes = 0;
  for (const object of receipt.objects) {
    if (!LOCALES.includes(object.locale) || !HASH_PATTERN.test(object.sha256 ?? "")) {
      throw new Error("Candidate receipt object is invalid");
    }
    const path = resolve(downloadRoot, "raw", `${object.locale}.json.zst`);
    if (statSync(path).size !== object.bytes) {
      throw new Error(`Downloaded Raw asset size mismatch for ${object.locale}`);
    }
    if (sha256(path) !== object.sha256) {
      throw new Error(`Downloaded Raw asset SHA-256 mismatch for ${object.locale}`);
    }
    verifiedBytes += object.bytes;
  }
  return { verifiedObjects: receipt.objects.length, verifiedBytes };
}

function parseOptions(args) {
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index];
    const value = args[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error("Invalid command options");
    }
    options[key.slice(2)] = value;
  }
  return options;
}

function writeGithubOutput(values) {
  const outputPath = process.env.GITHUB_OUTPUT;
  if (!outputPath) return;
  const lines = Object.entries(values).map(([key, value]) => `${key}=${value}`).join("\n");
  appendFileSync(outputPath, `${lines}\n`, "utf8");
}

function runCli() {
  const [command, ...rest] = process.argv.slice(2);
  const options = parseOptions(rest);
  if (command === "plan") {
    const receipt = createCandidateReceipt({
      versionRoot: options["version-root"],
      runId: options["run-id"],
      runAttempt: options["run-attempt"],
    });
    const receiptPath = resolve(options.receipt);
    writeFileSync(receiptPath, `${JSON.stringify(receipt)}\n`, "utf8");
    writeGithubOutput({
      candidate_prefix: receipt.prefix,
      receipt_path: receiptPath,
    });
    process.stdout.write(
      `${JSON.stringify({ event: "candidate_planned", prefix: receipt.prefix, objects: 2 })}\n`,
    );
    return;
  }
  if (command === "entries") {
    const receipt = JSON.parse(readFileSync(resolve(options.receipt), "utf8"));
    for (const object of receipt.objects) {
      process.stdout.write(
        `${object.sourcePath}\t${object.objectKey}\t${object.locale}\t${object.bytes}\t${object.sha256}\n`,
      );
    }
    return;
  }
  if (command === "verify") {
    const receipt = JSON.parse(readFileSync(resolve(options.receipt), "utf8"));
    const result = verifyDownloadedCandidates({
      receipt,
      downloadRoot: resolve(options["download-root"]),
    });
    process.stdout.write(`${JSON.stringify({ event: "candidate_verified", ...result })}\n`);
    return;
  }
  throw new Error("Expected plan, entries, or verify command");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    runCli();
  } catch (error) {
    process.stderr.write(`${JSON.stringify({ event: "candidate_error", message: error.message })}\n`);
    process.exitCode = 1;
  }
}
