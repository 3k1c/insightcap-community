use super::AuthError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LoginGuard {
    pub version: u32,
    pub fail_count: u32,
    /// Unix 秒數，None 表示未鎖定
    pub locked_until_unix: Option<u64>,
}

impl LoginGuard {
    pub fn record_failure(&mut self) {
        self.fail_count += 1;
        let wait_secs: u64 = match self.fail_count {
            0..=4 => 0,
            5..=9 => 30,
            10..=19 => 300,    // 5 分鐘
            20..=49 => 3600,   // 1 小時
            _ => u64::MAX / 2, // 永久鎖定（只能用恢復碼）
        };
        if wait_secs > 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.locked_until_unix = Some(now.saturating_add(wait_secs));
        }
    }

    pub fn record_success(&mut self) {
        self.fail_count = 0;
        self.locked_until_unix = None;
    }

    /// 回傳剩餘鎖定時間，None 表示未鎖定
    pub fn remaining_lock(&self) -> Option<Duration> {
        let until = self.locked_until_unix?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if until > now {
            Some(Duration::from_secs(until - now))
        } else {
            None
        }
    }

    pub fn is_permanently_locked(&self) -> bool {
        self.fail_count >= 50
    }
}

/// 原子寫入：先寫 .json.tmp，再 rename（NTFS 同卷 rename 為原子操作）
pub fn persist_login_guard(guard: &LoginGuard, path: &Path) -> Result<(), AuthError> {
    let tmp_path = path.with_extension("json.tmp");
    let json = serde_json::to_string(guard).map_err(|e| AuthError::Serialization(e.to_string()))?;
    std::fs::write(&tmp_path, &json).map_err(|e| AuthError::Io(e.to_string()))?;
    std::fs::rename(&tmp_path, path).map_err(|e| AuthError::Io(e.to_string()))?;
    Ok(())
}

/// 讀取失敗計數；文件損毀或遺失時回退 fail_count = 0（寬鬆策略）
pub fn load_login_guard(path: &Path) -> LoginGuard {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
