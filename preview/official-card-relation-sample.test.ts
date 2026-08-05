import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";

const htmlPath = resolve(process.cwd(), "preview/official-card-relation-sample.html");
const html = readFileSync(htmlPath, "utf8");
const snapshotMatch = html.match(
  /<script id="snapshot" type="application\/json">([\s\S]*?)<\/script>/,
);

if (!snapshotMatch) {
  throw new Error("official relation snapshot was not found");
}

const snapshot = JSON.parse(snapshotMatch[1]);

function openMock() {
  const applicationMatch = html.match(
    /<script>\s*(\(\(\) => \{[\s\S]*?\}\)\(\);)\s*<\/script>\s*<\/body>/,
  );

  if (!applicationMatch) {
    throw new Error("official relation application script was not found");
  }

  const markup = html.replace(
    /<script>\s*\(\(\) => \{[\s\S]*?\}\)\(\);\s*<\/script>\s*<\/body>/,
    "</body>",
  );
  const window = new Window({ url: `file:///${htmlPath.replaceAll("\\", "/")}` });
  window.document.write(markup);
  window.eval(applicationMatch[1]);
  return window;
}

describe("official card relation samples", () => {
  it("records that the live Core cards came from the paginated Standard list", () => {
    const parent = snapshot.cards.find((card: { id: number }) => card.id === 69557);
    const child = snapshot.cards.find((card: { id: number }) => card.id === 69571);

    expect(snapshot.fetch.mode).toBe("standard-list-pages");
    expect(snapshot.fetch.individualLookups).toEqual([
      { id: 69557, status: 404 },
      { id: 69571, status: 404 },
    ]);
    expect(snapshot.listQuery.matchedIds).toEqual([69557, 69571, 125917, 127507]);
    expect(parent.childIds).toEqual([69571]);
    expect(child.parentId).toBe(69557);
    expect(child.collectible).toBe(0);
  });

  it("records every official relation row normalized for the sample cards", () => {
    expect(snapshot.pipeline.sqliteCards).toEqual([69557, 69571, 125917, 127507]);
    expect(
      snapshot.pipeline.sqliteRelations.filter(
        (relation: { sourceCardId: number }) =>
          relation.sourceCardId === 69557 || relation.sourceCardId === 69571,
      ),
    ).toEqual([
      {
        sourceCardId: 69557,
        relationKind: "child",
        sourceField: "childIds",
        targetCardId: 69571,
        displayOrder: 0,
      },
      {
        sourceCardId: 69557,
        relationKind: "copy_of",
        sourceField: "copyOfCardId",
        targetCardId: 68468,
        displayOrder: 0,
      },
      {
        sourceCardId: 69557,
        relationKind: "copy_of",
        sourceField: "copyOfCardId",
        targetCardId: 976,
        displayOrder: 1,
      },
      {
        sourceCardId: 69571,
        relationKind: "copy_of",
        sourceField: "copyOfCardId",
        targetCardId: 68469,
        displayOrder: 0,
      },
      {
        sourceCardId: 69571,
        relationKind: "copy_of",
        sourceField: "copyOfCardId",
        targetCardId: 1078,
        displayOrder: 1,
      },
      {
        sourceCardId: 69571,
        relationKind: "parent",
        sourceField: "parentId",
        targetCardId: 69557,
        displayOrder: 0,
      },
    ]);
  });

  it("includes a live relation pair from the newest expansion", () => {
    const latest = snapshot.pairs.latest;
    const parent = snapshot.cards.find((card: { id: number }) => card.id === latest.parentId);
    const child = snapshot.cards.find((card: { id: number }) => card.id === latest.childId);

    expect(latest.set).toEqual({
      id: 1988,
      slug: "escape-from-violet-hold",
      name: "보랏빛 요새 탈출 작전",
    });
    expect(parent.name).toBe("임프 무리 앞잡이");
    expect(parent.childIds).toEqual([127507]);
    expect(child.name).toBe("할머니 임프");
    expect(child.parentId).toBe(125917);
    expect(child.collectible).toBe(0);
  });

  it("renders the actual list-fetch provenance and non-collectible child status", () => {
    const window = openMock();
    const document = window.document;

    expect(document.getElementById("overall-result")?.textContent).toBe("8 / 8 PASS");
    expect(document.getElementById("parent-card")?.textContent).toContain("#69557");
    expect(document.getElementById("child-card")?.textContent).toContain("#69571");
    expect(document.getElementById("child-card")?.textContent).toContain("게임 요소 · 덱 편입 불가");
    expect(document.getElementById("checks")?.textContent).toContain("목록 API 수집");
    expect(document.getElementById("checks")?.textContent).toContain("copyOfCardId 4행");

    document.querySelector<HTMLButtonElement>("[data-version='latest']")?.click();
    expect(document.getElementById("parent-card")?.textContent).toContain("임프 무리 앞잡이");
    expect(document.getElementById("child-card")?.textContent).toContain("할머니 임프");
    expect(document.getElementById("scope-label")?.textContent).toContain(
      "보랏빛 요새 탈출 작전",
    );
  });
});
