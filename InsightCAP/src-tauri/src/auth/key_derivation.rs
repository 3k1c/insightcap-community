use super::AuthError;
use argon2::{Algorithm, Argon2, Params, Version};
use bip39::{Language, Mnemonic};
use rand::Rng;

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

pub fn generate_mnemonic() -> String {
    let mut entropy = [0u8; 32];
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy).expect("32-byte entropy always valid");
    mnemonic.to_string()
}

pub fn derive_recovery_key_new(mnemonic_phrase: &str) -> Result<([u8; 32], [u8; 16]), AuthError> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_phrase)
        .map_err(|_| AuthError::InvalidMnemonic)?;
    let entropy = mnemonic.to_entropy();

    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);

    let key = derive_db_key(&hex::encode(&entropy), &salt)?;
    Ok((key, salt))
}

pub fn derive_recovery_key_verify(
    mnemonic_phrase: &str,
    stored_salt: &[u8; 16],
) -> Result<[u8; 32], AuthError> {
    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic_phrase)
        .map_err(|_| AuthError::InvalidMnemonic)?;
    let entropy = mnemonic.to_entropy();
    derive_db_key(&hex::encode(&entropy), stored_salt)
}
