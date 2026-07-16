# Offline Hearthstone Card Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a copyable, fully offline Korean Hearthstone card preview containing 12 real cards and their full-card images.

**Architecture:** A Node build script validates the existing fixture, downloads each image once, and emits a classic JavaScript data file that works under `file://`. A static HTML page loads the generated data and a small standalone filtering engine without making runtime network requests.

**Tech Stack:** Node.js 26 built-in modules, Node test runner, plain HTML/CSS/JavaScript, PNG assets

## Global Constraints

- Work only under `C:\Users\main\Desktop\claude_project\hearthstone-card-library`.
- The preview must open by double-clicking `preview/cards.html`; no local server is allowed.
- The generated preview must contain no remote runtime request, web font, script, stylesheet, or image.
- Exactly 12 unique Korean cards from `data/fixtures/official-cards.ko-KR.semantic-sample.json` must be included.
- Card images must use `preview/assets/cards/<card-id>.png`.
- Tests use only `node --test`; do not add a package dependency.
- The source fixture remains identified as a temporary website-proxy sample, not the production API contract.

---

## File Map

- `preview/filtering.js`: pure search, semantic classification, and AND-filter behavior shared by the page and tests.
- `scripts/build-offline-preview.mjs`: fixture validation, runtime-card transformation, image download, and `cards.js` generation.
- `preview/cards.html`: self-contained layout, styles, controls, rendering, sanitization, and missing-image fallback.
- `preview/data/cards.js`: generated local runtime data with no source image URLs.
- `preview/assets/cards/*.png`: generated local card images.
- `tests/filtering.test.mjs`: filtering unit tests.
- `tests/offline-preview-builder.test.mjs`: builder unit tests with an injected fake downloader.
- `tests/offline-preview-shell.test.mjs`: static HTML contract tests.
- `tests/offline-preview-bundle.test.mjs`: generated bundle integrity and offline tests.

### Task 1: Filtering engine

**Files:**
- Create: `tests/filtering.test.mjs`
- Create: `preview/filtering.js`

**Interfaces:**
- Consumes: card objects with `name`, `text`, optional `plainText`, and optional `semantic` fields.
- Produces: `globalThis.CardFilters.stripMarkup(text)`, `classifyCard(card)`, and `matchesCard(card, filters)`.

- [ ] **Step 1: Write the failing filtering tests**

Create `tests/filtering.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const modulePath = new URL("../preview/filtering.js", import.meta.url);

function loadFilters() {
  assert.ok(existsSync(modulePath), "preview/filtering.js should exist");
  const context = {};
  runInNewContext(readFileSync(modulePath, "utf8"), context);
  assert.equal(typeof context.CardFilters?.matchesCard, "function");
  return context.CardFilters;
}

const apexBlast = {
  name: "에펙시스 폭발",
  text: "피해를 5 줍니다. 내 덱에 하수인이 없으면, 비용이 5인 <b>무작위</b> 하수인을 소환합니다.",
};

test("classifies the four semantic sample predicates", () => {
  const { classifyCard } = loadFilters();
  assert.deepEqual(
    { ...classifyCard(apexBlast) },
    { random: true, costFive: true, odd: false, noMinion: true },
  );
});

test("matches normalized Korean text and combines enabled filters with AND", () => {
  const { matchesCard } = loadFilters();
  assert.equal(matchesCard(apexBlast, { query: "  에펙시스  " }), true);
  assert.equal(matchesCard(apexBlast, { random: true, costFive: true }), true);
  assert.equal(matchesCard(apexBlast, { random: true, odd: true }), false);
});

test("empty controls return every card", () => {
  const { matchesCard } = loadFilters();
  assert.equal(matchesCard(apexBlast, {}), true);
});
```

- [ ] **Step 2: Run the test and verify RED**

Run: `node --test tests/filtering.test.mjs`

Expected: FAIL with `preview/filtering.js should exist`.

- [ ] **Step 3: Implement the filtering engine**

Create `preview/filtering.js`:

