審查 InsightCAP IPC Command Layer，確認符合「只做參數驗證和服務調用」的架構原則。

步驟：
1. 讀取 InsightCAP_3/src-tauri/src/commands/mod.rs（取得完整命令列表）
2. 讀取最近修改的 1-2 個 command 檔案
3. 逐一檢查每個 #[tauri::command] 函數：
   - 是否包含業務邏輯（違規：應移至 Service 層）
   - 是否只做參數驗證後呼叫 Service
   - mod.rs register 清單與實際命令是否一致
4. 列出違規項目，說明應如何重構到正確的 Service 層
