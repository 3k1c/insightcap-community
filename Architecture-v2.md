# InsightCAP — 架構文件

> 本文件是開發的唯一架構依據。

---

## 核心定義

InsightCAP 是**經驗調用系統**。

知識管理系統讓你找到資料；經驗調用系統在你需要的時候，主動把正確的過去經驗帶進當前工作。

核心承諾：**你做過的每個項目，都會讓下一個項目更容易完成。**

---

## 設計原則

- **本地優先**：Embedding、向量搜尋、LLM 推斷均在本機運行，雲端為可選項
- **記憶有層次**：三種記憶類型（Data / Pattern / Log）有不同的生命週期、召回策略、AI 角色
- **用戶看來源，系統看 chunk**：用戶介面以原文件/網址/圖片為單位，chunk 是純後台概念
- **Space 是後台聚類，不是前台容器**：AI 自動維護，用戶可參考但不需要管理
- **哲學貫穿實現**：每個設計決策都能追溯到記憶理論
- **個人版/商業版同一技術棧，不同知識源接口**：通過 `KnowledgeSource` trait 分離
- **UI 有設計語言**：所有介面元素來自統一 Design Token，不允許樣式寫死
- **全介面多語言**：所有文字通過 i18n，不允許寫死任何語言字串
- **DB 操作安全第一**：所有危險操作完成後強制重啟，不在運行時切換狀態

---

## 產品形態

### InsightCAP Personal（個人版）
- 單用戶，本地優先
- 知識源：sources + captures + memory_chunks
- 向量索引：通用 MultilingualE5Small（384 維）
- 登入：本地密碼 + Argon2id + Keychain

### InsightCAP Enterprise（商業版）
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

---

## 系統架構

```
┌────────────────────────────────────────────────────────────┐
│                   Tauri 2.0 Application                    │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                  React Frontend                       │  │
│  │  IC Design System（四主題 Token，無寫死樣式）          │  │
│  │  i18n（react-i18next，繁中/簡中/英文）                 │  │
│  │                                                      │  │
│  │  Chat Page + Editor  |  知識庫頁  |  Settings Page   │  │
│  │         ↕ Zustand（統一狀態管理）                    │  │
│  └───────────────────────┬──────────────────────────────┘  │
│                          │ Tauri IPC (invoke)              │
│  ┌───────────────────────▼──────────────────────────────┐  │
│  │                  Rust Backend                         │  │
│  │                                                      │  │
│  │  Command Layer（IPC 邊界，只做參數驗證和服務調用）     │  │
│  │                         │                            │  │
│  │  Core Services                                       │  │
│  │  CaptureEngine  ConversationEngine  MemoryEngine     │  │
│  │  RAGEngine      PatternEngine       SpaceEngine      │  │
│  │  TagEngine      AuthService                          │  │
│  │                         │                            │  │
│  │  Abstraction Layer                                   │  │
│  │  KnowledgeSource  LLMProvider  Embedder              │  │
│  │                         │                            │  │
│  │  Data Layer                                          │  │
│  │  SQLite（sources + captures + memory_chunks          │  │
│  │          + spaces + tags + projects + ...）          │  │
│  │  usearch（主索引 + 外部 KB 索引）                    │  │
│  │                                                      │  │
│  │  Background Services                                 │  │
│  │  CaptureProcessor  ConversationScheduler             │  │
│  │  PatternPromotion  SpaceRecluster                    │  │
│  │  OCRWorker         CloudSyncWatcher                  │  │
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
對話結束 / 切換
    ↓
ConversationScheduler 生成摘要
    ↓
Tagger 推斷 knowledge_type（對話總結走深度推斷路徑）
    信心度 >= 0.75 → 直接寫入 memory_chunks
    信心度 < 0.75  → 寫入 memory_chunks（pending_confirm = 1）
                     推送用戶確認 toast（非阻塞）
                     確認 → 更新 knowledge_type
                     忽略 → 保持 data
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

三種類型在 system prompt 裡有不同的語意角色，不是拍平列表：

```
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

