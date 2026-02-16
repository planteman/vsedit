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

/// Generates a deterministic salt from a counter (for testing reproducibility).
pub fn generate_salt(len: usize) -> Vec<u8> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut salt = vec![0u8; len];
    for i in 0..len {
        salt[i] = ((counter.wrapping_mul(31).wrapping_add(i as u64)) & 0xFF) as u8;
    }
    salt
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes to a base64 string.
pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Decodes a base64 string back to bytes. Returns `None` on invalid input.
pub fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    fn decode_char(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    }

    let bytes = encoded.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut result = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() != 4 {
            return None;
        }
        let a = decode_char(chunk[0])?;
        let b = decode_char(chunk[1])?;
        let c = decode_char(chunk[2])?;
        let d = decode_char(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        result.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            result.push((triple & 0xFF) as u8);
        }
    }
    Some(result)
}

/// Metadata about a derived key.
pub struct KeyInfo {
    pub algorithm: String,
    pub key_length: usize,
    pub iterations: u32,
    pub salt: Option<Vec<u8>>,
}

/// Configuration for encryption parameters.
pub struct EncryptionConfig {
    pub algorithm: String,
    pub iterations: u32,
    pub salt_length: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: "XOR".to_string(),
            iterations: 1000,
            salt_length: 16,
        }
    }
}

/// Verifies a passphrase by encrypting a known marker and comparing.
pub fn verify_passphrase(passphrase: &str, encrypted_marker: &[u8], key: &[u8]) -> bool {
    let svc = EncryptionService::new(key.to_vec());
    let decrypted = svc.decrypt(encrypted_marker);
    let expected_marker = passphrase.as_bytes();
    decrypted == expected_marker
}

/// Simple HMAC-like signing: XOR key with a hash of the data.
pub fn hmac_sign(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut hash = vec![0u8; 32];
    for (i, &b) in data.iter().enumerate() {
        hash[i % 32] ^= b;
        hash[i % 32] = hash[i % 32].wrapping_add(b.wrapping_mul(17));
        hash[(i + 1) % 32] = hash[(i + 1) % 32].wrapping_add(b.wrapping_mul(13));
    }
    let mut signature = vec![0u8; 32];
    for i in 0..32 {
        signature[i] = hash[i] ^ key[i % key.len()];
    }
    signature
}

/// Verifies an HMAC-like signature.
pub fn hmac_verify(key: &[u8], data: &[u8], signature: &[u8]) -> bool {
    let expected = hmac_sign(key, data);
    expected == signature
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

    /// Encrypt data and return as a base64-encoded string.
    pub fn encrypt_to_base64(&self, data: &[u8]) -> String {
        let encrypted = self.encrypt(data);
        base64_encode(&encrypted)
    }

    /// Decrypt from a base64-encoded string.
    pub fn decrypt_from_base64(&self, encoded: &str) -> Option<Vec<u8>> {
        let encrypted = base64_decode(encoded)?;
        Some(self.decrypt(&encrypted))
    }

    /// Returns metadata about the encryption key.
    pub fn get_key_info(&self) -> KeyInfo {
        KeyInfo {
            algorithm: "XOR".to_string(),
            key_length: self.key.len(),
            iterations: 1000,
            salt: None,
        }
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

    #[test]
    fn generate_salt_has_correct_length() {
        let salt = generate_salt(16);
        assert_eq!(salt.len(), 16);
        let salt2 = generate_salt(32);
        assert_eq!(salt2.len(), 32);
    }

    #[test]
    fn generate_salt_increments() {
        let s1 = generate_salt(8);
        let s2 = generate_salt(8);
        assert_ne!(s1, s2, "sequential salts should differ");
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_round_trip() {
        let data = b"Hello, world!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base64_known_value() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn base64_decode_invalid() {
        assert!(base64_decode("!!!").is_none());
        assert!(base64_decode("A").is_none());
    }

    #[test]
    fn encrypt_to_base64_round_trip() {
        let svc = EncryptionService::from_passphrase("test-key");
        let data = b"secret message";
        let encoded = svc.encrypt_to_base64(data);
        let decrypted = svc.decrypt_from_base64(&encoded).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn decrypt_from_base64_invalid() {
        let svc = EncryptionService::from_passphrase("key");
        assert!(svc.decrypt_from_base64("!!!").is_none());
    }

    #[test]
    fn key_info_defaults() {
        let svc = EncryptionService::from_passphrase("pass");
        let info = svc.get_key_info();
        assert_eq!(info.algorithm, "XOR");
        assert_eq!(info.key_length, 32);
        assert_eq!(info.iterations, 1000);
        assert!(info.salt.is_none());
    }

    #[test]
    fn encryption_config_default() {
        let config = EncryptionConfig::default();
        assert_eq!(config.algorithm, "XOR");
        assert_eq!(config.iterations, 1000);
        assert_eq!(config.salt_length, 16);
    }

    #[test]
    fn hmac_sign_verify() {
        let key = b"my-secret-key";
        let data = b"important message";
        let sig = hmac_sign(key, data);
        assert_eq!(sig.len(), 32);
        assert!(hmac_verify(key, data, &sig));
    }

    #[test]
    fn hmac_verify_wrong_data() {
        let key = b"key";
        let sig = hmac_sign(key, b"data1");
        assert!(!hmac_verify(key, b"data2", &sig));
    }

    #[test]
    fn hmac_verify_wrong_key() {
        let data = b"data";
        let sig = hmac_sign(b"key1", data);
        assert!(!hmac_verify(b"key2", data, &sig));
    }

    #[test]
    fn verify_passphrase_correct() {
        let passphrase = "my-pass";
        let key = derive_key(passphrase);
        let svc = EncryptionService::new(key.clone());
        let marker = svc.encrypt(passphrase.as_bytes());
        assert!(verify_passphrase(passphrase, &marker, &key));
    }

    #[test]
    fn verify_passphrase_wrong() {
        let passphrase = "my-pass";
        let key = derive_key(passphrase);
        let svc = EncryptionService::new(key.clone());
        let marker = svc.encrypt(passphrase.as_bytes());
        let wrong_key = derive_key("wrong-pass");
        assert!(!verify_passphrase(passphrase, &marker, &wrong_key));
    }
}
