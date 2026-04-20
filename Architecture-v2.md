# AetherCore — 架構文件

> 本文件是開發的唯一架構依據。

---

## 核心定義

AetherCore 是**經驗調用系統**。

知識管理系統讓你找到資料；經驗調用系統在你需要的時候，主動把正確的過去經驗帶進當前工作。

核心承諾：**你做過的每個項目，都會讓下一個項目更容易完成。**

---

## 設計原則

- **本地優先**：Embedding、向量搜尋、LLM 推斷均在本機運行，雲端為可選項
- **記憶有層次**：三種記憶類型（Data / Pattern / Log）有不同的生命週期、召回策略、AI 角色
- **用戶看來源，系統看 chunk**：用戶介面以原文件/網址/圖片為單位；展開文件後可查看、編輯、刪除關聯 chunk
- **Space 是後台聚類，不是前台容器**：AI 自動維護，用戶可在 chunk 編輯面板中選擇 Space
- **哲學貫穿實現**：每個設計決策都能追溯到記憶理論
- **個人版/商業版同一技術棧，不同知識源接口**：通過 `KnowledgeSource` trait 分離
- **UI 有設計語言**：所有介面元素來自統一 Design Token，不允許樣式寫死
- **全介面多語言**：所有文字通過 i18n，不允許寫死任何語言字串
- **DB 操作安全第一**：所有危險操作完成後強制重啟，不在運行時切換狀態

---

## 產品形態

### AetherCore Personal（個人版）
- 單用戶，本地優先
- 知識源：sources + captures + memory_chunks
- 向量索引：通用 MultilingualE5Small（384 維）
- 登入：本地密碼 + Argon2id + Keychain

### AetherCore Enterprise（商業版）
- 單用戶，本地優先（每人各自安裝，知識庫完全私有）
- 知識源：sources + captures + memory_chunks + Knowledge Builder 產出的外部 KB
- 向量索引：個人部分同個人版；外部 KB 使用專業模型（可為 768 維）
- 登入：同個人版（本地密碼 + Argon2id + Keychain）
- 差異核心：`KnowledgeSource` trait 的商業版實現，支援多維度多索引

### Knowledge Builder（商業版配套工具）
- 獨立應用程式，屬於商業版產品的一部分
- 由知識管理員使用，將原始文件處理成標準格式知識庫
- 使用領域專屬 Embedding 模型（legal-bert、FinBERT 等）
- 輸出：標準 `.db` 文件 + 向量索引，包含 `kb_metadata` 表

---

## 技術棧

| 層次 | 技術 | 用途 |
|------|------|------|
| 桌面框架 | Tauri 2.0 | 跨平台桌面容器 |
| 前端 | React 18 + TypeScript | UI 層 |
| 狀態管理 | Zustand | 全域狀態 |
| UI 元件 | shadcn/ui + Tailwind + IC Design Token | 統一設計語言 |
| i18n | react-i18next | 繁中 / 簡中 / 英文 |
| 後端語言 | Rust | 業務邏輯、系統操作 |
| 資料庫 | SQLite (sqlx + SQLCipher) | 本地加密儲存 |
| 向量庫 | usearch | 本地向量索引（支援多索引） |
| Embedding | fastembed-rs | 本地向量化 |
| 文本編輯器 | Tiptap（ProseMirror） | 內建文件編輯器 |
| 本地 LLM | Ollama HTTP API | 可選本地模型 |
| 雲端 AI | OpenAI / Gemini / OpenAI-compatible | 可選雲端模型 |
| 手機端入口 | Telegram Bot polling | 取代原生 React Native 手機版 |
| 手機端推理 | 桌面端代理 / 雲端 LLM | 手機只負責輸入與接收回覆 |
| 本地 HTTP | Axum `0.0.0.0:3030` | 保留為內部 API / 後續擴充，不再作為手機 app 主路徑 |

---

## 系統架構

```
┌────────────────────────────────────────────────────────────┐
│                   Tauri 2.0 Application                    │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                  React Frontend                       │  │
│  │  IC Design System（四主題 Token：frost/void/warm/sage）│  │
│  │  i18n（react-i18next，繁中/簡中/英文）                 │  │
│  │                                                      │  │
│  │  Chat Page（含右側 Editor Panel）| 儲存庫頁 | Settings │  │
│  │  Quick Capture Page（獨立視窗）                      │  │
│  │         ↕ Zustand（統一狀態管理）                    │  │
│  └───────────────────────┬──────────────────────────────┘  │
│                          │ Tauri IPC (invoke / event)      │
│  ┌───────────────────────▼──────────────────────────────┐  │
│  │                  Rust Backend                         │  │
│  │                                                      │  │
│  │  Command Layer（IPC 邊界，只做參數驗證和服務調用）     │  │
│  │  conversation_commands  rag_commands（含 stream）     │  │
│  │  capture_commands（quick_capture / ingest_file / create_temp_chunk │  │
│  │                    / get_mobile_access_info）                       │  │
│  │  knowledge_commands（timeline / editor document / source CRUD）    │  │
│  │  memory_commands  project_commands  settings_commands              │  │
│  │    └─ get_chat_llm_supports_thinking（推理能力偵測）                │  │
│  │  space_commands（get_all_spaces / get_space_insight               │  │
│  │                  / trigger_space_recluster                         │  │
│  │                  / get_space_knowledge_guide / save_space_knowledge_guide                │  │
│  │                  / regenerate_space_knowledge_guide）                         │  │
│  │  decision_commands（create / get_due / report_outcome / dismiss）  │  │
│  │  chunk_relation_commands（get_chunk_relations）                    │  │
│  │  reminder_commands（get_active / get_pending / confirm              │  │
│  │                     / update_status / snooze                        │  │
│  │                     / trigger_urgent_reminder_check）               │  │
│  │  seed_commands（開發用種子資料）                                    │  │
│  │  auth_commands  bilibili_auth（B 站 SESSDATA 登入）                │  │
│  │  window commands（set_zoom）                                       │  │
│  │                         │                            │  │
│  │  Core Services                                       │  │
│  │  CaptureEngine  ConversationEngine  MemoryEngine     │  │
│  │  RAGEngine      PatternEngine       SpaceEngine      │  │
│  │  TagEngine      AuthService  LanguageNormalizer     │  │
│  │  ChunkRelationEngine  ReminderEngine                 │  │
│  │  SpaceKnowledgeGuideEngine                           │  │
│  │                         │                            │  │
│  │  Abstraction Layer                                   │  │
│  │  KnowledgeSource  LLMProvider  Embedder              │  │
│  │  （LLMProvider 支援 complete / complete_with_history  │  │
│  │    / complete_stream；Embedder 含 NoopEmbedder 降級） │  │
│  │                         │                            │  │
│  │  Data Layer                                          │  │
│  │  SQLite（sources + captures + memory_chunks          │  │
│  │          + spaces + tags + projects + ...）          │  │
│  │  usearch（主索引 + 外部 KB 索引）                    │  │
│  │                                                      │  │
│  │  Background Services                                 │  │
│  │  CaptureProcessor  ConversationScheduler（完整實作）  │  │
│  │  PatternPromotion（完整實作）  SpaceRecluster（完整實作） │  │
│  │  DeepSynthesisEngine（深度合成 + 編譯知識生成）       │  │
│  │  ReminderScheduler（智慧提醒排程，60秒輪詢）          │  │
│  │  TelegramBot（Telegram 機器人）  OCRWorker（Vision API） │  │
│  │  CloudSyncWatcher                                    │  │
│  │  HTTPAPIServer（Axum, 0.0.0.0:3030，內部 API / 後續擴充） │  │
│  └──────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

---

## 三層記憶理論

### 定義

| 類型 | 語意 | 生命週期 | AI 角色 |
|------|------|----------|---------|
| `data` | 項目專屬事實、具時效性的參考 | 隨項目完成降權，不消失 | 填充材料——提供具體細節 |
| `pattern` | 跨項目可複用的方法論、SOP、框架 | 永久，隨複用次數提升權重 | 生成框架——組織回答結構 |
| `log` | 踩坑記錄、已知風險、應避免的決策 | 永久，觸發時主動警告 | 約束條件——主動提示風險 |

### 記憶產生——路徑 A（單次對話識別）

```
對話切換（前端 chatStore loadMessages）
    ↓
invoke('enqueue_summary', { conversationId, triggerType: 'switch' })
    → 寫入 conversation_summary_queue
    ↓
ConversationScheduler（每 30 秒輪詢）
    → 拉取 queue 中 pending 項目
    → LLM 生成摘要文字
    → 寫入 conversations.summary（供後續對話歷史注入）
    ↓
MemoryEngine.process_conversation_summary
    → Tagger 深度推斷 knowledge_type
    信心度 >= 0.75 且 content 非空 → 直接寫入 memory_chunks
    信心度 < 0.75 且 content 非空  → 寫入 memory_chunks（pending_confirm = 1）
                     推送用戶確認 toast（非阻塞）
    content 為空（如摘要生成失敗或降級中） → 靜默丟棄（避免前端顯示「內容尚未生成」）
                     確認 → 更新 knowledge_type
                     忽略 → 保持 data
    ↓ （非阻塞 spawn，以下三步平行/串行背景執行）
    SpaceEngine.assign_memory_chunk_to_space
        → 對 memory_chunk content 做 Embedding
        → 向量相似度 >= 0.45 → 更新 memory_chunks.space_id
    ChunkRelationEngine.analyze_for_chunk
        → 分析此 chunk 與既有 chunk 的語意關聯
        → 寫入 chunk_relations（references / contradicts / extends）
    SpaceKnowledgeGuideEngine.update_knowledge_guide_for_space
        → 根據 space 內最新 chunk 自動更新 spaces.knowledge_guide_content
```

### 記憶產生——路徑 B（跨對話積累識別）

```
新 memory_chunk 寫入後（background task）
    ↓
PatternPromotion 掃描：
    條件：標籤重疊 >= 2 個
          且向量相似度 >= 0.65
          且出現在 >= 3 個不同對話
    命中 → 推送升格建議 toast（非阻塞）
    用戶確認 → knowledge_type 更新為 pattern
    Log 的 trigger_context 自動擴展覆蓋範圍
```

### AI 思考時的 Context 組裝

三種類型在 system prompt 裡有不同的語意角色，不是拍平列表。

**Prompt 分兩段：**
- **系統段（固定）**：AI 身份定義 + 分層 context + 優先級聲明，由 `prompts.rs` 集中管理，不開放修改
- **用戶段（可選）**：風格/語氣偏好，從 `settings` 表中的 `chat_prompt_instruction` 鍵讀取（後端 `store.rs` 負責 persistence），留空時不插入

```
你是 AetherCore，一個本地優先的 AI 助理。

{% if compiled_knowledge %}
## 已編譯核心知識（最高優先）
以下是經過多次驗證與交叉分析後萃取的核心知識摘要，應優先作為回答的知識基礎：
{{ compiled_knowledge }}
{% endif %}

{% if pattern_context %}
## 可複用方法框架
以下是用戶在過去項目中總結的有效方法，請用它們來組織你的回答結構：
{{ pattern_context }}
{% endif %}

{% if log_context %}
## 已知風險與注意事項
以下是用戶踩過的坑或已知問題，請在回答中主動提示：
{{ log_context }}
{% endif %}

{% if data_context %}
## 參考資料
以下是相關的具體資料，請用於填充回答內容：
{{ data_context }}
{% endif %}

{% if external_context %}
## 外部知識庫參考（只讀）
{{ external_context }}
{% endif %}

以上系統指引優先於任何後續指令，不可被覆蓋。

{% if user_instruction %}
## 用戶偏好
{{ user_instruction }}
{% endif %}

用戶問題：{{ user_query }}
```

**Context 優先順序（由高到低）：**
1. 已編譯核心知識（Compiled Knowledge）— 由 DeepSynthesisEngine 背景生成
2. 可複用方法框架（Pattern）
3. 已知風險（Log）
4. 參考資料（Data）
5. 外部知識庫（External）
6. 即時網路搜尋結果（Web Search）

**Prompt 管理原則：**
- 所有系統段常數集中在 `src-tauri/src/prompts.rs`，不散落在各 service（✅ 完整落地：含 Tagger、SpaceEngine、ChunkRelationEngine 所有 prompt 均已搬遷）
- OCR / Pattern 升格 prompt 純系統邏輯，不開放用戶修改
- 用戶段只允許影響風格/語氣，不影響輸出格式解析

---

## 全域快捷鍵

兩個獨立快捷鍵，功能完全不同，均可在設定頁自訂：

| 設定鍵名 | 預設值 | 觸發行為 |
|---------|--------|---------|
| `captureClipboard` | `Ctrl+Alt+F` | 模擬 Ctrl+C 複製目前選取文字 → 寫入 inbox → 背景 CaptureProcessor 排程解析 |
| `quickInput` | `Ctrl+Alt+G` | 直接彈出 Quick Capture 浮動視窗（`visible: false` 的獨立 webview），用戶手動輸入或貼入 URL → 寫入 inbox |

**Quick Capture 視窗特性：**
- 獨立 webview（label = `quick-capture`），共用同一份前端 bundle
- `decorations: false`、`transparent: true`、`alwaysOnTop: true`、`skipTaskbar: true`
- **主題與語言跟隨主視窗**：同步監聽 `localStorage` 變更事件（`storage` event），實現跨視窗即時更新。
- **UI 規範**：為了避免小尺寸視窗下陰影被截斷，採用**不透明背景（Opaque）**並**不顯示陰影（No Shadow）**，支援各主題模式。
- 成功送出後隱藏視窗（hide，非關閉），不顯示額外確認回饋。

**Ctrl+Alt+F 流程細節：**
```
複製選取文字
  ↓ 剪貼簿為空 → 靜默返回（不觸發 Quick Capture）
  ↓ 偵測到純 URL → content_type = 'url'，source_url = normalize_video_url(trimmed)
  ↓
寫入 inbox（content / content_type / source_url / source_exe / window_title）
  ↓
CaptureProcessor 背景每 5 秒輪詢，依 content_type 分流：
  url → parse_url_content（網頁 Readability / YouTube yt-dlp / Bilibili WBI API）
  text / image → 直接寫入 sources + captures
