# InsightCAP — 待開發功能清單
> 按開發難度排列（低 → 高）
> 基準版本：Architecture-v2.8 | 日期：2026-04

---

## 難度定義

| 等級 | 定義 |
|------|------|
| ⭐ 低 | 純前端或純 prompt 改動，不改 schema，1-3 天 |
| ⭐⭐ 中 | 後端新增欄位或輕量服務，不破壞現有邏輯，3-7 天 |
| ⭐⭐⭐ 高 | 新增獨立模組或重構現有服務，7-14 天 |
| ⭐⭐⭐⭐ 極高 | 架構級改動或全新子系統，14 天以上 |

---

## ⭐ 低難度

### 1. Space 知識可用性視圖
**目標：** 讓用戶打開 Space 時，看到的不是 chunk 列表，而是「我在這個領域能做什麼」。

**呈現方式：**
```
「活動策劃」Space

你現在可以直接處理：
  ✅ 場地選擇（8 個 Pattern）
  ✅ 流程設計（12 個 Pattern）

需要小心的：
  ⚠️  嘉賓突發缺席（1 個 Log）

還沒有經驗的：
  ❌ 直播技術
  ❌ 超過 500 人規模
```

**技術實作：**
- 純前端統計現有 `memory_chunks` 的 `knowledge_type` + `tags` 分佈
- 呼叫一次 LLM 分析 chunk 標籤群，識別「空白區域」
- 不改任何 schema，不改任何後端邏輯

**影響檔案：** `SpacePage.tsx`（或新增 `SpaceInsightPanel.tsx`）

---

### 2. RAG 內容來源分層（placed_by 權重）
**目標：** 一手資料（用戶擷取）優先於 AI 生成內容（對話摘要）。

**技術實作：**
- `captures.is_user_edited = 1` 或 `memory_chunks.placed_by = 'user'` → RAG 分數 +0.05
- `memory_chunks.placed_by = 'ai'` → RAG 分數不變（現有邏輯）
- 改動點：`rag_commands.rs` 加一條加權規則

**影響檔案：** `src-tauri/src/commands/rag_commands.rs`

---

### 3. ContextHintBanner 強化
**目標：** 對話開始時的 Banner 現在只顯示數量，改為顯示具體內容預覽。

**現況：**
```
◆ 發現可複用方法  ▲ 2 條風險記錄  ● 5 個相關來源
```

**改後：**
```
◆ 場地選擇標準框架（Pattern）
⚠️  嘉賓確認需雙重核實（Log）— 點擊展開
```

**影響檔案：** `ContextHintBanner.tsx`、`rag_commands.rs`（回傳 title 欄位）

---

### 4. Project 模板
**目標：** 新建 Project 時，預載該類型的標準 Pattern 和 Log，讓新用戶第一天就有東西可用。

**模板類型（內建）：**
- 活動策劃
- 研究報告
- 客戶提案
- 產品開發

**技術實作：**
- 新建 `project_templates/` 目錄，每個模板是一個 JSON 檔案
- 包含預設 `memory_chunks`（knowledge_type = pattern / log）
- 新建 Project 時選擇模板，後端批量寫入 memory_chunks
- 不改 schema

**影響檔案：** `project_commands.rs`、`ProjectCreateModal.tsx`、新增 `src-tauri/templates/`

---

### 5. 對話完成儀式感
**目標：** Project 標記為完成時，顯示「這個項目你學到了什麼」的總結頁。

**內容：**
```
項目完成：周年晚宴 2026

本次新增：
  3 個可複用 Pattern
  1 個踩坑記錄

與上次同類項目比較：
  效率提升：準備時間縮短（AI 估算）
  新學到：危機處理框架
```

**技術實作：**
- 純前端統計 `memory_chunks` 在這個 project_id 下的數量變化
- 呼叫一次 LLM 生成自然語言總結
- 不改 schema

**影響檔案：** `ProjectCompletionModal.tsx`（新增）

---

## ⭐⭐ 中難度

### 6. Decision 層（ADR-029）
**目標：** 靜默記錄框架外變數的決策，事後回顧決策品質。

