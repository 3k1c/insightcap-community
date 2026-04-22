掃描 InsightCAP 最近程式碼變更，對照 Architecture-v2.md，找出文件與程式碼的差異。

步驟：
1. 執行 `git diff HEAD~5 -- InsightCAP_3/src-tauri/src/` 查看 Rust 後端變更
2. 執行 `git diff HEAD~5 -- InsightCAP_3/src/` 查看前端變更
3. 讀取 Architecture-v2.md 的 Core Services、Background Services、Command Layer 列表
4. 輸出差異清單，分三類：
   - 新增（程式碼有但文件未記錄）
   - 修改（介面或結構已變動）
   - 過時（文件有但程式碼已刪除）
5. 顯示建議更新內容後，問：「是否覆蓋 Architecture-v2.md？」