用戶問題：{{ user_query }}
```

---

## Space 設計

Space 是**後台 AI 聚類概念**，不是用戶管理的容器。

- 由 AI 自動生成名稱和聚類內容，用戶可修正名稱
- 每增加一個新 Space，SpaceRecluster 重新計算所有 chunk 相似度，動態重新聚合
- 用戶不需要手動管理 chunk 屬於哪個 Space
- 前台只作為篩選工具，顯示在知識庫頁左側列表
- 不在 Project 裡明確綁定，不作為 @ 引用的對象，不作為 RAG 的硬邊界

---

## 標籤系統

**兩種來源：**
- AI 自動生成：內容入庫時 Tagger 提取，寫入 tags 表
- 用戶手動加入：對話輸入框輸入 `#標籤`，或在知識庫頁手動編輯

**對話時的知識範圍控制：**

```
對話預設開啟知識庫
    ↓
自動推薦近期高頻標籤（近期收集文件出現頻率最高的標籤）
顯示在輸入框上方作為 chip
    ↓
用戶點擊 chip 切換選中/取消
用戶輸入 #標籤 動態加入篩選範圍
    ↓
系統根據選中標籤召回相近知識
```

---

## 知識庫頁 UX

### 核心原則

用戶看到的是原文件/來源（source），不是 chunk。chunk 是後台計算單位，不在介面中顯示。

### 頁面結構

```
┌─────────────────────────────────────────────────────┐
│  🔍 搜尋...                                          │
├───────────────┬─────────────────────────────────────┤
│ Space 篩選    │ 雙行排序                             │
│               │                                     │
│ 全部          │ 最近擷取                             │
│               │  📄 活動計劃書.md   2天前  3段      │
│ AI 聚類       │  🌐 example.com    5天前  8段      │
│ 周年晚宴  12  │  🖼️ 截圖_0301     1週前  1段      │
│ 競品研究   8  │                                     │
│ 客戶 A     5  │ 常用參考                             │
│               │  🌐 競品分析網站   頻率 ★★★★       │
│               │  📄 SOP 手冊.pdf  頻率 ★★★        │
└───────────────┴─────────────────────────────────────┘
```

點擊來源卡片 → 展開顯示完整原文 + 該來源所有片段內容

**快速擷取的來源顯示：**
- 截圖：「截圖 YYYY-MM-DD HH:mm」+ 縮圖
- 剪貼簿：來源應用名稱或「剪貼簿擷取」
- 快速輸入：「手動輸入」

---

## 對話頁 UX

### 輸入框

```
┌──────────────────────────────────────────────────┐
│ 知識範圍：[周年晚宴 ×] [場地 ×] [+ 更多]         │
├──────────────────────────────────────────────────┤
│ 輸入問題... #標籤 @來源                  [傳送]  │
├──────────────────────────────────────────────────┤
│ [知識庫 ●] [聯網搜尋]                             │
└──────────────────────────────────────────────────┘
```

### @ 引用

- `@` 觸發引用選單，顯示原文件層面的對象（文件名/網址/圖片）
- 選中後系統自動把該 source 所有相關 chunk 納入 RAG context
- 用戶看到「@活動計劃書.md」，不看到 chunk

### ContextHintBanner

對話開始時若有相關知識，從頂部滑入（非阻塞）：

```
◆ 發現可複用方法  ▲ 2 條風險記錄  ● 5 個相關來源
```

---

## 文本編輯器

**定位：** Chat Page 側邊 Panel，雙窗佈局，不作為獨立頁面。

**技術：** Tiptap（ProseMirror），跟隨 IC Design System 主題。

**自動入庫機制（匯出觸發）：**

```
用戶匯出文件
    ↓
以文件標題寫入 sources 表（type = 'editor'）
    ↓
按 Tiptap JSON 中的標題節點（H1/H2/H3）分段
    ↓
每段作為獨立 capture，source_id 指向同一份 source
    ↓
Embedding → usearch 索引
    ↓
若相同 content_hash 的 source 已存在 → 覆蓋（刪舊 captures 重建）
```

- 不在 autosave 時入庫（草稿不入庫）
- 匯出 = 用戶認為文件足夠好的時刻
- 工具列 / 右鍵選單跟隨 IC Design System 重新設計

---

## 資料庫安全設計

### 文件存放原則

```
app_data_dir/（本地，不受雲端同步影響）
├── bootstrap.json     ← 工作區路徑指針，永遠在本地
└── db_state.json      ← DB 操作狀態記錄

kb_path/（可能在雲端同步目錄）
└── .insightcap/
    ├── insightcap.db
    ├── insightcap.db-wal
    ├── insightcap.db-shm
    └── vectors/
```

`bootstrap.json` 必須存在 app_data_dir，不能放在 kb_path，防止雲端同步在重啟瞬間覆蓋路徑指針。

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

