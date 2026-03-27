use argon2::{Argon2, Params, Algorithm, Version};
use bip39::{Mnemonic, Language};
use rand::Rng;
use super::AuthError;

/// 從用戶密碼衍生數據庫加密 key
/// Argon2id：記憶體 64MB，迭代 3 次，單線程，輸出 256-bit
pub fn derive_db_key(password: &str, salt: &[u8]) -> Result<[u8; 32], AuthError> {
    let params = Params::new(
        65536, // 64 MB
        3,     // iterations
        1,     // parallelism
        Some(32),
    )
    .map_err(|e| AuthError::KeyDerivation(e.to_string()))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| AuthError::KeyDerivation(e.to_string()))?;

    Ok(key)
}

/// 生成 24 個單詞的 BIP-39 恢復碼短語
pub fn generate_mnemonic() -> String {
    // 生成 256-bit entropy → 24 個單詞
    let mut entropy = [0u8; 32];
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy).expect("32-byte entropy always valid");
    mnemonic.to_string()
}

/// 建立新 recovery key（生成隨機 16-byte salt）
/// 回傳 (recovery_key, salt)，salt 存入 recovery.bin header
pub fn derive_recovery_key_new(
    mnemonic_phrase: &str,
) -> Result<([u8; 32], [u8; 16]), AuthError> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_phrase)
        .map_err(|_| AuthError::InvalidMnemonic)?;
    let entropy = mnemonic.to_entropy();

    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);

    let key = derive_db_key(&hex::encode(&entropy), &salt)?;
    Ok((key, salt))
}

/// 從現有 recovery.bin 驗證（使用 header 中儲存的 salt）
pub fn derive_recovery_key_verify(
    mnemonic_phrase: &str,
    stored_salt: &[u8; 16],
) -> Result<[u8; 32], AuthError> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_phrase)
        .map_err(|_| AuthError::InvalidMnemonic)?;
    let entropy = mnemonic.to_entropy();
    derive_db_key(&hex::encode(&entropy), stored_salt)
}