```js
(function attachCardFilters(root) {
  "use strict";

  function stripMarkup(value) {
    return String(value ?? "").replace(/<[^>]*>/g, "");
  }

  function normalize(value) {
    return stripMarkup(value).normalize("NFKC").toLocaleLowerCase("ko-KR").trim();
  }

  function classifyCard(card) {
    const text = stripMarkup(card.plainText ?? card.text);
    return {
      random: text.includes("무작위"),
      costFive: text.includes("비용이 5인"),
      odd: text.includes("홀수"),
      noMinion: /하수인이 없|다른 하수인이 없/.test(text),
    };
  }

  function matchesCard(card, filters = {}) {
    const query = normalize(filters.query);
    const haystack = normalize(`${card.name ?? ""} ${card.plainText ?? card.text ?? ""}`);
    if (query && !haystack.includes(query)) return false;

    const semantic = card.semantic ?? classifyCard(card);
    return ["random", "costFive", "odd", "noMinion"].every(
      (key) => !filters[key] || semantic[key] === true,
    );
  }

  root.CardFilters = Object.freeze({ stripMarkup, classifyCard, matchesCard });
})(globalThis);
```

- [ ] **Step 4: Run the test and verify GREEN**

Run: `node --test tests/filtering.test.mjs`

Expected: 3 tests pass, 0 fail.

- [ ] **Step 5: Commit the filtering engine**

```powershell
git add -- tests/filtering.test.mjs preview/filtering.js
git commit -m "feat: add offline card filtering engine"
```

### Task 2: Offline bundle builder

**Files:**
- Create: `tests/offline-preview-builder.test.mjs`
- Create: `scripts/build-offline-preview.mjs`
- Track: `data/fixtures/official-cards.ko-KR.semantic-sample.json`

**Interfaces:**
- Consumes: the existing JSON fixture and an injectable `fetchImpl(url)` function.
- Produces: `validateFixture(fixture)`, `buildPreviewCards(fixture)`, `renderDataScript(cards, source)`, `ensureLocalImages(cards, imageDir, fetchImpl)`, and `buildOfflinePreview(options)`.

- [ ] **Step 1: Write the failing builder tests**

Create `tests/offline-preview-builder.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { mkdtemp, readFile, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const builderUrl = new URL("../scripts/build-offline-preview.mjs", import.meta.url);
const fixtureUrl = new URL("../data/fixtures/official-cards.ko-KR.semantic-sample.json", import.meta.url);

async function loadBuilder() {
  assert.ok(existsSync(builderUrl), "build-offline-preview.mjs should exist");
  return import(builderUrl.href);
}

async function loadFixture() {
  return JSON.parse(await readFile(fixtureUrl, "utf8"));
}

test("validates and transforms exactly 12 unique cards", async () => {
  const { validateFixture, buildPreviewCards } = await loadBuilder();
  const fixture = await loadFixture();
  assert.equal(validateFixture(fixture).length, 12);
  const cards = buildPreviewCards(fixture);
  assert.equal(new Set(cards.map((card) => card.id)).size, 12);
  assert.ok(cards.every((card) => card.imagePath === `assets/cards/${card.id}.png`));
  assert.ok(cards.some((card) => card.semantic.random));
  assert.ok(cards.some((card) => card.semantic.costFive));
  assert.ok(cards.some((card) => card.semantic.odd));
  assert.ok(cards.some((card) => card.semantic.noMinion));
});

test("data script contains local runtime fields but no remote image URL", async () => {
  const { buildPreviewCards, renderDataScript } = await loadBuilder();
  const fixture = await loadFixture();
  const script = renderDataScript(buildPreviewCards(fixture), fixture.source);
  assert.match(script, /window\.HEARTHSTONE_CARDS/);
  assert.doesNotMatch(script, /https?:\/\//);
});

test("downloads every missing image through the injected downloader", async () => {
  const { buildPreviewCards, ensureLocalImages } = await loadBuilder();
  const cards = buildPreviewCards(await loadFixture());
  const imageDir = await mkdtemp(path.join(os.tmpdir(), "hs-preview-"));
  const png = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const calls = [];
  const fetchImpl = async (url) => {
    calls.push(url);
    return {
      ok: true,
      status: 200,
      arrayBuffer: async () => png.buffer.slice(png.byteOffset, png.byteOffset + png.byteLength),
    };
  };

  await ensureLocalImages(cards, imageDir, fetchImpl);
  assert.equal(calls.length, 12);
  for (const card of cards) {
    assert.ok((await stat(path.join(imageDir, `${card.id}.png`))).size > 0);
  }
});
```

- [ ] **Step 2: Run the test and verify RED**

