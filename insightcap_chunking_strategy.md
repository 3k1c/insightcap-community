# InsightCAP RAG Chunking 策略全指南

> **適用範圍**：InsightCAP 所有支援的文件來源與格式
> **核心目標**：建立一套可長期演進、以召回精準度與生成品質為最大化目標的 Chunking 架構
> **文件狀態**：技術決策基準文件（Architecture Decision Record）

---

## 一、為什麼 Chunking 是 RAG 的核心決策

RAG 的品質由三個環節決定：**索引（Indexing）→ 召回（Retrieval）→ 生成（Generation）**。Chunking 發生在索引階段，但它的錯誤在召回與生成時放大，且難以在下游修正。

常見的誤解是認為 Chunking 只是「把文字切短」。事實上，Chunking 決定了三件深遠的事：

**向量空間的語意密度**。Chunk 越長，embedding 的焦點越模糊，相當於把一整段維基百科壓縮成一個向量，它跟任何問題的相似度都是「有點像但不夠準確」。Chunk 過短則語意不完整，無法匹配上下文豐富的查詢。甜蜜點在於讓一個 Chunk 只表達一個連貫的概念。

**LLM 的工作記憶使用效率**。被召回的 Chunk 直接佔用 context window。一個包含大量無關雜訊的 Chunk，等同於用寶貴的 token 預算裝垃圾，導致 AI 能同時參考的有效資料變少，洞察深度下降。

**系統的可長期維護性**。Chunking 邏輯隱含了對文件結構的假設。支援的格式越多，這些假設越容易出錯。今天新增一種文件類型，若沿用舊有的通用邏輯，召回品質會靜默劣化（silent degradation）——系統不報錯，但答案品質在悄悄下降。

---

## 二、核心原則（不可妥協）

### 原則一：語意完整性優先於大小均勻性

Chunk 的邊界應落在語意自然停頓點（句子 → 段落 → 章節），而非固定字元數。一個語意完整但略長的 Chunk，優於一個大小剛好但在句子中間截斷的 Chunk。字元上限（2400）是守門員，不是切割點。

### 原則二：不同來源類型，使用不同策略

InsightCAP 支援超過 30 種格式，涵蓋文件、程式碼、圖片、影片、網頁。這些來源的結構本質根本不同。套用統一的字元切分策略，意味著在所有格式上都次優化。正確做法是建立**策略路由器（Strategy Router）**，在索引時依格式分派對應的切分邏輯。

### 原則三：Chunk 必須攜帶結構化元資料

純文字向量化是有損的。每個 Chunk 在寫入向量資料庫時，應同時儲存元資料，用於召回後的引用顯示、過濾與排序：

```json
{
  "chunk_id": "uuid",
  "doc_id": "uuid",
  "source_type": "pdf | docx | xlsx | pptx | code | image | url | youtube | bilibili | ...",
  "chunk_type": "text | table | code | caption | slide | subtitle",
  "chunk_index": 3,
  "section_title": "第三章：財務分析",
  "section_breadcrumb": "財務報告 > 第三章 > 3.2 損益表",
  "page_number": 12,
  "timestamp_start": "00:14:32",
  "timestamp_end": "00:17:05",
  "char_count": 1840,
  "language": "zh-TW | en | ...",
  "source_url": "https://...",
  "crawled_at": "2026-04-26T10:00:00Z",
  "ocr_confidence": 0.94
}
```

### 原則四：重疊（Overlap）是雙面刃

Overlap 的目的是避免跨 Chunk 邊界的語意斷裂。建議重疊率維持在 **8–12%**（2400 字元 Chunk 配 192–288 字元 overlap）。

過高的 overlap 帶來三個問題：重複召回相似 Chunk（浪費 context window）；向量空間中相鄰 Chunk 過度相似，干擾排名；索引體積線性膨脹。

### 原則五：格式轉換統一為 Markdown 中間格式

所有來源在 Chunking 前，應先轉換為結構化的 Markdown 中間格式。這樣 Chunking 邏輯只需處理一種格式，大幅降低維護複雜度，也便於日後更換 Chunking 引擎。

---

## 三、來源類型分類與策略全覽

InsightCAP 的所有來源類型可歸納為六個策略群組：

