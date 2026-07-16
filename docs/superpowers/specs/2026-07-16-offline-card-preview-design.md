# Offline Hearthstone Card Preview Design

**Date:** 2026-07-16
**Status:** Approved in conversation
**Scope:** Temporary, fully local preview under `C:\Users\main\Desktop\claude_project\hearthstone-card-library`

## Goal

Produce a local HTML preview from the 12-card Korean fixture already fetched from the official Hearthstone website. A user must be able to copy the preview directory to another location, disconnect from the network, double-click the HTML file, and still see the card list and card images.

This preview validates the source data and basic presentation before the Tauri application is scaffolded. It is not the final application architecture.

## Chosen Approach

Use a fully local preview bundle:

- `preview/cards.html` contains the layout, styling, filter controls, and rendering code.
- `preview/data/cards.js` exposes the selected card data as a JavaScript value. A script file is used instead of `fetch()` so the preview works under `file://` without a local server.
- `preview/assets/cards/<card-id>.png` contains a downloaded full-card image for every fixture card.
- `scripts/build-offline-preview.mjs` rebuilds local data and images from the source fixture.
- `tests/offline-preview.test.mjs` verifies the generated bundle.

No runtime request, web font, remote script, remote stylesheet, or remote image is allowed in the generated preview.

## User Interface

The page shows:

- a header identifying the preview as a temporary Korean card snapshot;
- the number of visible cards;
- text search across card name and effect text;
- quick filters for random effects, 5-cost references, odd-cost conditions, and no-minion conditions;
- a responsive card grid;
- each card's local image, mana cost, attack, health, type IDs, and formatted effect text;
- a visible fallback when a local image cannot be loaded.

Filters combine with AND semantics: text search and every selected quick filter must match. Clearing the controls restores all 12 cards.

## Data Flow

1. Read `data/fixtures/official-cards.ko-KR.semantic-sample.json`.
2. Validate that the fixture contains 12 unique cards and each card has an `image` URL.
3. Download each full-card image to `preview/assets/cards/<id>.png`.
4. Generate `preview/data/cards.js` with local image paths and the original card fields needed by the page.
5. Open `preview/cards.html` directly from disk; it renders only local data and assets.

The existing fixture came from the official website's internal proxy and remains clearly marked as a temporary sample. The production data source will later be replaced by the documented Blizzard Game Data API.

## Error Handling

- The build fails with a clear card ID when the fixture is malformed, an ID is duplicated, or an image download fails.
- Existing image files may be reused only when they are non-empty.
- The HTML shows an inline placeholder and card name if an image is missing or corrupt at viewing time.
- Empty attack or health fields render as a dash rather than a misleading zero.
- Card effect markup is limited to the existing `b` and `i` tags from the trusted fixture; all other rendered values are inserted as text.

## Testing

Use Node's built-in test runner so the temporary preview needs no package installation. Tests must be written and observed failing before production files are added.

The tests verify:

- the preview HTML and generated data files exist;
- exactly 12 unique cards are rendered;
- every card references an existing, non-empty local PNG;
- generated image paths contain no `http://` or `https://` URL;
- the preview has no remote runtime dependencies;
- the four semantic quick-filter predicates match at least one fixture card;
- an empty search returns all cards and combined filters use AND semantics.

## Non-Goals

- Tauri integration or application packaging;
- automatic synchronization with Blizzard;
- production credential handling;
- full semantic parsing or deck building;
- golden-card animation;
- downloading the complete Hearthstone image catalog.

## Completion Criteria

The work is complete when the tests pass, all 12 images are stored locally, `preview/cards.html` opens without a server, and disabling the network does not remove any card data or image from the page.
