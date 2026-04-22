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
        .map_err(|e| format!("Decryption failed: {}", e))
}

pub fn is_encrypted(data: &str) -> bool {
    if data.is_empty() {
        return false;
    }
    decrypt(data).is_ok()
}