| 策略群組 | 適用格式 | 核心切分依據 |
|---------|---------|------------|
| **結構感知** | `.pdf`, `.docx`, `.rtf`, `.epub`, `.html/.htm` | 標題層級、段落邊界 |
| **表格序列化** | `.xlsx`, `.csv` | 工作表、欄位語意 |
| **投影片拆解** | `.pptx` | 單張投影片為原子單位 |
| **語法感知** | 所有程式碼格式 | 函式/類別/模組邊界 |
| **視覺轉文字** | `.png`, `.jpg`, `.jpeg`, `.webp`, `.gif` | OCR + Vision 描述後按段落切 |
| **時序文字流** | YouTube, Bilibili 字幕、彈幕 | 時間戳 + 話題邊界 |
| **純文字 Fallback** | `.txt`, `.log`, `.md` | 段落邊界 + 句子邊界對齊 |

---

## 四、各格式詳細策略

### 4.1 PDF（`.pdf`）

PDF 是結構最複雜的來源，因為它的設計目標是視覺呈現，而非語意結構。原始文字提取常伴隨頁首/頁尾混入、跨頁段落黏合、表格提取失真等問題。

**建議策略：結構感知切分（Structure-Aware Chunking）**

處理流程如下：

1. **提取**：使用 pymupdf（速度快）或 pdfplumber（表格準確）提取，同時保留字型大小、粗體、位置資訊。
2. **標題偵測**：字型大小 > 正文均值 + 1.5 標準差，且為粗體 → 視為標題，建立層級樹（H1 > H2 > H3）。
3. **頁首/頁尾清理**：識別在每頁固定位置重複出現的文字（頁碼、公司名稱、文件編號），予以移除。
4. **跨頁段落修復**：若段落末尾無標點符號，且下頁首段無縮排，視為同一段落，自動合併。
5. **主切分邊界**：優先在標題邊界切分，每個標題區段成為獨立 Chunk 群組。
6. **二次切分**：若標題區段超過 2400 字元，在最近句子邊界切分（見 §6.1）。
7. **表格獨立處理**：表格序列化為 Markdown，整張表格作為獨立 Chunk，標記 `chunk_type: table`。

**元資料**：`page_number`、`section_breadcrumb`（如「第三章 > 3.2 損益表」）、`pdf_filename`

**陷阱**：
- 掃描版 PDF 必須先過 OCR。OCR 信心分數 < 0.85 的頁面，應在元資料標記低品質，召回時降低權重或提示使用者。
- 雙欄排版（學術論文、雜誌）的文字提取順序可能錯亂，需要依 x 座標分欄後再逐欄提取。
- 加密 PDF 無法提取，應在前置階段就回報錯誤，避免索引到空 Chunk。

---

### 4.2 Word 文件（`.docx`、`.rtf`）

Word 的優勢在於結構明確（Heading 樣式有語意、表格有清晰邊界），應充分利用，而非退化為純文字處理。

**建議策略：樣式感知切分（Style-Aware Chunking）**

處理流程：

1. 使用 python-docx 解析，讀取每段落的 `paragraph.style.name`。
2. `Heading 1/2/3` → 作為切分邊界，並記錄為 `section_title` 元資料。
3. 正文段落累積至 2400 字元後，在段落邊界切分（不在段落中間切）。
4. 表格 → 序列化為 Markdown，整張作為獨立 Chunk。若表格超過 2400 字元，按行區塊切分，每個 Chunk 必須保留表頭。
5. 清單（List）→ 將同一層級的相關項目合併為單一 Chunk，避免每個項目成為語意孤立的碎片。
6. 若文件有目錄（TOC），提取 TOC 作為整份文件的摘要 Chunk，在召回的粗篩階段優先使用。

`.rtf` 建議先轉換為 `.docx` 再處理，減少一個格式的維護負擔。

---

### 4.3 電子書（`.epub`）

ePub 本質是一系列 HTML 文件的打包。其優勢在於有明確的章節（chapter）結構。

**建議策略**：解壓 ePub，按章節 HTML 文件切分為大塊，再對每個章節 HTML 套用 DOM 結構切分（同 §4.7）。章節邊界是最高優先的切分點。元資料保留書名、章節名稱、作者。

---

### 4.4 Excel 試算表（`.xlsx`、`.csv`）

表格資料的語意結構與散文根本不同，不應當作純文字切分。

