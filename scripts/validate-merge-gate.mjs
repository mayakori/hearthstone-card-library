import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const trackingFiles = ["docs/TODO.md", "docs/DONE.md", "docs/kanban.html"];
const branchPattern = /^codex\/hcl-(\d{3,})-[a-z0-9]+(?:-[a-z0-9]+)*$/;

function lines(value) {
  return String(value ?? "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function isIsoTimestamp(value) {
  return typeof value === "string"
    && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(value)
    && !Number.isNaN(Date.parse(value));
}

export function validateMergeGate({
  taskId,
  branch,
  mainSha,
  headSha,
  statusPorcelain,
  trackingDiff,
  vaResult,
}) {
  const errors = [];
  const branchMatch = String(branch ?? "").match(branchPattern);

  if (!/^HCL-\d{3,}$/.test(String(taskId ?? ""))) {
    errors.push("task ID must match HCL-###");
  }
  if (!branchMatch) {
    errors.push("branch must match codex/hcl-###-slug");
  } else if (`HCL-${branchMatch[1]}` !== taskId) {
    errors.push(`branch task does not match ${taskId}`);
  }
  if (lines(statusPorcelain).length) {
    errors.push("worktree must be clean before merge");
  }

  const changedTrackingFiles = lines(trackingDiff);
  if (changedTrackingFiles.length) {
    errors.push(`feature branch changes main-only tracking files: ${changedTrackingFiles.join(", ")}`);
  }

  if (!vaResult || typeof vaResult !== "object" || Array.isArray(vaResult)) {
    errors.push(".va-result.json is missing or invalid");
    return errors;
  }
  if (vaResult.status !== "clean") {
    errors.push("VA status must be clean");
  }
  if (!isIsoTimestamp(vaResult.checkedAt)) {
    errors.push("VA checkedAt must be an ISO-8601 timestamp");
  }
  if (vaResult.taskId !== taskId) {
    errors.push(`VA taskId does not match ${taskId}`);
  }
  if (vaResult.branch !== branch) {
    errors.push(`VA branch does not match ${branch}`);
  }
  if (vaResult.mainSha !== mainSha) {
    errors.push("VA mainSha is stale");
  }
  if (vaResult.headSha !== headSha) {
    errors.push("VA headSha is stale");
  }

  return errors;
}

function git(repoRoot, args) {
  return execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trimEnd();
}

function readVaResult(repoRoot) {
  try {
    return JSON.parse(readFileSync(path.join(repoRoot, ".va-result.json"), "utf8"));
  } catch {
    return null;
  }
}

export function validateCurrentWorktree(taskId, cwd = process.cwd()) {
  const repoRoot = git(cwd, ["rev-parse", "--show-toplevel"]);
  const branch = git(repoRoot, ["branch", "--show-current"]);
  const mainSha = git(repoRoot, ["rev-parse", "main"]);
  const headSha = git(repoRoot, ["rev-parse", "HEAD"]);
  const statusPorcelain = git(repoRoot, ["status", "--porcelain"]);
  const trackingDiff = git(repoRoot, [
    "diff",
    "--name-only",
    "main...HEAD",
    "--",
    ...trackingFiles,
  ]);
  const vaResult = readVaResult(repoRoot);

  return {
    repoRoot,
    errors: validateMergeGate({
      taskId,
      branch,
      mainSha,
      headSha,
      statusPorcelain,
      trackingDiff,
      vaResult,
    }),
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  const taskId = process.argv[2];
  if (!taskId) {
    console.error("Usage: npm run merge:check -- HCL-###");
    process.exitCode = 2;
  } else {
    try {
      const { errors } = validateCurrentWorktree(taskId);
      if (errors.length) {
        console.error("Merge gate blocked:");
        for (const error of errors) console.error(`- ${error}`);
        process.exitCode = 1;
      } else {
        console.log(`Merge gate OK: ${taskId}`);
      }
    } catch (error) {
      console.error(`Merge gate blocked:\n- ${error.message}`);
      process.exitCode = 1;
    }
  }
}
