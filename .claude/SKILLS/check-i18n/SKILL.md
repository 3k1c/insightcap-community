檢查 InsightCAP 前端的 i18n 完整性，找出硬編碼字串與缺漏翻譯。

步驟：
1. 讀取 InsightCAP_3/src/i18n/types.ts（取得所有 key 定義）
2. 讀取 InsightCAP_3/src/i18n/index.ts（取得三語言翻譯內容）
3. 讀取最近修改的 2-3 個 .tsx 頁面檔案
4. 找出以下問題：
   - 未用 useT / t() 的硬編碼中文或英文字串
   - zh-TW / zh-CN / en 三者中任一缺少翻譯的 key
   - 使用 t('key') 但 types.ts 未宣告的 key
5. 列出每條問題的檔案路徑與行號，建議補全方式