**建議策略：語意序列化切分（Semantic Serialization Chunking）**

處理流程：

1. **工作表識別**（xlsx）：每個工作表作為獨立文件單元，工作表名稱記入元資料。
2. **表頭偵測**：識別第一行/列是否為欄位標題（透過資料類型一致性判斷）。
3. **小表格（< 50 行）**：整張表序列化為 Markdown，作為單一 Chunk。格式如下：

   ```
   工作表：Q3 銷售報表
   | 地區 | 產品 | 銷售額 | 成長率 |
   |------|------|--------|--------|
   | 北區 | A款  | 120萬  | 12%    |
   ```

4. **大表格（≥ 50 行）**：按語意群組切分（如同一地區的所有行），每個 Chunk 必須保留完整表頭。每個 Chunk 不超過 30 行，確保 LLM 能完整處理。
5. **CSV**：同上，但無工作表概念，以檔名作為文件標識。

**重要認知**：Excel 常包含公式、格式化數字。提取時應解析公式結果值，而非公式本身。`=SUM(A1:A10)` 對 RAG 無用，`=1,234,567` 才有語意。

---

### 4.5 PowerPoint（`.pptx`）

投影片的特性是：每張投影片是一個獨立的語意單元，設計上就是「一個概念一張」。

**建議策略：投影片原子切分（Slide-Atomic Chunking）**

核心原則：**每張投影片為一個 Chunk，不跨投影片合併，不在投影片內部切分。**

處理流程：

1. 使用 python-pptx 解析每張投影片。
2. 提取順序：標題 → 主文 → 備忘稿（Speaker Notes） → 替代文字（圖片的 alt text）。
3. 序列化格式：

   ```
   [投影片 7/32] 標題：Q3 財務表現摘要
   
   主文：
   - 營收：NT$1.2B，年增 18%
   - 毛利率：42.3%，較去年提升 3.2pp
   - EBITDA：NT$380M
   
   備忘稿：
   強調毛利率的提升主要來自產品組合改善，而非成本削減。
   ```

4. 若單張投影片文字超過 2400 字元（罕見但可能），在段落邊界切分，並在元資料標記為同一投影片的分段。
5. 元資料記錄：`slide_number`、`total_slides`、`presentation_title`、投影片標題。

**備忘稿是高價值資訊**。演講者備忘稿通常包含投影片未呈現的完整論述，務必納入同一 Chunk，而非忽略。

---

### 4.6 程式碼（`.py`, `.js`, `.ts`, `.tsx`, `.jsx`, `.rs`, `.go`, `.java`, `.cpp`, `.c`, `.h`, `.swift`, `.rb`, `.php`）

程式碼是語意結構最強的格式，但也是被最多 RAG 系統誤處理的格式。純字元切分會在函式中間截斷，導致召回的 Chunk 無法獨立理解。

**建議策略：語法感知切分（Syntax-Aware Chunking）**

**理想方案**：使用 Tree-sitter 做語法樹解析，以函式（function）、類別（class）、模組（module）邊界為切分點。

處理流程：

1. **語法樹解析**：用 Tree-sitter 識別頂層宣告（top-level declarations）：函式定義、類別定義、模組/命名空間。
2. **切分粒度**：以「完整函式」或「完整類別」為原子單位。一個函式 = 一個 Chunk（含其 docstring/註解）。
3. **大函式處理**：若單一函式超過 2400 字元，以內部邏輯區塊（`if/else` 大區塊、迴圈體）為次級邊界，並在元資料標記為同一函式的分段。
4. **Chunk 前綴注入**：每個程式碼 Chunk 前加入上下文前綴，確保 Chunk 能獨立被理解：

   ```
   # 語言：Python | 檔案：src/chunking.py | 類別：ChunkProcessor | 函式：split_semantic_windows
   
   def split_semantic_windows(self, text: str, max_chars: int = 2400) -> List[Chunk]:
       """將文字切分為語意視窗..."""
       ...
   ```

5. **關於現有的「過濾冗餘註解」策略**：需要重新評估。函式的 docstring 和重要的行內註解是高密度語意資訊，不應過濾。真正應過濾的是：自動生成的版權標頭（重複 N 次的公司聲明）、被註解掉的廢棄程式碼區塊（`// old code`）。
6. **import 區塊**：不作為獨立 Chunk，可附加至第一個函式 Chunk 的前綴，或完全略過（import 幾乎不包含可查詢的語意）。

