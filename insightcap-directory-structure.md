# InsightCAP Directory Structure

更新日期：2026-05-08

本文件只記錄需要維護的 source、設定與 installer 相關檔案。以下內容刻意排除 build output、dependency cache、runtime cache、測試產物與使用者資料，例如 `dist/`、`node_modules/`、`target/`、`.fastembed_cache/`、runtime notes 與本機知識庫資料。

```text
InsightCAP/
├── Architecture-v2.md
├── insightcap-directory-structure.md
└── InsightCAP/
    ├── changelog.md
    ├── components.json
    ├── index.html
    ├── package.json
    ├── package-lock.json
    ├── postcss.config.js
    ├── tailwind.config.js
    ├── tsconfig.json
    ├── tsconfig.node.json
    ├── vite.config.ts
    ├── scripts/
    │   ├── cleanup-legacy-url-chunks.py
    │   └── knowledge_builder/
    │       └── build_kb.py
    ├── src/
    │   ├── App.tsx
    │   ├── main.tsx
    │   ├── setupTests.ts
    │   ├── vite-env.d.ts
    │   ├── components/
    │   │   ├── chat/
    │   │   ├── knowledge/
    │   │   ├── layout/
    │   │   ├── memory/
    │   │   └── ui/
    │   ├── design-system/
    │   ├── hooks/
    │   ├── i18n/
    │   │   └── locales/
    │   ├── lib/
    │   ├── pages/
    │   └── stores/
    └── src-tauri/
        ├── build.rs
        ├── Cargo.toml
        ├── installer.nsh
        ├── tauri.conf.json
        ├── capabilities/
        ├── icons/
        ├── migrations/
        ├── resources/
        ├── skills/
        ├── tests/
        └── src/
            ├── error.rs
            ├── http_server.rs
            ├── lib.rs
            ├── main.rs
            ├── prompts.rs
            ├── tray_status.rs
            ├── whisper_transcribe.rs
            ├── auth/
            ├── background/
            ├── bin/
            ├── capture/
            │   └── extractors/
            ├── commands/
            ├── db/
            ├── knowledge_source/
            ├── ocr/
            ├── providers/
            │   ├── embedding/
            │   └── llm/
            ├── services/
            ├── settings/
            ├── utils/
            └── vector_store/
```

## 維護原則

- `src/` 是 React + Vite frontend source。
- `src-tauri/src/` 是 Rust / Tauri backend source。
- `src-tauri/migrations/` 是 SQL migration，屬於 app schema contract，必須納入版本管理。
- `src-tauri/skills/` 是內建 skill prompt 文件，屬於 source，不是 runtime draft。
- `scripts/` 只保留仍可重跑、仍有維護價值的工具腳本。
- 不把 dependency、build output、cache、user data 或 installer 產物寫入此文件。

## 已排除類型

- Dependency：`node_modules/`
- Frontend build：`dist/`
- Rust build：`src-tauri/target/`
- Generated schemas：`src-tauri/gen/`
- Embedding cache：`src-tauri/.fastembed_cache/`
- Runtime draft/test notes：`src-tauri/notes/draft_*.md`
- Nested or accidental paths：`src-tauri/src-tauri/`