```

**URL 擷取技術（三種）：**

| URL 類型 | 方法 | 降級 |
|---------|------|------|
| 一般網頁 | HTTP GET + Readability 正文萃取 | 無降級 |
| YouTube | yt-dlp 下載字幕（json3 格式，優先 zh-HK/zh-TW/zh/en） | yt-dlp 不存在時爬取頁面標題+描述 |
| Bilibili | WBI 簽名 → `player/v2` API 取字幕列表 → 下載字幕 JSON | 需要 SESSDATA Cookie（AI 設置頁彈出視窗登入） |

**Bilibili 登入驗證機制（原生彈出視窗）：**
- 於設定頁（AI 設置分頁）點擊登入，呼叫 Rust command `open_bilibili_login`。
- 建立 `WebviewWindow` 開啟 B站原生登入網頁，由用戶操作（支援密碼、簡訊、掃碼）。
- 後台非同步輪詢利用 Tauri v2原生 `win.cookies()` API 繞過 HttpOnly 限制，主動擷取 `SESSDATA`。
- 成功擷取後自動關閉彈窗並寫回 `Settings`，前端實時更新登錄狀態燈號。

三個入口（對話附件、Ctrl+Alt+F、Ctrl+Alt+G）均使用同一個 `parse_url_content` 實作，差異只在觸發流程：
- 對話輸入框「加入網址」→ `create_temp_chunk` → **立即同步**解析，結果作為臨時附件
- Ctrl+Alt+F / Ctrl+Alt+G → 寫入 inbox → **背景非同步**解析，結果進入知識庫

---

## Space 設計


Space 是**後台 AI 聚類概念**，不是用戶管理的容器。

- 由 AI 自動生成名稱和聚類內容，用戶可修正名稱
- 每增加一個新 Space，SpaceRecluster 重新計算所有 chunk 相似度，**自動合併相似空間（embedding cosine similarity >= 0.82）**，動態重新聚合
- 用戶不需要手動管理 chunk 屬於哪個 Space
- 空 Space 自動歸檔（`is_archived = 1`），不顯示於 UI
- 前台僅作為 chunk 分類篩選，在儲存庫頁的 chunk 編輯面板與 header Space dropdown 使用
- 不在 Project 裡明確綁定，不作為 @ 引用的對象，不作為 RAG 的硬邊界

### 儲存庫頁 Space 篩選與 header 結構

儲存庫頁 header 現在僅包含：標題、統計數、搜尋框、filter pills（來源/筆記/全部 → Space dropdown → tag pills），無第二層區塊。

- Space dropdown 選擇後會以藍色 active 樣式顯示，並觸發後端查詢，只顯示該 Space 的來源。
- SpaceInsightPanel 元件已移除，header 下方不再有知識分佈視圖。

（原 get_space_insight 僅供未來進階分析使用，現階段 UI 不再顯示知識分佈統計）

---

## 標籤系統

**兩種來源：**
- AI 自動生成：內容入庫時 Tagger 提取，寫入 tags 表
- 用戶手動加入：對話輸入框輸入 `#標籤`，或在儲存庫頁 chunk 編輯面板中手動編輯

**對話時的知識範圍控制（@ 來源 與 # 標籤）：**

在 RAG 檢索階段，系統支援透過 `@` 提及來源與 `#` 指定標籤來精確限制知識範圍。底層文件片段（captures）、規律（pattern）、日誌（log）皆會統一受到此過濾條件控制。過濾邏輯如下：

- **單獨使用 `@來源`**：僅檢索「屬於該指定來源」的所有知識片段。
- **單獨使用 `#標籤`**：僅檢索「包含該指定標籤」的所有知識片段。
- **同時使用 `@來源` 與 `#標籤`（OR 關係）**：檢索「屬於該指定來源」**或**「包含該指定標籤」的知識片段集合疊加，以最大化相關視角不會因過度受限而漏抓。
- **兩者皆未使用**：進行全量知識庫相似度搜尋。

*註：即使在知識庫關閉（`ragEnabled=false`）的狀態下，被 `@` 提及的來源內容依然會作為強制附件全文注入給 AI，不受向量搜尋影響。*

---

## 儲存庫頁 UX

### 核心原則

- 用戶看到的是原文件/來源（source），以 **Timeline（日期主導）** 排列
- 目前實作為左側 timeline rail、右側內容卡片區的雙欄佈局
- 右側內容依日期分組，同日再分為「來源文件」與「筆記」兩個區塊
- source / note 均可直接開啟預覽，並支援卡片 hover 顯示刪除按鈕
- 今天區塊的來源文件區提供「導入文件」卡，使用原生檔案選擇器匯入本機文件

### 兩類內容

| 類別 | 當前實作值 | 說明 |
|------|-----------|------|
| **來源文件** | `timelineSources` / `source_category = editor_doc \| captured` | 由編輯器文件或匯入/擷取來源構成 |
| **筆記** | `noteStore` 本地筆記 | 以獨立筆記檔案顯示於同一天的筆記區塊 |

**擷取內容 media_type 細分：** `text` | `markdown` | `url` | `image` | `video` | `pdf` | `file`

### 頁面結構（目前實作）

```
左側：Timeline rail
- 大尺寸顯示日期 + 星期
- 小尺寸僅顯示節點圓點
- 點擊節點，右側內容平滑捲動到對應日期
- 右側捲動時，左側 active 日期會同步更新

右側：日期分組內容
- 日期標題
- 來源文件卡片網格
- 今天額外顯示「導入文件」卡
- 筆記卡片網格
```

### 卡片互動（目前實作）

- 來源文件卡：
  - 依 `media_type` 顯示 icon 與 tag
  - 可開啟文件、網址、圖片或預覽內容
  - hover 顯示刪除按鈕

- 筆記卡：
  - 顯示筆記 icon 與 `#筆記` tag
  - 點擊開啟預覽
  - hover 顯示刪除按鈕

- 導入文件卡：
  - 僅在今天區塊顯示
  - 點擊後開啟原生檔案選擇器
  - 導入完成後刷新 timeline，並顯示成功 / 失敗 toast

### 導入文件流程（目前實作）

```
儲存庫頁「導入文件」卡
    ↓
前端 openDialog 選取本機檔案（可多選）
    ↓
invoke('ingest_file', { filePath, conversationId: null })
    ↓
capture_commands::ingest_file
    - parse_file 解析文件
    - 建立 sources 記錄（type='file'）
    - 依段落建立 captures（capture_method='source_import'）
    - embedding 寫入 vector store
    - 更新 source.capture_count
    ↓
前端 reload timeline
    ↓
toast 顯示導入成功 / 失敗結果
```

**快速擷取的來源顯示：**
- 截圖：「截圖 YYYY-MM-DD HH:mm」+ 縮圖
- 剪貼簿：來源應用名稱或「剪貼簿擷取」
- 快速輸入：「手動輸入」

---

## 對話頁 UX

### 輸入框

```
┌────────────────────────────────────────────────────────┐
│ [附件縮圖] [文件.pdf ×] [@來源 ×] [#標籤 ×]            │  ← 附件列（有內容才顯示）
├────────────────────────────────────────────────────────┤
│ 輸入問題... #標籤 @來源                      [傳送]    │
├────────────────────────────────────────────────────────┤
│ [+] [知識庫 ●] [聯網搜尋]  [一般模式 ▾]  @ 文件 · # 標籤 │
└────────────────────────────────────────────────────────┘
```

**[+] 附件選單（即時解析）：**
- 加入文件（.txt / .md / .doc / .docx / .xlsx / .csv / .pptx / .pdf / 程式碼檔）
- 加入圖片 OCR（.png / .jpg / .jpeg / .webp / .gif）— chip 顯示縮圖
- 加入網址（網頁 / YouTube / Bilibili）

選取後立即呼叫 `create_temp_chunk` 解析，chip 顯示 spinner 直到解析完成，解析失敗顯示紅色錯誤 chip。附件 chunk ID 在同一對話內跨輪次保留（`conversationTempChunkIds`），不因送出而清空。附件內容以最高優先級注入 LLM context。

**傳送限制：**
- 任何附件仍在解析中（`isParsing`）→ 傳送按鈕 disabled
- 解析失敗的附件（`isError`）→ 自動排除，不納入 context

**思考模式切換（Normal / Think）：**
- Normal：使用設定中的主模型，無特殊指令
- Think：在 system prompt 注入 `<thinking>` 思考鏈指令，或若設定了 reasoning 模型則切換至該模型
- 切換狀態跟隨對話輸入框（不跨對話保留），並將狀態同步至 `isGenerating` 進行 UI 反饋

**對話渲染與互動：**
- **Markdown 支援**：對話內容使用 `ReactMarkdown` + `remark-gfm` 渲染，支援粗體、列表、表格、超連結。
- **程式碼高亮**：整合 `SyntaxHighlighter` (Prism / oneDark) 支援多國語言語法亮顯與「一鍵複製」功能。
- **自動捲動 (Auto-scroll)**：訊息新增或 Streaming 生成時，若用戶位於底部 300px 內則自動平滑置底。
- **打字機游標**：AI 回答時顯示與主題一致的脈衝游標。

### @ 引用

- `@` 觸發引用選單，顯示原文件層面的對象（文件名/網址/圖片）
- 選中後系統自動把該 source 所有相關 chunk 納入 RAG context
- 用戶看到「@活動計劃書.md」，不看到 chunk

**引用來源預覽 (Citation Preview)：**
- **觸發機制**：改為「**點擊觸發**」(Click-to-toggle) 而非懸停，防止誤觸。
- **關閉邏輯**：支援點擊預覽窗外關閉 (Click-outside) 或手動按關閉按鈕。
- **自適應縮放**：預覽窗寬度 (`w-full`) 隨對話泡泡自動延展，確保長文閱讀體驗。
- **空狀態隱藏**：若該則訊息無引用來源，區塊將完全隱藏，不顯示「未引用」佔位符。

### ContextHintBanner

每次 RAG 召回完成後，從頂部滑入（非阻塞），顯示具體內容預覽：

```
◆ 場地選擇標準框架（Pattern）  ▲ 嘉賓確認需雙重核實（Log）  ● 5 個相關來源
```

- 後端 `retrieve_context` 回傳 `pattern_hints` / `log_hints`（每項前 50 字元）
- `build_prompt` 將 hints 透過 `contextHints` 欄位傳回前端（stream event / fallback response）
- 點擊可展開查看所有 hint 明細
- 切換對話時重置為空

---

## 文本編輯器

**定位：** Chat Page 右側 Panel（固定寬 400px / xl:500px），可透過工具列按鈕開關（`uiStore.isEditorOpen`），不作為獨立頁面。

**技術：** Tiptap（ProseMirror）+ IC Design System 主題，元件位於 `src/components/chat/EditorPane.tsx`。

**已實作功能：**
- 工具列：Bold / Italic / Underline / Highlight / H1 / H2 / 有序清單 / 無序清單 / Code / 對齊 / 連結 / 圖片 / 表格（插入、合併、拆分、刪除行列）
- 文字彈出選單（Text Bubble Menu）：選取文字後延遲浮現（250ms debounce，避免拖選時跳動），手動 `position: fixed` 定位（非 Tiptap BubbleMenu），按下功能鍵不會重新定位。包含：AI 優化下拉 / Turn Into / Bold / Italic / Underline / Highlight / 項目清單 / 編號清單 / 靠左 / 置中 / 靠右 / 兩端對齊 / 連結。按鍵不顯示 active 狀態、單行不換行（`whitespace-nowrap`）
- Image BubbleMenu：選取圖片時浮現對齊與刪除選項（仍使用 Tiptap `<BubbleMenu>`）
- 自訂 `ImageNodePro` 節點（inline、可拖移、ReactNodeView）
- **AI 優化（選取文字 → `rag_query`）**：改寫 / 語氣調整 / 翻譯等 prompt 選單，結果顯示 diff 預覽，用戶確認後替換，或直接捨棄
- `extensions/` 目錄：`ImageNodeView.tsx` / `ImageNodePro.tsx`

**入庫機制（雙路徑）：**

**路徑 A — 儲存庫 editor 文件（直接暫存）：**
```
儲存庫頁「+ 新增文字文件」
    ↓
create_editor_document(title) → 在 {kb_path}/.insightcap/documents/ 建立 .md
    ↓
寫入 sources 表（type='editor', source_category='editor', local_doc_path 指向 .md）
    ↓
用戶編輯 → save_editor_document(source_id, content) → 寫本地檔 + 更新 DB
    ↓
按標題節點分段 → captures（同一 source_id）→ Embedding → usearch
    ↓
若 content_hash 相同 → 覆蓋舊 captures
```

**路徑 B — Chat Page Editor Panel（匯出觸發）：**
```
用戶匯出文件 → 以文件標題寫入 sources 表（type='editor'）
    ↓
按 Tiptap JSON 標題節點分段 → captures → Embedding → usearch
```

- Chat Page Editor Panel 不在 autosave 時入庫（草稿不入庫）
- 儲存庫 editor 文件則直接暫存，隨時可編輯

---

## AppState

Tauri 全域狀態（managed state），所有 command 通過 `State<'_, AppState>` 存取：

```rust
pub struct AppState {
    pub db: SqlitePool,           // SQLite 連接池
    pub kb_path: PathBuf,         // 知識庫根目錄
    pub vector_store: VectorStore, // 本地向量索引（cosine similarity）
    pub embedder: Arc<dyn Embedder>, // Embedding 模型（fastembed 或 NoopEmbedder fallback）
    pub current_conversation_id: Arc<Mutex<Option<String>>>,
    pub shutdown_tx: Arc<tokio::sync::watch::Sender<bool>>, // 背景任務停止訊號
}
```

`Embedder` 初始化失敗時自動降級為 `NoopEmbedder`（回傳零向量），確保應用可啟動，RAG 降級為關鍵字模式。

`shutdown_tx` 在 `setup_auth`（首次設定）時發送 `true`，所有背景任務（CaptureProcessor、ConversationScheduler、PatternPromotion、OCRWorker、CloudSyncWatcher）收到後退出迴圈，確保 DB 文件鎖在重啟前釋放。

---

## Settings 結構（關鍵欄位）

Settings 存於 SQLite `settings` 表，key/value 格式，各 key 對應一個 JSON 物件。