Run: `node --test tests/offline-preview-builder.test.mjs`

Expected: FAIL with `build-offline-preview.mjs should exist`.

- [ ] **Step 3: Implement the builder**

Create `scripts/build-offline-preview.mjs`:

```js
import { readFile, writeFile, mkdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const defaultFixturePath = path.join(repoRoot, "data", "fixtures", "official-cards.ko-KR.semantic-sample.json");
const defaultPreviewRoot = path.join(repoRoot, "preview");

function stripMarkup(value) {
  return String(value ?? "").replace(/<[^>]*>/g, "");
}

function semanticFlags(card) {
  const text = stripMarkup(card.text);
  return {
    random: text.includes("무작위"),
    costFive: text.includes("비용이 5인"),
    odd: text.includes("홀수"),
    noMinion: /하수인이 없|다른 하수인이 없/.test(text),
  };
}

export function validateFixture(fixture) {
  if (!fixture || !Array.isArray(fixture.cards)) throw new Error("Fixture must contain a cards array");
  if (fixture.cards.length !== 12) throw new Error(`Expected 12 cards, received ${fixture.cards.length}`);
  const ids = new Set();
  for (const card of fixture.cards) {
    if (!Number.isInteger(card.id)) throw new Error("Every card must have an integer id");
    if (ids.has(card.id)) throw new Error(`Duplicate card id: ${card.id}`);
    if (typeof card.image !== "string" || !card.image.startsWith("https://")) {
      throw new Error(`Card ${card.id} is missing an HTTPS image URL`);
    }
    ids.add(card.id);
  }
  return fixture.cards;
}

export function buildPreviewCards(fixture) {
  return validateFixture(fixture).map((card) => ({
    id: card.id,
    name: card.name,
    text: card.text ?? "",
    plainText: stripMarkup(card.text),
    manaCost: card.manaCost ?? null,
    attack: card.attack ?? null,
    health: card.health ?? null,
    cardTypeId: card.cardTypeId ?? null,
    cardSetId: card.cardSetId ?? null,
    semantic: semanticFlags(card),
    imagePath: `assets/cards/${card.id}.png`,
    sourceImageUrl: card.image,
  }));
}

export function renderDataScript(cards, source = {}) {
  const runtimeCards = cards.map(({ sourceImageUrl, ...card }) => card);
  const runtimeSource = { locale: source.locale ?? "ko_KR", fetchedAtUtc: source.fetchedAtUtc ?? null };
  return `window.HEARTHSTONE_SOURCE = Object.freeze(${JSON.stringify(runtimeSource, null, 2)});\nwindow.HEARTHSTONE_CARDS = Object.freeze(${JSON.stringify(runtimeCards, null, 2)});\n`;
}

async function isNonEmptyFile(filePath) {
  try {
    return (await stat(filePath)).size > 0;
  } catch {
    return false;
  }
}

export async function ensureLocalImages(cards, imageDir, fetchImpl = globalThis.fetch) {
  if (typeof fetchImpl !== "function") throw new Error("A fetch implementation is required");
  await mkdir(imageDir, { recursive: true });
  for (const card of cards) {
    const destination = path.join(imageDir, `${card.id}.png`);
    if (await isNonEmptyFile(destination)) continue;
    const response = await fetchImpl(card.sourceImageUrl);
    if (!response.ok) throw new Error(`Image download failed for card ${card.id}: HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.length === 0) throw new Error(`Image download returned no bytes for card ${card.id}`);
    await writeFile(destination, bytes);
  }
}

