驗證 InsightCAP 三層記憶系統的實作是否符合 Architecture-v2.md 定義的流程。

步驟：
1. 讀取 InsightCAP_3/src-tauri/src/background/conversation_scheduler.rs
2. 讀取 InsightCAP_3/src-tauri/src/background/pattern_promotion.rs
3. 讀取 InsightCAP_3/src-tauri/src/prompts.rs

驗證項目：

**路徑 A（單次對話識別）：**
- ConversationScheduler 對話結束後是否呼叫 Tagger 推斷 knowledge_type
- 信心度 >= 0.75 直接寫入 memory_chunks
- 信心度 < 0.75 寫入並標記 pending_confirm = 1，推送 toast

**路徑 B（跨對話積累識別）：**
- PatternPromotion 三個條件是否完整實現：
  - 標籤重疊 >= 2 個
  - 向量相似度 >= 0.65
  - 出現在 >= 3 個不同對話

**Prompt 管理：**
- prompts.rs 是否包含 pattern / log / data / external 四段 context 常數
- 確認無 prompt 字串散落在其他 service 檔案

輸出：符合 ✓ / 不符合 ✗ 清單，不符合項目附上具體差異說明
