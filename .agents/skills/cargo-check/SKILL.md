在 InsightCAP_3/src-tauri/ 執行 cargo check，解析並整理編譯結果。

步驟：
1. 執行 `cd InsightCAP_3/src-tauri && cargo check 2>&1`
2. 解析輸出，分兩類：
   - **Error**（必須修復，列出檔案路徑、行號、錯誤訊息、建議修復方式）
   - **Warning**（建議處理，列出檔案路徑、行號、警告內容）
3. 如有 Error，直接讀取對應檔案的相關行，提供具體修復建議
4. 如有 Warning，**直接自動修復**常見類型後回報：
   - `unused variable`：將參數改名為 `_xxx`
   - `unused import`：移除該 use 行
   - `dead_code`：視情況加 `#[allow(dead_code)]` 或移除
5. 輸出摘要：Error N 個，Warning N 個（已自動修復 N 個）
