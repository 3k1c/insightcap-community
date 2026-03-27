# Pattern Extractor Skill

## 用途
從文章、書籍摘要、技術文件中萃取可重複使用的「思維框架」或「方法論」，
以結構化格式儲存，方便日後在對話中直接引用或套用。

## 模型建議
使用 `contentProcessorLlm`（後台處理模型），適合深度分析任務。

## Prompt 格式

```
你是一個知識萃取專家，擅長從內容中找出可複用的思維框架與方法論。

請分析以下內容，找出其中包含的核心框架或方法（若無則回傳空陣列）：

內容：
{content}

請以 JSON 陣列格式輸出，每個框架包含以下欄位：
- name: 框架名稱（10字以內）
- category: 分類（從以下選擇：決策框架、問題分析、執行方法、溝通技巧、學習策略、其他）
- summary: 一句話說明（30字以內）
- steps: 執行步驟陣列（每步15字以內，2～6步）
- use_when: 適用場景（20字以內）

輸出格式（只輸出 JSON，不加說明）：
[
  {
    "name": "框架名稱",
    "category": "決策框架",
    "summary": "用於...",
    "steps": ["步驟一", "步驟二", "步驟三"],
    "use_when": "當你需要..."
  }
]
```

## 觸發邏輯
- 知識片段的 `knowledge_type` 為 `framework`、`methodology` 或 `concept` 時觸發
- 也可在使用者手動標記「萃取框架」時觸發
- `content` 取 chunk 的 `clean_content`（最多 2000 字元）

## 輸出處理
解析 JSON 後，每個框架儲存為獨立知識片段，`knowledge_type` 設為 `framework`，
`tags` 包含 `category` 值與 `pattern` 標籤。

## 錯誤處理
若 JSON 解析失敗，記錄 warning 並略過，不影響原始片段的儲存。
