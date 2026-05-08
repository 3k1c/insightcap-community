# InsightCAP Chunking Strategy

更新日期：2026-05-08

本文件記錄目前已落地的 content-aware chunking 與 source grouping 行為。現行實作以 `InsightCAP/src-tauri/src/capture/chunking.rs`、`InsightCAP/src-tauri/src/capture/source_group.rs` 與 migration `016_source_group_ingestion.sql` 為準。

## 目的

Chunking 的目標不是把所有來源切成固定長度文字，而是讓每個 chunk 帶有足夠的內容類型、知識類型與來源上下文，方便後續 RAG、搜尋、引用與除錯。

目前 ingestion 會為每個 chunk 寫入：

- `content_type`
- `knowledge_type`
- `chunk_strategy`
- `chunk_metadata`

這些欄位由 migration `016_source_group_ingestion.sql` 加入 `captures` 表。

## Source Grouping

`source_groups` 用來把同一來源或同一內容歸組，避免同一 URL、影片、檔案或 clipboard 內容在不同 ingestion 路徑下被視為完全無關資料。

目前 identity 規則：

- File：以內容 hash 形成 `file:<hash>`。
- URL：先 canonicalize URL，再形成 `url:<canonical_url>`。
- Video：YouTube watch URL 與 Bilibili video URL 會歸為 `video` 類型。
- Clipboard：使用 normalized title + content hash 形成 identity。

重複來源命中既有 `canonical_key` 時，會更新 `source_groups.title`、`content_hash` 與 `updated_at`，並沿用原 `source_group_id`。

## Chunk Profile

`chunking.rs` 會先根據 `source_type`、`chunk_type` 與內容特徵建立 profile，再選擇切分策略。

| 條件 | content_type | knowledge_type | chunk_strategy |
| --- | --- | --- | --- |
| image / OCR 內容 | `image_ocr` | `data` | `semantic_fallback` |
| log 或看起來像 log | `log` | `log` | `log_event` |
| csv / xlsx 或表格內容 | `table` | `data` | `table_serialization` |
| pptx | `slide` | `data` | `slide_atomic` |
| markdown | `prose` | `data` | `markdown_structure` |
| code 或看起來像 code | `code` | `data` | `syntax_aware` |
| reminder 內容 | `prose` | `data` | `semantic_fallback` |
| pattern 內容 | `prose` | `pattern` | `structure_aware` |
| pdf / docx / rtf / epub / html | `prose` | `data` | `structure_aware` |
| 其他內容 | `prose` | `data` | `semantic_fallback` |

注意：目前 `syntax_aware` 會落到預設 semantic window split，尚未實作 AST 或語法樹級切分。

## Chunk Size

目前核心參數：

- `MAX_CHUNK_CHARS = 2400`
- `OVERLAP_CHARS = 240`
- `MAX_TABLE_CHUNK_CHARS = 2200`

一般文字會以段落累積到上限附近，跨 chunk 時保留尾端 overlap。硬切分仍會遵守 `MAX_CHUNK_CHARS`，避免單一 chunk 過大。

表格 chunk 會預留約 200 字元空間給 header re-injection，因此使用 `MAX_TABLE_CHUNK_CHARS`。

## Splitting Rules

- `log_event`：按 log event 邊界累積，過長時再硬切。
- `table_serialization`：以表格行與 header 為核心，避免寬表超出上限。
- `markdown_structure` / `structure_aware`：優先依 heading 切分。
- `slide_atomic`：每個 slide 儘量保持 atomic；超長 slide 再使用語意窗口。
- `semantic_fallback`：依段落與字元上限切分，必要時加入 overlap。

OCR 失敗內容會直接跳過 indexing，避免把無語意的錯誤輸出寫入知識庫。

## Metadata

每個 chunk 會產生 JSON metadata，至少包含來源 metadata、split index、split count、字元數與 overlap 狀態。這些 metadata 會寫入 `captures.chunk_metadata`，供後續引用、除錯與 RAG 調權使用。

## 維護原則

- 修改 chunk strategy 前，先更新 `chunking.rs` 測試，確認 chunk 大小與 profile 判斷仍正確。
- 新增 `content_type`、`knowledge_type` 或 `chunk_strategy` 字串時，要同步檢查 migration、RAG 查詢與前端顯示是否有 hardcoded enum 假設。
- Source grouping 規則變更會影響既有資料歸併語意；Beta 階段可以做破壞性調整，但必須在 changelog 記錄。
- 文件只描述已落地行為；未實作的策略不要寫成現行能力。
