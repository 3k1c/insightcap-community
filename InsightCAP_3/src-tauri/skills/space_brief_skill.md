# Space Brief Skill

## 用途
根據一個 Space 中的知識片段，自動生成該 Space 的專案簡介（`projectSummary`）。
讓用戶在側欄或 Space 詳情頁快速了解這個知識庫的主題與範疇。

## 模型建議
使用 `contentProcessorLlm`（後台處理模型）。

## Prompt 格式

```
你是一個知識管理助手。以下是一個知識空間（Space）中的部分知識片段，請根據這些內容生成一段簡介。

Space 名稱：{space_name}
片段數量：{chunk_count}
知識片段摘要：
{chunks_summary}

請生成一段 2～4 句的空間簡介，說明：
1. 這個空間主要涵蓋什麼主題
2. 包含哪些類型的知識（概念、方法、案例等）
3. 適合用於什麼場景或目的

語言：繁體中文
格式：純段落文字，不用條列，不用標題
```

## 觸發邏輯
- 由後台任務或使用者手動「重新整理簡介」時觸發
- `chunks_summary`：取該 Space 前 20 個片段的 `clean_content` 前 200 字，換行拼接

## 輸出處理
將回傳文字 trim 後寫入 `spaces.description` 或獨立的 `project_summary` 欄位
