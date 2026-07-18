import { describe, expect, it } from "vitest";

import { APP_NAME, SETUP_PARTS } from "./app-meta";

describe("project bootstrap", () => {
  it("exposes the app name and the three baseline parts", () => {
    expect(APP_NAME).toBe("Hearthstone Card Lab");
    expect(SETUP_PARTS).toHaveLength(3);
  });
});
