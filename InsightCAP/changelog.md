# 1.0.0 (2026-04-30)

## 🎉 核心亮點 (Highlights)
*   **InsightCAP 1.0.0 正式版發佈**

## 🔒 安全性更新 (Security)
*   **修復套件漏洞**：更新前端依賴套件（Vite, DOMPurify, PostCSS）修復中高度安全漏洞。
*   **增強金鑰安全**：移除所有硬編碼 API 金鑰，並全面引入系統金鑰環 (Keyring) 加密機制，大幅提升資訊安全。

## ✨ 新功能與改進 (Features & Improvements)
*   **模型支援升級**：Video 字幕提取新增 Audio to Text 語音辨識模型支援。
*   **效能與解析優化**：優化 Video 解析器，加入 WBI 簽名機制與智慧去重演算法，並改善中文亂碼與編碼問題。
*   **UI/UX 優化**：
    *   優化大螢幕 (XL screens) 的知識庫排版佈局。
    *   調整全域 Toast 通知顯示位置，避免遮擋 UI 元件。
    *   優化設定面板的使用者介面及本地模型管理。
    *   修復排程頁面 (Schedule Page) 的多國語言切換問題。
*   **架構調整**：更新並優化 Whisper 音訊處理架構及 Tray Status 生命週期機制。
