# InsightCAP — 分步遷移指引

> 每個 Phase 只複製該 Phase 需要的模組。
> 用到才複製，不提前複製。
> 舊程式碼位於 `/reference`，原文件保留不動。
> 新架構規格見 `Architecture-v2.md`。

---

## 狀態說明

| 符號 | 說明 |
|------|------|
| 📋 複製 | 從 `/reference` 直接複製，不需修改 |
| 🔧 複製後調整 | 複製後調整接口或依賴，舊邏輯保留 |
| 🆕 全新 | 無對應舊文件，從零編寫 |
| 🔄 重寫 | 參考舊文件邏輯，按新架構重新設計 |

---

## Phase 1：基礎建設

**目標：** Design System、i18n、DB 安全、Schema、Auth、Trait 定義。
完成後可啟動應用、登入、建立空白 DB，無任何業務功能。

### 前端

| 動作 | 路徑 | 說明 |
|------|------|------|
| 🆕 全新 | `src/design-system/tokens.css` | IC Design Token 四主題完整定義 |
| 🆕 全新 | `src/design-system/index.css` | 全域樣式 |
| 🆕 全新 | `src/i18n/index.ts` | react-i18next 初始化 |
| 🆕 全新 | `src/i18n/types.ts` | key 型別定義 |
| 🆕 全新 | `src/i18n/locales/zh-TW.json` | 繁體中文 |
| 🆕 全新 | `src/i18n/locales/zh-CN.json` | 簡體中文 |
| 🆕 全新 | `src/i18n/locales/en.json` | 英文 |
| 🆕 全新 | `src/components/ui/` | 基礎元件庫（Button、Input、Badge、Card 等） |
| 🔧 複製後調整 | `src/stores/themeStore.ts` | 從 `/reference/src/stores/themeStore.ts` 複製，更新為四主題 |
| 🆕 全新 | `src/stores/languageStore.ts` | 語言切換狀態 |
| 🆕 全新 | `src/pages/SetupPage.tsx` | 首次啟動設定流程（跟隨新 Design System） |
| 🆕 全新 | `src/pages/LoginPage.tsx` | 登入介面 |

### 後端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🆕 全新 | — | `src-tauri/src/db/connection.rs` | 健康檢查、bootstrap.json 原子寫入、db_state.json，見架構文件「資料庫安全設計」 |
| 🆕 全新 | — | `src-tauri/migrations/001_init.sql` | 全新 Schema：sources、captures、memory_chunks、spaces、tags、projects、conversations、messages、inbox、settings、conversation_summary_queue |
| 📋 複製 | `src-tauri/src/auth/mod.rs` | `src-tauri/src/auth/mod.rs` | |
| 📋 複製 | `src-tauri/src/auth/key_derivation.rs` | `src-tauri/src/auth/key_derivation.rs` | |
| 📋 複製 | `src-tauri/src/auth/recovery.rs` | `src-tauri/src/auth/recovery.rs` | |
| 📋 複製 | `src-tauri/src/auth/login_guard.rs` | `src-tauri/src/auth/login_guard.rs` | |
| 📋 複製 | `src-tauri/src/commands/auth_commands.rs` | `src-tauri/src/commands/auth_commands.rs` | |
| 📋 複製 | `src-tauri/src/commands/settings_commands.rs` | `src-tauri/src/commands/settings_commands.rs` | |
| 📋 複製 | `src-tauri/src/utils/` | `src-tauri/src/utils/` | 全目錄複製 |
| 🆕 全新 | — | `src-tauri/src/knowledge_source/mod.rs` | KnowledgeSource trait 定義 |
| 🆕 全新 | — | `src-tauri/src/providers/llm/mod.rs` | LLMProvider trait 定義 |
| 🆕 全新 | — | `src-tauri/src/providers/embedding/mod.rs` | Embedder trait 定義 |

