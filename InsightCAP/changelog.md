# 0.9.0-beta.1 (2026-05-08)

## 狀態

InsightCAP 進入 Beta 測試版。此版本用於收集真實使用者回饋，尚不標記為正式 1.0 穩定版。

## 重點變更

- 版本從 `1.0.0` 調整為 `0.9.0-beta.1`，避免誤導為正式版。
- 預設主題改為 dark mode。
- 首次設定新建知識庫只接受不存在或空資料夾，避免覆蓋既有知識庫或使用者資料。
- Whisper models 改存到 `%LOCALAPPDATA%\com.insightcap.app\models`。
- FastEmbed cache 改存到 `%LOCALAPPDATA%\com.insightcap.app\.fastembed_cache`。
- Uninstall 清理選項改為清楚描述 app-owned data，不自動刪除自定義知識庫或 Windows Credential Manager 解密金鑰。

## 安全性

- 移除硬編碼 API 金鑰，改由系統金鑰環 (Keyring) 儲存敏感資料。
- 維持 SQLCipher + Argon2id + recovery phrase 的本機加密知識庫設計。

## 新功能與改進

- 影片字幕提取新增 Audio to Text / Whisper 本地降級流程。
- 優化 Video 解析器，加入 WBI 簽名機制與智慧去重演算法。
- 改善初始設定流程、品牌識別、本地模型管理與安裝/解除安裝語意。
- 修正多處 UI/UX 與多語系顯示問題。