5. 用 db_key 打開 DB（PRAGMA key）
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

### bootstrap.json 原子寫入

```rust
// 正確做法：先寫臨時文件，再原子重命名
let tmp_path = bootstrap_path.with_extension("tmp");
std::fs::write(&tmp_path, &json)?;
let file = std::fs::File::open(&tmp_path)?;
file.sync_all()?;  // 確保 flush 到磁碟
std::fs::rename(&tmp_path, &bootstrap_path)?;  // 原子操作
```

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
  id            TEXT PRIMARY KEY,
  type          TEXT NOT NULL,
  -- file | url | image | editor | clipboard | screenshot
  title         TEXT NOT NULL,
  url           TEXT,
  file_path     TEXT,
  thumbnail     TEXT,
  clean_content TEXT NOT NULL DEFAULT '',
  content_hash  TEXT,
  capture_count INTEGER DEFAULT 0,
  use_frequency INTEGER DEFAULT 0,
  captured_at   TEXT NOT NULL,
  updated_at    TEXT NOT NULL
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
  image_path       TEXT,
  capture_method   TEXT NOT NULL,
  -- hotkey | import | mobile | editor_export | url
  tags             TEXT DEFAULT '[]',
  chunk_index      INTEGER DEFAULT 0,
  vector_id        INTEGER,
  status           TEXT DEFAULT 'inbox',
  -- inbox | processed | archived | pending_ocr
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
  placed_by        TEXT DEFAULT 'ai',
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

### 資料流向