**元資料**：`language`、`file_path`（在 repo 中的路徑）、`function_name`、`class_name`、`module_name`。

---

### 4.7 圖片（`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`）

圖片是唯一需要在 Chunking 前先做模態轉換（modality conversion）的格式。

**兩階段處理：轉文字 → 切分**

**第一階段：視覺轉文字（模態轉換）**

依圖片類型選擇提取方式：

| 圖片類型 | 主要提取方式 | 補充 |
|---------|------------|------|
| 截圖、掃描文件 | OCR（PaddleOCR / Tesseract） | 保留空間佈局資訊 |
| 圖表（折線圖、長條圖、圓餅圖） | Vision 模型描述 + 數值提取 | 轉為 Markdown 表格 |
| 流程圖、架構圖 | Vision 模型描述 | 重點描述節點與關係 |
| 自然場景照片 | Vision 模型描述 | 場景、物件、文字 |
| 動圖（GIF） | 取代表性幀（首幀 + 中間幀）進行 Vision 分析 | — |

OCR 信心分數應記錄在元資料（`ocr_confidence`），低於 0.80 的結果應在召回時降低優先級或警示使用者。

**第二階段：Chunking**

轉換後的文字按一般文字策略切分。通常圖片轉出的文字量不大，多數情況下整張圖片的描述/OCR 結果作為單一 Chunk 即可。

**元資料**：`original_filename`、`extraction_method`（ocr / vision）、`ocr_confidence`、`image_dimensions`。

---

### 4.8 網頁（Web URL、`.html`、`.htm`）

網頁最大的挑戰是雜訊：導覽列、廣告、頁尾、Cookie 提示、側欄推薦等若進入索引，會嚴重污染向量空間。

**建議策略：DOM 結構切分（DOM-Structure Chunking）**

處理流程：

1. **內容萃取**：使用 Trafilatura 或 Mozilla Readability 先萃取主要內文，移除導覽、廣告、頁尾。這一步的品質決定後續一切。
2. **動態渲染偵測**：若萃取結果字元數 < 500 但頁面明顯是有內容的，觸發 Playwright headless browser 渲染後再提取（JavaScript SPA 的必要處理）。
3. **HTML 結構解析**：基於 `h1~h6` 建立章節樹，`p` 標籤累積為段落。
4. **主切分邊界**：`h2`/`h3` 標題邊界。
5. **段落累積**：`p` 標籤累積超過 2400 字元時，在段落邊界切分。
6. **特殊元素**：
   - `<code>`/`<pre>` → 整塊作為獨立 Chunk，標記 `chunk_type: code`，套用程式碼 Chunk 邏輯
   - `<blockquote>` → 保留，作為引用語境的一部分
   - `<table>` → 序列化為 Markdown，同 Excel 策略
7. **文件層級元資料**：`<title>`、`<meta description>`、`og:title`、`og:description`，作為文件摘要記錄，用於粗篩。

**URL 元資料**：`url`、`domain`、`crawled_at`、`last_modified`（HTTP header）、`http_status`。

**陷阱**：需要登入的頁面、付費牆後的內容，爬蟲無法取得。應在失敗時回報原因（403/401/paywall 偵測），而非索引空 Chunk。

---

### 4.9 影片字幕（YouTube、Bilibili）

字幕是**時間序列文字流**，有時間戳這個天然的結構資訊。Bilibili 的彈幕是額外的群眾智慧層，需要特別處理。

**YouTube / Bilibili 字幕：話題感知時間窗切分**

處理流程：

1. **字幕獲取**：
   - YouTube：優先使用人工字幕（準確度高），退而求其次使用自動生成字幕。讀取 SRT/VTT，取得 `(start_time, end_time, text)` 三元組。
   - Bilibili：解析 XML 格式字幕，同樣提取時間戳與文字。
2. **基礎合併**：時間間隔 < 1.5 秒的連續字幕行合併為同一段落。
3. **話題邊界偵測**（擇一或組合）：
   - **靜默偵測**：時間間隔 > 3 秒 → 強制切分邊界（成本低，效果尚可）
   - **語意飄移偵測**：計算相鄰 sliding window embedding 的 cosine 距離，距離 > 0.45 → 切分（成本高，效果更好）
