-- 007: 補救 local_doc_path 欄位
-- 若 006 migration 已記錄但 ALTER TABLE 未成功，此處重新嘗試
-- 若欄位已存在，migration runner 會自動跳過 "duplicate column" 錯誤

ALTER TABLE sources ADD COLUMN local_doc_path TEXT;