### Phase 1 完成標準
- [ ] 可正常啟動、登入、登出
- [ ] 四主題可切換，語言可切換
- [ ] DB 健康檢查通過
- [ ] 修改密碼、更改工作區路徑流程完整（含強制重啟）
- [ ] 空白 DB 建立成功

---

## Phase 2：核心引擎

**目標：** CaptureEngine、TagEngine、SpaceEngine、MemoryEngine、RAGEngine、ConversationEngine。
完成後可擷取內容、進行對話、知識入庫、RAG 召回。

### 前端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🆕 全新 | — | `src/lib/types.ts` | Source、Capture、MemoryChunk 等新型別 |
| 🔧 複製後調整 | `src/lib/tauri.ts` | `src/lib/tauri.ts` | 更新 invoke 函數對應新 Command 名稱 |
| 🔧 複製後調整 | `src/stores/uiStore.ts` | `src/stores/chatStore.ts` | 保留對話 UI 狀態，移除 Space 相關狀態 |
| 🆕 全新 | — | `src/stores/tagStore.ts` | 推薦標籤、選中標籤狀態 |
| 🔧 複製後調整 | `src/stores/knowledgeStore.ts` | `src/stores/knowledgeStore.ts` | 對應新的 Source/Capture 結構 |
| 🔧 複製後調整 | `src/components/chat/MessageList.tsx` | `src/components/chat/MessageList.tsx` | Citation badge 對應新三層記憶視覺語言 |
| 🔄 重寫 | `src/components/chat/InputArea.tsx` | `src/components/chat/InputArea.tsx` | 加入標籤推薦 chip、# 輸入、@ 引用改為 source 層級 |
| 🆕 全新 | — | `src/components/memory/ContextHintBanner.tsx` | 三層記憶提示 banner |
| 🆕 全新 | — | `src/components/memory/CitationBadge.tsx` | 統一引用徽章 |

### 後端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 📋 複製 | `src-tauri/src/capture/clipboard.rs` | `src-tauri/src/capture/clipboard.rs` | |
| 📋 複製 | `src-tauri/src/capture/keyboard.rs` | `src-tauri/src/capture/keyboard.rs` | |
| 📋 複製 | `src-tauri/src/capture/readability.rs` | `src-tauri/src/capture/readability.rs` | |
| 🔧 複製後調整 | `src-tauri/src/embedding/local.rs` | `src-tauri/src/providers/embedding/fastembed.rs` | 封裝為 Embedder trait 實現 |
| 🔧 複製後調整 | `src-tauri/src/models/remote.rs` | `src-tauri/src/providers/llm/openai.rs` | 封裝為 LLMProvider trait 實現 |
| 🔧 複製後調整 | `src-tauri/src/models/config.rs` | `src-tauri/src/providers/llm/mod.rs` | 加入 LLMProvider trait 實現 |
| 📋 複製 | `src-tauri/src/models/vision.rs` | `src-tauri/src/providers/llm/vision.rs` | |
| 🔧 複製後調整 | `src-tauri/src/vector_store/local.rs` | `src-tauri/src/vector_store/local.rs` | 調整為支援 sources 主索引 + 外部 KB 索引 |
| 🔧 複製後調整 | `src-tauri/src/vector_store/multi_index.rs` | `src-tauri/src/vector_store/multi_index.rs` | 調整索引路徑結構 |
| 🔄 重寫 | `src-tauri/src/capture/processor.rs` | `src-tauri/src/background/capture_processor.rs` | 寫入 sources + captures，不是舊的 chunks |
| 🆕 全新 | — | `src-tauri/src/services/tag_engine.rs` | 標籤 CRUD、頻率統計 |
| 🆕 全新 | — | `src-tauri/src/services/space_engine.rs` | AI 聚類管理 |
| 🆕 全新 | — | `src-tauri/src/background/space_recluster.rs` | Space 重新聚類 |
| 🔄 重寫 | `src-tauri/src/knowledge/tagger.rs` | `src-tauri/src/services/memory_engine.rs` | 分陣邏輯，擷取固定 data，對話總結深度推斷 |
| 🔄 重寫 | `src-tauri/src/conversation/extractor.rs` | `src-tauri/src/background/conversation_scheduler.rs` | 對話總結流程重新設計 |
| 🔧 複製後調整 | `src-tauri/src/processor/scheduler.rs` | `src-tauri/src/background/conversation_scheduler.rs` | 隊列機制保留，觸發邏輯調整 |
| 🆕 全新 | — | `src-tauri/src/services/rag_engine.rs` | 分層 context 召回，調用 KnowledgeSource trait |
| 🆕 全新 | — | `src-tauri/src/knowledge_source/personal.rs` | 個人版 KnowledgeSource 實現 |
| 🔄 重寫 | `src-tauri/src/commands/knowledge_commands.rs` | `src-tauri/src/commands/knowledge_commands.rs` | 對應新 sources/captures/memory_chunks |
| 🔄 重寫 | `src-tauri/src/commands/llm_commands.rs` | `src-tauri/src/commands/rag_commands.rs` | RAG 部分拆出，分層 context 組裝 |
| 🔧 複製後調整 | `src-tauri/src/commands/llm_commands.rs` | `src-tauri/src/commands/conversation_commands.rs` | 對話管理部分保留，RAG 部分已拆出 |
| 🆕 全新 | — | `src-tauri/src/commands/tag_commands.rs` | 標籤 IPC 接口 |
| 🔄 重寫 | `src-tauri/skills/tagger_skill.md` | `src-tauri/skills/tagger_skill.md` | 分陣邏輯兩套 prompt |
| 📋 複製 | `src-tauri/skills/tagger_conversation_skill.md` | `src-tauri/skills/tagger_conversation_skill.md` | |

