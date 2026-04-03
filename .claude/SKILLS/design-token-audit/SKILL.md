掃描 InsightCAP 前端，找出未遵守 IC Design Token 規範的樣式寫法。

步驟：
1. 讀取 InsightCAP_3/src/design-system/tokens.css，取得有效的 ic-* token 清單
2. 讀取 InsightCAP_3/src/design-system/index.css
3. 掃描最近修改的 3-5 個 .tsx 檔案
4. 找出以下違規：
   - 硬編碼顏色（#xxx、rgb()、hsl()）
   - 具體 Tailwind 顏色 class（如 bg-gray-100、text-blue-500）應改用 ic-* token
   - 寫死的 spacing / font-size（應使用 token 變數）
5. 每條列出：檔案路徑、行號、違規內容、建議替換的 IC token