4. **大小控制**：累積字幕至 2–3 分鐘或 800 字元後，在自然停頓點切分。字幕 Chunk 比其他格式短是合理的，因為口語密度較低。
5. **口語清理**：移除填充詞（嗯、啊、呃、那個、so、like、you know），但保留有語意的語氣詞。
6. **說話者識別**（若字幕有標記）：說話者切換也是切分邊界。

**Bilibili 彈幕的處理**（可選，但有獨特價值）：

彈幕是觀眾的即時反應，可以作為「社群摘要層」：

```
[彈幕摘要 Chunk - 00:14:00~00:17:00]
高頻彈幕詞：重點、幹貨、這個觀點很新、記筆記
情感分布：正向 82%，疑問 15%，負向 3%
```

彈幕 Chunk 不用於主要召回，但可作為輔助過濾器：若使用者問「這個影片最精彩的部分是哪裡」，高密度彈幕的時段是有力的信號。

**時間戳深層連結**：這是影片來源的獨特優勢。每個字幕 Chunk 的召回結果，應附上可點擊的時間戳連結（YouTube 格式：`https://youtu.be/{id}?t={seconds}`），讓使用者一鍵跳轉到原始片段。

**元資料**：`video_id`、`platform`（youtube/bilibili）、`video_title`、`channel_name`、`timestamp_start`、`timestamp_end`、`subtitle_type`（manual/auto）、`language`。

---

### 4.10 純文字 Fallback（`.txt`, `.log`, `.md`）

這是最後的通用策略，適用於無法識別明確結構的文字檔案。

**Markdown（`.md`）**：有 `#` 標題語法，應先解析 Markdown 結構，以標題邊界切分，退化為結構感知策略，而非純字元切分。

**純文字（`.txt`）與日誌（`.log`）**：

1. 嘗試識別是否有重複的行結構（日誌格式通常是 `[timestamp] [level] message`），若有，每一個「事件群組」作為一個 Chunk。
2. 若為一般散文，按段落邊界（`\n\n`）切分，超過 2400 字元再按句子邊界切分。
3. 日誌檔案的時間戳應提取為元資料（`log_start_time`、`log_end_time`）。

---

## 五、策略路由器設計

所有格式應通過統一的策略路由器分派，確保邏輯集中管理：

```rust
pub enum SourceType {
    Pdf, Docx, Rtf, Epub,
    Xlsx, Csv,
    Pptx,
    Code(Language),
    Image(ImageFormat),
    HtmlFile, WebUrl,
    YoutubeSubtitle, BilibiliSubtitle, BilibiliDanmaku,
    PlainText, Markdown, Log,
}

pub enum ChunkingStrategy {
    StructureAware,       // PDF, DOCX, RTF, EPUB, HTML
    TableSerialization,   // XLSX, CSV
    SlideAtomic,          // PPTX
    SyntaxAware,          // 所有程式碼格式
    VisionThenText,       // 圖片（先做模態轉換）
    TemporalTopic,        // YouTube / Bilibili 字幕
    DanmakuSummary,       // Bilibili 彈幕
    SemanticFallback,     // TXT, LOG（含句子邊界對齊）
    MarkdownStructure,    // MD（Markdown 標題感知）
}

pub fn route_strategy(source: &SourceType) -> ChunkingStrategy {
    match source {
        SourceType::Pdf | SourceType::Docx | SourceType::Rtf
        | SourceType::Epub | SourceType::HtmlFile | SourceType::WebUrl
            => ChunkingStrategy::StructureAware,
        SourceType::Xlsx | SourceType::Csv
            => ChunkingStrategy::TableSerialization,
        SourceType::Pptx
            => ChunkingStrategy::SlideAtomic,
        SourceType::Code(_)
            => ChunkingStrategy::SyntaxAware,
        SourceType::Image(_)
            => ChunkingStrategy::VisionThenText,
        SourceType::YoutubeSubtitle | SourceType::BilibiliSubtitle
            => ChunkingStrategy::TemporalTopic,
        SourceType::BilibiliDanmaku
            => ChunkingStrategy::DanmakuSummary,
        SourceType::Markdown
            => ChunkingStrategy::MarkdownStructure,
        SourceType::PlainText | SourceType::Log
            => ChunkingStrategy::SemanticFallback,
    }
}
```

