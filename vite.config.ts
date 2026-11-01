/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import svgr from "vite-plugin-svgr";

export default defineConfig({
  plugins: [
    react(),
    // svgo desactivado: eliminaría ids, <style> y viewBox del rig (ver BEE_GUIDE §7.3)
    svgr({ svgrOptions: { svgo: false } }),
  ],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "chrome120", sourcemap: false },
  test: { environment: "jsdom", include: ["src/**/*.test.{ts,tsx}"], setupFiles: ["src/test/setup.ts"] },
});