**技術實作：**
- 新增 `decisions` 表（獨立，不改現有表）
- `chat_core_skill` 新增框架外變數識別邏輯
- `tagger_skill` 新增決策結果信號識別
- `DecisionScheduler`：14 天後主動詢問（路徑 A）
- 從後續對話自動萃取結果（路徑 B）
- 前端：輕量詢問 UI（Toast / Banner）

**新增 Schema：**
```sql
CREATE TABLE decisions (
  id              TEXT PRIMARY KEY,
  project_id      TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  variable_desc   TEXT NOT NULL,
  options         TEXT NOT NULL,       -- JSON
  chosen_option   TEXT NOT NULL,
  outcome_source  TEXT,                -- 'conversation' | 'user_report'
  outcome_rating  TEXT,                -- 'good'|'ok'|'bad'|'critical'
  outcome_note    TEXT,
  status          TEXT DEFAULT 'pending',
  trigger_at      TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
```

**影響檔案：** 新增 `decision_commands.rs`、`DecisionScheduler`、`DecisionToast.tsx`

**依賴：** ADR-029 已設計完成，可直接實作

---

### 7. PatternPromotion 路徑 B 完整實作
**目標：** 跨對話自動識別重複模式，推送升格建議。

**現況：** Architecture-v2 已定義邏輯，但標注為 Stub。

**技術實作：**
- `PatternPromotion` 後台服務完整實作
- 條件：標籤重疊 ≥ 2 + 向量相似度 ≥ 0.65 + 出現在 ≥ 3 個不同對話
- 推送非阻塞 toast，用戶確認後更新 `knowledge_type`
- `Log` 的 `trigger_context` 自動擴展覆蓋範圍

**影響檔案：** `src-tauri/src/services/pattern_promotion.rs`、前端 Toast 元件

---

### 8. ConversationScheduler 完整實作
**目標：** 對話切換時自動生成摘要並寫入 memory_chunks。

**現況：** Architecture-v2 標注為 Stub。

**技術實作：**
- `conversation_summary_queue` 表的消費邏輯
- 每 30 秒輪詢，LLM 生成摘要
- Tagger 深度推斷 `knowledge_type`
- 信心度 < 0.75 → 推送用戶確認 toast
- 摘要寫入 `conversations.summary`（供後續對話歷史注入）

**影響檔案：** `src-tauri/src/services/conversation_scheduler.rs`

---

### 9. MigratePage（目錄遷移解鎖）
**目標：** 用戶直接複製 KB 目錄到新機器，能用密碼或恢復碼解鎖。

**現況：** Architecture-v2.5（2026-04-03）已定義，未實作前端。

**技術實作：**
- `get_auth_status` 偵測 `isMigrated` 條件（Keychain 空 + auth.json 存在 + DB 存在）
- 前端渲染 `MigratePage`（密碼 / 恢復碼兩個路徑）
- 後端 `unlock_migrated_with_password` 和 `unlock_migrated_with_mnemonic` commands
- 成功後寫 Keychain，呼叫 `initApp()`

**影響檔案：** `MigratePage.tsx`（新增）、`auth_commands.rs`（新增兩個 command）、`get_auth_status` 邏輯修改

---

### 10. SpaceRecluster 完整實作
**目標：** 新 Space 建立時，重新計算所有 chunk 的 Space 歸屬。

**現況：** Architecture-v2 標注為 Stub。

**技術實作：**
- 每次新增 Space 時觸發後台重聚類
- 更新 `captures.space_id` / `memory_chunks.space_id`
- 需要處理大量 chunk 的效能問題（批次處理，避免阻塞）

**影響檔案：** `src-tauri/src/services/space_recluster.rs`

---

### 11. 反向鏈接（Chunk 關聯關係）
**目標：** 顯式記錄 chunk 之間的引用關係，讓 RAG 能跨文件串聯資訊。

**新增 Schema：**
```sql
CREATE TABLE chunk_relations (
  id          TEXT PRIMARY KEY,
  from_id     TEXT NOT NULL,   -- capture_id 或 memory_chunk_id
  to_id       TEXT NOT NULL,
  from_type   TEXT NOT NULL,   -- 'capture' | 'memory_chunk'
  to_type     TEXT NOT NULL,
  relation    TEXT NOT NULL,   -- 'references' | 'contradicts' | 'extends'
  confidence  REAL DEFAULT 1.0,
  created_at  TEXT NOT NULL
);
```

