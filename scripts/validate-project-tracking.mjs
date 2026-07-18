import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const defaultRepoRoot = path.resolve(path.dirname(modulePath), "..");
const taskHeadingPattern = /^## (HCL-[A-Za-z0-9-]+) — (\S(?:.*\S)?)\s*$/gm;
const hclHeadingPattern = /^## HCL-[^\r\n]*$/gm;

const requiredColumns = ["backlog", "ready", "in_progress", "verify", "done"];
const allowedStatuses = new Set(requiredColumns);
const allowedPriorities = new Set(["P0", "P1", "P2", "P3"]);
const allowedTypes = new Set(["feature", "bug", "design", "research", "chore"]);
const requiredTodoFields = ["status", "priority", "type", "updated"];
const requiredCardFields = [
  "id",
  "title",
  "status",
  "priority",
  "type",
  "updated",
  "codex",
  "branch",
  "worktree",
  "dependsOn",
  "spec",
  "plan",
  "nextGate",
  "blocked",
];

const fieldNames = new Map([
  ["Status", "status"],
  ["Priority", "priority"],
  ["Type", "type"],
  ["Updated", "updated"],
  ["Codex", "codex"],
  ["Branch", "branch"],
  ["Worktree", "worktree"],
  ["Depends on", "dependsOn"],
  ["Spec", "spec"],
  ["Plan", "plan"],
  ["Next gate", "nextGate"],
  ["Blocked", "blocked"],
]);

function unwrap(value) {
  const trimmed = value.trim();
  const unwrapped = trimmed.startsWith("`") && trimmed.endsWith("`")
    ? trimmed.slice(1, -1)
    : trimmed;
  return unwrapped === "—" ? "" : unwrapped;
}

function headingMatches(markdown) {
  return [...String(markdown).matchAll(taskHeadingPattern)];
}

