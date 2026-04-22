use super::AuthError;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::Rng;
use std::path::Path;
use zeroize::Zeroize;

pub const RECOVERY_BIN_VERSION: u8 = 0x01;
pub const RECOVERY_BIN_SIZE: usize = 77;

pub struct RecoveryBin {
    pub salt: [u8; 16],
}

pub fn write_recovery_bin(
    path: &Path,
    db_key: &[u8; 32],
    recovery_key: &[u8; 32],
    salt: &[u8; 16],
) -> Result<(), AuthError> {
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);

    let cipher = ChaCha20Poly1305::new(Key::from_slice(recovery_key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, db_key.as_ref())
        .map_err(|e| AuthError::Encryption(e.to_string()))?;

    let mut buf = [0u8; RECOVERY_BIN_SIZE];
    buf[0] = RECOVERY_BIN_VERSION;
    buf[1..17].copy_from_slice(salt);
    buf[17..29].copy_from_slice(&nonce_bytes);
    buf[29..77].copy_from_slice(&ciphertext); // 48 bytes

    let tmp_path = path.with_extension("bin.tmp");
    std::fs::write(&tmp_path, &buf).map_err(|e| AuthError::Io(e.to_string()))?;
    std::fs::rename(&tmp_path, path).map_err(|e| AuthError::Io(e.to_string()))?;

    Ok(())
}

pub fn read_recovery_bin(
    path: &Path,
    recovery_key: &[u8; 32],
) -> Result<([u8; 32], RecoveryBin), AuthError> {
    let data = std::fs::read(path).map_err(|e| AuthError::Io(e.to_string()))?;

    if data.len() != RECOVERY_BIN_SIZE {
        return Err(AuthError::DecryptionFailed);
    }

    let version = data[0];
    if version != RECOVERY_BIN_VERSION {
        return Err(AuthError::UnsupportedFormat(version));
    }

    let mut salt = [0u8; 16];
    salt.copy_from_slice(&data[1..17]);

    let nonce = Nonce::from_slice(&data[17..29]);
    let ciphertext = &data[29..77]; // 32 ciphertext + 16 tag = 48 bytes

    let cipher = ChaCha20Poly1305::new(Key::from_slice(recovery_key));
    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AuthError::DecryptionFailed)?;

    let mut db_key = [0u8; 32];
    db_key.copy_from_slice(&plaintext);
    plaintext.zeroize();

    Ok((db_key, RecoveryBin { salt }))
}
