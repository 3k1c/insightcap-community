# InsightCAP 目錄結構圖

```text
InsightCAP/
├─ .cargo/
├─ .git/
├─ .vscode/
├─ dist/
│  └─ assets/
├─ node_modules/
├─ scripts/
│  ├─ cleanup-legacy-url-chunks.py
│  └─ knowledge_builder/
│     └─ build_kb.py
├─ src/
│  ├─ App.tsx
│  ├─ main.tsx
│  ├─ setupTests.ts
│  ├─ vite-env.d.ts
│  ├─ components/
│  │  ├─ chat/
│  │  │  ├─ EditorPane.tsx
│  │  │  ├─ InputArea.tsx
│  │  │  ├─ MessageList.tsx
│  │  │  └─ extensions/
│  │  │     ├─ ImageNodePro.tsx
│  │  │     └─ ImageNodeView.tsx
│  │  ├─ knowledge/
│  │  │  └─ SpaceInsightPanel.tsx
│  │  ├─ layout/
│  │  │  └─ MainLayout.tsx
│  │  ├─ memory/
│  │  │  ├─ CitationBadge.tsx
│  │  │  ├─ ContextHintBanner.tsx
│  │  │  ├─ DecisionToast.tsx
│  │  │  ├─ PendingConfirmDrawer.tsx
│  │  │  └─ ReminderToast.tsx
│  │  └─ ui/
│  │     ├─ Badge.tsx
│  │     ├─ Button.tsx
│  │     ├─ Card.tsx
│  │     ├─ Dialog.tsx
│  │     └─ Input.tsx
│  ├─ design-system/
│  │  ├─ editor.css
│  │  ├─ index.css
│  │  └─ tokens.css
│  ├─ hooks/
│  │  └─ useT.ts
│  ├─ i18n/
│  │  ├─ index.ts
│  │  ├─ types.ts
│  │  └─ locales/
│  │     ├─ en.json
│  │     ├─ zh-CN.json
│  │     └─ zh-TW.json
│  ├─ lib/
│  │  ├─ noteStore.ts
│  │  ├─ tauri.ts
│  │  ├─ types.ts
│  │  ├─ utils.test.ts
│  │  └─ utils.ts
│  ├─ pages/
│  │  ├─ ChatPage.tsx
│  │  ├─ LoginPage.tsx
│  │  ├─ MigratePage.tsx
│  │  ├─ QuickCapturePage.tsx
│  │  ├─ RepositoryPage.tsx
│  │  ├─ SchedulePage.tsx
│  │  ├─ SettingsPage.tsx
│  │  └─ SetupPage.tsx
│  └─ stores/
│     ├─ chatStore.ts
│     ├─ knowledgeStore.ts
│     ├─ languageStore.ts
│     ├─ tagStore.ts
│     ├─ themeStore.ts
│     └─ uiStore.ts
├─ src-tauri/
│  ├─ .cargo/
│  │  └─ .cargo/
│  ├─ .fastembed_cache/
│  ├─ capabilities/
│  ├─ gen/
│  │  └─ schemas/
│  ├─ icons/
│  ├─ migrations/
│  ├─ notes/
│  ├─ resources/
│  ├─ skills/
│  ├─ src/
│  │  ├─ error.rs
│  │  ├─ http_server.rs
│  │  ├─ lib.rs
│  │  ├─ main.rs
│  │  ├─ prompts.rs
│  │  ├─ tray_status.rs
│  │  ├─ whisper_transcribe.rs
│  │  ├─ auth/
│  │  │  ├─ key_derivation.rs
│  │  │  ├─ login_guard.rs
│  │  │  ├─ mod.rs
│  │  │  └─ recovery.rs
│  │  ├─ background/
│  │  │  ├─ capture_processor.rs
│  │  │  ├─ cloud_sync_watcher.rs
│  │  │  ├─ conversation_scheduler.rs
│  │  │  ├─ deep_synthesis_engine.rs
│  │  │  ├─ mod.rs
│  │  │  ├─ ocr_worker.rs
│  │  │  ├─ pattern_promotion.rs
│  │  │  ├─ reminder_scheduler.rs
│  │  │  ├─ space_recluster.rs
│  │  │  └─ telegram_bot.rs
│  │  ├─ bin/
│  │  │  ├─ debug_schema.rs
│  │  │  ├─ fix_spaces.rs
│  │  │  ├─ memory_evolution.rs
│  │  │  ├─ reminder_batch.rs
│  │  │  ├─ reminder_stress.rs
│  │  │  └─ space_evolution.rs
│  │  ├─ capture/
│  │  │  ├─ attachment_manager.rs
│  │  │  ├─ chunking.rs
│  │  │  ├─ clipboard.rs
│  │  │  ├─ encoding.rs
│  │  │  ├─ file_parser.rs
│  │  │  ├─ keyboard.rs
│  │  │  ├─ metadata.rs
│  │  │  ├─ mod.rs
│  │  │  ├─ readability.rs
│  │  │  ├─ source_group.rs
│  │  │  ├─ video_parser.rs
│  │  │  └─ extractors/
│  │  │     ├─ code.rs
│  │  │     ├─ csv.rs
│  │  │     ├─ docx.rs
│  │  │     ├─ epub.rs
│  │  │     ├─ html.rs
│  │  │     ├─ md.rs
│  │  │     ├─ mod.rs
│  │  │     ├─ pdf.rs
│  │  │     ├─ pptx.rs
│  │  │     ├─ preclean.rs
│  │  │     ├─ rtf.rs
│  │  │     ├─ txt.rs
│  │  │     └─ xlsx.rs
│  │  ├─ commands/
│  │  │  ├─ auth_commands.rs
│  │  │  ├─ bilibili_auth.rs
│  │  │  ├─ capture_commands.rs
│  │  │  ├─ chunk_relation_commands.rs
│  │  │  ├─ conversation_commands.rs
│  │  │  ├─ decision_commands.rs
│  │  │  ├─ editor_commands.rs
│  │  │  ├─ knowledge_commands.rs
│  │  │  ├─ memory_commands.rs
│  │  │  ├─ mod.rs
│  │  │  ├─ project_commands.rs
│  │  │  ├─ rag_commands.rs
│  │  │  ├─ reminder_commands.rs
│  │  │  ├─ settings_commands.rs
│  │  │  ├─ space_commands.rs
│  │  │  └─ tag_commands.rs
│  │  ├─ db/
│  │  │  ├─ connection.rs
│  │  │  └─ mod.rs
│  │  ├─ knowledge_source/
│  │  │  ├─ enterprise.rs
│  │  │  ├─ mod.rs
│  │  │  └─ personal.rs
│  │  ├─ ocr/
│  │  │  ├─ macos.rs
│  │  │  ├─ mod.rs
│  │  │  ├─ postprocess.rs
│  │  │  ├─ preprocess.rs
│  │  │  └─ windows.rs
│  │  ├─ providers/
│  │  │  ├─ mod.rs
│  │  │  ├─ embedding/
│  │  │  │  ├─ fastembed.rs
│  │  │  │  └─ mod.rs
│  │  │  └─ llm/
│  │  │     ├─ mod.rs
│  │  │     ├─ model_caps.rs
│  │  │     ├─ openai.rs
│  │  │     └─ vision.rs
│  │  ├─ services/
│  │  │  ├─ chunk_relation_engine.rs
│  │  │  ├─ language_normalizer.rs
│  │  │  ├─ memory_engine.rs
│  │  │  ├─ mod.rs
│  │  │  ├─ pattern_engine.rs
│  │  │  ├─ rag_engine.rs
│  │  │  ├─ reminder_engine.rs
│  │  │  ├─ space_engine.rs
│  │  │  ├─ space_knowledge_guide_engine.rs
│  │  │  ├─ tag_engine.rs
│  │  │  └─ web_search.rs
│  │  ├─ settings/
│  │  │  ├─ mod.rs
│  │  │  ├─ security.rs
│  │  │  └─ store.rs
│  │  ├─ utils/
│  │  │  ├─ mod.rs
│  │  │  ├─ skills.rs
│  │  │  └─ title_cleaner.rs
│  │  └─ vector_store/
│  │     ├─ local.rs
│  │     ├─ mod.rs
│  │     └─ multi_index.rs
│  ├─ src-tauri/
│  │  └─ src/
│  │     └─ bin/
│  ├─ target/
│  └─ tests/
├─ .gitignore
├─ .git_commit_msg
├─ changelog.md
├─ components.json
├─ index.html
├─ package-lock.json
├─ package.json
├─ postcss.config.js
├─ tailwind.config.js
├─ tsconfig.json
├─ tsconfig.node.json
└─ vite.config.ts
```

