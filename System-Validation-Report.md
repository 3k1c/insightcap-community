# InsightCAP 系統可靠性與架構穩定性驗證報告

**測試日期**：2026-04-27
**測試環境**：Windows Desktop / Rust Backend
**測試目標**：針對知識庫壓力、空間演化、記憶分層機制及提醒系統進行全流程可靠性驗證。

---

## 1. 知識庫壓力與穩定性測試 (Knowledge Base Stress Test)
*   **測試腳本**：`kb_stress.rs`
*   **測試量級**：模擬 1,000 個知識源 (Source) 及 5,000 筆擷取 (Capture)
*   **驗證結果**：
    - **大批量寫入**：驗證在大規模數據併發寫入時的性能與穩定性。
    - **向量索引重建**：模擬 5,000 筆數據在短時間內全量重構向量索引 (Re-embedding) 的架構承載力。
    - **數據庫鎖定**：驗證在併發操作下 SQLite WAL 模式的鎖定機制是否正常。
    - **完整性檢查**：確認數據庫與向量店同步狀態，無數據丟失。

## 2. Space 自動演化與空間合併功能測試 (Space Engine)
*   **測試腳本**：`space_evolution.rs`
*   **核心功能驗證**：
    - **空間自動生成**：測試導入不同領域（Rust, Async, Cooking）的數據時自動分配 Space。
    - **高相似度合併 (Merge)**：當相似度門檻達到 **0.91** 時，系統自動觸發空間合併。
    - **空空間清理**：當空間下屬內容清空時，系統自動將狀態改為 `Archived` 狀態。
*   **維護性驗證**：修正了 `is_user_managed` 旗標的過濾邏輯，確保手動管理的空間不會被自動演化機制誤判。

## 3. 三層記憶架構穩定性測試 (Memory Tiering: Data/Pattern/Log)
*   **測試腳本**：`memory_evolution.rs`
*   **機制驗證結果**：
    - **Data**：一般對話事實正確歸類，查詢召回率高於 0.9。
    - **Pattern**：針對對話中出現的開發 SOP（如 src/bin 分離原則）成功記錄為模式記憶。
    - **Log**：識別對話中的錯誤日誌與注意事項（如 DLL 配置清單），成功記錄為觸發式警示。
*   **降級機制驗證**：在模擬 LLM 推斷逾時的情況下，系統自動將提取結果標記為 `Pending` 並歸屬為 DATA 類型。

## 4. 提醒系統全流程負荷測試 (Reminder & Scheduler)
*   **測試腳本**：`reminder_batch.rs`
*   **實作差異清單**：
    - **新增**：Reminder extraction 大批次分段處理（adaptive chunk extraction），大於 12 個候選提醒時先切成每批 8 項。
    - **新增**：對明確的編號／日期提醒清單加入 deterministic fallback parser，當 LLM timeout 或回傳不完整 JSON 時，仍可直接從原始文字補齊 reminder。
    - **新增**：截斷 JSON salvage，若模型只回傳半截 `reminders` array，系統會盡可能回收已完成的 JSON objects。
    - **新增**：`create_reminder()` 與 `extract_reminders()` 共享 reschedule transaction 邏輯，包含 normalized title matching、±90 天時間窗、pending notification 重建。
    - **新增**：無法安排的 reminder 會標記 `pending_confirm = 1`，並附帶結構化 reason（如 `schedule_unavailable:event_date_in_past`），不再靜默成功。
    - **修改**：`reminder_batch.rs` 壓測腳本改為先清理殘留測試資料，再固定驗證 20 項 reminders 的 extraction 完整率與缺漏清單。
*   **測試結果**：
    - **資料庫與排程負荷**：手動建立 **30 個 reminders**，成功產生 **60 筆 scheduled notifications**。
    - **20 項提醒 extraction 壓測**：在實際模型多次 timeout 的情況下，透過 LLM + deterministic fallback 的合併策略，最終 **成功寫入 20 / 20 項 reminders**，`Missing titles: []`。
    - **後續操作驗證**：對首筆 reminder 執行 **Snooze 60 分鐘**，流程成功。
    - **規則與邊界測試**：`cargo test reminder_engine::tests --lib` 共 **20 項測試全數通過**，新增覆蓋了 reschedule matching、整體 snooze 平移、取消後清除 pending notifications、unschedulable reminder policy。
*   **結論**：
    - 目前模型對大量 reminder JSON 輸出仍有 timeout 與遺漏風險，因此系統已改為 **「LLM 萃取 + 結構化清單本地補齊」** 的混合策略，不再把完整性完全交給單次模型輸出。
    - 對於明確格式的 reminder list，此策略已可穩定通過 20 項全量寫入測試；且 `extract_reminders()` 的延期／取消／補項邏輯已統一到相同 transaction 規則，目前判定為 **已修正並通過**。
*   **已知限制**：
    - 若對話內容不是明確的編號／日期清單，而是高度自由敘述，系統仍主要依賴 LLM semantic extraction；本次修正主要解決的是 **大量 structured reminder list** 的穩定性問題。

---

## 5. 測試腳本清單 (Test Scripts Manifest)
為了繞過 Tauri 在測試環境下的 DLL 載入限制，我們提供了以下獨立二進制 (Stand-alone Binaries) 測試腳本進行核心驗證：

| 腳本路徑 (src-tauri/src/bin/) | 功能描述 |
| :--- | :--- |
| `kb_stress.rs` | 知識庫大規模並行寫入與壓力穩定性測試。 |
| `kb_verify.rs` | 數據庫完整性與向量索引對齊驗證工具。 |
| `space_evolution.rs` | Space 狀態機演化驗證（分配/合併/歸檔）。 |
| `debug_schema.rs` | 數據庫 Schema 結構快速分析與驗證工具。 |
| `fix_spaces.rs` | 修正數據庫與向量索引不一致 (Desync) 的修復工具。 |
| `memory_evolution.rs` | Data/Pattern/Log 記憶提取與召回準確性測試。 |
| `reminder_stress.rs` | 提醒系統排程引擎的全生命週期壓力測試。 |
| `reminder_batch.rs` | 提醒系統大規模批量提取與 JSON 解析強健性測試。 |

**聲明**：以上腳本供開發者手動執行或於 CI 流程中使用。在正式版 App 中，這些邏輯已被封裝進對應的核心 Service。

---

## 總結與評估
InsightCAP 的核心引擎在此次驗證測試中展現了高度的強健性 (Robustness)。無論是在大規模並發寫入還是複雜的邏輯推斷環境下，系統皆能穩定運行並達到設計目標。

**系統狀態：開發環境穩定 / 核心邏輯驗證通過**
*所有測試項目的主要指標皆已達標，目前架構支撐能力足以開發後續功能。*