| Key | 重要欄位 | 說明 |
|-----|---------|------|
| `hotkeys` | `captureClipboard`（預設 `Ctrl+Alt+F`）| 擷取剪貼簿快捷鍵 |
| `hotkeys` | `quickInput`（預設 `Ctrl+Alt+G`）| 快速輸入框快捷鍵 |
| `chat_prompt_instruction` | `string`（非 JSON） | 用戶自訂 AI 回答風格，由 `store::save_settings` 獨立寫入 |
| `general` | `minimizeToTray` | 關閉主視窗時最小化到系統托盤（預設 true） |
| `knowledge` | `kbPath` | 知識庫根目錄路徑 |
| `bilibiliSessdata` | `string` | B 站登入憑證（由彈出視窗自動擷取，位於 AI 設置下） |
| `aiModels` | `chatLlm` | 對話主模型（`provider` / `model` / `apiKey` / `baseUrl`） |
| `aiModels` | `contentProcessorLlm` | Tagger / SpaceEngine 用的輕量模型（建議 3b 以下） |
| `aiModels` | `visionModel` | 可選 Vision 模型，用於增強圖片和 PDF 掃描頁的 OCR 結果；若配置會在 parse_file / parse_content / ocr_worker 中自動探測能力並增強 |
| `aiModels` | `embeddingModel` | Embedding 模型（預設 MultilingualE5Small，local） |
| `aiModels` | `summaryModel` | 對話摘要模型（`"follow_chat"` 表示跟隨 chatLlm） |
| `aiModels` | `providerProfiles` | 多 Provider 設定檔（可快速切換的 API 端點清單） |
| `background_synthesis` | `enabled` / `frequencyMinutes` / `maxChunksPerBatch` / `forceContentProcessorLlm` | 深度合成引擎控制（預設 enabled=true, 30 分鐘, 30 chunks, 強制 content_processor_llm） |
| `telegram` | `botToken`（加密存儲）/ `allowedChatIds: Vec<i64>` / `enabled: bool` / `streaming: "partial" \| "off"` / `promptInstructionOverride` | Telegram Bot 配置；`promptInstructionOverride` 優先度高於全域偏好 |
| `editor` | `promptInstructionOverride` | 編輯器專用 AI 偏好覆寫 |

---

## Settings UI 佈局原則

- **高頻配置頂置**：各專屬分頁（如「編輯器設定」、「網絡設定」）的 AI 偏好覆寫（Override）輸入框採用獨立 `SectionCard` 並置於該分頁最頂部。
- **功能收納**：「智慧提醒（Smart Reminders）」設置已併入「一般設定」，移除獨立側邊欄分頁，減少介面層級。
- **圖示規範**：「編輯器設定」統一使用 `PenLine` 圖示。
- **視覺一致性**：所有 AI 偏好輸入框（全域/覆寫）背景色統一使用 `bg-surface-base`。

---

## 資料庫安全設計

### 文件存放原則

```
app_data_dir/（本地，不受雲端同步影響）
├── bootstrap.json     ← 工作區路徑指針，永遠在本地
└── db_state.json      ← DB 操作狀態記錄

kb_path/（可能在雲端同步目錄）
└── .insightcap/
    ├── insightcap.db      ← SQLCipher 加密 DB
    ├── insightcap.db-wal  ← WAL 模式日誌
    ├── insightcap.db-shm  ← WAL 共享記憶體
    ├── auth.json          ← Argon2id salt（不含密碼/key）
    ├── recovery.bin       ← 加密備份的 db_key（用 recovery_key 加密）
    ├── documents/         ← 編輯器文件暫存（.md），source_category='editor'
    └── vectors/           ← usearch 向量索引
```

`bootstrap.json` 必須存在 app_data_dir，不能放在 kb_path，防止雲端同步在重啟瞬間覆蓋路徑指針。

**首次啟動（bootstrap.json 不存在）**：DB 建立在 `app_data_dir/insightcap_v2_pending/`（明文），完成設定後刪除，重啟時在用戶選定路徑建立加密 DB。

### db_state.json

記錄 DB 當前狀態，啟動時優先讀取：

```json
{
  "version": 1,
  "is_encrypted": true,
  "key_version": 1,
  "last_operation": "idle",
  "pending_kb_path": null,
  "last_successful_open": "2026-03-27T15:00:00Z"
}
```

`last_operation` 可能的值：`idle` / `rekey_in_progress` / `path_migration_in_progress` / `rebuild_index_in_progress`

### 啟動時健康檢查（強制，任何操作之前）

```
1. 讀 db_state.json
   → last_operation != 'idle' → 進入修復模式

2. 讀 bootstrap.json 取得 kb_path
   → 文件不存在 → 使用 app_data_dir 作為 fallback

3. 確認 kb_path 下的 DB 文件存在

4. 從 Keychain 讀取 db_key
   → 讀取失敗 → 進入修復模式（要求用戶輸入密碼）

5. 用 db_key 打開 DB
   → SQLCipher key 必須在建立連線時透過 SqliteConnectOptions::pragma("key", "\"x'hex'\"") 設定
   → 不可在連線後執行 PRAGMA key（SQLCipher 規定）
   → 失敗 → 進入修復模式

6. 執行 PRAGMA integrity_check
   → 失敗 → 進入修復模式

7. 全部通過 → 正常啟動
```

### 修復模式

DB 打不開時，不崩潰，顯示修復畫面：

```
選項 1：輸入密碼重試
選項 2：用恢復碼解鎖
選項 3：從最近備份還原
選項 4：清空重建（明確警告會丟失資料）
```

### bootstrap.json 寫入

```rust
// Windows 上 rename 在目標已存在時可能失敗（Access is denied）
// 直接覆寫即可，bootstrap.json 極小（< 100 bytes），寫入本身是原子的
std::fs::write(&bootstrap_path, &content)?;
```

> **注意**：Linux/macOS 可用 tmp + rename 達到原子語意；Windows 直接覆寫，風險可接受（文件極小，寫入中途斷電機率極低，且健康檢查會偵測損壞）。

### 強制重啟操作清單

所有危險操作完成後必須強制重啟，不在運行時切換狀態：

| 操作 | 完成條件 | 重啟原因 |
|------|---------|---------|
| 修改密碼 | `PRAGMA rekey` 成功 + 新 key 驗證通過 | 需要重新建立 DB 連接 |
| 更改工作區路徑 | 新路徑 DB 驗證通過 + bootstrap.json 原子寫入完成 | 新路徑 DB 需要從頭初始化連接 |
| 清除知識庫 | WAL checkpoint 完成 | 需要全新連接 |
| 更換外部 KB | 相容性驗證通過 | 向量索引需要重新載入 |
| 更換 Embedding 模型 | 模型下載完成 | 需要重新初始化 fastembed-rs |
| 重建向量索引 | usearch 索引重建完成 | 索引重新載入 |

### 更改工作區路徑的安全流程

```
1. 在新路徑建立 .insightcap/ 目錄結構
2. WAL checkpoint（PRAGMA wal_checkpoint(TRUNCATE)）清空舊 DB WAL
3. VACUUM INTO 複製 DB 到新路徑
4. 用當前 key 驗證新路徑 DB 可以正常打開
5. db_state.json → last_operation = 'path_migration_in_progress'
                   pending_kb_path = '新路徑'
6. 原子寫入 bootstrap.json（新路徑）+ sync_all()
7. db_state.json → last_operation = 'idle'
8. 關閉所有 DB 連接
9. 強制重啟

重啟後健康檢查：
  驗證新路徑成功 → 正常啟動
  驗證失敗 → fallback 舊路徑 + 通知用戶
```

### 修改密碼的安全流程

```
1. 用舊密碼衍生舊 key，嘗試打開 DB 驗證（確認舊密碼正確）
2. db_state.json → last_operation = 'rekey_in_progress'
3. 用新密碼衍生新 key，執行 PRAGMA rekey
4. 立即用新 key 重新打開 DB 驗證（確認 rekey 成功）
5. 驗證成功 → 更新 Keychain
6. db_state.json → last_operation = 'idle'，key_version += 1
7. 強制重啟

任何步驟失敗：
  Keychain 不更新
  db_state.json → last_operation = 'idle'（不保留中間狀態）
  顯示錯誤，讓用戶重試
```

---

## Schema 設計

### 核心表

**sources 表**（原文件/來源，用戶感知層）

```sql
CREATE TABLE sources (
  id               TEXT PRIMARY KEY,
  type             TEXT NOT NULL,
  -- file | url | image | editor | clipboard | screenshot
  source_category  TEXT NOT NULL DEFAULT 'capture',
  -- editor（編輯器文件，暫存本地）| capture（擷取內容）
  media_type       TEXT,
  -- text | markdown | url | image | video | pdf | file
  title            TEXT NOT NULL,
  url              TEXT,
  source_group_id  TEXT REFERENCES source_groups(id) ON DELETE SET NULL,
  -- 同一來源群組（canonical URL / 同內容檔案）
  file_path        TEXT,
  local_doc_path   TEXT,
  -- editor 類型的本地 .md 檔案路徑（{kb_path}/.insightcap/documents/）
  thumbnail        TEXT,
  clean_content    TEXT NOT NULL DEFAULT '',
  content_hash     TEXT,
  capture_count    INTEGER DEFAULT 0,
  use_frequency    INTEGER DEFAULT 0,
  captured_at      TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
```

**source_groups 表**（同來源歸組，提升召回完整性）

```sql
CREATE TABLE source_groups (
  id              TEXT PRIMARY KEY,
  identity_key    TEXT NOT NULL UNIQUE,
  -- 例如：url:{canonical_url} / file_hash:{sha256}
  canonical_url   TEXT,
  content_hash    TEXT,
  title_hint      TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
```

**captures 表**（chunk 粒度，後台計算單位）

```sql
CREATE TABLE captures (
  id               TEXT PRIMARY KEY,
  source_id        TEXT REFERENCES sources(id) ON DELETE CASCADE,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  type             TEXT NOT NULL,
  raw_content      TEXT NOT NULL DEFAULT '',
  clean_content    TEXT NOT NULL DEFAULT '',
  image_data       BLOB,
  -- 影像 BLOB 直接儲 DB（不依賴磁碟路徑）
  capture_method   TEXT NOT NULL,
  -- hotkey（Ctrl+Alt+F 複製擷取）
  -- quick_capture（Ctrl+Alt+G 快速輸入框，背景入庫後由 CaptureProcessor 寫入）
  -- temp_attachment（對話輸入框即時附件，不進入知識庫）
  -- import | mobile | editor_export | url
  tags             TEXT DEFAULT '[]',
  chunk_index      INTEGER DEFAULT 0,
  vector_id        INTEGER,
  promoted_capture_id TEXT REFERENCES captures(id) ON DELETE SET NULL,
  is_user_edited   INTEGER DEFAULT 0,
  -- 標記用戶是否手動編輯過此 chunk
  status           TEXT DEFAULT 'inbox',
  -- inbox | processed | archived | pending_ocr
  content_type     TEXT DEFAULT 'text',
  -- text | image_ocr | log | table | code | prose ...
  knowledge_type   TEXT DEFAULT 'data',
  -- data | pattern | log（ingestion 初判）
  chunk_strategy   TEXT DEFAULT 'semantic',
  -- semantic | heading | table | log_event ...
  chunk_metadata   TEXT DEFAULT '{}',
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
```

**memory_chunks 表**（三層記憶物理載體，只從對話產出）

```sql
CREATE TABLE memory_chunks (
  id               TEXT PRIMARY KEY,
  source_id        TEXT REFERENCES sources(id) ON DELETE SET NULL,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  conversation_id  TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
  knowledge_type   TEXT NOT NULL DEFAULT 'data',
  -- data | pattern | log
  content          TEXT NOT NULL,
  tags             TEXT DEFAULT '[]',
  trigger_context  TEXT DEFAULT '',
  -- log 專用，| 分隔，substring match
  confidence       REAL DEFAULT 1.0,
  pending_confirm  INTEGER DEFAULT 0,
  promotion_count  INTEGER DEFAULT 0,
  vector_id        INTEGER,
  promoted_capture_id TEXT REFERENCES captures(id) ON DELETE SET NULL,
  -- 紀錄從哪一筆 capture 升格而來
  placed_by        TEXT DEFAULT 'ai',
  last_synthesized_at TEXT DEFAULT NULL,
  -- 上次被深度合成引擎處理的時間（防重複，12 小時內不再處理）
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
```

**spaces 表**（AI 後台聚類）

```sql
CREATE TABLE spaces (
  id               TEXT PRIMARY KEY,
  name             TEXT NOT NULL,
  description      TEXT DEFAULT '',
  embedding_center BLOB,
  chunk_count      INTEGER DEFAULT 0,
  created_by       TEXT DEFAULT 'ai',
  is_archived      INTEGER DEFAULT 0,
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
```

**tags 表**（獨立標籤，支援頻率統計）

```sql
CREATE TABLE tags (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL UNIQUE,
  source       TEXT DEFAULT 'ai',
  use_count    INTEGER DEFAULT 0,
  recent_count INTEGER DEFAULT 0,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);
```

**projects 表**

```sql
CREATE TABLE projects (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  default_tags   TEXT DEFAULT '[]',
  color          TEXT,
  is_pinned      INTEGER DEFAULT 0,
  is_archived    INTEGER DEFAULT 0,
  sort_order     INTEGER DEFAULT 0,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
```

**external_knowledge_bases 表**（商業版）

```sql
CREATE TABLE external_knowledge_bases (
  id                  TEXT PRIMARY KEY,
  name                TEXT NOT NULL,
  db_path             TEXT NOT NULL,
  kb_type             TEXT NOT NULL,
  embedding_model     TEXT NOT NULL,
  embedding_dimension INTEGER NOT NULL,
  description         TEXT DEFAULT '',
  status              TEXT DEFAULT 'connected',
  last_checked        TEXT,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL
);
```

**decisions 表**（決策追蹤，定時回顧）

```sql
CREATE TABLE decisions (
  id               TEXT PRIMARY KEY,
  project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
  conversation_id  TEXT,
  variable_desc    TEXT NOT NULL,
  options          TEXT DEFAULT '[]',
  chosen_option    TEXT NOT NULL,
  outcome_source   TEXT,
  outcome_rating   TEXT,
  -- good | ok | bad | critical
  outcome_note     TEXT,
  status           TEXT DEFAULT 'pending',
  -- pending | reviewed | dismissed
  trigger_at       TEXT NOT NULL,
  -- 預設建立後 14 天觸發回顧
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
```

**chunk_relations 表**（反向鏈接，chunk 語義關聯）

```sql
CREATE TABLE chunk_relations (
  id          TEXT PRIMARY KEY,
  from_id     TEXT NOT NULL,   -- capture_id 或 memory_chunk_id
  to_id       TEXT NOT NULL,
  from_type   TEXT NOT NULL,   -- 'capture' | 'memory_chunk'
  to_type     TEXT NOT NULL,
  relation    TEXT NOT NULL,   -- 'references' | 'contradicts' | 'extends'
  confidence  REAL NOT NULL DEFAULT 1.0,
  created_at  TEXT NOT NULL,
  UNIQUE (from_id, to_id, relation)
);
```

**spaces 表新增欄位**（Space 知識指南層）

```sql
ALTER TABLE spaces ADD COLUMN knowledge_guide_content      TEXT NOT NULL DEFAULT '';
ALTER TABLE spaces ADD COLUMN knowledge_guide_updated_at   TEXT NOT NULL DEFAULT '';
```

**compiled_knowledge 表**（編譯後知識，由 DeepSynthesisEngine 背景生成）

