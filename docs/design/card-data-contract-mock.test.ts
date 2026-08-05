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

describe("related card contract sample", () => {
  it("preserves Vulcanos childIds and both official child payloads", () => {
    const parent = contract.official_raw_samples.vulcanos;
    const children = contract.official_raw_samples.vulcanos_related_cards;

    expect(parent).toMatchObject({
      id: 123665,
      name: "불카노스",
      childIds: [123666, 128032],
      keywordIds: [231],
    });
    expect(children).toHaveLength(2);
    expect(children.map((card: { id: number }) => card.id)).toEqual([123666, 128032]);
    expect(children.every((card: { parentId: number }) => card.parentId === 123665)).toBe(true);
    expect(new Set(children.map((card: { image: string }) => card.image)).size).toBe(2);
  });

  it("hydrates both non-collectible children in the frontend CardDetail", () => {
    const detail = contract.ipc_wire_mocks.vulcanos_card_detail;

    expect(detail.summary.id).toBe(123665);
    expect(detail.relations.child_ids).toEqual([123666, 128032]);
    expect(detail.relations.children).toHaveLength(2);
    expect(detail.relations.children).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 123666, name: "불카노스의 융기", collectible: false }),
        expect.objectContaining({ id: 128032, name: "불카노스의 융기", collectible: false }),
      ]),
    );
    expect(
      new Set(
        detail.relations.children.map(
          (card: { image: { normal: { cache_key: string } } }) => card.image.normal.cache_key,
        ),
      ).size,
    ).toBe(2);
  });

  it("shows and deep-links the Vulcanos related-card view in the local HTML mock", () => {
    const html = readFileSync(htmlPath, "utf8");
    const scripts = [...html.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/g)].map(
      (match) => match[1],
    );
    const markup = html.replace(/<script(?:\s[^>]*)?>[\s\S]*?<\/script>/g, "");
    const window = new Window({ url: "file:///card-data-schema-explorer.html#relations" });

    expect(html).toContain("불카노스의 융기");
    expect(html).toContain("123666");
    expect(html).toContain("128032");

    window.document.write(markup);
    window.HTMLElement.prototype.scrollTo = () => undefined;
    window.eval(scripts[0]);

    expect(window.document.getElementById("view-relations")?.classList.contains("active")).toBe(
      true,
    );
  });
});
