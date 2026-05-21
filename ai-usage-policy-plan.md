# AI Usage Policy 開發規劃

## 目的

在不犧牲主要聊天體驗的前提下，降低雲端模型產生巨額支出的風險。做法是建立統一的 AI 使用策略，將日常聊天、深度思考、背景任務分級管理，並逐步加入成本提示與預算控制。

## 現有架構評估

目前 backend 已有 `LLMOptions`，包含：

- `temperature`
- `max_tokens`
- `stream`
- `think_mode`

OpenAI / OpenAI-compatible provider 會在 `OpenAiProvider::build_request_body()` 中把 `max_tokens` 寫入 request body。reasoning model 則使用 `max_completion_tokens`。

主要呼叫點包含：

- `commands/rag_commands.rs`
  - 主聊天 / RAG streaming
  - think mode 時使用 `8192`
  - 非 think mode 使用 `LLMOptions::default()`，目前是 `2048`

- `services/rag_engine.rs`
  - 非 streaming RAG
  - think mode 使用 `8192`

- `background/deep_synthesis_engine.rs`
  - 背景 synthesis 使用 `2048`
  - compiled knowledge 使用 `1024`

- 其他背景任務
  - title / relation / memory extraction 等多數已有較小 cap，例如 `20`、`60`、`200`、`600`

設定集中在：

- `src-tauri/src/settings/store.rs`
- frontend 型別在 `src/lib/types.ts`
- 設定 UI 在 `src/pages/SettingsPage.tsx`

結論：可以加一個 `usage_policy` 設定區塊，再用 helper 統一產生或調整 `LLMOptions`。不建議直接在各呼叫點散落更多 magic number。

## 建議實作方向

### Phase 1：Backend Policy Layer

新增：

- `AIUsageSettings`
- `AIUsageMode`
- `LLMTaskKind`
- `apply_llm_usage_policy()`

建議模式：

- `economy`
  - 一般輸出 cap：`1024`
  - think mode cap：`4096`
  - 背景任務更嚴格

- `balanced`
  - 預設模式
  - 一般輸出 cap：`2048`
  - think mode cap：`8192`
  - 背景任務保守

- `quality`
  - 一般輸出 cap：`4096`
  - think mode cap：`8192`
  - 背景任務仍不自動放大

Task kind 建議：

- `InteractiveChat`
- `InteractiveThink`
- `EditorRewrite`
- `BackgroundSummary`
- `BackgroundSynthesis`
- `TinyClassification`

這樣不同任務不需要自己猜 token cap。

### Phase 2：套用到主要 LLM 呼叫點

優先改：

1. `rag_commands.rs`
   - 主聊天 streaming
   - think mode 手動觸發可以維持高 cap

2. `rag_engine.rs`
   - 非 streaming RAG

3. `background/deep_synthesis_engine.rs`
   - 背景任務走 background cap

4. 小型任務保留現有低 cap
   - title generation
   - relation extraction
   - reminder extraction

這階段先不做金額估算，只先做到「模式化 token cap」。

### Phase 3：Settings UI

在 AI 設定頁新增「AI 使用策略」。

UI 建議只顯示簡單模式：

- 省錢
- 平衡
- 高品質

進階項目可以先不做，避免 UI 複雜化。

需要同步更新：

- `src/lib/types.ts`
- `SettingsPage.tsx`
- `zh-TW.json`
- `zh-CN.json`
- `en.json`

### Phase 4：成本提示與預算控制

這階段才做估算和預算。

新增功能：

- 雲端 provider 判斷
  - `openai`
  - `google`
  - `xai`
  - `openrouter`
  - 非 `ollama` / `local` 都視為可能產生成本

- 單次高成本提示
  - think mode
  - 長 context
  - output cap >= `8192`

- 背景雲端保護
  - 可設定「背景任務允許使用雲端模型」
  - 預設關閉或 balanced 下保守開啟

月費預算可以放後一點，因為要追蹤 usage，牽涉資料表或 settings state，不應混在第一版。

## 測試規劃

Backend tests：

- `AIUsageSettings` legacy JSON deserialize 時有預設值
- `balanced` 一般聊天輸出 cap 是 `2048`
- `balanced` think mode cap 是 `8192`
- `economy` 背景 synthesis cap 低於 interactive chat
- `quality` 不會放大 background task 到無限制
- `OpenAiProvider` 仍正確送出：
  - reasoning model 使用 `max_completion_tokens`
  - 其他 model 使用 `max_tokens`

Frontend tests：

- Settings 型別包含 `aiUsage`
- SettingsPage 能顯示並儲存 usage mode
- legacy settings 缺少 `aiUsage` 時不 crash

## 建議實作順序

1. 新增 backend settings 結構與預設值
2. 新增 `llm_usage_policy.rs`
3. 寫 backend unit tests
4. 套用到 `rag_commands.rs`
5. 套用到 `rag_engine.rs`
6. 套用到 `deep_synthesis_engine.rs`
7. 更新 frontend types
8. 更新 SettingsPage UI
9. 更新三語系文案
10. 跑 Rust tests 與 frontend targeted tests
11. Commit

## 不建議第一版做的事

- 不做精準美元費用估算
- 不做完整 monthly budget ledger
- 不自動切換模型
- 不改 provider profile 架構
- 不重構所有 LLM 呼叫點

第一版先做到「穩定、可理解、可回退」。
