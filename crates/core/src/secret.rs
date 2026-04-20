use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngExt;

use crate::{ApplicationError, ApplicationResult};

const SECRET_PREFIX: &str = "enc:v1:";

#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256GcmSiv,
}

impl SecretBox {
    pub fn new(encoded_key: impl AsRef<str>) -> ApplicationResult<Self> {
        let key = decode_key(encoded_key.as_ref())?;
        let cipher = Aes256GcmSiv::new_from_slice(&key)
            .map_err(|_| ApplicationError::Validation("invalid data encryption key".into()))?;
        Ok(Self { cipher })
    }

    pub fn is_encrypted(&self, value: &str) -> bool {
        value.starts_with(SECRET_PREFIX)
    }

    pub fn encrypt(&self, value: &str) -> ApplicationResult<String> {
        if self.is_encrypted(value) {
            return Ok(value.to_owned());
        }
        let nonce = rand::rng().random::<[u8; 12]>();
        let nonce_block: Nonce = nonce.into();
        let ciphertext = self
            .cipher
            .encrypt(&nonce_block, value.as_bytes())
            .map_err(|_| ApplicationError::Infrastructure("failed to encrypt secret".into()))?;
        Ok(format!(
            "{SECRET_PREFIX}{}:{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, value: &str) -> ApplicationResult<String> {
        let Some(payload) = value.strip_prefix(SECRET_PREFIX) else {
            return Ok(value.to_owned());
        };
        let (nonce, ciphertext) = payload.split_once(':').ok_or_else(|| {
            ApplicationError::Infrastructure("invalid encrypted secret payload".into())
        })?;
        let nonce = URL_SAFE_NO_PAD.decode(nonce).map_err(|_| {
            ApplicationError::Infrastructure("invalid encrypted secret nonce".into())
        })?;
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| {
            ApplicationError::Infrastructure("invalid encrypted secret nonce".into())
        })?;
        let nonce_block: Nonce = nonce.into();
        let ciphertext = URL_SAFE_NO_PAD.decode(ciphertext).map_err(|_| {
            ApplicationError::Infrastructure("invalid encrypted secret ciphertext".into())
        })?;
        let plaintext = self
            .cipher
            .decrypt(&nonce_block, ciphertext.as_ref())
            .map_err(|_| ApplicationError::Infrastructure("failed to decrypt secret".into()))?;
        String::from_utf8(plaintext)
            .map_err(|_| ApplicationError::Infrastructure("invalid decrypted secret".into()))
    }

    pub fn encrypt_option(&self, value: Option<&str>) -> ApplicationResult<Option<String>> {
        value.map(|value| self.encrypt(value)).transpose()
    }

    pub fn decrypt_option(&self, value: Option<&str>) -> ApplicationResult<Option<String>> {
        value.map(|value| self.decrypt(value)).transpose()
    }
}

fn decode_key(value: &str) -> ApplicationResult<[u8; 32]> {
    let trimmed = value.trim();
    let bytes = if trimmed.len() == 64 && trimmed.chars().all(|char| char.is_ascii_hexdigit()) {
        decode_hex(trimmed)?
    } else {
        URL_SAFE_NO_PAD
            .decode(trimmed)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(trimmed))
            .map_err(|_| ApplicationError::Validation("invalid data encryption key".into()))?
    };
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ApplicationError::Validation("data encryption key must be 32 bytes".into()))?;
    Ok(key)
}

fn decode_hex(value: &str) -> ApplicationResult<Vec<u8>> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk)
                .map_err(|_| ApplicationError::Validation("invalid data encryption key".into()))?;
            u8::from_str_radix(text, 16)
                .map_err(|_| ApplicationError::Validation("invalid data encryption key".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SecretBox;

    #[test]
    fn encrypts_and_decrypts_roundtrip() {
        let secret_box =
            SecretBox::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                .expect("secret box");
        let encrypted = secret_box.encrypt("secret-value").expect("encrypt");

        assert!(encrypted.starts_with("enc:v1:"));
        assert_ne!(encrypted, "secret-value");
        assert_eq!(
            secret_box.decrypt(&encrypted).expect("decrypt"),
            "secret-value"
        );
    }

    #[test]
    fn decrypt_accepts_legacy_plaintext() {
        let secret_box =
            SecretBox::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                .expect("secret box");

        assert_eq!(
            secret_box.decrypt("legacy-plaintext").expect("plaintext"),
            "legacy-plaintext"
        );
    }
}