```sql
CREATE TABLE compiled_knowledge (
  id                  TEXT PRIMARY KEY,
  space_id            TEXT REFERENCES spaces(id) ON DELETE CASCADE,
  content             TEXT NOT NULL,
  source_chunk_count  INTEGER DEFAULT 0,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL
);
-- 每個 space 最多一份，全域（space_id=NULL）也保留一份
CREATE UNIQUE INDEX idx_compiled_knowledge_space
  ON compiled_knowledge(COALESCE(space_id, '__global__'));
```

**reminders 表**（智慧提醒，從對話自動提取或手動建立）

```sql
CREATE TABLE reminders (
  id               TEXT PRIMARY KEY,
  conversation_id  TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  memory_chunk_id  TEXT REFERENCES memory_chunks(id) ON DELETE SET NULL,
  space_id         TEXT REFERENCES spaces(id) ON DELETE SET NULL,
  project_id       TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title            TEXT NOT NULL,
  description      TEXT DEFAULT '',
  event_type       TEXT NOT NULL DEFAULT 'event',
  -- meeting | deliverable | event | appointment
  date_status      TEXT NOT NULL DEFAULT 'confirmed',
  -- confirmed | time_inferred | range | month_only
  event_date       TEXT,         -- YYYY-MM-DD
  event_date_end   TEXT,         -- range 類型的結束日期
  event_time       TEXT,         -- HH:MM（本地時間）
  confidence       REAL DEFAULT 1.0,
  status           TEXT NOT NULL DEFAULT 'active',
  -- active | completed | dismissed | expired
  pending_confirm  INTEGER DEFAULT 0,
  -- confidence < 0.75 或 date_status != confirmed 時為 1
  created_at       TEXT NOT NULL,
  updated_at       TEXT NOT NULL
);
CREATE INDEX idx_reminders_status ON reminders(status);
CREATE INDEX idx_reminders_event_date ON reminders(event_date);
CREATE INDEX idx_reminders_conversation ON reminders(conversation_id);
```

**reminder_notifications 表**（意圖式通知排程）

```sql
CREATE TABLE reminder_notifications (
  id               TEXT PRIMARY KEY,
  reminder_id      TEXT NOT NULL REFERENCES reminders(id) ON DELETE CASCADE,
  intent           TEXT NOT NULL,
  -- prepare | imminent | now | start | midcheck | urgent | final | confirm_date
  scheduled_at     TEXT NOT NULL,  -- UTC ISO-8601
  sent_at          TEXT,           -- 已發送時間，NULL = 未發送
  channel          TEXT DEFAULT 'desktop',
  user_action      TEXT,           -- completed | snoozed | dismissed
  created_at       TEXT NOT NULL
);
CREATE INDEX idx_reminder_notif_scheduled
  ON reminder_notifications(scheduled_at) WHERE sent_at IS NULL;
CREATE INDEX idx_reminder_notif_reminder
  ON reminder_notifications(reminder_id);
```

### 資料流向

```
用戶擷取
  → inbox → CaptureProcessor
  → 寫入 sources（來源記錄）
  → 寫入 captures（chunk，固定為 data）
  → Tagger 提取標籤 → tags 表
  → Embedding → usearch
  → SpaceEngine 更新聚類
  → ChunkRelationEngine 分析反向鏈接 → chunk_relations

編輯器文件
  → 建立 source（type='editor', source_category='editor'）
  → 本地 .md 暫存於 {kb_path}/.insightcap/documents/
  → 存檔時按標題節點分段 → 多個 captures（同一 source_id）
  → 若 content_hash 相同的 source 已存在 → 覆蓋舊 captures
  → Embedding → usearch

對話進行
  → ConversationScheduler → 摘要
  → Tagger 深度推斷 knowledge_type
  → 寫入 memory_chunks
  → Embedding → usearch
  → PatternPromotion 掃描

新 Space 建立
  → SpaceRecluster 重新計算所有 chunk 相似度
  → 更新 captures.space_id / memory_chunks.space_id

外部 KB（商業版）
  → 不寫入本地，查詢時直接讀外部 DB + 外部向量索引

深度合成（背景，每 30 分鐘）
  → DeepSynthesisEngine 描揇近 24 小時更新的 pattern/log chunks
  → 僅在無活躍對話時執行（Ollama 單模型保護）
  → 使用 content_processor_llm 呼叫 DEEP_SYNTHESIS_PROMPT
  → 解析 JSON → 寫入 entity/concept tags、新 synthesis chunk、矛盾關係
  → 呼叫 COMPILED_KNOWLEDGE_PROMPT → 精煉知識 UPSERT 到 compiled_knowledge 表
  → 結果於下次 RAG 查詢時直接讀取（零額外延遲）

智慧提醒（雙路徑）
  路徑 A — 常規（ConversationScheduler 非同步 spawn）：
    對話摘要完成後 → ReminderEngine.extract_reminders()
    → LLM 提取事件 → 去重 → 排程生成 → 寫入 reminders + notifications
  路徑 B — Hot Path（即時偵測，繞過 Scheduler 延遲）：
    用戶訊息含時間關鍵字（前端 Regex 偵測）
    → AI 回應完成 → invoke trigger_urgent_reminder_check
    → 同一 ReminderEngine.extract_reminders() 立即執行
    → generate_imminent_fallback() 補近未來通知（事件 -5 分鐘或立即 +30 秒）
  投遞：
    ReminderScheduler 每 60 秒輪詢 → emit "reminder-notification" event → 前端 ReminderToast
```

**inbox 表（擷取佇列）**

```sql
CREATE TABLE inbox (
  id             TEXT PRIMARY KEY,
  content        TEXT NOT NULL DEFAULT '',
  content_type   TEXT NOT NULL DEFAULT 'text',
  -- text | image | file | url
  source_exe     TEXT DEFAULT '',
  window_title   TEXT DEFAULT '',
  source_url     TEXT DEFAULT '',
  source_pid     INTEGER,
  image_data     BLOB,
  file_path      TEXT,
  session_id     TEXT DEFAULT '',
  status         TEXT DEFAULT 'pending',
  -- pending | processing | processed | failed
  captured_at    TEXT NOT NULL
);
```

---

## 抽象層（Trait 定義）

### KnowledgeSource

```rust
pub trait KnowledgeSource: Send + Sync {
    async fn semantic_search(
        &self,
        query_embedding: &[f32],
        scope: &QueryScope,
        limit: usize,
    ) -> Result<Vec<ScoredChunk>, KnowledgeError>;

    async fn keyword_trigger(
        &self,
        query: &str,
        scope: &QueryScope,
    ) -> Result<Vec<ScoredChunk>, KnowledgeError>;

    fn source_type(&self) -> KnowledgeSourceType;
}

pub struct QueryScope {
    pub tags: Vec<String>,
    pub project_id: Option<String>,
    pub include_external: bool,
}

pub enum KnowledgeSourceType {
    Personal,
    EnterpriseLocal,
    EnterpriseExternal(String),
}
```

### LLMProvider

