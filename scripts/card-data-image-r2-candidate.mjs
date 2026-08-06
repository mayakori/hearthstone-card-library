import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const HASH_PATTERN = /^[0-9a-f]{64}$/;
const PREFIX_PATTERN =
  /^candidates\/images\/(\d+\.\d+\.\d+-build\d+-r[1-9]\d*)\/runs\/[1-9]\d*-[1-9]\d*$/;
const PACK_PATTERN = /^packs\/(ko_KR|en_US)-(normal|crop)-\d{3}\.tar\.zst$/;
const MAP_PATTERN = /^maps\/(ko_KR|en_US)\.json\.zst$/;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readReceipt(candidateRoot) {
  const root = resolve(candidateRoot);
  const receiptPath = join(root, "receipt.json");
  let receipt;
  try {
    receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
  } catch {
    throw new Error("Unable to read image candidate receipt");
  }
  const match = PREFIX_PATTERN.exec(receipt?.candidatePrefix ?? "");
  if (
    receipt?.schemaVersion !== 1 ||
    !match ||
    receipt.dataVersion !== match[1] ||
    !Array.isArray(receipt.packs) ||
    !Array.isArray(receipt.maps)
  ) {
    throw new Error("Image candidate receipt is invalid");
  }
  return { root, receipt, receiptPath };
}

function checkedEntry({ root, receipt, object, kind }) {
  const pattern = kind === "pack" ? PACK_PATTERN : MAP_PATTERN;
  if (
    !pattern.test(object?.path ?? "") ||
    !Number.isSafeInteger(object?.bytes) ||
    object.bytes < 0 ||
    !HASH_PATTERN.test(object?.sha256 ?? "")
  ) {
    throw new Error(`Image candidate ${kind} path or digest is invalid`);
  }
  const sourcePath = resolve(root, ...object.path.split("/"));
  if (!sourcePath.startsWith(`${root}\\`) && !sourcePath.startsWith(`${root}/`)) {
    throw new Error(`Image candidate ${kind} path is unsafe`);
  }
  if (statSync(sourcePath).size !== object.bytes) {
    throw new Error(`Image candidate ${kind} size mismatch`);
  }
  if (sha256(readFileSync(sourcePath)) !== object.sha256) {
    throw new Error(`Image candidate ${kind} SHA-256 mismatch`);
  }
  return {
    sourcePath,
    objectKey: `${receipt.candidatePrefix}/${object.path}`,
    relativePath: object.path,
    bytes: object.bytes,
    sha256: object.sha256,
    contentType: "application/zstd",
  };
}

export function candidateEntries({ candidateRoot }) {
  const { root, receipt } = readReceipt(candidateRoot);
  const entries = [
    ...receipt.packs.map(object => checkedEntry({ root, receipt, object, kind: "pack" })),
    ...receipt.maps.map(object => checkedEntry({ root, receipt, object, kind: "map" })),
  ];
  const paths = new Set(entries.map(entry => entry.relativePath));
  if (
    paths.size !== entries.length ||
    receipt.maps.length !== 2 ||
    !["maps/ko_KR.json.zst", "maps/en_US.json.zst"].every(path => paths.has(path))
  ) {
    throw new Error("Image candidate object path set is invalid");
  }
  return entries;
}

export function receiptEntry({ candidateRoot }) {
  const { receipt, receiptPath } = readReceipt(candidateRoot);
  const bytes = readFileSync(receiptPath);
  return {
    sourcePath: receiptPath,
    objectKey: `${receipt.candidatePrefix}/receipt.json`,
    relativePath: "receipt.json",
    bytes: bytes.length,
    sha256: sha256(bytes),
    contentType: "application/json",
  };
}

function parseOptions(args) {
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    if (!args[index]?.startsWith("--") || args[index + 1] === undefined) {
      throw new Error("Invalid command options");
    }
    options[args[index].slice(2)] = args[index + 1];
  }
  return options;
}

function writeEntry(entry) {
  process.stdout.write(
    [
      entry.sourcePath,
      entry.objectKey,
      entry.relativePath,
      entry.bytes,
      entry.sha256,
      entry.contentType,
    ].join("\t") + "\n",
  );
}

function runCli() {
  const [command, ...args] = process.argv.slice(2);
  const options = parseOptions(args);
  if (command === "entries") {
    for (const entry of candidateEntries({ candidateRoot: options["candidate-root"] })) {
      writeEntry(entry);
    }
    return;
  }
  if (command === "receipt-entry") {
    writeEntry(receiptEntry({ candidateRoot: options["candidate-root"] }));
    return;
  }
  throw new Error("Expected entries or receipt-entry command");
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    runCli();
  } catch (error) {
    process.stderr.write(
      `${JSON.stringify({ event: "image_candidate_error", message: error.message })}\n`,
    );
    process.exitCode = 1;
  }
}
