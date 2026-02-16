//! Simple XOR-based encryption service (not cryptographically secure).

/// Trait for encryption services.
pub trait Encryptor {
    fn encrypt(&self, data: &[u8]) -> Vec<u8>;
    fn decrypt(&self, data: &[u8]) -> Vec<u8>;
}

/// Derives a key from a passphrase using a simple hash-based approach.
pub fn derive_key(passphrase: &str) -> Vec<u8> {
    let bytes = passphrase.as_bytes();
    let key_len = 32;
    let mut key = vec![0u8; key_len];
    for (i, &b) in bytes.iter().enumerate() {
        key[i % key_len] ^= b;
        key[i % key_len] = key[i % key_len].wrapping_add(b.wrapping_mul(31));
    }
    for i in 1..key_len {
        key[i] ^= key[i - 1].wrapping_add(i as u8);
    }
    key
}

/// Derives a key from a passphrase and salt using PBKDF2-style iterated hashing.
pub fn derive_key_with_salt(password: &str, salt: &[u8], iterations: u32) -> Vec<u8> {
    let mut key = derive_key(password);
    // Mix in salt
    let key_len = key.len();
    for (i, &s) in salt.iter().enumerate() {
        key[i % key_len] ^= s;
    }
    // Iterate to strengthen
    for round in 0..iterations {
        for i in 0..key.len() {
            key[i] = key[i]
                .wrapping_add(round as u8)
                .wrapping_mul(31)
                .wrapping_add(key[(i + 1) % key.len()]);
        }
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

    pub fn from_passphrase(passphrase: &str) -> Self {
        Self::new(derive_key(passphrase))
    }

    pub fn from_passphrase_with_salt(password: &str, salt: &[u8]) -> Self {
        Self::new(derive_key_with_salt(password, salt, 1000))
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

impl Encryptor for EncryptionService {
    /// Encrypt data by XOR-ing with the repeating key.
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ self.key[i % self.key.len()])
            .collect()
    }

    /// Decrypt data. XOR is symmetric so this is identical to encrypt.
    fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.encrypt(data)
    }
}

/// Stub AES-256-GCM encryption service (interface only, uses XOR internally).
pub struct AesEncryption {
    inner: EncryptionService,
}

impl AesEncryption {
    /// Create from a 32-byte key. Panics if key length is not 32.
    pub fn new(key: Vec<u8>) -> Self {
        assert_eq!(key.len(), 32, "AES-256 requires a 32-byte key");
        Self {
            inner: EncryptionService::new(key),
        }
    }

    pub fn from_password(password: &str, salt: &[u8]) -> Self {
        let key = derive_key_with_salt(password, salt, 1000);
        Self::new(key)
    }
}

impl Encryptor for AesEncryption {
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        self.inner.encrypt(data)
    }

    fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.inner.decrypt(data)
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

    #[test]
    fn derive_key_with_salt_deterministic() {
        let k1 = derive_key_with_salt("pass", b"salt", 100);
        let k2 = derive_key_with_salt("pass", b"salt", 100);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn derive_key_with_salt_different_salt() {
        let k1 = derive_key_with_salt("pass", b"salt1", 100);
        let k2 = derive_key_with_salt("pass", b"salt2", 100);
        assert_ne!(k1, k2);
    }

    #[test]
    fn from_passphrase_round_trip() {
        let svc = EncryptionService::from_passphrase("my-password");
        let data = b"secret";
        assert_eq!(svc.decrypt(&svc.encrypt(data)), data);
    }

    #[test]
    fn aes_encryption_round_trip() {
        let aes = AesEncryption::from_password("password", b"random-salt");
        let data = b"important data";
        let encrypted = aes.encrypt(data);
        let decrypted = aes.decrypt(&encrypted);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn encryptor_trait_polymorphism() {
        let svc: Box<dyn Encryptor> = Box::new(EncryptionService::from_passphrase("key"));
        let data = b"trait test";
        let enc = svc.encrypt(data);
        let dec = svc.decrypt(&enc);
        assert_eq!(dec, data);
    }
}