**技術實作：**
- 新 chunk 入庫時，LLM 分析與現有 chunk 的關係
- RAG 查詢時，找到相關 chunk 後，額外拉取其反向鏈接的 chunk
- 前端在引用預覽中顯示關聯文件

**影響檔案：** 新增 `chunk_relation_commands.rs`、`rag_commands.rs` 修改、引用預覽 UI

---

## ⭐⭐⭐ 高難度

### 12. Space Wiki 層（知識可用性視圖進階版）
**目標：** 每個 Space 有一份 AI 持續維護的結構化知識文件，讓用戶看到「我在這個領域知道什麼」。

**設計：**
```
「活動策劃」Wiki（AI 自動維護，用戶可編輯）

## 核心框架
  標準流程：選場地 → 找嘉賓 → 老闆講話 → 遊戲 → 吃飯 → 抽獎

## 已掌握方法
  場地選擇框架（3 次驗證）
  嘉賓確認流程（雙重核實機制）

## 已知風險
  ⚠️  場地需提前 3 個月確認
  ⚠️  嘉賓確認需雙重核實

## 知識空白
  直播技術、超過 500 人規模活動
```

**技術實作：**
- `spaces` 表新增 `wiki_content` 欄位（Markdown）
- 新增 `SpaceWikiEngine` 服務：每次新 memory_chunk 寫入 Space 時觸發更新
- LLM 重新生成 wiki（非全量，增量更新）
- 前端 Space 頁面新增 Wiki 分頁（Tiptap 渲染，支援用戶手動編輯）

**影響檔案：** `spaces` 表 migration、新增 `SpaceWikiEngine`、`SpacePage.tsx` 大改

---

### 13. 手機版擷取（Phase 6）
**目標：** 手機端能將資料擷取到桌面端知識庫。

**技術實作：**
- HTTP API Server（Axum，127.0.0.1:3030）骨架已完成
- React Native 應用（或 PWA）
- 擷取寫入 inbox → 桌面端 CaptureProcessor 處理
- 需要 WiFi 同網段，或透過雲端同步

**影響檔案：** `src-tauri/src/services/http_api.rs`（擴充）、全新 React Native 專案

---

### 14. 企業版 Knowledge Builder
**目標：** 管理員工具，將原始文件處理成標準格式外部 KB。

**技術實作：**
- 獨立 Tauri 應用程式
- 使用領域專屬 Embedding 模型（legal-bert、FinBERT，768 維）
- 輸出標準 `.db` + 向量索引，含 `kb_metadata` 表
- InsightCAP Enterprise 透過 `KnowledgeSource` trait 載入

**影響檔案：** 全新獨立應用，InsightCAP 端只需確保 `external_knowledge_bases` 表兼容

---

## ⭐⭐⭐⭐ 極高難度

### 15. 雲端同步與多設備（Phase 5）
**目標：** 多設備間知識庫同步，不依賴第三方雲端服務。

**現況：** 透過系統雲端磁碟共享 kb_path，CloudSyncWatcher 已有基礎。

**挑戰：**
- SQLite WAL 模式在多設備同時寫入時的衝突處理
- 向量索引的同步策略（體積大，不適合直接同步）
- 加密 DB 在雲端服務商端的隱私保護

**技術實作：**
- 完整實作 `CloudSyncWatcher`（目前部分實作）
- 合併衝突策略設計
- 向量索引的增量同步

---

## 優先開發建議

```
立即（本週）：
  #1 Space 知識可用性視圖
  #4 Project 模板
  #9 MigratePage

短期（本月）：
  #6 Decision 層
  #7 PatternPromotion 路徑 B
  #8 ConversationScheduler

中期（有真實用戶後）：
  #3 ContextHintBanner 強化
  #11 反向鏈接
  #12 Space Wiki 層

長期（商業化後）：
  #13 手機版
  #14 Knowledge Builder
  #15 雲端同步
```

---

*文件日期：2026-04 | 對應架構：Architecture-v2.8*