```
用戶擷取
  → inbox → CaptureProcessor
  → 寫入 sources（來源記錄）
  → 寫入 captures（chunk，固定為 data）
  → Tagger 提取標籤 → tags 表
  → Embedding → usearch
  → SpaceEngine 更新聚類

編輯器匯出
  → 寫入 sources（type = 'editor'）
  → 按標題節點分段 → 多個 captures（同一 source_id）
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

```rust
pub trait LLMProvider: Send + Sync {
    async fn complete(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<String, LLMError>;

    async fn complete_json(
        &self,
        prompt: &str,
        options: LLMOptions,
    ) -> Result<serde_json::Value, LLMError>;
}

pub struct LLMOptions {
    pub temperature: f32,
    pub max_tokens: usize,
    pub stream: bool,
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
```

**第二階段：組裝分層 Context**

按語意角色分組（pattern / log / data / external），注入 system prompt（見三層記憶理論章節）。
在 Phase 5 企業版架構中，RAG 引擎的 `retrieve_context` 將另外查詢狀態為 `connected` 的所有外部 SQLite 資料庫（掛載於 `external_knowledge_bases`），動態獲取其 `captures` 表中相關的知識片段，前綴加上 `[外部知識庫]` 並與本地結果一同交給 LLM 推理。

---

## Core Services

| 服務 | 職責 |
|------|------|
| CaptureEngine | 擷取、清洗、OCR、寫入 sources + captures |
| ConversationEngine | 對話管理、RAG 組裝、訊息儲存 |
| MemoryEngine | memory_chunks CRUD、tagger、pending_confirm 流程 |
| RAGEngine | 統一召回，調用 KnowledgeSource trait，組裝分層 context |
| PatternEngine | 路徑 B 跨對話識別，升格建議 |
| SpaceEngine | AI 聚類管理，維護 embedding_center |
| TagEngine | 標籤 CRUD、頻率統計、推薦 |
| AuthService | 認證、加密、健康檢查 |

## Background Services

| 服務 | 職責 | 觸發方式 |
|------|------|---------|
| CaptureProcessor | inbox → sources + captures + Embedding | inbox 有新項目 |
| ConversationScheduler | 對話切換/關閉時生成摘要 | 對話狀態變化 |
| PatternPromotion | 掃描新 memory_chunk，判斷升格 | memory_chunk 寫入後 |
| SpaceRecluster | 重新計算聚類 | 新 Space 建立後 |
| OCRWorker | 大型 PDF 背景 OCR | pending_ocr 狀態 |
| CloudSyncWatcher | 偵測外部磁碟同步 | 30 秒輪詢 |

---

## Design System

### 四主題

| 主題 class | 名稱 | 底色 | 強調色 |
|-----------|------|------|--------|
| `.theme-light` | 淺色系 | `#FFFFFF` | `#2563EB` |
| `.theme-dark` | 深黑系 | `#0F1117` | `#7C6FF7` |
| `.theme-casual` | 休閒系 | `#FAF6EF` | `#C2722A` |
| `.theme-fresh` | 清新系 | `#F0F5F1` | `#1A7F5A` |

預設：淺色系。用戶手動選擇，不跟系統。啟動時最早套用防止 FWOT。儲存在 settings 表。

### IC Design Token

```css
/* 背景 */
--ic-bg-base        --ic-bg-surface
--ic-bg-elevated    --ic-bg-sunken

/* 文字 */
--ic-text-primary   --ic-text-secondary
--ic-text-muted     --ic-text-inverse

/* 強調 */
--ic-accent         --ic-accent-hover    --ic-accent-subtle

/* 邊框 */
--ic-border-default --ic-border-strong   --ic-border-focus

/* 三層記憶類型（跨主題語意一致）*/
--ic-memory-data         --ic-memory-data-bg      --ic-memory-data-text
--ic-memory-pattern      --ic-memory-pattern-bg   --ic-memory-pattern-text
--ic-memory-log          --ic-memory-log-bg       --ic-memory-log-text

/* 固定值 */
--ic-space-1: 4px;   --ic-space-2: 8px;   --ic-space-3: 12px;
--ic-space-4: 16px;  --ic-space-6: 24px;  --ic-space-8: 32px;
--ic-radius-sm: 4px; --ic-radius-md: 8px; --ic-radius-lg: 12px;
--ic-radius-full: 9999px;
```

### 三層記憶視覺語言（全系統統一）

| 類型 | 符號 | Token |
|------|------|-------|
| `data` | ● | `--ic-memory-data` |
| `pattern` | ◆ | `--ic-memory-pattern` |
| `log` | ▲ | `--ic-memory-log` |

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

---

## 模組結構

### 前端

```
src/
├── design-system/
│   ├── tokens.css          # IC Design Token（四主題完整定義）
│   └── index.css
├── i18n/
│   ├── index.ts
│   ├── types.ts
│   └── locales/
├── components/
│   ├── ui/                 # 基礎元件庫
│   ├── memory/             # ContextHintBanner、CitationBadge
│   ├── chat/
│   ├── knowledge/          # 來源列表、Space 篩選
│   ├── editor/             # Tiptap
│   └── settings/
├── stores/
│   ├── themeStore.ts
│   ├── languageStore.ts
│   ├── chatStore.ts
│   ├── knowledgeStore.ts
│   └── tagStore.ts
├── pages/
│   ├── ChatPage.tsx
│   ├── KnowledgePage.tsx
│   └── SettingsPage.tsx
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
│   ├── tag_commands.rs
│   ├── rag_commands.rs
│   ├── auth_commands.rs
│   └── settings_commands.rs
├── services/
│   ├── capture_engine.rs
│   ├── conversation_engine.rs
│   ├── memory_engine.rs
│   ├── rag_engine.rs
│   ├── pattern_engine.rs
│   ├── space_engine.rs
│   ├── tag_engine.rs
│   └── auth_service.rs
├── knowledge_source/
│   ├── mod.rs              # KnowledgeSource trait
│   ├── personal.rs
│   └── enterprise.rs
├── providers/
│   ├── llm/
│   │   ├── mod.rs          # LLMProvider trait
│   │   ├── ollama.rs
│   │   └── openai.rs
│   └── embedding/
│       ├── mod.rs          # Embedder trait
│       └── fastembed.rs
├── background/
│   ├── capture_processor.rs
│   ├── conversation_scheduler.rs
│   ├── pattern_promotion.rs
│   ├── space_recluster.rs
│   ├── ocr_worker.rs
│   └── cloud_sync_watcher.rs
├── db/
│   ├── connection.rs       # DB 連接、健康檢查、原子寫入
│   └── migrations/
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

**Phase 4：知識庫頁 UX**
17. 來源視圖（以 sources 為單位）
18. Space 篩選側邊欄
19. @ 引用（原文件層面）
20. 標籤推薦 + `#` 輸入

**Phase 5：商業版**
21. KnowledgeSource Enterprise 實現
22. 外部 KB 加載
23. Knowledge Builder（獨立應用）

**Phase 6：手機版**
24. 本地 HTTP API（axum）
25. React Native 應用

---

*版本：v2.2 | 日期：2026-03-28*
（已更新 Phase 5：商業版 KnowledgeSource Trait 實作與外部索引掛載與混合 RAG 機制）