export async function buildOfflinePreview({
  fixturePath = defaultFixturePath,
  previewRoot = defaultPreviewRoot,
  fetchImpl = globalThis.fetch,
} = {}) {
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));
  const cards = buildPreviewCards(fixture);
  const dataDir = path.join(previewRoot, "data");
  const imageDir = path.join(previewRoot, "assets", "cards");
  await mkdir(dataDir, { recursive: true });
  await ensureLocalImages(cards, imageDir, fetchImpl);
  await writeFile(path.join(dataDir, "cards.js"), renderDataScript(cards, fixture.source), "utf8");
  return { cardCount: cards.length, previewRoot };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  buildOfflinePreview()
    .then(({ cardCount, previewRoot }) => console.log(`Built ${cardCount} cards in ${previewRoot}`))
    .catch((error) => {
      console.error(error.message);
      process.exitCode = 1;
    });
}
```

- [ ] **Step 4: Run builder tests and verify GREEN**

Run: `node --test tests/offline-preview-builder.test.mjs`

Expected: 3 tests pass, 0 fail.

- [ ] **Step 5: Commit the builder and source fixture**

```powershell
git add -- scripts/build-offline-preview.mjs tests/offline-preview-builder.test.mjs data/fixtures/official-cards.ko-KR.semantic-sample.json
git commit -m "feat: add offline card preview builder"
```

### Task 3: Local HTML shell

**Files:**
- Create: `tests/offline-preview-shell.test.mjs`
- Create: `preview/cards.html`

**Interfaces:**
- Consumes: `globalThis.CardFilters` from `preview/filtering.js` and `window.HEARTHSTONE_CARDS` from generated `preview/data/cards.js`.
- Produces: a directly openable `preview/cards.html` with search, four AND-combined quick filters, card rendering, markup sanitization, and missing-image fallback.

- [ ] **Step 1: Write the failing HTML contract test**

Create `tests/offline-preview-shell.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";

const htmlUrl = new URL("../preview/cards.html", import.meta.url);

