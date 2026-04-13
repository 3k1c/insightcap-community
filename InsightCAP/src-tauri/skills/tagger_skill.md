# Tagger Skill — 標籤提取規範

## 用途

本規範定義 InsightCAP 在背景處理內容時，呼叫 LLM 進行標籤（Keywords）提取的 Prompt 格式與輸出期望。
適用場景：
- 內容入庫（`process_inbox_batch`）
- 對話總結後的自動標註（`extractor.rs`）

## Prompt 格式

```
Analyze the following text and perform two tasks:
1. Extract 1 to 5 relevant tags (short keywords).
2. Classify the knowledge type into one of these categories:
   - 'data': General information, project-specific data, reference links, or facts.
   - 'pattern': Reusable frameworks, structures, SOPs, methodologies, or design patterns.
   - 'log': Errors, warnings, bug fixes, lessons learned, or technical pitfalls.

Output ONLY a valid JSON object with the keys "tags" and "knowledge_type". 
Example: {"tags": ["rust", "memory"], "knowledge_type": "pattern"}
Do not output any other text or markdown.

Text:
{content}
```

## 模型選擇建議

| 模型等級 | 建議選擇 | 原因 |
|----------|----------|------|
| 輕量級 (跑 CPU) | qwen2.5:3b / 7b | 速度快，邏輯跟隨能力強 |
| 中量級 (跑 VRAM) | qwen2.5:7b | 分類精準，適合多語言內容 |
| 其他選項 | llama3.2:3b | 適合英文內容標註 |

## 輸出解析邏輯 (ADR-021)

系統應實施多層容錯解析機制：

1. **標準解析**：嘗試將整段回傳文字作為 JSON 陣列解析。
2. **區塊提取**：若標準解析失敗，使用正則表達式尋找 `[...]` 區塊並嘗試解析。
3. **物件映射**：若回傳為 `{"tag": ["..."]}` 格式，應自動映射至內容陣列。
4. **清理規範**：
   - 移除空字串。
   - 所有標籤轉為小寫（Normalization）。
   - 去除首尾空白位。

## Pattern Chunk 約定格式

當 `knowledge_type = 'pattern'` 時，chunk 的內容應遵循以下結構（純文字，無需更改 schema）：

```
## 適用場景
[什麼情況觸發這個 Pattern，例如：「需要快速評估競品定價時」]

## 執行步驟
1. [步驟一]
2. [步驟二]

## 已知風險
[關聯的 Log ID 或描述，例如：「參見 Log#42：直接爬取競品網站可能違反 ToS」]
```

**目的**：讓 RAG 在取回 Pattern 時，能同步帶出已知風險提醒，形成「Pattern → 主動警告 Log」的關聯鏈。

## Log Chunk trigger_context 欄位

當 `knowledge_type = 'log'` 時，應同時填入 `trigger_context` 欄位：

- **格式**：以 `|` 分隔的場景描述，例如：`競品分析 | 收集定價資料 | 爬蟲`
  - 每段可以是短語，不需要是單一詞（中文整句直接做 substring match）
  - 勿使用逗號，避免與 CSV 格式混淆
- **用途**：RAG pipeline 在語意搜索之外，額外對 `trigger_context` 做 substring match，主動將相關 Log 警告推送給使用者
- **match 邏輯**：`trigger_context` 按 `|` 切分後，檢查每段是否為 user query 的 substring（或反向 substring）
- **填入時機**：入庫時由使用者手動補充，或由 LLM 根據 chunk 內容自動推斷

## 失敗與降級處理

當 LLM 無法回傳效標籤時（timeout、模型錯誤）：
- **不阻塞**：背景處理器應跳過標籤步驟，繼續儲存內容。
- **後設標註**：可標記為 `["untagged"]` 供後續補強。
- **超時限制**：設定 15 秒硬性超時（Hard Timeout）。
