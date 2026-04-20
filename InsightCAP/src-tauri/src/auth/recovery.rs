use super::AuthError;
/// recovery.bin 二進制格式 v1（共 77 bytes）
///
/// 偏移  長度  欄位
/// 0     1     版本號（0x01）
/// 1     16    Argon2id salt（用於從 mnemonic 衍生 recovery_key）
/// 17    12    ChaCha20-Poly1305 nonce
/// 29    32    密文（db_key 32 bytes 加密後仍 32 bytes）
/// 61    16    Poly1305 認證標籤
///       77    total
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
    /// Argon2id salt（偏移 1–16），存入文件，讀取時回傳供 verify 使用
    pub salt: [u8; 16],
}

/// 將 db_key 用 recovery_key 加密後寫入 recovery.bin
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

    // AEAD 加密：32 bytes 明文 → 32 bytes 密文 + 16 bytes tag（crate 自動附加）
    let ciphertext = cipher
        .encrypt(nonce, db_key.as_ref())
        .map_err(|e| AuthError::Encryption(e.to_string()))?;
    // ciphertext.len() == 48 (32 密文 + 16 tag)

    let mut buf = [0u8; RECOVERY_BIN_SIZE];
    buf[0] = RECOVERY_BIN_VERSION;
    buf[1..17].copy_from_slice(salt);
    buf[17..29].copy_from_slice(&nonce_bytes);
    buf[29..77].copy_from_slice(&ciphertext); // 48 bytes

    // 原子寫入：先寫 .tmp，再 rename
    let tmp_path = path.with_extension("bin.tmp");
    std::fs::write(&tmp_path, &buf).map_err(|e| AuthError::Io(e.to_string()))?;
    std::fs::rename(&tmp_path, path).map_err(|e| AuthError::Io(e.to_string()))?;

    Ok(())
}

/// 讀取 recovery.bin 並用 recovery_key 解密，回傳 db_key
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
    let ciphertext = &data[29..77]; // 32 密文 + 16 tag = 48 bytes

    let cipher = ChaCha20Poly1305::new(Key::from_slice(recovery_key));
    let mut plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| AuthError::DecryptionFailed)?;

    let mut db_key = [0u8; 32];
    db_key.copy_from_slice(&plaintext);
    plaintext.zeroize();

    Ok((db_key, RecoveryBin { salt }))
}
