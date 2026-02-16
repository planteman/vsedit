//! Simple XOR-based encryption service (not cryptographically secure).

/// Derives a key from a passphrase using a simple hash-based approach.
pub fn derive_key(passphrase: &str) -> Vec<u8> {
    let bytes = passphrase.as_bytes();
    let key_len = 32;
    let mut key = vec![0u8; key_len];
    for (i, &b) in bytes.iter().enumerate() {
        key[i % key_len] ^= b;
        // mix bits to improve distribution
        key[i % key_len] = key[i % key_len].wrapping_add(b.wrapping_mul(31));
    }
    // second pass to spread entropy
    for i in 1..key_len {
        key[i] ^= key[i - 1].wrapping_add(i as u8);
    }
    key
}

/// XOR-based encryption service.
pub struct EncryptionService {
    key: Vec<u8>,
}

impl EncryptionService {
    pub fn new(key: Vec<u8>) -> Self {
        assert!(!key.is_empty(), "encryption key must not be empty");
        Self { key }
    }

    /// Encrypt data by XOR-ing with the repeating key.
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ self.key[i % self.key.len()])
            .collect()
    }

    /// Decrypt data. XOR is symmetric so this is identical to encrypt.
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.encrypt(data)
    }

    /// Encrypt a UTF-8 string.
    pub fn encrypt_string(&self, text: &str) -> Vec<u8> {
        self.encrypt(text.as_bytes())
    }

    /// Decrypt data back into a UTF-8 string, returning `None` if invalid.
    pub fn decrypt_string(&self, data: &[u8]) -> Option<String> {
        let decrypted = self.decrypt(data);
        String::from_utf8(decrypted).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encryption() {
        let svc = EncryptionService::new(vec![0xAB, 0xCD, 0xEF]);
        let original = b"Hello, world!";
        let encrypted = svc.encrypt(original);
        let decrypted = svc.decrypt(&encrypted);
        assert_eq!(decrypted, original);
        assert_ne!(encrypted, original.to_vec());
    }

    #[test]
    fn string_encrypt_decrypt() {
        let key = derive_key("my secret passphrase");
        let svc = EncryptionService::new(key);
        let text = "sensitive data 🔑";
        let encrypted = svc.encrypt_string(text);
        let decrypted = svc.decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, text);
    }

    #[test]
    fn different_keys_produce_different_output() {
        let svc1 = EncryptionService::new(vec![0x01, 0x02]);
        let svc2 = EncryptionService::new(vec![0x03, 0x04]);
        let data = b"same input data";
        let enc1 = svc1.encrypt(data);
        let enc2 = svc2.encrypt(data);
        assert_ne!(enc1, enc2);
    }
}
