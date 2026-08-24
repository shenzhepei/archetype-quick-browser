use std::path::Path;

use arch_store::EncryptedCookieState;
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use sha2::{Digest, Sha256};
use thiserror::Error;

const COOKIE_KEY_BYTES: usize = 32;
const COOKIE_NONCE_BYTES: usize = 24;
const COOKIE_STATE_AAD: &[u8] = b"archetype-cookie-state-v1";
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.shenzhepei.archetype-quick-browser.cookies";

#[derive(Debug, Error)]
pub enum CookieCipherError {
    #[error("could not access profile Cookie key: {0}")]
    KeyAccess(String),
    #[error("profile Cookie key has an invalid length")]
    InvalidKey,
    #[error("encrypted Cookie state has an invalid nonce")]
    InvalidNonce,
    #[error("could not encrypt profile Cookie state")]
    Encrypt,
    #[error("could not decrypt profile Cookie state")]
    Decrypt,
    #[cfg(not(target_os = "macos"))]
    #[error("persistent Cookie profiles require macOS Keychain")]
    UnsupportedPlatform,
}

pub struct CookieCipher {
    key: [u8; COOKIE_KEY_BYTES],
}

impl CookieCipher {
    pub fn for_profile(path: &Path) -> Result<Self, CookieCipherError> {
        profile_key(path).map(|key| Self { key })
    }

    pub fn ephemeral() -> Result<Self, CookieCipherError> {
        let mut key = [0_u8; COOKIE_KEY_BYTES];
        getrandom::fill(&mut key)
            .map_err(|error| CookieCipherError::KeyAccess(error.to_string()))?;
        Ok(Self { key })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedCookieState, CookieCipherError> {
        let mut nonce = [0_u8; COOKIE_NONCE_BYTES];
        getrandom::fill(&mut nonce)
            .map_err(|error| CookieCipherError::KeyAccess(error.to_string()))?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| CookieCipherError::InvalidKey)?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: COOKIE_STATE_AAD,
                },
            )
            .map_err(|_| CookieCipherError::Encrypt)?;
        Ok(EncryptedCookieState {
            nonce: nonce.to_vec(),
            ciphertext,
        })
    }

    pub fn decrypt(&self, state: &EncryptedCookieState) -> Result<Vec<u8>, CookieCipherError> {
        let nonce: [u8; COOKIE_NONCE_BYTES] = state
            .nonce
            .as_slice()
            .try_into()
            .map_err(|_| CookieCipherError::InvalidNonce)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&self.key)
            .map_err(|_| CookieCipherError::InvalidKey)?;
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &state.ciphertext,
                    aad: COOKIE_STATE_AAD,
                },
            )
            .map_err(|_| CookieCipherError::Decrypt)
    }

    pub(crate) const fn from_key(key: [u8; COOKIE_KEY_BYTES]) -> Self {
        Self { key }
    }
}

#[cfg(target_os = "macos")]
fn profile_key(path: &Path) -> Result<[u8; COOKIE_KEY_BYTES], CookieCipherError> {
    use security_framework::passwords::{get_generic_password, set_generic_password};
    use security_framework_sys::base::errSecItemNotFound;

    let account = profile_account(path)?;
    let key = match get_generic_password(KEYCHAIN_SERVICE, &account) {
        Ok(key) => key,
        Err(error) if error.code() == errSecItemNotFound => {
            let mut key = vec![0_u8; COOKIE_KEY_BYTES];
            getrandom::fill(&mut key)
                .map_err(|error| CookieCipherError::KeyAccess(error.to_string()))?;
            set_generic_password(KEYCHAIN_SERVICE, &account, &key)
                .map_err(|error| CookieCipherError::KeyAccess(error.to_string()))?;
            key
        }
        Err(error) => return Err(CookieCipherError::KeyAccess(error.to_string())),
    };
    key.try_into().map_err(|_| CookieCipherError::InvalidKey)
}

#[cfg(not(target_os = "macos"))]
fn profile_key(_path: &Path) -> Result<[u8; COOKIE_KEY_BYTES], CookieCipherError> {
    Err(CookieCipherError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
fn profile_account(path: &Path) -> Result<String, CookieCipherError> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|error| CookieCipherError::KeyAccess(error.to_string()))?
            .join(path)
    };
    let identity = match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(file_name)) => parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_owned())
            .join(file_name),
        _ => absolute,
    };
    let digest = Sha256::digest(identity.to_string_lossy().as_bytes());
    Ok(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypts_cookie_values_and_authenticates_ciphertext() {
        let cipher = CookieCipher::from_key([7; COOKIE_KEY_BYTES]);
        let plaintext = br#"{"cookie":"session=plain-secret"}"#;
        let encrypted = cipher.encrypt(plaintext).unwrap();

        assert_eq!(encrypted.nonce.len(), COOKIE_NONCE_BYTES);
        assert!(
            !encrypted
                .ciphertext
                .windows(b"plain-secret".len())
                .any(|window| window == b"plain-secret")
        );
        assert_eq!(cipher.decrypt(&encrypted).unwrap(), plaintext);

        let mut tampered = encrypted;
        tampered.ciphertext[0] ^= 1;
        assert!(matches!(
            cipher.decrypt(&tampered),
            Err(CookieCipherError::Decrypt)
        ));
    }
}
