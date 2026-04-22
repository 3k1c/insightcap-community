# AGENTS.md

## 核心規則
- Context 超過 50% 時，主動建議執行 /compact
- 每次只處理一份文件或一個任務，完成後等我確認才繼續
- 只讀必要檔案（最多 3–5 個），禁止 @codebase 或整個資料夾
- 全部用繁體中文撰寫，技術名詞保留英文
- 檔名一律使用 kebab-case（例如：api-error-handling.md）

## 前端修改規則
- 修改或移除 import 前，先 `grep -n` 掃描該 symbol 在檔案中的所有使用點，確認全部處理後再動 import
- 新增 import 時，若同一來源已有 import 行，合併到同一行，不新增重複 import 行
- 變數 / 函式改名時，必須同時搜尋並更新同檔案內所有引用，不能只改定義處
- 使用 `absolute` 定位的 dropdown 選單，必須確認其最近祖先有 `relative`，否則定位會跑掉；標準做法是將選單放在觸發按鈕的 `relative` 容器內
- 從 store 新增使用的 action，必須同步加入元件頂層的解構列表
- 修改跨多個介面共用的型別定義時（如 attachedFiles），必須同時搜尋所有使用該型別的 Props / interface / store 定義並同步更新，避免型別不一致
- 涉及第三方 UI library 的定位 / 寬度 / 顯示行為（如 BubbleMenu、Tooltip、Popover）修改前，必須先讀 `node_modules/<pkg>/dist/index.js` 確認 library 內部如何設定這些樣式，不可假設可以從外部直接覆蓋
- 大幅重構 JSX 結構後（移除 condition block、新增或移除 wrapper div），必須執行以下指令驗證無語法錯誤，再交付：`node -e "require('./node_modules/@babel/parser').parse(require('fs').readFileSync('./src/...tsx','utf8'),{sourceType:'module',plugins:['typescript','jsx']}); console.log('OK')"`
- 涉及 Tauri API 或外部 library API 呼叫時，修改前必須先讀 `.d.ts` 型別定義確認正確的參數簽名與物件結構，不可假設
- 新增跨元件 UI state（如 store action）時，必須檢查所有相關元件的互動邏輯是否協調一致（例如隱藏對話後，nav 按鈕點擊行為要恢復對話）


## Rust 修改規則
- 新增 Tauri command 參數時，若該參數暫時未使用，必須加 `_` 前綴（如 `_conversation_id`），避免 cargo warning
- 新增 command 後必須同步在 `lib.rs` 的 `generate_handler![]` 內登記，否則前端呼叫會 runtime 失敗

## 文件生成
- 直接開始生成文件，完成後顯示完整內容 + 建議檔名
- 文件結構依需要取捨：目的 / 原則 / 具體實現 / 程式碼範例 / 注意事項 / 相關檔案
- 最後問：「是否寫入檔案？」

## Code-Doc Sync
1. 先用 `git diff` 和 `git log --oneline -10` 掃描最近變更
2. 直接列出差異清單（新增 / 修改 / 刪除）
3. 顯示更新內容後，問：「是否覆蓋原檔案？」