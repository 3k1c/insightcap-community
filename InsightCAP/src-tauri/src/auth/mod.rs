pub mod key_derivation;
pub mod login_guard;
pub mod recovery;

pub use key_derivation::{derive_db_key, derive_recovery_key_new, derive_recovery_key_verify};
pub use login_guard::{load_login_guard, persist_login_guard, LoginGuard};
pub use recovery::{read_recovery_bin, write_recovery_bin, RecoveryBin};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("Invalid mnemonic phrase")]
    InvalidMnemonic,
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption failed (wrong key or corrupted data)")]
    DecryptionFailed,
    #[error("Unsupported recovery.bin format version: {0}")]
    UnsupportedFormat(u8),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Keychain error: {0}")]
    Keychain(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
}