### Phase 2 完成標準
- [ ] 熱鍵擷取可正常入庫（寫入 sources + captures）
- [ ] 對話可進行，RAG 可召回相關內容
- [ ] 對話總結可自動生成並寫入 memory_chunks
- [ ] ContextHintBanner 可正確顯示三層記憶提示
- [ ] 標籤自動生成，Space 自動聚類

---

## Phase 3：記憶理論完整落地

**目標：** PatternEngine、用戶確認流程、文本編輯器。
完成後路徑 B 跨對話識別運作，編輯器可匯出入庫。

### 前端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🆕 全新 | — | `src/components/editor/` | Tiptap 編輯器，跟隨 Design System，含匯出入庫邏輯 |
| 🔧 複製後調整 | `src/components/chat/ChatView.tsx` | `src/components/chat/ChatView.tsx` | 加入編輯器雙窗佈局 |

### 後端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🆕 全新 | — | `src-tauri/src/services/pattern_engine.rs` | 路徑 B 跨對話識別 |
| 🆕 全新 | — | `src-tauri/src/background/pattern_promotion.rs` | Pattern 升格 background task |
| 📋 複製 | `src-tauri/skills/pattern_extractor_skill.md` | `src-tauri/skills/pattern_extractor_skill.md` | |
| 📋 複製 | `src-tauri/skills/summarizer_skill.md` | `src-tauri/skills/summarizer_skill.md` | |
| 📋 複製 | `src-tauri/skills/follow_up_skill.md` | `src-tauri/skills/follow_up_skill.md` | |
| 📋 複製 | `src-tauri/skills/capture_title_skill.md` | `src-tauri/skills/capture_title_skill.md` | |
| 📋 複製 | `src-tauri/skills/daily_digest_skill.md` | `src-tauri/skills/daily_digest_skill.md` | |
| 📋 複製 | `src-tauri/skills/space_brief_skill.md` | `src-tauri/skills/space_brief_skill.md` | |
| 📋 複製 | `src-tauri/src/background/cloud_sync_watcher.rs` | `src-tauri/src/background/cloud_sync_watcher.rs` | |
| 📋 複製 | `src-tauri/src/processor/ocr_worker.rs` | `src-tauri/src/background/ocr_worker.rs` | |