function malformedTaskHeadings(markdown) {
  return [...String(markdown).matchAll(hclHeadingPattern)]
    .map(([heading]) => heading)
    .filter((heading) => !/^## HCL-[A-Za-z0-9-]+ — \S(?:.*\S)?\s*$/.test(heading));
}

function kanbanColumnCounts(html) {
  const counts = new Map(requiredColumns.map((status) => [status, 0]));
  for (const match of String(html).matchAll(/\bdata-column\s*=\s*["']([^"']+)["']/gi)) {
    if (counts.has(match[1])) counts.set(match[1], counts.get(match[1]) + 1);
  }
  return counts;
}

function metadataFrom(source, matches, index) {
  const start = matches[index].index + matches[index][0].length;
  const end = matches[index + 1]?.index ?? source.length;
  const fields = {};
  for (const line of source.slice(start, end).split(/\r?\n/)) {
    const field = line.match(/^- ([A-Za-z][A-Za-z ]+):\s*(.+)$/);
    const key = field && fieldNames.get(field[1]);
    if (key) fields[key] = unwrap(field[2]);
  }
  return fields;
}

export function parseTodoTasks(markdown) {
  const source = String(markdown);
  const matches = headingMatches(source);
  return matches.map((match, index) => ({
    id: match[1],
    title: match[2].trim(),
    ...metadataFrom(source, matches, index),
  }));
}

export function parseDoneTasks(markdown) {
  const source = String(markdown);
  const matches = headingMatches(source);
  return matches.map((match, index) => {
    const metadata = metadataFrom(source, matches, index);
    return {
      id: match[1],
      title: match[2].trim(),
      spec: metadata.spec ?? "",
      plan: metadata.plan ?? "",
    };
  });
}

export function parseKanbanCards(html) {
  const block = String(html).match(
    /<script\s+id=["']kanban-data["']\s+type=["']application\/json["']\s*>([\s\S]*?)<\/script>/i,
  );
  if (!block) throw new Error("Kanban data block is missing");
  let data;
  try {
    data = JSON.parse(block[1]);
  } catch (error) {
    throw new Error(`Kanban JSON is invalid: ${error.message}`);
  }
  if (!data || !Array.isArray(data.cards)) throw new Error("Kanban data must contain a cards array");
  return data.cards;
}

function duplicateIds(items) {
  const seen = new Set();
  const duplicates = new Set();
  for (const { id } of items) {
    if (seen.has(id)) duplicates.add(id);
    seen.add(id);
  }
  return [...duplicates].sort();
}

function compareField(todoTask, card, field, errors) {
  if (todoTask[field] !== card[field]) {
    errors.push(`${todoTask.id} ${field} mismatch: TODO=${todoTask[field]}, Kanban=${card[field]}`);
  }
}

function resolveTrackedPath(repoRoot, trackedPath) {
  const candidate = path.resolve(repoRoot, trackedPath.replaceAll("/", path.sep));
  const rootPrefix = `${path.resolve(repoRoot)}${path.sep}`;
  return candidate.startsWith(rootPrefix) ? candidate : null;
}

export function validateTracking({
  todoMarkdown,
  doneMarkdown,
  kanbanHtml,
  repoRoot = defaultRepoRoot,
  fileExists = existsSync,
}) {
  const errors = [];
  const todoTasks = parseTodoTasks(todoMarkdown);
  const doneTasks = parseDoneTasks(doneMarkdown);
  let cards;
  try {
    cards = parseKanbanCards(kanbanHtml);
  } catch (error) {
    return [error.message];
  }

  for (const heading of malformedTaskHeadings(todoMarkdown)) {
    errors.push(`Malformed TODO task heading: ${heading}`);
  }
  for (const heading of malformedTaskHeadings(doneMarkdown)) {
    errors.push(`Malformed DONE task heading: ${heading}`);
  }
  for (const [status, count] of kanbanColumnCounts(kanbanHtml)) {
    if (count !== 1) {
      errors.push(`Kanban column must appear exactly once: ${status} (found ${count})`);
    }
  }

  for (const id of duplicateIds(todoTasks)) errors.push(`Duplicate TODO ID: ${id}`);
  for (const id of duplicateIds(doneTasks)) errors.push(`Duplicate DONE ID: ${id}`);
  for (const id of duplicateIds(cards)) errors.push(`Duplicate Kanban ID: ${id}`);

  const doneIds = new Set(doneTasks.map(({ id }) => id));
  for (const task of doneTasks) {
    if (!/^HCL-\d{3,}$/.test(task.id)) errors.push(`Invalid DONE ID: ${task.id}`);
    for (const field of ["spec", "plan"]) {
      if (!task[field]) continue;
      const resolved = resolveTrackedPath(repoRoot, task[field]);
      if (!resolved || !fileExists(resolved)) {
        errors.push(`${task.id} archived ${field} does not exist: ${task[field]}`);
      }
    }
  }
  for (const { id } of todoTasks) {
    if (doneIds.has(id)) errors.push(`ID exists in both TODO and DONE: ${id}`);
  }

  const todoById = new Map(todoTasks.map((task) => [task.id, task]));
  const cardById = new Map(cards.map((cardItem) => [cardItem.id, cardItem]));

  for (const task of todoTasks) {
    if (!/^HCL-\d{3,}$/.test(task.id)) errors.push(`Invalid TODO ID: ${task.id}`);
    for (const field of requiredTodoFields) {
      if (!task[field]) errors.push(`${task.id} missing TODO field: ${field}`);
    }
    if (task.status && !allowedStatuses.has(task.status)) {
      errors.push(`${task.id} invalid TODO status: ${task.status}`);
    }
    if (task.priority && !allowedPriorities.has(task.priority)) {
      errors.push(`${task.id} invalid TODO priority: ${task.priority}`);
    }
    if (task.type && !allowedTypes.has(task.type)) {
      errors.push(`${task.id} invalid TODO type: ${task.type}`);
    }
    if (!cardById.has(task.id)) errors.push(`TODO task missing from Kanban: ${task.id}`);

    if (task.status === "done") {
      for (const field of ["spec", "plan"]) {
        if (!task[field]) continue;
        const resolved = resolveTrackedPath(repoRoot, task[field]);
        if (!resolved || !fileExists(resolved)) {
          errors.push(`${task.id} completed ${field} does not exist: ${task[field]}`);
        }
      }
    }
  }

  for (const cardItem of cards) {
    if (!/^HCL-\d{3,}$/.test(cardItem.id ?? "")) errors.push(`Invalid Kanban ID: ${cardItem.id}`);
    for (const field of requiredCardFields) {
      const value = cardItem[field];
      if (value === undefined || value === null) errors.push(`${cardItem.id ?? "Unknown card"} missing Kanban field: ${field}`);
    }
    if (cardItem.status && !allowedStatuses.has(cardItem.status)) {
      errors.push(`${cardItem.id} invalid Kanban status: ${cardItem.status}`);
    }
    if (cardItem.priority && !allowedPriorities.has(cardItem.priority)) {
      errors.push(`${cardItem.id} invalid Kanban priority: ${cardItem.priority}`);
    }
    if (cardItem.type && !allowedTypes.has(cardItem.type)) {
      errors.push(`${cardItem.id} invalid Kanban type: ${cardItem.type}`);
    }
    if (!Array.isArray(cardItem.dependsOn)) errors.push(`${cardItem.id} Kanban dependsOn must be an array`);

    const task = todoById.get(cardItem.id);
    if (!task) {
      errors.push(`Kanban card missing from TODO: ${cardItem.id}`);
      continue;
    }
    for (const field of ["title", "status", "priority", "type"]) {
      compareField(task, cardItem, field, errors);
    }
  }

  return errors;
}

export function validateTrackingFiles(repoRoot = defaultRepoRoot) {
  const todoMarkdown = readFileSync(path.join(repoRoot, "docs", "TODO.md"), "utf8");
  const doneMarkdown = readFileSync(path.join(repoRoot, "docs", "DONE.md"), "utf8");
  const kanbanHtml = readFileSync(path.join(repoRoot, "docs", "kanban.html"), "utf8");
  const errors = validateTracking({ todoMarkdown, doneMarkdown, kanbanHtml, repoRoot });
  return {
    errors,
    activeCount: parseTodoTasks(todoMarkdown).length,
    archivedCount: parseDoneTasks(doneMarkdown).length,
  };
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  try {
    const result = validateTrackingFiles(defaultRepoRoot);
    if (result.errors.length) {
      console.error("Project tracking validation failed:");
      for (const error of result.errors) console.error(`- ${error}`);
      process.exitCode = 1;
    } else {
      console.log(`Project tracking OK: ${result.activeCount} active cards, ${result.archivedCount} archived tasks.`);
    }
  } catch (error) {
    console.error(`Project tracking validation failed:\n- ${error.message}`);
    process.exitCode = 1;
  }
}