---

## 六、通用技術改善

無論哪種策略，以下技術改善適用於所有格式。

### 6.1 硬切分改為句子邊界對齊

這是目前最值得立即實作的改善。當累積字元數接近上限時，不在第 2400 個字元處直接截斷，而是尋找最近的語意停頓點：

```rust
/// 在 target 位置附近（±tolerance 字元）找最合適的切分點
/// 優先順序：句子終結符 > 逗號/分號 > 空格 > 硬切
fn find_sentence_boundary(text: &str, target: usize, tolerance: usize) -> usize {
    // 安全範圍：target-tolerance 到 target（往前找）
    let search_start = target.saturating_sub(tolerance);
    let candidate = &text[search_start..target.min(text.len())];

    // 1. 優先：句子終結符（中英文句號、問號、驚嘆號、換行）
    if let Some(pos) = candidate.rfind(|c| matches!(c, '。'|'！'|'？'|'.'|'!'|'?'|'\n')) {
        let byte_pos = search_start + pos;
        // 確保切點不在 UTF-8 多位元字元中間
        return text.ceil_char_boundary(byte_pos + 1);
    }

    // 2. 退而求其次：逗號、分號
    if let Some(pos) = candidate.rfind(|c| matches!(c, '，'|'；'|','|';')) {
        let byte_pos = search_start + pos;
        return text.ceil_char_boundary(byte_pos + 1);
    }

    // 3. 最後：空格（詞語邊界）
    if let Some(pos) = candidate.rfind(' ') {
        return search_start + pos + 1;
    }

    // 4. 真正的最後手段：硬切，確保在字元邊界
    text.ceil_char_boundary(target)
}
```

tolerance 建議設定為 80 字元（約 30–40 個中文詞），足以找到停頓點，又不會讓 Chunk 大小差異過大。

### 6.2 Chunk 品質監控指標

建立即時監控，索引時記錄以下指標：

| 指標 | 正常範圍 | 警告閾值 | 意義 |
|------|----------|----------|------|
| 平均 Chunk 大小 | 600–2000 字元 | > 2400 或 < 150 | 太長語意模糊，太短資訊量不足 |
| Chunk 大小標準差 | < 700 | > 1200 | 大波動代表切分邏輯不穩定 |
| 超長 Chunk 比例 | < 2% | > 5% | 硬切分邏輯失效 |
| 極短 Chunk 比例 | < 5% | > 15% | 清洗過激或格式解析失敗 |
| 空 Chunk 數量 | 0 | > 0 | 管道錯誤，需立即調查 |
| OCR 低信心比例（圖片）| < 10% | > 25% | 掃描品質差，需人工確認 |

---

## 七、長期架構演進

### 7.1 階層式索引（Hierarchical Indexing）

目前系統使用單一層級的 Chunk。長期建議引入兩層結構：

```
文件層（Document Summary）
  └── 512 字元摘要 Chunk（由 LLM 生成）← 用於粗篩
       └── 段落層（Paragraph Chunk）
             └── 2400 字元詳細 Chunk ← 用於精確召回與生成
```

召回流程：先用摘要 Chunk 找到相關文件（Top-10）→ 再在這 10 份文件內，用段落 Chunk 精確定位（Top-3）→ 傳給 LLM。

效益：對長文件（書籍、長篇報告）的召回精準度提升 20–40%，且 LLM 收到的 context 更精練。

### 7.2 Late Chunking

Late Chunking（2024 年提出）是一種在 embedding 後再切分的技術。傳統做法是先切分、再分別 embedding；Late Chunking 是先對整份文件做 full-context embedding（需支援長 context embedding model，如 jina-embeddings-v3），再切分 token embeddings 並取平均。

優勢在於每個 Chunk 的向量能感知整份文件的上下文，而非只看自身 2400 字元。對長文件（技術手冊、法律合約）效果顯著。缺點是需要更換 embedding model，且處理成本較高。建議在系統穩定後，針對長文件類型（PDF、DOCX、EPUB）進行 A/B 測試評估。

### 7.3 針對程式碼的 Code Graph 增強

程式碼的 Chunking 有一個純文字策略無法解決的問題：函式之間有呼叫關係。`function A` 呼叫 `function B`，純切分後兩者的 Chunk 是孤立的，召回 A 時不會自動關聯到 B。

