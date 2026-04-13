# Tagger Conversation Skill — 對話總結深度推斷

## 用途

適用於對話總結（conversation extractor 生成）的深度分析。
對話總結是 `pattern` / `log` 的主要來源，需要更精確的知識類型判斷。

## Prompt 格式

```
Analyze this conversation summary and perform two tasks:

1. Extract 1 to 5 relevant tags (short keywords).
2. Classify the knowledge type using these strict criteria:

   - "pattern": Choose this ONLY if the conversation contains a reusable method,
     workflow, SOP, or approach that was CONFIRMED to work. Look for signals like:
     "this works", "use this approach", "decided to", "confirmed that", "next time do".
     The method must be generalizable beyond this specific conversation.

   - "log": Choose this ONLY if the conversation documents a specific error,
     failure, wrong direction, or lesson learned. Look for signals like:
     "this failed", "don't do this", "mistake was", "wrong approach", "issue was".
     The problem must be concrete and specific, not just a general discussion.

   - "data": Choose this for everything else — research, reference material,
     general discussion, exploration, or anything that doesn't clearly fit
     pattern or log.

   When uncertain, default to "data". Only use "pattern" or "log" when the
   signal is clear and strong.

Output ONLY a valid JSON object with keys "tags" and "knowledge_type".
Example: {"tags": ["rust", "async"], "knowledge_type": "pattern"}
Do not output any other text or markdown.

Conversation Summary:
{content}
```

## 模型選擇建議

| 模型等級 | 建議選擇 | 原因 |
|----------|----------|------|
| 輕量級 | qwen2.5:7b | 邏輯推斷能力足夠，速度快 |
| 中量級 | qwen2.5:14b | 分類更精準，適合複雜對話 |

## 推斷原則

- **保守策略**：不確定時一律 `"data"`，避免誤分類
- **Pattern 信號**：需有明確「這個方法有效」或「下次用這個」的語意
- **Log 信號**：需有明確的失敗事實或教訓，不是泛泛的討論
- **溫度設定**：建議使用低 temperature（0.1）提高一致性

## Pattern / Log Chunk 格式約定

參見 `tagger_skill.md` 中的 Pattern Chunk 與 Log `trigger_context` 規範。
