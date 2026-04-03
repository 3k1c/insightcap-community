# Editor 匯出與知識庫同步流程（2026/04 更新）

## 目的
確保使用者在匯出編輯器內容時，能同時：
1. 寫入指定本地檔案
2. 自動將快照保存至 `.insightcap/documents` 目錄
3. 觸發知識庫分塊、標籤、分類等 pipeline

---

## 原則
- 匯出後，所有內容都會進入知識庫（sources/captures），且有一份 markdown 快照保存在 app 內部 documents 目錄，方便後續檢索與還原。
- `local_doc_path` 欄位會同步更新，指向最新快照路徑。

---

## 具體實現

### 1. 前端
- 匯出時呼叫 `tauriCmd.saveEditorToKnowledge(title, md)`，不需傳路徑，Rust 端自動處理快照與 pipeline。

### 2. Rust backend
- `save_editor_to_knowledge` 新增 `State<'_, AppState>` 參數，取得 `kb_path`。
- 建立/更新 source 後，會自動將內容寫入 `{kb_path}/.insightcap/documents/{title}_{id}.md`，並更新 `sources.local_doc_path` 欄位。
- pipeline：分塊（split_markdown_by_headings）、標籤生成（TagEngine）、分類（SpaceEngine）皆自動觸發。

### 3. 資料表
- `sources`：記錄 type/title/content_hash/local_doc_path
- `captures`：分塊內容、標籤、分類

---

## 程式碼範例

**Rust**
```rust
// src-tauri/src/commands/editor_commands.rs
#[tauri::command]
pub async fn save_editor_to_knowledge(
    pool: State<'_, SqlitePool>,
    state: State<'_, AppState>,
    title: String,
    content: String,
) -> Result<(), String> {
    // ...建立/更新 source...
    // 快照寫入
    let doc_dir = state.kb_path.join(".insightcap").join("documents");
    fs::create_dir_all(&doc_dir)?;
    let safe_name = title.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let file_name = format!("{}_{}.md", safe_name, &source_id[..8]);
    let doc_path = doc_dir.join(&file_name);
    fs::write(&doc_path, &clean_content)?;
    // 更新 local_doc_path
    sqlx::query("UPDATE sources SET local_doc_path = ? WHERE id = ?")
        .bind(&doc_path.to_string_lossy())
        .bind(&source_id)
        .execute(pool.inner())
        .await?;
    // ...分塊/標籤/分類...
}
```

---

## 注意事項
- 若 title 重複，會覆蓋同名 source，快照檔名會根據 id 保證唯一。
- 前端/後端皆不需手動指定 documents 路徑，統一由 backend 控制。
- pipeline 皆為非同步，匯出後可立即進行其他操作。

---

## 相關檔案
- [src-tauri/src/commands/editor_commands.rs](InsightCAP_3/src-tauri/src/commands/editor_commands.rs)
- [src/lib/tauri.ts](InsightCAP_3/src/lib/tauri.ts)
- [src/components/chat/EditorPane.tsx](InsightCAP_3/src/components/chat/EditorPane.tsx)
- [src-tauri/migrations/006_repository_timeline.sql](InsightCAP_3/src-tauri/migrations/006_repository_timeline.sql)
