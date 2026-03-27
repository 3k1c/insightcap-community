use magic_crypt::{new_magic_crypt, MagicCrypt256, MagicCryptTrait};
use std::sync::OnceLock;

static MC: OnceLock<MagicCrypt256> = OnceLock::new();

fn get_mc() -> &'static MagicCrypt256 {
    MC.get_or_init(|| new_magic_crypt!("insightcap-internal-secure-key-256", 256))
}

pub fn encrypt(data: &str) -> String {
    if data.is_empty() {
        return String::new();
    }
    get_mc().encrypt_str_to_base64(data)
}

pub fn decrypt(data: &str) -> Result<String, String> {
    if data.is_empty() {
        return Ok(String::new());
    }
    get_mc()
        .decrypt_base64_to_string(data)
        .map_err(|e| format!("解密失敗: {}", e))
}

/// 檢查字串是否為加密過的（簡單啟發式判斷）
pub fn is_encrypted(data: &str) -> bool {
    // 如果可以成功解密，則認為是加密過的
    // 這裡使用 base64 格式判斷作為初步過濾
    if data.is_empty() {
        return false;
    }
    decrypt(data).is_ok()
}
