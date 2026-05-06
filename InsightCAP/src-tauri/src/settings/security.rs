use keyring::Entry;
use magic_crypt::{new_magic_crypt, MagicCrypt256, MagicCryptTrait};
use std::sync::OnceLock;
use uuid::Uuid;

static MC: OnceLock<MagicCrypt256> = OnceLock::new();
const OLD_HARDCODED_KEY: &str = "insightcap-internal-secure-key-256";

fn get_encryption_key() -> String {
    let entry_result = Entry::new("insightcap", "internal_secure_key");
    if let Ok(entry) = entry_result {
        if let Ok(key) = entry.get_password() {
            return key;
        }

        // Generate a new unique key if not found
        let new_key = Uuid::now_v7().to_string();
        if entry.set_password(&new_key).is_ok() {
            return new_key;
        }
    }

    // Fallback to old key if keyring is unavailable (not ideal, but ensures app works)
    OLD_HARDCODED_KEY.to_string()
}

fn get_mc() -> &'static MagicCrypt256 {
    MC.get_or_init(|| {
        let key = get_encryption_key();
        new_magic_crypt!(key, 256)
    })
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

    // Try with the current key (likely keyring-based)
    let mc = get_mc();
    if let Ok(decrypted) = mc.decrypt_base64_to_string(data) {
        return Ok(decrypted);
    }

    // Fallback: Try with the old hardcoded key for backward compatibility
    let old_mc = new_magic_crypt!(OLD_HARDCODED_KEY, 256);
    old_mc
        .decrypt_base64_to_string(data)
        .map_err(|e| format!("Decryption failed: {}", e))
}

pub fn is_encrypted(data: &str) -> bool {
    if data.is_empty() {
        return false;
    }
    decrypt(data).is_ok()
}
