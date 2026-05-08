/// <reference types="vitest" />
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

import path from "path";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/setupTests.ts",
    css: true,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
        protocol: "ws",
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          "vendor-react": ["react", "react-dom", "react-i18next"],
          "vendor-tauri": ["@tauri-apps/api", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-opener"],
          "vendor-app": ["i18next", "zustand", "sonner"],
          "vendor-icons": ["lucide-react"],
          "vendor-editor": [
            "@tiptap/core",
            "@tiptap/extension-bubble-menu",
            "@tiptap/extension-character-count",
            "@tiptap/extension-color",
            "@tiptap/extension-document",
            "@tiptap/extension-font-family",
            "@tiptap/extension-highlight",
            "@tiptap/extension-horizontal-rule",
            "@tiptap/extension-image",
            "@tiptap/extension-link",
            "@tiptap/extension-paragraph",
            "@tiptap/extension-placeholder",
            "@tiptap/extension-table",
            "@tiptap/extension-table-cell",
            "@tiptap/extension-table-header",
            "@tiptap/extension-table-row",
            "@tiptap/extension-text",
            "@tiptap/extension-text-align",
            "@tiptap/extension-text-style",
            "@tiptap/extension-underline",
            "@tiptap/react",
            "@tiptap/starter-kit",
            "tiptap-extension-resize-image",
            "tiptap-markdown",
          ],
          "vendor-markdown": ["react-markdown", "remark-gfm", "rehype-raw"],
          "vendor-syntax": ["react-syntax-highlighter"],
          "vendor-pdf": ["jspdf"],
          "vendor-canvas": ["html2canvas"],
          "vendor-doc-export": ["turndown", "html-docx-js-typescript"],
        },
      },
    },
    chunkSizeWarningLimit: 700,
  },
}));