長期建議引入 Code Graph：在索引時建立函式呼叫圖，存儲為圖資料庫（Neo4j 或 pgvector 的 metadata）。召回 A 時，自動附帶其直接依賴（depth-1 neighbors）作為輔助 context。這對「解釋一段程式碼的完整工作原理」類查詢效果顯著。

---

## 八、重新索引策略

**更改任何 Chunking 邏輯後，所有相關格式的文件必須重新索引。** Chunking 邏輯的改變會使舊 Chunk 與新 Chunk 的邊界不一致，混合索引會導致召回結果不可預測。

標準遷移流程：

```
1. 建立新的向量資料庫集合（不覆蓋舊集合）
2. 批次重新處理所有文件，寫入新集合
3. 在評估集上計算新舊集合的 Recall@5、MRR 指標
4. 新集合指標 ≥ 舊集合 → 切換生產流量
5. 舊集合保留 2 週作為回滾保險，確認無問題後刪除
```

永遠不要在生產集合上就地修改 Chunking 邏輯。

---

## 九、評估框架

### 9.1 離線評估（每次重大 Chunking 改動前後執行）

建立 **100–200 個問題—答案—來源文件三元組**，覆蓋所有格式類型：

- **Recall@K**：正確答案是否出現在前 K 個召回 Chunk 中（建議測試 K = 3, 5, 10）
- **MRR（Mean Reciprocal Rank）**：正確 Chunk 平均排名的倒數，越高越好，完美 = 1.0
- **Context Precision**：召回的 Chunk 中，真正相關的比例
- **按格式分層評估**：不要只看整體指標，需要分別看 PDF 的 Recall、程式碼的 Recall 等，才能診斷哪個格式的策略失效

### 9.2 線上評估（持續進行）

- 使用者的明確回饋（按讚/踩）
- AI 回答中引用的來源是否被點擊（若有引用介面）
- 回答中包含「找不到相關資料」或「我沒有足夠資訊」的頻率
- 按來源格式分類的以上指標，找出召回最差的格式

---

## 十、優先執行順序

| 優先級 | 行動項目 | 效益 | 實作成本 |
|--------|---------|------|---------|
| **P0** | 硬切分改為句子邊界對齊 | 所有格式語意完整性 ↑ | 極低（< 2h） |
| **P0** | 所有 Chunk 補齊結構化元資料 | 可引用性 ↑，可觀測性 ↑ | 低 |
| **P0** | 建立 Chunk 品質監控（大小分布、空 Chunk 告警）| 問題早期發現 | 低 |
| **P1** | 建立策略路由器架構 | 為後續所有格式優化奠基 | 中 |
| **P1** | PPTX 改為投影片原子切分 | 投影片召回精準度 ↑↑ | 低 |
| **P1** | 程式碼改為語法感知切分（Tree-sitter）| 程式碼召回完整性 ↑↑ | 中 |
| **P1** | Excel/CSV 改為語意序列化（保留表頭）| 表格資料召回 ↑↑ | 中 |
| **P2** | PDF 標題層級感知切分 | 長文件召回 ↑↑ | 中 |
| **P2** | 字幕話題感知切分 + 時間戳深層連結 | 使用者體驗 ↑↑ | 中 |
| **P2** | Word 樣式感知切分（讀取 Heading 樣式）| 報告類文件召回 ↑ | 低 |
| **P2** | OCR 信心分數記錄與低品質標記 | 圖片索引品質可觀測 ↑ | 低 |
| **P3** | Bilibili 彈幕摘要 Chunk | 額外的群眾智慧信號 | 中 |
| **P3** | Markdown 標題感知切分（區分於純文字）| MD 文件召回 ↑ | 極低 |
| **P3** | 階層式索引（Document Summary + Paragraph）| 長文件系統效能 ↑↑↑ | 高 |
| **P3** | Late Chunking 評估（長文件類型）| 上下文感知嵌入 ↑↑ | 高 |
| **Future** | 程式碼 Code Graph 增強 | 程式碼跨函式理解 ↑↑↑ | 極高 |

---

*最後更新：2026 年 4 月*
*文件版本：v2.0（覆蓋 InsightCAP 全格式）*
*適用模組：chunking.rs、indexing pipeline、metadata schema*
