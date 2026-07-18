import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";

const contractPath = resolve(process.cwd(), "docs/design/card-data-contract-mock.ko-KR.json");
const htmlPath = resolve(process.cwd(), "docs/design/card-data-schema-explorer.html");
const contract = JSON.parse(readFileSync(contractPath, "utf8"));

describe("card data contract detail sample", () => {
  it("preserves the complete official Hot Spring Glider card payload", () => {
    const card = contract.official_raw_samples.hot_spring_glider;

    expect(card?.id).toBe(117382);
    expect(card).toMatchObject({
      artistName: "Patrik Bjorkstrom",
      flavorText:
        "운고로 분화구에서 살아남으려면 눈에 띄지 않아야 합니다. 가르글은 오래 버티지 못할 겁니다.",
      keywordIds: [3, 8, 351],
      minionTypeId: 14,
      imageGold: "",
    });
  });

  it("keeps every metadata join required by the official detail view", () => {
    const catalog = contract.ipc_wire_mocks.metadata_catalog;
    const common = catalog.rarities.find((rarity: { id: number }) => rarity.id === 1);

    expect(catalog.minion_types ?? []).toContainEqual({ id: 14, slug: "murloc", name: "멀록" });
    expect(common).toMatchObject({
      crafting_cost: { normal: 40, golden: 400 },
      dust_value: { normal: 5, golden: 50 },
    });
    expect(catalog.keywords).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 3, slug: "divine-shield", name: "천상의 보호막" }),
        expect.objectContaining({ id: 8, slug: "battlecry", name: "전투의 함성" }),
        expect.objectContaining({ id: 351, slug: "kindred", name: "유사" }),
      ]),
    );
  });

  it("returns a self-contained CardDetail for the frontend", () => {
    const detail = contract.ipc_wire_mocks.card_detail;

    expect(detail.summary.id).toBe(117382);
    expect(detail.taxonomy).toMatchObject({
      classes: [{ id: 5, slug: "paladin", name: "성기사" }],
      type: { id: 4, slug: "minion", name: "하수인" },
      minion_type: { id: 14, slug: "murloc", name: "멀록" },
      set: { id: 1952, slug: "the-lost-city-of-ungoro", name: "운고로의 잃어버린 도시" },
      rarity: { id: 1, slug: "common", name: "일반" },
    });
    expect(detail.economy).toEqual({
      crafting_cost: { normal: 40, golden: 400 },
      dust_value: { normal: 5, golden: 50 },
    });
    expect(detail.keywords).toHaveLength(3);
  });

  it("shows the expanded detail sample in the local HTML mock", () => {
    const html = readFileSync(htmlPath, "utf8");

    expect(html).toContain("온천 활공꾼");
    expect(html).toContain("제작 비용");
    expect(html).toContain("마력 추출");
    expect(html).toContain("Patrik Bjorkstrom");
  });

  it("opens the detail sample directly from the detail hash", () => {
    const html = readFileSync(htmlPath, "utf8");
    const scripts = [...html.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/g)].map(
      (match) => match[1],
    );
    const markup = html.replace(/<script(?:\s[^>]*)?>[\s\S]*?<\/script>/g, "");
    const window = new Window({ url: "file:///card-data-schema-explorer.html#detail" });

    window.document.write(markup);
    window.HTMLElement.prototype.scrollTo = () => undefined;
    window.eval(scripts[0]);

    expect(window.document.getElementById("view-detail")?.classList.contains("active")).toBe(true);
  });
});