### Phase 3 完成標準
- [ ] 路徑 B 可偵測重複 pattern 並推送升格建議
- [ ] pending_confirm 確認流程正常運作
- [ ] 編輯器可正常編輯，匯出後自動入庫（按標題分段，覆蓋更新）
- [ ] OCR、雲端同步偵測正常運作

---

## Phase 4：知識庫頁 UX 重設計

**目標：** 以 sources 為單位的知識庫介面、Space 篩選、@ 引用、標籤推薦。
完成後用戶可瀏覽原文件，不再看到 chunk。

### 前端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🔄 重寫 | `src/pages/KnowledgePage.tsx` | `src/pages/KnowledgePage.tsx` | 改為以 sources 為單位顯示 |
| 🔄 重寫 | `src/components/knowledge/ChunkList.tsx` | `src/components/knowledge/SourceList.tsx` | 顯示原文件，可展開看片段 |
| 🆕 全新 | — | `src/components/knowledge/SpaceFilter.tsx` | Space 篩選側邊欄 |
| 🆕 全新 | — | `src/components/knowledge/SourceCard.tsx` | 來源卡片（文件/網址/圖片） |

### Phase 4 完成標準
- [ ] 知識庫頁以原文件為單位顯示
- [ ] Space 篩選可正常使用
- [ ] @ 引用顯示原文件名稱，不顯示 chunk
- [ ] 標籤推薦在對話輸入框出現，# 輸入正常運作

---

## Phase 5：商業版

**目標：** KnowledgeSource 商業版實現、外部 KB 加載、Knowledge Builder。

### 後端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🔧 複製後調整 | `src-tauri/src/knowledge/external_kb.rs` | `src-tauri/src/knowledge_source/enterprise.rs` | 整合進 EnterpriseKnowledgeSource，調整為新接口 |
| 🆕 全新 | — | `src-tauri/migrations/002_enterprise.sql` | external_knowledge_bases 表 |

### Phase 5 完成標準
- [ ] 外部 KB 可加載，相容性檢查通過
- [ ] 商業版 RAG 可同時召回本地和外部 KB 內容
- [ ] Knowledge Builder 可輸出標準格式知識庫

---

## Phase 6：手機版

**目標：** 本地 HTTP API、React Native 應用。

### 後端

| 動作 | 來源（/reference/） | 新路徑 | 說明 |
|------|---------------------|--------|------|
| 🆕 全新 | — | `src-tauri/src/sync/server.rs` | axum HTTP API（port 7431） |
| 🆕 全新 | — | `src-tauri/src/sync/mdns.rs` | mDNS 廣播 |
| 🆕 全新 | — | `src-tauri/src/sync/pairing.rs` | QR Code 配對 |

### Phase 6 完成標準
- [ ] 桌面版 HTTP API 可在區域網路存取
- [ ] 手機版可擷取並同步至桌面版

---

## 複製時的通用調整規則

每次複製舊模組後，檢查以下四點：

**1. 資料表引用**
所有引用 `chunks` 表的 SQL 查詢，按以下規則替換：
- 用戶擷取內容 → `sources` + `captures`
- 對話記憶 → `memory_chunks`
- `conversation_folders` → `projects`

**2. Embedding 調用**
所有直接調用 `fastembed` 的地方，改為通過 `Embedder` trait：
```rust
// 舊
embedding::local::embed(text).await
// 新
self.embedder.embed(text).await
```

**3. LLM 調用**
所有直接調用 Ollama/OpenAI 的地方，改為通過 `LLMProvider` trait：
```rust
// 舊
models::generate_with_content_llm(&state, &prompt).await
// 新
self.llm_provider.complete_json(&prompt, options).await
```

**4. UI 文字和樣式**
- 寫死的顏色 → IC Design Token（`var(--ic-*)`）
- 寫死的介面文字 → `t('key')`
- `neutral-*`、`emerald-*` 等 Tailwind 顏色 → IC Token 對應值

---

*版本：v1.0 | 日期：2026-03-27*
*對應架構文件：Architecture-v2.md v2.1*