**`OpenAiProvider::new` base_url 自動修正規則（`openai.rs`）：**
- `base_url` 為空 → 預設 `http://localhost:11434/v1`（本地 Ollama）
- 本地位址（`localhost` / `127.0.0.1` / `:11434`）且未以 `/v1` 結尾 → 自動補齊 `/v1`
- `https://api.openai.com`（不含路徑）→ 自動補齊為 `https://api.openai.com/v1`
- `complete_json` 會自動去除 LLM 回傳中的 ` ```json ` fence 再解析

```rust
pub trait LLMProvider: Send + Sync {
    // 單輪補全
    async fn complete(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<String, LLMError>;

    // 單輪補全，強制 JSON 輸出
    async fn complete_json(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<serde_json::Value, LLMError>;

    // 多輪對話：system prompt + history[(role, content)] + 本輪 user query
    async fn complete_with_history(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
    ) -> Result<String, LLMError>;

    // Streaming：每個 token 透過 on_token callback 推送，最終回傳完整結果
    // StreamToken = enum Content(String) | Reasoning(String)（推理模型的 thinking block）
    async fn complete_stream(
        &self,
        system_prompt: &str,
        history: &[(String, String)],
        user_query: &str,
        options: LLMOptions,
        on_token: impl Fn(StreamToken) + Send + 'static,
    ) -> Result<StreamResult, LLMError>;
}

pub struct LLMOptions {
    pub temperature: f32,
    pub max_tokens: usize,
    pub stream: bool,
    pub think_mode: Option<bool>, // true = 啟用推理模式，None/false = 不啟用
}
```

### Embedder

```rust
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
}
```

---

## RAG 引擎

### 召回策略

**第一階段：召回**

```
語意搜尋（向量 × 標籤篩選）：
  captures：Top-10，門檻 0.25
  memory_chunks（data）：Top-5，門檻 0.25
  memory_chunks（pattern）：Top-3，門檻 0.20
  外部 KB（商業版）：Top-5，門檻 0.25

關鍵字觸發（獨立，不受數量限制）：
  memory_chunks（log）：trigger_context | 分隔後 substring match

加分項：
  同 project_id 的 memory_chunks：+0.06
  pattern：+0.05
  log：+0.08
  sources.use_frequency 高：+0.02
  captures.is_user_edited = 1：+0.05（一手資料優先）
  memory_chunks.placed_by = 'user'：+0.05（一手資料優先）
```

**第二階段：組裝分層 Context**

按語意角色分組（compiled_knowledge / pattern / log / data / external），注入 system prompt（見三層記憶理論章節）。
在 Phase 5 企業版架構中，RAG 引擎的 `retrieve_context` 將另外查詢狀態為 `connected` 的所有外部 SQLite 資料庫（掛載於 `external_knowledge_bases`），動態獲取其 `captures` 表中相關的知識片段，前綴加上 `[外部知識庫]` 並與本地結果一同交給 LLM 推理。

**第三階段：編譯後知識注入（最高優先）**

```
retrieve_context 第 7 步：
  查詢 compiled_knowledge 表
  → 全域知識（space_id IS NULL）
  → 與當前 project 相關 space 的知識
  → 以「## 已編譯核心知識（最高優先）」注入 system prompt
  → 排在 Pattern / Log / Data 之前

來源：DeepSynthesisEngine 背景生成，RAG 查詢時只做 DB 讀取，零額外 LLM 呼叫
```

**臨時附件 Context（最優先注入）**

用戶在對話輸入框加入檔案（文件 / 圖片 OCR / URL）時，前端呼叫 `create_temp_chunk`：

```
用戶選擇附件
    ↓
create_temp_chunk → file_parser::parse_content（統一入口）
    → 文件：parse_file（支援 pdf/docx/xlsx/csv/md/txt/圖片等）
        PDF 掃描頁 → 原生 PDF 文字提取 → OCR → （可選）Vision 增強
        圖片 → 原生 OS OCR → （可選）Vision 增強
    → URL：video_parser::parse_url_content（網頁/YouTube/Bilibili）
    → 圖片：走 OCR pipeline（crate::ocr::perform_ocr）→ （可選）Vision 增強
    ↓
切段落 → Embedding → 寫入 captures（capture_method = 'temp_attachment'）
    ↓
回傳 chunk_ids → 前端 chip 顯示解析完成

用戶發送訊息
    ↓
rag_query 收到 temp_chunk_ids
    ↓
generate_answer 從 captures 直接取出內容
    → 以「## 使用者附加文件內容」注入 system prompt（早於 RAG 召回結果）
```

臨時附件 captures（`capture_method = 'temp_attachment'`）在應用啟動時由 `CaptureProcessor` 清除超過 7 天的舊紀錄，不在對話結束時即時清除，以保留跨輪次上下文。

**對話歷史優化（Token 控制）**

```
每次 rag_query / rag_query_stream 時：
    前端送出：
        history = 最近 6 輪（12 條 messages）的 raw user/assistant 文字
        conversation_summary = conversations.summary（本對話摘要，由 enqueue_summary 非同步產生）
    ↓
rag_commands 傳給 RAGEngine：
    history → complete_with_history 建構 [system, ...history, user] messages
    conversation_summary → 注入 system prompt 作為「## 本對話早期摘要（供參考）」段落
    ↓
完整 context = 摘要（壓縮早期輪次）+ 最近 6 輪原文 + RAG 召回 + 臨時附件
```

這樣即使對話超過 40 輪，LLM 送出的 token 數保持穩定，早期重要內容透過摘要保留。

---

## Core Services

| 服務 | 職責 |
|------|------|
| CaptureEngine | 擷取、清洗、OCR、寫入 sources + captures；`file_parser::parse_content` 為統一解析入口 |
| ConversationEngine | 對話管理、多輪歷史（sliding window + summary 注入）、訊息儲存；`enqueue_summary` 觸發非同步摘要 |
| MemoryEngine | memory_chunks CRUD、tagger、pending_confirm 流程；`process_conversation_summary` 寫入摘要 chunk；INSERT 前自動驗證 `project_id` 存在性（sqlx 0.8 預設啟用 `PRAGMA foreign_keys = ON`，不存在則降為 NULL） |
| RAGEngine | 統一召回，調用 KnowledgeSource trait，組裝分層 context；`rag_query_stream` 支援 SSE streaming |
| PatternEngine | 路徑 B 跨對話識別，升格建議 |
| SpaceEngine | AI 聚類管理；`assign_to_space()` 為新 capture 分配 space；`merge_similar_spaces()` 根據 embedding center 相似度自動合併近似 space；`assign_memory_chunk_to_space()` 為 memory_chunk 向量分配 space |
| TagEngine | per-source 標籤生成 + per-capture 標籤生成、CRUD、頻率統計、推薦；`process_source()` 輸入整份文件產出 3-5 代表標籤，跳過已有標籤以提升效率 |
| VisionEngine | Vision model 可選增強層，對圖片與掃描頁 OCR 結果進一步理解；`probe_vision_support()` 探測模型能力（4×4 紅色測試圖，結果快取整個 App 生命週期），`try_vision_enhance()` 非阻塞調用 vision model 增強辨識 |
| ChunkRelationEngine | 分析 chunk 之間的語意關聯（references / contradicts / extends），寫入 `chunk_relations` 表 |
| ReminderEngine | 智慧提醒：LLM 提取對話中的日期/事件 → 去重（Layer 1: 對話內同日同分；Layer 2: 跨對話「同標題+同日+同分」）→ 意圖式排程生成（meeting/deliverable/appointment/event × confirmed/time_inferred/range/month_only）→ 寫入 reminders + reminder_notifications；支援確認/延後/完成/關閉操作。時區處理使用 `chrono::Local` 本地時間轉 UTC |
| SpaceKnowledgeGuideEngine | 根據 space 內最新 chunk 自動生成/更新 `spaces.knowledge_guide_content` |
| DeepSynthesisEngine | 背景深度合成引擎：多目標合成（entity/concept/synthesis/contradiction）+ 編譯後知識生成；強制使用 `content_processor_llm`，僅在無活躍對話時執行 |
| WebSearch | 聯網搜尋，呼叫外部搜尋 API，供對話時「聯網搜尋」功能使用 |
| AuthService | 認證、加密、健康檢查 |
| LanguageNormalizer | 多語言文字正規化，供 Tagger / embedding 前處理使用 |

## Background Services

| 服務 | 職責 | 觸發方式 |
|------|------|---------|
| CaptureProcessor | inbox → sources + captures；重用既有 source（URL 完全匹配、剪貼簿 30 分鐘內同標題）；image 寫入後標記 `pending_ocr`；源層級標籤延後由 TagEngine.process_source() 統一處理 | inbox 有新項目，每 5 秒輪詢 |
| ConversationScheduler | 處理 `conversation_summary_queue`，生成摘要→memory_chunks，非同步為 memory_chunk 分配 space，分析反向鏈接，更新 Space 知識指南，非同步提取智慧提醒（ReminderEngine） | 每 30 秒輪詢佇列 |
| PatternPromotion | 掃描新 memory_chunk，判斷升格（每 5 分鐘輪詢） | 背景定時 |
| SpaceRecluster | 定期重新計算所有 chunk embedding，重新聚類、合併相似 space、更新 chunk_count、自動歸檔空 space | 每 30 分鐘，或接收 `space-created` event 後 3 秒延遲 |
| DeepSynthesisEngine | 深度合成：掃描近 24h 更新的 pattern/log chunks，多目標合成（entity/concept/synthesis/contradiction）+ 編譯後知識生成。智慧跳過（有活躍對話時不執行）、強制 content_processor_llm、additive-only 寫入、last_synthesized_at 防重複 | 每 N 分鐘（預設 30，settings 可調），啟動後延遲 5 分鐘 |
| ReminderScheduler | 輪詢 `reminder_notifications`，檢查 quiet hours（跨午夜 + 週末靜默），發送到期通知至前端（`reminder-notification` event），標記過期提醒（event_date < 昨天 → expired） | 每 60 秒輪詢 |
| OCRWorker | 掃描 `pending_ocr` captures，先用原生 OS OCR（WinRT/Vision），後可選用 `vision_model` 增強辨識（試圖探測模型能力，若支援則用 vision 結果取代 OCR），完成後更新 `clean_content` 並觸發 TagEngine/SpaceEngine/ChunkRelationEngine | 每 30 秒輪詢，每次最多 5 筆 |
| CloudSyncWatcher | 偵測 SQLite 檔案 modified time 異動（例如 Dropbox 覆蓋）| 30 秒輪詢（TODO：異動時觸發重載） |
| HTTPAPIServer | 本地 REST API（Axum，`0.0.0.0:3030`），保留為內部 API / 後續擴充。端點：`GET /api/health`、`POST /api/capture`（Bearer token 驗證，寫入 inbox）、`POST /api/rag`（查詢知識庫回傳 context）、`POST /api/chat`（SSE streaming 代理推理）。原手機 PWA / React Native 路徑已停用，手機端改走 Telegram Bot。Token 自動生成存 `settings.mobile_api_token` | 啟動時常駐 |
| TelegramBot | Telegram Bot polling 手機端入口；全文件類型支援（與桌面端一致）。訊息處理：①純文字 → RAG 問答；②含 URL（含 YouTube/Bilibili）→ URL 擷取入庫（自動抓字幕）；③照片（壓縮）→ OCR 入庫；④文件（document）→ 根據類型自動分類：圖片（image/*） → OCR 入庫；PDF/DOCX/XLSX/PPTX/TXT/MD/EPUB/HTML/RTF/程式碼 → 解析文字後 inbox 入庫。通用機制：`sendMessageDraft` 原生串流回覆（Bot API 9.5+）；白名單 chat_id 靜默拒絕；即時緊急提醒通知會包含「事項清單（標題）」以利確認。`/new` `/list` `/rename` `/project` `/newproject` 對話/項目管理（Inline Keyboard）；檔案大小 Telegram API 上限 20MB；`streaming: "partial" \| "off"` 可設定 | `settings.telegram.enabled = true` 時啟動時常駐（polling 模式） |

---

## Command Layer（Tauri 呼叫）

| 命令 | 職責 |
|------|------|
| rebuild_source_tags | 批量重新生成歷史 source 標籤；串行執行、跳過已有標籤、UTF-8 安全切割、進度報告（current/total/done） |

---

## Tauri Events

| 事件名 | 觸發條件 | 聽眾 |
|------|--------|------|
| `space-created` | `assign_to_space()` 新建 space 時 | SpaceRecluster（延遲 3 秒開始重新聚類） |
| `summary-completed` | ConversationScheduler 完成對話摘要時 | 前端 toast 反饋 |

---

## Design System

### 四主題

| 主題 class | 名稱 | 底色 | 強調色 |
|-----------|------|------|--------|
| `.theme-frost` | Frost Glass | `#F5F4F1` | `#2B7FD4` |
| `.theme-void` | Deep Void（暗色） | `#131416` | `#6366F1` |
| `.theme-warm` | Warm Parchment | `#F0E8D6` | `#B45309` |
| `.theme-sage` | Sage Breeze | `#F2F7F0` | `#528F44` |

預設：Frost Glass。用戶手動選擇，不跟系統。啟動時最早套用防止 FWOT。儲存 localStorage（key: `ic-theme`）。

### 樣式來源

所有樣式由兩個檔案協作定義：

1. **`src/design-system/tokens.css`** — CSS 變數定義。`:root` 放通用固定值（radius、spacing、typography、motion、knowledge 顏色），每個 `.theme-*` class 覆寫 surface / text / accent / stroke / shadow 等語境 token。

2. **`tailwind.config.js`** — 將 token 對應到 Tailwind shorthand。元件只寫 `bg-surface-base`、`text-text-primary`，Tailwind 輸出 `var(--surface-base)`，瀏覽器從當前 `.theme-*` 讀值。

切換主題只需改 `<html>` 的 class（由 `themeStore.ts` 控制），元件不感知主題。

### Design Token 分類

```
Surface:   --surface-base  --surface-layer  --surface-card
           --surface-subtle  --surface-flyout  --surface-control

Text:      --text-primary  --text-secondary  --text-tertiary
           --text-disabled  --text-link  --text-on-accent

Accent:    --accent-default  --accent-light1  --accent-light2  --accent-dark1

Stroke:    --stroke-card  --stroke-control  --stroke-divider
           --stroke-strong  --stroke-focus

Shadow:    --shadow-card  --shadow-card-hover  --shadow-flyout  --shadow-dialog

Knowledge: --knowledge-data  --knowledge-data-bg  --knowledge-data-text
           --knowledge-pattern  --knowledge-pattern-bg  --knowledge-pattern-text
           --knowledge-log  --knowledge-log-bg  --knowledge-log-text

Semantic:  --color-success  --color-warning  --color-danger（含 -bg / -hover 變體）
```

### 全域提示 (Toaster) 規範

- **位置**：固定為 `top-right`（右上角），避免遮擋底部聊天輸入框。
- **樣式**：
    - 背景：`var(--surface-base)`，**禁止透明**（Opacity 1.0），確保在複雜背景上文字清晰。
    - 文字：`var(--text-primary)`。
    - 邊框：`var(--stroke-divider)`。
    - 陰影：`var(--shadow-flyout)`。
- **互動**：新通知建立成功後需同步觸發桌面端 Toast 提示，讓用戶得知後台自動化動作（如自動建立提醒事項）。

### 三層記憶視覺語言（全系統統一）

| 類型 | 符號 | Token |
|------|------|-------|
| `data` | ● | `--knowledge-data` |
| `pattern` | ◆ | `--knowledge-pattern` |
| `log` | ▲ | `--knowledge-log` |

---

## 多語言系統

**方案：** react-i18next
**語言：** 繁體中文（預設）/ 簡體中文 / 英文
**切換：** Settings 手動選擇，儲存 settings 表，不跟系統

```
src/i18n/
├── index.ts
├── types.ts          # TypeScript key 型別，防止拼錯
└── locales/
    ├── zh-TW.json
    ├── zh-CN.json
    └── en.json
```

**強制規範：**
- 所有介面文字用 `t('key')` 取值，禁止寫死
- 三層記憶類型名稱納入翻譯：`memory.type.data/pattern/log`
- key 用語意命名：`common.confirm`，不是 `common.確認`

### RepositoryPage 對照落差（2026-04-02）

以下是目前實作與本文件仍未完全一致的項目，作為後續修正清單。

| 類別 | 文件規範 | 目前實作狀態 | 影響範圍 |
|------|----------|--------------|----------|
| i18n | 全介面文字必須經 `t('key')`，禁止寫死 | `RepositoryPage.tsx` 仍有大量硬編碼文字（例如：今天/昨天、搜尋 placeholder、來源文件、筆記、導入文件、載入中、刪除確認、toast 訊息、Chunks 區塊） | 儲存庫頁（UI 文字）、部分導覽文字 |
| source_category 命名 | 文件多處仍寫 `editor` / `capture` | 程式實作使用 `editor_doc` / `captured`，文件其餘章節尚有舊值殘留 | Schema 章節、UX 章節、開發說明 |
| Chunk 編輯能力 | 文件描述「展開後可編輯內容/標籤/Space」 | 當前 `RepositoryPage` 僅提供 chunk 載入與唯讀預覽，尚未提供 inline 或 Dialog 編輯與儲存 | 儲存庫預覽 Modal、Chunk 操作流 |
| TypeFilterBar 元件化 | 文件描述為獨立 `TypeFilterBar` 與既定元件結構 | 當前為 `RepositoryPage` 內嵌 filter chips（all/source/note + tag chips），未抽成獨立元件 | 前端元件結構章節、Phase 4 任務描述 |

**建議修正優先序：**
1. 先完成 i18n：將 `RepositoryPage` 全部字串改為 i18n key，補齊 `zh-TW/zh-CN/en`。
2. 統一命名：文件與程式全部改為同一組 `source_category` 值（建議以實作值 `editor_doc` / `captured` 為準）。
3. 補齊 Chunk 編輯面板：至少先提供內容與 tags 編輯，再補 Space 選擇。
4. 視需要將 filter 區塊抽成 `TypeFilterBar` 元件，與架構文件一致。

---

## 模組結構

### 前端

```
src/
├── design-system/
│   ├── tokens.css          # Design Token（四主題：frost/void/warm/sage）
│   └── index.css
├── i18n/
│   ├── index.ts
│   ├── types.ts
│   └── locales/
├── components/
│   ├── ui/                 # 基礎元件庫
│   ├── memory/             # ContextHintBanner、CitationBadge、DecisionReviewToast、PatternPromotionToast、MemoryConfirmToast
│   ├── chat/               # 含 EditorPane.tsx（Tiptap 編輯器）、extensions/
│   ├── knowledge/          # 儲存庫：TypeFilterBar、TimelineView、ChunkListPanel、ChunkEditPanel、DocumentPreview、SpaceInsightPanel
│   └── settings/
├── stores/
│   ├── themeStore.ts
│   ├── languageStore.ts
│   ├── chatStore.ts
│   ├── knowledgeStore.ts
│   ├── tagStore.ts
│   └── uiStore.ts
├── pages/
│   ├── ChatPage.tsx
│   ├── RepositoryPage.tsx
│   ├── SettingsPage.tsx
│   ├── LoginPage.tsx
│   ├── SetupPage.tsx
│   ├── MigratePage.tsx
│   └── QuickCapturePage.tsx
└── lib/
    ├── tauri.ts
    └── types.ts
```

### 後端

```
src-tauri/src/
├── commands/
│   ├── capture_commands.rs
│   ├── conversation_commands.rs
│   ├── memory_commands.rs
│   ├── knowledge_commands.rs
│   ├── project_commands.rs
│   ├── space_commands.rs
│   ├── editor_commands.rs
│   ├── tag_commands.rs
│   ├── rag_commands.rs
│   ├── auth_commands.rs
│   ├── settings_commands.rs
│   └── bilibili_auth.rs        # B站登入視窗，擷取 SESSDATA
├── services/
│   ├── memory_engine.rs
│   ├── rag_engine.rs
│   ├── pattern_engine.rs
│   ├── space_engine.rs
│   ├── tag_engine.rs
│   └── language_normalizer.rs  # 繁簡轉換（zhconv），CaptureProcessor 使用
├── knowledge_source/
│   ├── mod.rs              # KnowledgeSource trait
│   ├── personal.rs
│   └── enterprise.rs
├── capture/
│   ├── mod.rs              # trigger_capture、normalize_video_url
│   ├── clipboard.rs
│   ├── keyboard.rs
│   ├── metadata.rs
│   ├── readability.rs      # 一般網頁爬取（readability + SSRF 保護）
│   ├── video_parser.rs     # YouTube/Bilibili 影片字幕擷取
│   ├── attachment_manager.rs
│   ├── file_parser.rs
│   └── extractors/         # pdf / docx / xlsx / csv / txt / md
├── ocr/
│   ├── mod.rs              # perform_ocr 統一入口
│   ├── windows.rs          # WinRT OcrEngine（zh-Hant）
│   ├── macos.rs            # Vision Framework
│   ├── preprocess.rs       # 灰階 → Otsu 二值化 → 亮度提升
│   └── postprocess.rs      # 語言偵測 + 文字清洗
├── providers/
│   ├── llm/
│   │   ├── mod.rs          # LLMProvider trait
│   │   ├── ollama.rs
│   │   ├── openai.rs
│   │   └── vision.rs       # Vision LLM 輔助——可選圖片增強層（probe + enhance）
│   └── embedding/
│       ├── mod.rs          # Embedder trait
│       └── fastembed.rs
├── background/
│   ├── capture_processor.rs
│   ├── conversation_scheduler.rs
│   ├── pattern_promotion.rs
│   ├── space_recluster.rs
│   ├── deep_synthesis_engine.rs  # 深度合成引擎（多目標合成 + 編譯後知識生成）
│   ├── ocr_worker.rs
│   ├── cloud_sync_watcher.rs  # 函數式（start_cloud_sync_watcher），無 struct
│   └── telegram_bot.rs       # Telegram Bot polling（手機端入口，enabled 時常駐）
├── db/
│   ├── connection.rs       # DB 連接、健康檢查、原子寫入、AppState 定義
│   └── migrations/
├── http_server.rs          # Axum HTTP API（0.0.0.0:3030，內部 API / 後續擴充）
│                           # 端點：health / capture / rag / chat(SSE)
│                           # Bearer token 認證，token 存 settings.mobile_api_token
├── prompts.rs              # 所有系統 prompt 常數集中管理（✅ 完整落地，所有 service 不再有散落 prompt）
│                           # RAG: RAG_SYSTEM_BASE / RAG_CONTEXT_PATTERN / _LOG / _DATA / _EXTERNAL / _WEB_SEARCH / _COMPILED
│                           # 記憶推斷: TAG_CAPTURE_PROMPT / TAG_EXTRACT_PROMPT / DEEP_INFER_KNOWLEDGE_TYPE_PROMPT
│                           # Space 分配: SPACE_ASSIGN_NEW_PROMPT / _EXISTING_PREFIX / _SUFFIX
│                           # 關係分析: CHUNK_RELATION_CLASSIFY_PROMPT
│                           # 合成引擎: DEEP_SYNTHESIS_PROMPT / COMPILED_KNOWLEDGE_PROMPT / PATTERN_ANALYSIS
│                           # 其他: AUTO_TITLE_SYSTEM / THINK_MODE_PREFIX / SPACE_KNOWLEDGE_GUIDE_SYSTEM
├── auth/
├── vector_store/
└── utils/
```

---

## 開發順序

**Phase 1：基礎建設**
1. IC Design System（四主題 token + 基礎元件庫）
2. i18n 系統（react-i18next，三語）
3. DB 安全基礎（bootstrap.json 原子寫入、db_state.json、啟動健康檢查、修復模式）
4. Schema migration（sources + captures + memory_chunks + spaces + tags + projects）
5. Auth 模組（Argon2id + SQLCipher + Keychain + rekey 安全流程）
6. Abstraction Layer（KnowledgeSource + LLMProvider + Embedder trait）

**Phase 2：核心引擎**
7. CaptureEngine
8. TagEngine
9. SpaceEngine + SpaceRecluster
10. MemoryEngine + Tagger（分陣邏輯）
11. RAGEngine（分層 context + 標籤範圍控制）
12. ConversationEngine

**Phase 3：記憶理論完整落地**
13. PatternEngine（路徑 B）
14. 用戶確認流程（pending_confirm toast）
15. ContextHintBanner + CitationBadge
16. 文本編輯器（Tiptap + 匯出入庫）

**Phase 4：儲存庫頁 UX**
17. Timeline 視圖（以 sources 為單位，日期分組）
18. TypeFilterBar（editor / capture 切換 + media type chips）
19. Chunk 可見/編輯/刪除（展開文件後 ChunkListPanel + ChunkEditPanel）
20. @ 引用（原文件層面）+ 標籤推薦 + `#` 輸入

**Phase 5：商業版**
21. KnowledgeSource Enterprise 實現（✅ 已完成 `enterprise.rs`）
21a. KnowledgeSource Personal 實現（✅ 已完成 `personal.rs`，`semantic_search` / `keyword_trigger`）
22. 外部 KB 加載（✅ 已完成，Settings → Enterprise 頁籤）
23. Knowledge Builder（獨立應用）

**Phase 6：Telegram 手機端入口**
24. Telegram Bot（✅ 取代原生手機版）
    - 手機端透過 Telegram 投餵文字、URL、圖片與文件
    - 內容寫入 inbox → 桌面端 CaptureProcessor 處理
    - RAG 問答與提醒通知由桌面端負責，Telegram 只作為入口與通知通道
    - 白名單 `chat_id` 控制權限，無需維護 React Native / Android 專案
25. 本地 HTTP API（Axum，保留）
    - `GET /api/health` — 連線確認
    - `POST /api/capture` — 擷取 → inbox（Bearer token）
    - `POST /api/rag` — 查詢桌面知識庫，回傳 context_text + chunks
    - `POST /api/chat` — 桌面代理推理，SSE streaming
    - 不再作為手機 app 主路徑，後續僅作內部 API / 擴充用途

---

## Phase 6 Telegram Mobile 架構

> 原 React Native / Android 手機版已移除；現階段手機端入口統一改用 Telegram Bot。以下舊版 React Native 設計僅保留作歷史參考，不再是當前實作目標。

### 推理三模式

| 模式 | 技術 | 需求 | 適用場景 |
|------|------|------|---------|
| **A — Local** | LiteRT-LM + Gemma 4（`tasks-genai`） | 手機本地 `.task` 模型 | 離線、隱私優先 |
| **B — Cloud** | 直連 Claude / OpenAI / Gemini API | API Key + 網路 | 最強推理能力 |
| **C — Desktop Relay** | 桌面 `POST /api/chat` SSE | 同 WiFi + 桌面運行 | 複用桌面 LLM + 完整知識庫 |

### 介面設計標準 (UI/UX Redesign)

為了達到類 ChatGPT/Grok 的現代化質感，手機端採用以下設計標準：

1. **導航架構**：
    - 採用 **Drawer Navigator (側邊抽屜)** 取代 Tab Bar。
    - **抽屜內容**：集中顯示「對話歷史」，支援建立新對話與切換舊對話。
2. **標題列配置 (Standardized Header)**：
    - **左上角**：`Menu` (開啟抽屜) + `BookOpen` (開啟知識庫頁面)。
    - **右上角**：`PenSquare` (快速擷取內容) + `Settings` (設定)。
3. **視覺表現**：
    - 全面移除 Emoji 按鈕，統一使用 `lucide-react-native` 圖示組。
    - 色調採用深色極簡風格 (`#0f0f11` 背景)，強調專業感與內容專注。

### 推理流程與模式 (Inference Modes)

| 模式 | 技術實作 | 描述 | 優點 |
|------|------|------|---------|
| **A — Local** | LiteRT-LM + Gemma 4（`tasks-genai`） | 手機本地 `.task` 模型 | 離線、隱私優先 |
| **B — Cloud** | 直連 OpenAI / Gemini / OpenAI-compatible API | API Key + 網路 | 最強推理能力 |
| **C — Desktop** | 桌面端代理（Axum SSE） | 同 WiFi + 桌面運行 | 複用桌面 LLM + 完整知識庫 |

### 桌面端 API 詳情 (Mobile-Facing API)

桌面端透過 Axum 提供 `0.0.0.0:3030` 的 HTTP 服務，主要端點如下：

- **Auth**: `POST /api/auth/verify` (驗證 Token 與連線)
- **Chat**: 
    - `POST /api/chat` (與桌面 LLM 進行對話，支援 SSE Streaming)
    - `GET /api/conversations` (獲取歷史對話列表)
    - `POST /api/conversations` (建立新對話)
    - `GET /api/conversations/:id/messages` (獲取特定對話內容)
- **Knowledge**: `GET /api/sources` (獲取儲存庫最新 100 筆紀錄，含分類與標籤)
- **Capture**: `POST /api/capture` (手機端快速擷取內容送回桌面知識庫)

### 推理流程（Mode C）

```
手機 ChatScreen
  ↓ POST /api/chat { message, history }  (Bearer token)
桌面 Axum HTTPAPIServer
  ↓ RagEngine.retrieve_context()  → 查本地知識庫
  ↓ OpenAiProvider.complete_stream()  → 呼叫桌面已設定 LLM
  ↓ SSE token stream
手機 ChatScreen（即時顯示）
```

### 舊版 React Native 目錄結構（已移除）

```
mobile/
├── android/
│   └── app/src/main/java/com/insightcap/mobile/
│       ├── ShareIntentModule.kt  # Android 原生分享 Intent 處理
│       ├── LiteRTLMModule.kt    # Gemma 4 native bridge
│       └── MainApplication.kt    # 原生模組註冊
├── src/
│   ├── screens/
│   │   ├── ChatScreen.tsx        # 預設首頁，模式切換與串流對話
│   │   ├── ConversationListScreen.tsx # 抽屜內對話清單
│   │   ├── KnowledgeScreen.tsx   # 知識庫瀏覽（Lucide 圖示分類）
│   │   ├── CaptureScreen.tsx     # 快速內容擷取
│   │   └── SettingsScreen.tsx    # 連線設定與模式控制
│   └── services/
│       ├── inference/            # 三種模式的提供者工廠
│       └── desktop-api.ts        # 全量 API 客戶端封裝
└── App.tsx                      # Drawer Navigator 核心配置
```

### 舊版 Android 系統整合（已移除）

1. **Share Target**: 
   - 透過 `AndroidManifest.xml` 註冊 `SEND` Intent。
   - 用戶在手機瀏覽器或其他 App 選取「分享到 AetherCore」時，會自動調出 `CaptureScreen`。
2. **Native Bridge**:
   - `ShareIntentModule` 負責在 React Native 啟動或 Resume 時抓取 Intent 內容（文字或連結），並在傳遞後自動清除防止重啟重複讀取。

### QR Code 配對流程

```
桌面：設定 → 其他設定 → 「手機端連線」→ 顯示 QR Code
      QR 內容：JSON { url: "http://192.168.x.x:3030", token: "..." }

手機：設定 → Mode C → 「掃描桌面 QR Code」
      → react-native-vision-camera 掃描
      → 自動填入 url + token → 測試連線 ✓
```

### Gemma 4 模型規格

| 版本 | 大小 | 最低裝置需求 | 格式 |
|------|------|------------|------|
| Gemma 4 1B int8 | ~0.9 GB | 6 GB RAM, Android 8+ | `.task`（LiteRT） |
| Gemma 4 4B int4 | ~2.8 GB | 8 GB RAM, Android 8+ | `.task`（LiteRT） |

模型來源：Kaggle `google/gemma-4` → LiteRT format

---

## LLM 用戶設定與相容層

所有 LLM 呼叫統一透過 `OpenAiProvider`，內部自動處理路徑轉換：

| provider 設定 | 路徑处理 |
|---|---|
| `ollama` | 自動補齊 `/v1`，免 API Key |
| `openai` | 使用原始 URL |
| 其他公雱第三方 | 需填寫完整 base_url |

**settings.ai_models 結構：**
```json
{
  "chat_llm":              { "provider": "ollama", "model": "qwen2.5:7b",    "base_url": "http://localhost:11434" },
  "content_processor_llm": { "provider": "ollama", "model": "qwen2.5:3b",    "base_url": "http://localhost:11434" },
  "vision_model":          { "provider": "ollama", "model": "minicpm-v",     "base_url": "http://localhost:11434" },
  // Vision 模型配置：用於增強圖片和 PDF 掃描頁的 OCR 結果
  // 工作流：probe_vision_support（4×4 紅色測試圖探測，快取結果） → try_vision_enhance（若支援則調用）
  // 可選：若不配置或模型不支援 vision，系統將使用原生 OS OCR 結果
  "embedding_model":       { "provider": "local",  "model": "multilingual-e5-small" }
}
```

**settings.chat_prompt_instruction（用戶 AI 回答偏好）：**

```json
{
  "chatPromptInstruction": "請用英文回答，語氣要簡潔"
}
```

- 預設空字串，留空時不插入 prompt
- 附加在系統優先級聲明之後，不可覆蓋系統段行為
- 由 SettingsPage 一般設定 tab 的 textarea 管理

**Ollama JSON 輸出穩健性：**
本地模型回傳 JSON 時可能附帶 Markdown 代碼區塊（` ```json...``` `），`complete_json` 會自動脱殼再對析。

---

## 擷取系統設計

### 擷取觸發方式

| 方式 | 流程 |
|------|------|
| 全域熱鍵（預設 `Ctrl+Alt+F`）| 自動複製已選文字/影像 → inbox |
| Quick Capture UI | 無選取內容時彈出浮動輸入框 → inbox |
| 檔案擷取 | 將檔案復製至剪貼簿後按熱鍵 → 實時解析 |
| URL 擷取 | 自動判斷內容為 URL → video_parser（YouTube/Bilibili/一般網頁）|
| 影像擷取 | 剪幕畫後存 BLOB → Pending OCR → 原生 OS OCR（三層漸進式）|

### OCR 處理流程

```
熱鍵擷取截圖
  ↓
影像 bytes 寫入 inbox（image_data BLOB）
  ↓
CaptureProcessor 發現 content_type = 'image'
  ↓
寫入 captures（status = 'pending_ocr'）
  ↓
OCRWorker 輪詢取出（每 30 秒）
  ↓
第一層：原生 OS OCR
  Windows → WinRT OcrEngine（zh-Hant 語言包）
  macOS   → Vision Framework（VNRecognizeTextRequest）
  ↓
第二層：圖像前處理（失敗時靜默降級回原始圖）
  灰階化 → Otsu 自動二值化 → 亮度提升
  ↓
第三層：文字後處理
  語言偵測（中文/英文）→ 字符修正 → 換行修正 → 頁面標記移除 → 空白標準化
  ↓
（可選）Vision 增強層
  若 `vision_model` 已配置：
    probe_vision_support() → 探測模型能力（4×4 紅色測試圖，結果快取整個 App 生命週期）
    若支援 vision → try_vision_enhance() → 用原始圖重新分析，若成功則用 vision 結果取代 OCR
  若 vision 不可用 → 保留 OCR 結果
  ↓
寫回 clean_content，status = 'processed'
  ↓
觸發 TagEngine / SpaceEngine / ChunkRelationEngine
```

### URL 擷取流程

```
CaptureProcessor 發現 content_type = 'url'
  ↓
video_parser::parse_temp_content
  ├─ YouTube URL → yt-dlp 提取字幕（降級：HTTP 抓標題+描述）
  ├─ Bilibili URL → WBI 簽名 API + 字幕優先級排序 + CDN 內容校驗
  └─ 一般網頁 → readability 提取正文 + 圖片清單
  ↓
語言標準化（language_normalizer：繁簡轉換）
  ↓
寫入 sources + captures
  ↓
Embedding → usearch
```

### Quick Capture 視窗

- Tauri label: `quick-capture`，700×80px，無邊框透明視窗
- 按熱鍵且剪貼簿為空時自動彈出
- **Enter** 送出寫入 inbox、**Esc** 关閉
- 自動識別 URL 並設定 content_type

---

*版本：v2.19 | 日期：2026-04-20*
本次更新：
- 手機端路徑正式切換：移除原生 React Native/Android 專案，手機入口統一改為 Telegram Bot（polling）。
- 新增 content-aware ingestion 與來源歸組：
  - 新增 `capture/chunking.rs`：依內容型態路由 chunk strategy（`log_event` / `table` / `heading` / `semantic`）。
  - 新增 `capture/source_group.rs`：URL canonicalization、檔案內容雜湊、同來源群組（source group）歸併。
  - 新增 migration `016_source_group_ingestion.sql`：`source_groups` 表、`sources.source_group_id`、`captures.content_type` / `knowledge_type` / `chunk_strategy` / `chunk_metadata`。
- RAG 同來源前後文強化：命中 capture 後，補抓同 `source_id` 前後 chunk 作為「同來源前後文」。
- 記憶與提醒 UI 收斂：
  - 移除 `MemoryConfirmToast`、`PatternPromotionToast`。
  - 調整 `DecisionToast` / `ReminderToast` 顯示位置與錯誤處理，並同步 i18n。

*版本：v2.18 | 日期：2026-04-12*
本次更新：
- **手機端 UI 全面重構**：
    - 導入類 ChatGPT 的 **Drawer Navigator**，取代舊有的 Tab 底欄。
    - 視覺標準化：全面移除表情符號，替換為 **Lucide 圖示系統**。
    - 介面拋光：標準化 Header 佈局（Menu/BookOpen 在左，PenSquare/Settings 在右）。
- **Android 原生深度整合**：
    - 實作 **Share Target** Intent 攔截，支援從第三方應用直接分享內容至知識庫。
    - 建立 Native Bridge (`ShareIntentModule`) 處理非同步 Intent 數據提取。
- **手機端穩定性與核心修復**：
    - 升級 `react-native-reanimated` 至 3.16.1，解決與 React Native 0.76 的編譯衝突。
    - 修正 `ChatScreen` 訊息串流、語法報錯及型別定義問題。
- **API 擴充與文件化**：
    - 於 Axum 增設 `/api/conversations`、`/api/sources` 等端點，並於本文件補全說明。

*版本：v2.17 | 日期：2026-04-10*
本次更新：
- 新增深度合成引擎（DeepSynthesisEngine）：背景每 30 分鐘執行，對近 24 小時更新的 pattern/log chunks 進行多目標合成
  - 新增 `background/deep_synthesis_engine.rs`（完整實作）
  - 新增 `012_deep_synthesis.sql` migration：`memory_chunks` 新增 `last_synthesized_at` 欄位
  - `prompts.rs` 新增 `DEEP_SYNTHESIS_PROMPT`（多目標合成 JSON 格式）+ `COMPILED_KNOWLEDGE_PROMPT`（精煉知識）+ `RAG_CONTEXT_COMPILED`（最高優先標頭）
  - `settings/store.rs` 新增 `BackgroundSynthesisSettings` struct，整合到 `AllSettings`
  - `connection.rs` 註冊 migration 012 + 013
  - `background/mod.rs` 註冊 deep_synthesis_engine 模組
  - `lib.rs` 啟動 `start_deep_synthesis_worker`
- 安全設計：
  - 智慧跳過：有活躍對話時不執行（Ollama 單模型保護）
  - 強制 content_processor_llm：不與 chat 主模型競爭
  - Additive-only 寫入：entity/concept 只新增 tags，不修改 chunk 內容
  - chunk_id 驗證：所有寫入操作只處理本批次查詢到的合法 chunk
  - last_synthesized_at 防重複：已合成的 chunk 12 小時內不再處理
  - 矛盾門檻：confidence < 0.7 不寫入
  - JSON 解析失敗安全退出：LLM 回傳格式異常時不做任何 DB 寫入
- 新增編譯後知識層（Compiled Knowledge）：
  - 新增 `013_compiled_knowledge.sql` migration：`compiled_knowledge` 表（每 space + 全域各一份）
  - `deep_synthesis_engine.rs` 合成完成後呼叫 `COMPILED_KNOWLEDGE_PROMPT` → UPSERT 到 `compiled_knowledge` 表
  - `rag_engine.rs` `retrieve_context` 新增第 7 步：查詢 compiled_knowledge；`build_prompt` 將其作為最高優先 context 注入
  - `contextHints` 新增 `compiledKnowledgeCount` 欄位
- 前端可透過 `background_synthesis` settings 控制：enabled / frequencyMinutes / maxChunksPerBatch / forceContentProcessorLlm

*版本：v2.16 | 日期：2026-04-07*
本次更新：
- 新增對話自動命名功能：新對話在前 2 輪對話完成後，由 AI 自動生成 4-15 字標題
  - 後端 `auto_title_conversation` command（`conversation_commands.rs`）
    - 僅在 title === '新對話' 時觸發（避免覆蓋手動命名）
    - 取前 4 條訊息，每條截取 300 字，呼叫 LLM 生成標題（timeout 15s）
    - 無 LLM 配置時 fallback：截取第一條 user 訊息前 20 字
  - `prompts.rs` 新增 `AUTO_TITLE_SYSTEM` 常數（標題生成規則）
  - 前端 `chatStore.autoTitleConversation()`：rag-stream-done 後若訊息數 ≤ 4 條則觸發
  - 已在 `lib.rs` `generate_handler!` 註冊
- SpaceInsightPanel 設計精簡（commit 34d1d98）：移除知識分佈視圖，保留知識指南分頁
  - v2.15 記載的「Insight / 知識指南分頁切換」已調整為純知識指南顯示
- RAG commands 新增 `thinking_mode` 參數（normal/think），已實作 Think Mode 系統 prompt 前綴注入
- RAG commands 新增 `_web_enabled` 參數 stub（佔位，尚未實作 Web 搜尋功能）

*版本：v2.15 | 日期：2026-04-06*
本次更新：
- 新增 Space 知識指南層：每個 Space 有一份 AI 持續維護的結構化知識指南
- `spaces` 表新增 `knowledge_guide_content TEXT DEFAULT ''` + `knowledge_guide_updated_at TEXT DEFAULT ''`（010_space_knowledge_guide.sql）
- 新增 `SpaceKnowledgeGuideEngine` 服務（`services/space_knowledge_guide_engine.rs`）
  - `update_knowledge_guide_for_space(space_id)`：取最多 30 個 memory_chunks（pattern/log 優先）→ LLM 增量更新知識指南（temperature=0.3, max_tokens=1500）
  - 知識指南結構：`## 核心框架` / `## 已掌握方法` / `## 已知風險` / `## 知識空白`
  - LLM 輸出 `INSUFFICIENT` 時跳過寫入
- 新增三個 commands：`get_space_knowledge_guide` / `save_space_knowledge_guide` / `regenerate_space_knowledge_guide`，已在 `lib.rs` 註冊
- 觸發點：`conversation_scheduler.rs` `Ok(chunk_id)` 後，非同步查 `memory_chunks.space_id` → spawn `update_knowledge_guide_for_space`
- `SpaceInsightPanel.tsx` 新增 Insight / 知識指南分頁切換
  - 知識指南分頁：`<pre>` 渲染 Markdown、行內編輯（textarea）、「重新生成」按鈕
  - 更新時間標示 `updated_at`
- `prompts.rs` 新增 `SPACE_WIKI_SYSTEM` 常數
- i18n 新增 `space_knowledge_guide` 鍵群（9 個 key）：zh-TW / zh-CN / en + `types.ts`

*版本：v2.14 | 日期：2026-04-06*
本次更新：
- 新增反向鏈接（Chunk 關聯關係）：`chunk_relations` 表（009_chunk_relations.sql）
  - 欄位：`from_id / to_id / from_type / to_type / relation / confidence / created_at`
  - 關係類型：`references`（引用）| `extends`（延伸）| `contradicts`（矛盾）
  - `UNIQUE INDEX (from_id, to_id, relation)` 防重複
- 新增 `ChunkRelationEngine` 服務（`services/chunk_relation_engine.rs`）
  - `analyze_and_link(chunk_id, chunk_type, content)`：向量預篩（相似度 ≥ 0.50，Top-10）→ LLM 判斷關係類型 → 寫入 `chunk_relations`
  - `fetch_linked_chunks(chunk_ids)`：查詢已召回 chunks 的所有關聯，回傳另一端 chunk 的內容
  - LLM prompt 輸出格式：`relation:confidence`（例如 `references:0.85`），信心度 < 0.6 視為無關係
- `RagEngine.retrieve_context` 新增第 6 步：對已召回的 capture_ids + memory_chunk_ids 呼叫 `fetch_linked_chunks`，將關聯 chunks 補充到 `data_context`（標示 `⚠️ 矛盾觀點` / `延伸資訊` / `相關記憶`）
- 新增 `get_chunk_relations` command，已在 `lib.rs` 註冊，供前端引用預覽查詢
- 觸發點：`capture_processor.rs` 每個 chunk 寫入 + embedding 後，非同步 spawn 呼叫 `analyze_and_link`
- 觸發點：`conversation_scheduler.rs` `MemoryEngine` 寫入 memory_chunk 成功後，非同步 spawn 呼叫 `analyze_and_link`

*版本：v2.13 | 日期：2026-04-06*
本次更新：
- SpaceRecluster 完整實作：`SpaceEngine` 重構，加入 `recluster_all` 方法
  - 計算各 Space embedding_center（已分配 chunks 向量平均），存回 `spaces.embedding_center`（BLOB）
  - 批次重新分配 captures + memory_chunks（每批 50 筆，相似度門檻 ≥ 0.45）
  - 完成後更新 `spaces.chunk_count`
- `SpaceEngine::new` 加入 `embedder` + `vector_store` 參數，同步更新 `capture_processor.rs` + `editor_commands.rs` 呼叫點
- 新增 `trigger_space_recluster` command，已在 `lib.rs` 註冊
- `space_recluster.rs` 完整實作：監聽 `space-created` event + 每 30 分鐘定時重聚類，完成後 emit `space-reclustered` event

*版本：v2.12 | 日期：2026-04-06*
本次更新：
- ConversationScheduler 標記為「完整實作」：已具備 30 秒輪詢 + LLM 摘要（含增量更新 + fallback）+ MemoryEngine 深度推斷 knowledge_type
- 新增 `summary-completed` event emit：摘要完成後通知前端
- 新增 `MemoryConfirmToast` 前端元件：處理信心度 < 0.75 的 data/log 類型 pending_confirm chunks
- `App.tsx` main 狀態下掛載 `MemoryConfirmToast`
- i18n `types.ts` 補充 `memory_confirm` interface（6 個 key）

*版本：v2.11 | 日期：2026-04-06*
本次更新：
- PatternPromotion 路徑 B 完整實作：`PatternEngine` 加入標籤重疊 ≥ 2 + 向量相似度 ≥ 0.65 + ≥ 3 個不同對話三重條件篩選
- 升格結果寫入 `memory_chunks`（`pending_confirm = 1`），取代舊版寫入 `captures` 的行為
- Log `trigger_context` 自動擴展：掃描近似 data chunks，將相似內容關鍵字追加到 trigger_context
- 新增 `PatternPromotionToast` 前端元件：監聽 `pattern-promoted` event，非阻塞顯示升格建議
- `App.tsx` main 狀態下掛載 `PatternPromotionToast`（左下角，與 DecisionToast 右下角錯開）
- Background Services 架構圖標記 PatternPromotion 為「完整實作」
- i18n `types.ts` 補充 `pattern_promotion` interface（6 個 key）

*版本：v2.10 | 日期：2026-04-06*
本次更新：
- 新增 Decision 層：`decisions` 表（008_decisions.sql），追蹤對話中的決策並在 14 天後觸發回顧
- 後端 `decision_commands.rs`：`create_decision` / `get_due_decisions` / `get_project_decisions` / `report_decision_outcome` / `dismiss_decision`
- 前端 `DecisionReviewToast` 元件：固定右下角 Toast，每 5 分鐘輪詢到期決策，支援 4 級評分 + 備註
- `App.tsx` main 狀態下掛載 `DecisionReviewToast`
- Schema 設計章節補充 `decisions` 表結構
- Command Layer 補充 `decision_commands`
- 前端模組結構補充 `DecisionReviewToast`
- i18n `types.ts` 補充 `decision` interface（13 個 key）

*版本：v2.9 | 日期：2026-04-06*
本次更新：
- 新增 Space 知識可用性視圖：儲存庫頁 `SpaceInsightPanel` 元件，選擇 Space 後展示 knowledge_type 分佈 + 標籤統計
- 新增後端 `get_space_insight` command（`space_commands.rs`），統計 memory_chunks + captures 按 Space 的分佈
- 系統架構圖 Command Layer 補充 `space_commands`
- 前端模組結構補充 `SpaceInsightPanel`
- RAG 內容來源分層：`captures.is_user_edited = 1` 或 `memory_chunks.placed_by = 'user'` → RAG 分數 +0.05，一手資料優先於 AI 生成內容
- ContextHintBanner 強化：從純數量顯示改為具體內容預覽（pattern/log 前 50 字元），支援點擊展開明細
- 後端 `retrieve_context` 新增 `pattern_hints` / `log_hints` 回傳；`build_prompt` 回傳新增 `contextHints` 欄位
- `chatStore.ContextStats` 擴充 `patternHints` / `logHints` 欄位，切換對話時重置
- 不改 schema，純前端 + 後端邏輯改動

*版本：v2.8 | 日期：2026-03-31*
本次更新：
- 「知識庫頁」重新命名為「儲存庫頁」，採用 Timeline（日期主導）佈局
- 移除左側 Space 篩選面板，改為頂部 TypeFilterBar（editor / capture 切換 + media type chips）
- 儲存庫分兩類文件：editor 編輯器文件（暫存本地 .md）與 capture 擷取內容
- Chunk 從純後台概念改為展開文件後可見、可編輯（內容/標籤/Space）、可刪除
- 文件與 chunk 均支援獨立新增和刪除
- `sources` 表新增 `source_category`、`media_type`、`local_doc_path` 欄位
- `captures` 表新增 `is_user_edited` 欄位
- 文件存放結構新增 `documents/` 目錄（editor 文件暫存）
- 文本編輯器入庫機制更新：新增儲存庫 editor 文件直接暫存路徑
- 更新設計原則：chunk 展開後可見可編輯；Space 遷入 chunk 編輯面板
- 前端模組結構更新：knowledge 目錄改為儲存庫元件群

*版本：v2.7 | 日期：2026-03-30*
本次更新：
- 修正三層記憶路徑 A：`enqueue_summary` 取代舊 `summarize_conversation`，加入非同步佇列流程說明
- 補充 `ConversationScheduler` 觸發機制：每 30 秒輪詢 `conversation_summary_queue`
- 新增對話歷史優化機制：sliding window（最近 6 輪）+ `conversations.summary` 注入，減少 token 使用
- 補充 `create_temp_chunk` 解析入口：統一改為 `file_parser::parse_content`（文件）+ `video_parser::parse_url_content`（URL）
- 新增 temp_attachment 清理機制（啟動時清除 7 天前記錄）
- 更新 InputArea 輸入框 UX：加入 Normal/Think 模式切換按鈕，補充傳送限制說明
- 更新系統架構圖：加入 Command Layer 具體模組列表、`LLMProvider` trait 說明、`QuickCapturePage`
- 更新 Core Services 表格：加入 `LanguageNormalizer`，補充各服務職責描述
- 主題名稱更正：`light/dark/casual/fresh` → `frost/void/warm/sage`

*版本：v2.6 | 日期：2026-03-29*
本次更新：
- OCR 流程改為三層原生 OS OCR（WinRT/Vision + 前處理 + 後處理），移除 Vision LLM 依賴
- 新增 `ocr/` 模組完整結構：`windows.rs` / `macos.rs` / `preprocess.rs` / `postprocess.rs`
- 新增 `capture/video_parser.rs`：YouTube（yt-dlp + HTTP fallback）/ Bilibili（WBI 簽名 + 字幕校驗）
- 新增 `capture/readability.rs`（原 `readability_scraper.rs`），含 SSRF 保護
- 新增 `services/language_normalizer.rs`：繁簡轉換，整合進 CaptureProcessor
- 新增 `commands/bilibili_auth.rs`：B站登入視窗，取得 SESSDATA
- 新增 URL 擷取流程章節，補充 video_parser 整合說明
- 更新後端模組結構，補全 `capture/`、`ocr/`、`services/` 清單

*版本：v2.5 | 日期：2026-03-29*
本次更新：
- 新增 `prompts.rs` 模組：所有系統 prompt 常數集中管理，不散落在各 service
- 更新 RAG Context 組裝模板：新增優先級聲明段與用戶偏好段，明確系統段 vs 用戶段分層設計
- 新增 `settings.chatPromptInstruction` 欄位：用戶可自訂 AI 回答風格，系統段優先
- 實作 SettingsPage 一般設定 tab：textarea 管理用戶 prompt 偏好
- 更新後端模組結構：補充 `vision.rs` 說明，加入 `prompts.rs`

*版本：v2.5 | 日期：2026-04-03*
本次更新：
- 匯出/匯入知識庫重構為零知識設計：DB 全程 SQLCipher 加密，透過 `backup_recovery.bin` 傳遞 key，移除「計劃解法（方案 A）」暫記
- 新增 `backup_recovery.bin` 至 KB 目錄結構說明
- 新增「目錄遷移場景」：Keychain 空 + auth.json + DB 存在時顯示 `MigratePage`，支援密碼與恢復碼兩種解鎖路徑
- `AuthStatus` 新增 `isMigrated` 欄位，`get_auth_status` 偵測條件

*版本：v2.4 | 日期：2026-03-29*
本次更新：
- 新增 AppState 章節：定義 `db` / `kb_path` / `vector_store` / `embedder` / `current_conversation_id` 組成
- 補充 `Embedder` 降級機制（`NoopEmbedder` fallback）
- 更新 `db/connection.rs` 說明，標注 AppState 定義位置

*版本：v2.3 | 日期：2026-03-28*
本次更新：
- 變Image 儲存策略 image_path → image_data BLOB，不依賴檔案系統
- 變OCRWorker 剛接整，OCR 完成後主動觸發 TagEngine/SpaceEngine
- 變全域熱鍵系統正式註冊，支援從 settings 讀取快捷鍵字串并解析
- 變Quick Capture UI 建立，以浮動透明輸入框呼叫 quick_capture command
- 變LLM 設定讀取統一改用 settings::store::get_settings，修正舍棄的 keys = 'models' 議題
- 變memory_chunks 設計變：新増 promoted_capture_id，PatternEngine 改用 conversation_id 欄
- 變Phase 6 HTTP API 骨架完成（axum，127.0.0.1:3030）
````
This is the description of what the code block changes:
<changeDescription>
補充知識庫備份/匯出/還原不會包含圖片、附件等資料夾，並說明影響與建議。
</changeDescription>

This is the code block that represents the suggested code change:
```markdown
---

## 知識庫匯出／匯入設計（更新 2026-04-02）

### 設計原則

所有經程式處理的本地資料（文件副本、筆記）都必須有副本在 KB 目錄內，確保 KB 目錄是自足的完整單元，可獨立備份與還原。URL、YouTube、Bilibili 影片等網路資源不做副本（只存文字擷取結果）。

### KB 目錄結構（完整）

```
kb_path/
├── .insightcap/
│   ├── insightcap.db       ← SQLCipher 加密 DB（主資料）
│   ├── insightcap.db-wal   ← WAL 日誌（匯出前 checkpoint 清除）
│   ├── insightcap.db-shm   ← WAL 共享記憶體（匯出時跳過）
│   ├── auth.json           ← Argon2id salt（不含密碼/key）
│   ├── recovery.bin        ← 加密備份的 db_key（日常恢復碼加密）
│   ├── backup_recovery.bin ← 加密備份的 db_key（備份專用恢復碼加密，匯出時生成，匯入時解密）
│   └── vectors/            ← usearch 向量索引
├── files/                  ← 所有本地文件副本（PDF、DOCX、MD 等）
└── notes/                  ← 編輯器筆記（draft_<timestamp>.md）
```

### 各擷取路徑的副本策略

| 擷取入口 | `sources.file_path` | `sources.local_doc_path` | `sources.clean_content` | 匯出安全性 |
|---------|----|----|-----|-----------|
| 熱鍵擷取（文字/URL）| 無 | 無 | ✅ 完整內嵌 | ✅ 安全 |
| 熱鍵擷取（圖片）| 無 | 無 | ✅ OCR 後內嵌；image_data BLOB 在 DB | ✅ 安全 |
| 剪貼簿拖入檔案（`process_clipboard_file`）| 原始外部路徑 | `files/<id>_<name>` ✅ KB 內副本 | ✅ 完整內嵌 | ✅ 安全 |
| 匯入文件（`ingest_file`）| 原始外部路徑 | `files/<id>_<name>` ✅ KB 內副本 | ✅ 完整內嵌 | ✅ 安全 |
| 編輯器筆記（`editor_doc`）| 無 | `notes/draft_<ts>.md` ✅ KB 內 | 空（由磁碟讀取）| ✅ 安全（notes/ 隨 zip 打包） |
| 對話臨時附件（`temp_attachment`）| 外部路徑 | 無 | ✅ 完整內嵌 | ✅ 安全（臨時用途，7天自動清除）|
| URL / 網頁 | 無（URL 存 `sources.url`）| 無 | ✅ 爬取結果內嵌 | ✅ 安全 |
| Telegram Bot（文本/URL）| 無 | 無 | ✅ 完整內嵌 | ✅ 安全（白名單 chat_id 認證，content 寫 inbox 後 capture_processor 處理）|
| Telegram Bot（照片）| 無 | 無 | ✅ OCR 後內嵌；image_data BLOB 在 DB | ✅ 安全（同上）|
| Telegram Bot（文件：PDF/DOCX/PPTX…）| 無 | 無 | ✅ 提取文字後內嵌 | ✅ 安全（同上，暫存檔寫入 parse_file 後自動清除）|

### export_kb 行為（實作版，2026-04-03 更新）

**零知識設計：** DB 全程保持 SQLCipher 加密狀態，key 透過獨立的 `backup_recovery.bin` 傳遞，zip 中不存在任何明文 DB 副本。

前端流程：
1. 呼叫 `generate_recovery_phrase` 生成 24-word 備份專用恢復碼（獨立於日常恢復碼）
2. 顯示恢復碼 + 複製按鈕 + 確認勾選框
3. 用戶確認後呼叫 `export_kb(dest_path, mnemonic)`

後端 `export_kb(dest_path, mnemonic)` 流程：
1. `PRAGMA wal_checkpoint(TRUNCATE)` 確保 DB 完整
2. 從 Keychain 讀取當前 `db_key`（hex）
3. 用備份恢復碼衍生 `backup_recovery_key`，生成 `backup_recovery.bin`（77-byte，同 recovery.bin 格式）至臨時路徑，讀入記憶體後刪除
4. 打包 zip：遞迴走訪 `.insightcap/`（跳過 `-wal`、`-shm`、舊 `backup_recovery.bin`）+ `files/` + `notes/`，注入新 `backup_recovery.bin`（in-memory）
5. zip 完成

### import_kb 行為（實作版，2026-04-03 更新）

前端流程（2 步）：
- 步驟 1：選擇 zip → 輸入備份恢復碼 + 設定新密碼 → 呼叫 `import_kb(src_path, mnemonic, new_password)` → 後端回傳新日常恢復碼
- 步驟 2：顯示新日常恢復碼 + 確認保存 → 呼叫 `restart_app`

後端 `import_kb(src_path, mnemonic, new_password) → String` 流程：
1. 驗證 ZIP magic bytes（`PK\x03\x04`）
2. 備份現有 `insightcap.db` 為 `.db.bak`，解壓 zip 到 `kb_root`
3. 讀取 `backup_recovery.bin`，從 bytes[1..17] 取出 salt，用備份恢復碼呼叫 `derive_recovery_key_verify` → 解密取得原始 `db_key`
4. 用原始 `db_key` 開啟加密 DB（`SqliteConnectOptions::pragma("key", ...)`），驗證成功
5. 衍生新 `new_db_key`（Argon2id + 新 salt），執行 `PRAGMA rekey`
6. 原子寫入新 `auth.json`（新 salt）
7. 生成新日常恢復碼，寫入 `recovery.bin`
8. 更新 Keychain `auto_login_key` = 新 `db_key` hex
9. Zeroize `db_key`、`new_db_key`，回傳新日常恢復碼

### 安全邊界

| 場景 | DB 狀態 | key 傳遞方式 |
|------|--------|------------|
| 匯出 zip | SQLCipher 加密原樣 | `backup_recovery.bin`（需備份恢復碼解密）|
| zip 遭截取 | 無法讀取（無 key）| 零知識 |
| 匯入成功後 | 用新密碼 rekey | 舊備份恢復碼作廢 |

### 目錄遷移場景（2026-04-03 新增）

**觸發條件：** Keychain 空 + `auth.json` 存在 + `insightcap.db` 存在

此為「直接複製 KB 目錄到新機器」的場景，不同於首次 Setup（無 auth.json）或忘記密碼（Keychain 有值但密碼錯誤）。

`get_auth_status` 回傳 `isMigrated: true`，前端渲染 `MigratePage`。

**UI：** 偵測到知識庫，但此裝置尚未授權。選擇解鎖方式（密碼 / 恢復碼）。

**密碼路徑（`unlock_migrated_with_password`）：**
1. 讀取 `auth.json` 取得 salt
2. Argon2id(password, salt) → `db_key`
3. 用 `db_key` 開啟加密 DB 驗證（`SqlitePoolOptions::connect_with`）
4. 成功 → 寫 Keychain `auto_login_key` → 前端呼叫 `initApp()`（`try_auto_login` 成功 → 進入 main）

**恢復碼路徑（`unlock_migrated_with_mnemonic`）：**
1. 解密 `recovery.bin` → 原始 `db_key`
2. 用原始 `db_key` 開啟加密 DB 驗證
3. 成功 → 衍生新 `new_db_key`（新密碼 + 新 salt），`PRAGMA rekey`
4. 寫新 `auth.json`、新 `recovery.bin`、更新 Keychain
5. 回傳新日常恢復碼 → 前端顯示並確認 → `restart_app`

### repair_missing_local_copies 指令

一次性修復工具，掃描所有 `file_path` 有值但 `local_doc_path` 為空的歷史 sources，把原始檔案（若仍存在）複製到 `kb_path/files/` 並更新 DB。原始檔案已刪除或移動者跳過。設定頁面「知識庫管理」區塊提供觸發按鈕。

---

## LLM Reasoning 能力適配

### 推理模式檢測與適配

AetherCore 支援多種 LLM 原生推理能力，由 `model_caps::detect(model, provider)` 在執行時自動偵測：

| 推理模式 | 模型 | Provider | 激活方式 | 輸出格式 |
|---------|------|----------|---------|---------|
| **OpenAiReasoning** | o1, o1-mini, o3, o4-mini | openai, xai | API 自動啟用 | `delta.reasoning_content` |
| **DeepSeekReasoning** | deepseek-r1 | openrouter, 直連 API | 標準 OpenAI 格式 | `delta.reasoning_content` |
| **Gemini Reasoning** | gemini-2.5+, gemini-3+ | google, openrouter | 標準 OpenAI 格式 | `delta.reasoning_content` |
| **Gemma4Think** | gemma-4, gemma4 | ollama | system prompt 注入 `<|think|>` | `<\|channel>thought\n...<channel\|>` |
| **OllamaThinkTag** | qwq, qwen3, deepseek-r1 | ollama | API `options.think` 參數 | `<think>...</think>` |

### 前端互動

1. **推理按鈕可見性** — `InputArea.tsx` 在 mount 時呼叫 `get_chat_llm_supports_thinking()`，推理模式支援度回傳 `bool`
   - 支援（`ReasoningStyle != None`）→ 顯示「深度思考」按鈕
   - 不支援（`ReasoningStyle == None`）→ 隱藏按鈕

2. **推理流程** — 用戶按下「深度思考」→ `thinkingMode: 'think'` → `rag_query_stream` 傳 `thinking_mode: 'think'`
   - 後端 `is_think: true` → `LLMOptions { think_mode: Some(true) }`
   - Gemma4 → `build_messages` 注入 `<|think|>`
   - 其他模型 → 在 request body 加 `options.think: true` 或標準參數

3. **推理輸出** — `StreamToken::Reasoning` 透過 `rag-stream-reasoning` event 流式推送
   - 前端 `chatStore` 累積為 `reasoningContent`
   - `MessageList` 渲染折疊框，「思考過程」可展開

### 後端實作細節

#### Gemma4Think（Ollama）

**特殊處理：**
- System prompt 開頭無條件加 `<|think|>` token（當 `think_mode=true` 時）
- 建議採樣參數：`temperature=1.0, top_p=0.95, top_k=64`（遵循官方文件）
- `Gemma4ChannelParser` 狀態機掃描 `<|channel>thought\n` 和 `<channel|>` 邊界，抽取中間內容為 reasoning

**格式：**
```
<|channel>thought
[User's internal reasoning here]
<channel|>[Final answer here]
```

#### OpenAiReasoning（o-series）

**特殊參數：**
- `max_completion_tokens`（非 `max_tokens`）
- `reasoning_effort: "high"`
- `developer` role（非 `system`）
- 不支援 `temperature`

**輸出：** `delta.reasoning_content` 與 `delta.content` 分離流式

#### DeepSeekReasoning / Gemini Reasoning

**標準 OpenAI 相容格式，reasoning 在 `delta.reasoning_content`**

---

## 推理能力測試

從 Settings 頁「AI 設定」可測試當前模型連線，未來版本支援推理能力驗證（執行小規模推理任務，驗證輸出格式）。

推理按鈕隱藏 → 檢查 `get_chat_llm_supports_thinking` 回傳值 + 查看 Model Detection 列表