test("HTML shell is file-safe and exposes every required control", () => {
  assert.ok(existsSync(htmlUrl), "preview/cards.html should exist");
  const html = readFileSync(htmlUrl, "utf8");
  assert.match(html, /<script src="\.\/filtering\.js"><\/script>/);
  assert.match(html, /<script src="\.\/data\/cards\.js"><\/script>/);
  for (const id of ["search", "random", "costFive", "odd", "noMinion", "card-grid", "result-count"]) {
    assert.match(html, new RegExp(`id="${id}"`));
  }
  assert.doesNotMatch(html, /<(?:script|link|img)[^>]+https?:\/\//i);
});
```

- [ ] **Step 2: Run the test and verify RED**

Run: `node --test tests/offline-preview-shell.test.mjs`

Expected: FAIL with `preview/cards.html should exist`.

- [ ] **Step 3: Create the local HTML page**

Create `preview/cards.html` with this complete document:

```html
<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>하스스톤 카드 의미 필터 샘플</title>
  <style>
    :root { color-scheme: dark; --bg:#12100e; --panel:#201a15; --line:#433528; --gold:#f2c46d; --text:#f7f0e5; --muted:#b8aa98; }
    * { box-sizing:border-box; }
    body { margin:0; min-height:100vh; background:radial-gradient(circle at top,#352517 0,#17120e 48%,#0e0c0a 100%); color:var(--text); font-family:"Malgun Gothic","Apple SD Gothic Neo",sans-serif; }
    header { padding:42px 24px 22px; text-align:center; }
    h1 { margin:0 0 10px; color:var(--gold); font-size:clamp(28px,5vw,48px); }
    header p { margin:0; color:var(--muted); }
    .toolbar { position:sticky; top:0; z-index:5; display:grid; gap:14px; max-width:1180px; margin:0 auto 24px; padding:16px; background:rgba(25,20,16,.94); border:1px solid var(--line); border-radius:16px; backdrop-filter:blur(8px); }
    #search { width:100%; padding:13px 15px; border:1px solid var(--line); border-radius:10px; background:#100d0b; color:var(--text); font-size:16px; }
    .filter-row { display:flex; flex-wrap:wrap; gap:9px; align-items:center; }
    .filter-pill { cursor:pointer; padding:8px 11px; border:1px solid var(--line); border-radius:999px; color:var(--muted); }
    .filter-pill:has(input:checked) { background:#5c3d1d; color:#fff4d7; border-color:#c18b45; }
    .filter-pill input { accent-color:#d49a4d; }
    #result-count { margin-left:auto; color:var(--gold); font-weight:700; }
    main { max-width:1240px; margin:auto; padding:0 24px 60px; }
    #card-grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(230px,1fr)); gap:20px; }
    .card { overflow:hidden; background:linear-gradient(160deg,#2b2119,#17120e); border:1px solid var(--line); border-radius:18px; box-shadow:0 14px 30px rgba(0,0,0,.28); }
    .art { position:relative; min-height:320px; display:grid; place-items:center; background:#0b0908; }
    .art img { display:block; width:100%; max-height:390px; object-fit:contain; }
    .art.is-missing img { display:none; }
    .fallback { display:none; padding:32px; color:var(--muted); text-align:center; }
    .art.is-missing .fallback { display:block; }
    .body { padding:16px; }
    .name-row { display:flex; align-items:flex-start; gap:10px; }
    .mana { flex:0 0 auto; display:grid; place-items:center; width:34px; height:34px; border-radius:50%; background:#2367b2; color:white; font-weight:800; }
    h2 { margin:3px 0 12px; font-size:19px; }
    .effect { min-height:74px; margin:0; color:#e8ded0; line-height:1.55; }
    .stats { display:flex; gap:8px; margin-top:14px; color:var(--muted); font-size:13px; }
    .stats span { padding:5px 8px; border:1px solid var(--line); border-radius:7px; }
    .empty { grid-column:1/-1; padding:70px 20px; text-align:center; color:var(--muted); }
    @media (max-width:600px) { .toolbar { top:0; border-radius:0; } #result-count { width:100%; margin-left:0; } }
  </style>
</head>
<body>
  <header>
    <h1>카드 효과 의미 필터</h1>
    <p>한국어 카드 12장으로 만든 완전 로컬 임시 샘플</p>
  </header>
  <section class="toolbar" aria-label="카드 필터">
    <input id="search" type="search" placeholder="카드명 또는 효과 검색" autocomplete="off">
    <div class="filter-row">
      <label class="filter-pill"><input id="random" type="checkbox"> 무작위</label>
      <label class="filter-pill"><input id="costFive" type="checkbox"> 비용이 5인</label>
      <label class="filter-pill"><input id="odd" type="checkbox"> 홀수</label>
      <label class="filter-pill"><input id="noMinion" type="checkbox"> 하수인 없음</label>
      <output id="result-count" aria-live="polite"></output>
    </div>
  </section>
  <main><div id="card-grid"></div></main>
  <script src="./filtering.js"></script>
  <script src="./data/cards.js"></script>
  <script>
    (() => {
      "use strict";
      const cards = window.HEARTHSTONE_CARDS ?? [];
      const controls = {
        query: document.querySelector("#search"),
        random: document.querySelector("#random"),
        costFive: document.querySelector("#costFive"),
        odd: document.querySelector("#odd"),
        noMinion: document.querySelector("#noMinion"),
      };
      const grid = document.querySelector("#card-grid");
      const count = document.querySelector("#result-count");

      function safeEffectMarkup(raw) {
        const template = document.createElement("template");
        template.innerHTML = String(raw ?? "");
        for (const element of [...template.content.querySelectorAll("*")]) {
          if (!["B", "I"].includes(element.tagName)) {
            element.replaceWith(document.createTextNode(element.textContent ?? ""));
          } else {
            for (const attribute of [...element.attributes]) element.removeAttribute(attribute.name);
          }
        }
        return template.innerHTML;
      }

      function valueOrDash(value) { return value === null || value === undefined ? "–" : value; }

      function renderCard(card) {
        const article = document.createElement("article");
        article.className = "card";
        article.innerHTML = `
          <div class="art"><img src="${card.imagePath}" alt=""><div class="fallback">이미지를 불러오지 못했습니다.<br><span></span></div></div>
          <div class="body"><div class="name-row"><span class="mana">${valueOrDash(card.manaCost)}</span><h2></h2></div>
          <p class="effect"></p><div class="stats"><span>공격 ${valueOrDash(card.attack)}</span><span>생명 ${valueOrDash(card.health)}</span><span>타입 ${valueOrDash(card.cardTypeId)}</span></div></div>`;
        article.querySelector("img").alt = card.name;
        article.querySelector(".fallback span").textContent = card.name;
        article.querySelector("h2").textContent = card.name;
        article.querySelector(".effect").innerHTML = safeEffectMarkup(card.text);
        article.querySelector("img").addEventListener("error", (event) => event.currentTarget.closest(".art").classList.add("is-missing"));
        return article;
      }

      function currentFilters() {
        return { query: controls.query.value, random: controls.random.checked, costFive: controls.costFive.checked, odd: controls.odd.checked, noMinion: controls.noMinion.checked };
      }

      function render() {
        const visible = cards.filter((card) => window.CardFilters.matchesCard(card, currentFilters()));
        count.textContent = `${visible.length} / ${cards.length}장`;
        grid.replaceChildren(...(visible.length ? visible.map(renderCard) : [Object.assign(document.createElement("p"), { className:"empty", textContent:"조건에 맞는 카드가 없습니다." })]));
      }

      for (const control of Object.values(controls)) control.addEventListener("input", render);
      render();
    })();
  </script>
</body>
</html>
```

- [ ] **Step 4: Run the HTML contract test and verify GREEN**

Run: `node --test tests/offline-preview-shell.test.mjs`

Expected: 1 test passes, 0 fail.

- [ ] **Step 5: Commit the HTML shell**

```powershell
git add -- preview/cards.html tests/offline-preview-shell.test.mjs
git commit -m "feat: add offline card preview page"
```

### Task 4: Generate and verify the complete offline artifact

**Files:**
- Create: `tests/offline-preview-bundle.test.mjs`
- Generate: `preview/data/cards.js`
- Generate: `preview/assets/cards/57217.png`
- Generate: `preview/assets/cards/110889.png`
- Generate: `preview/assets/cards/91872.png`
- Generate: `preview/assets/cards/120944.png`
- Generate: `preview/assets/cards/59609.png`
- Generate: `preview/assets/cards/48158.png`
- Generate: `preview/assets/cards/48445.png`
- Generate: `preview/assets/cards/86722.png`
- Generate: `preview/assets/cards/63347.png`
- Generate: `preview/assets/cards/117596.png`
- Generate: `preview/assets/cards/110780.png`
- Generate: `preview/assets/cards/9107.png`

**Interfaces:**
- Consumes: all earlier task outputs plus network access during the build only.
- Produces: a self-contained `preview/` directory that has 12 cards, 12 non-empty PNG files, and no runtime URL dependency.

- [ ] **Step 1: Write the failing generated-bundle test**

Create `tests/offline-preview-bundle.test.mjs`:

```js
import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync, statSync } from "node:fs";
import { runInNewContext } from "node:vm";

const htmlUrl = new URL("../preview/cards.html", import.meta.url);
const dataUrl = new URL("../preview/data/cards.js", import.meta.url);
const previewUrl = new URL("../preview/", import.meta.url);

test("generated preview is complete and contains no remote runtime asset", () => {
  assert.ok(existsSync(dataUrl), "preview/data/cards.js should be generated");
  const context = { window: {} };
  const dataScript = readFileSync(dataUrl, "utf8");
  runInNewContext(dataScript, context);
  const cards = context.window.HEARTHSTONE_CARDS;
  assert.equal(cards.length, 12);
  assert.equal(new Set(cards.map((card) => card.id)).size, 12);
  assert.doesNotMatch(dataScript, /https?:\/\//);

  for (const card of cards) {
    const imageUrl = new URL(card.imagePath, previewUrl);
    assert.ok(existsSync(imageUrl), `missing local image for ${card.id}`);
    assert.ok(statSync(imageUrl).size > 0, `empty local image for ${card.id}`);
  }

  const html = readFileSync(htmlUrl, "utf8");
  assert.doesNotMatch(html, /<(?:script|link|img)[^>]+https?:\/\//i);
});
```

- [ ] **Step 2: Run the bundle test and verify RED**

Run: `node --test tests/offline-preview-bundle.test.mjs`

Expected: FAIL with `preview/data/cards.js should be generated`.

- [ ] **Step 3: Generate the local data and all images**

Run: `node scripts/build-offline-preview.mjs`

Expected: `Built 12 cards in ...\preview` and 12 non-empty PNG files under `preview/assets/cards`.

- [ ] **Step 4: Run the full verification suite**

Run: `node --test tests/*.test.mjs`

Expected: 8 tests pass, 0 fail, with no warnings or errors.

Run: `git diff --check`

Expected: no output and exit code 0.

- [ ] **Step 5: Commit the generated offline artifact**

```powershell
git add -- tests/offline-preview-bundle.test.mjs preview/data/cards.js preview/assets/cards
git commit -m "feat: bundle offline Hearthstone card sample"
```

- [ ] **Step 6: Hand off the local file without opening a browser tab**

Report the absolute path `C:\Users\main\Desktop\claude_project\hearthstone-card-library\preview\cards.html`, the test count, total image bytes, and the final `git status --short`. Do not launch a browser automatically.
