import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "happy-dom",
    exclude: [
      ...configDefaults.exclude,
      ".worktrees/**",
      "tests/project-tracking*.test.mjs",
      "tests/merge-gate.test.mjs",
      "tests/card-data-raw-r2-workflow.test.mjs",
      "tests/card-data-image-r2-workflow.test.mjs",
      "tests/card-image-upscale-r2-workflow.test.mjs",
    ],
  },
});
