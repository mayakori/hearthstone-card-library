import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  base: "./",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2021",
    outDir: "dist",
    emptyOutDir: true,
  },
});
