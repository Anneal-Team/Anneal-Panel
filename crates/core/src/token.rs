use hmac::{Hmac, Mac};
use rand::{Rng, distr::Alphanumeric};
use sha2::Sha256;

use crate::{ApplicationError, ApplicationResult};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct TokenHasher {
    key: Vec<u8>,
}

impl TokenHasher {
    pub fn new(key: &str) -> ApplicationResult<Self> {
        let key = key.trim();
        if key.is_empty() {
            return Err(ApplicationError::Validation(
                "ANNEAL_TOKEN_HASH_KEY is required".into(),
            ));
        }
        Ok(Self {
            key: key.as_bytes().to_vec(),
        })
    }

    pub fn hash(&self, value: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("valid hmac key");
        mac.update(value.as_bytes());
        hex_encode(&mac.finalize().into_bytes())
    }
}

pub fn generate_token(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::TokenHasher;

    #[test]
    fn keyed_hash_is_stable() {
        let hasher = TokenHasher::new("secret-key").expect("hasher");
        assert_eq!(hasher.hash("value"), hasher.hash("value"));
        assert_ne!(hasher.hash("value"), hasher.hash("other"));
    }
}
