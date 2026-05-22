# Text-based PDF 匯出設計

## 目的

將 InsightCAP editor 的 PDF 匯出由目前的圖片化輸出，改為標準 text-based PDF。

用戶預期的 PDF 應可列印、可搜尋、可選取文字、可複製內容。圖片化 PDF 只適合畫面快照，不應作為主要匯出格式。

## 原則

- 「匯出 PDF」預設產生 text-based PDF。
- PDF 版面以列印穩定為優先，不追求完全還原 editor 畫面 CSS。
- 保留基本文件語意：標題、段落、列表、表格、圖片、連結。
- 長內容必須自動分頁。
- 匯出結果必須透過 Tauri `writeBinaryFile` 寫入使用者選擇的路徑。

## 具體實現

### 1. 新增 text-based PDF renderer

在 `src/lib/editor-export.ts` 中新增 HTML 到 PDF 的渲染流程：

- 使用 `DOMParser` 解析 editor HTML。
- 遞迴處理 DOM nodes。
- 用 `jsPDF.text()` 寫入文字。
- 用 `splitTextToSize()` 處理自動換行。
- 追蹤目前 cursor Y position。
- 超過頁面底部時自動 `addPage()`。

### 2. 支援內容類型

MVP 支援：

- `p`
- `h1` 至 `h6`
- `strong`
- `em`
- `ul`
- `ol`
- `li`
- `blockquote`
- `pre`
- `code`
- `a`
- `table`
- `img`

### 3. Table 處理

表格不圖片化，改用 PDF drawing：

- 計算欄數。
- 根據頁面寬度平均分欄。
- 每個 cell 文字使用 `splitTextToSize()`。
- 根據最高 cell 高度決定 row height。
- 使用 `rect()` 畫 border。
- `th` 使用淡灰背景與 bold font。
- row 太高或頁尾空間不足時自動換頁。

暫不支援高保真 merged cells；如果遇到 `colspan` / `rowspan`，先退化為普通 cell 並在 validation warning 中提示。

### 4. Image 處理

MVP 支援：

- `data:image/...;base64`
- 可讀取的 local/base64 image
- 依頁面寬度等比例縮放
- 圖片太高時換頁

外部 URL 圖片暫不主動下載，避免 network、CORS、隱私與失敗狀態複雜化。

### 5. 取消圖片化預設

目前 `html2canvas` 流程不再作為 PDF 預設匯出。

可選策略：

- 直接移除畫面快照 PDF。
- 或保留為內部 helper，未來另加「匯出畫面快照 PDF」選項。

MVP 建議先移除預設使用，避免用戶誤會。

## 測試

新增或更新測試：

- PDF 匯出必須呼叫 `pdf.output('arraybuffer')`。
- PDF 匯出必須透過 `tauriCmd.writeBinaryFile()` 寫入指定路徑。
- PDF 匯出不應呼叫 `pdf.save()`。
- paragraph / heading / list / table 渲染應呼叫對應 jsPDF API。
- 空文件應可匯出，但內容至少包含空白頁，不應 crash。

## 注意事項

- text-based PDF 的視覺效果不會完全等同 editor 畫面。
- 複雜 CSS、nested table、merged cells、浮動圖片暫不列入 MVP。
- 這是「標準可列印 PDF」路線，不是「高保真畫面截圖」路線。
- 若未來需要更高品質 Office/PDF publishing，可再評估 Rust 端 PDF library 或 headless browser print-to-PDF。

## 相關檔案

- `src/lib/editor-export.ts`
- `src/lib/editor-export.test.ts`
- `src/components/chat/EditorPane.tsx`
- `src/lib/editor-validation.ts`
