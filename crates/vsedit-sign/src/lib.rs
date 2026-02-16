//! Content signing and integrity verification.

/// Stub signature algorithm identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    HmacSha256Stub,
    Ed25519Stub,
}

/// A computed signature.
#[derive(Debug, Clone)]
pub struct Signature {
    pub algorithm: SignatureAlgorithm,
    pub value: Vec<u8>,
    pub signer: Option<String>,
}

/// Produce a stub signature by XOR-folding `content` with `key`.
pub fn sign_content(
    content: &[u8],
    key: &[u8],
    algorithm: SignatureAlgorithm,
) -> Signature {
    let value = xor_fold(content, key);
    Signature { algorithm, value, signer: None }
}

/// Verify a signature by recomputing and comparing.
pub fn verify_signature(content: &[u8], key: &[u8], signature: &Signature) -> bool {
    let expected = xor_fold(content, key);
    expected == signature.value
}

/// XOR-fold: XOR each content byte with the corresponding key byte (cycling).
fn xor_fold(content: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return content.to_vec();
    }
    content
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let content = b"hello world";
        let key = b"secret";
        let sig = sign_content(content, key, SignatureAlgorithm::HmacSha256Stub);
        assert!(verify_signature(content, key, &sig));
    }

    #[test]
    fn wrong_key_fails() {
        let content = b"hello world";
        let sig = sign_content(content, b"key1", SignatureAlgorithm::Ed25519Stub);
        assert!(!verify_signature(content, b"key2", &sig));
    }

    #[test]
    fn tampered_content_fails() {
        let content = b"original";
        let key = b"k";
        let sig = sign_content(content, key, SignatureAlgorithm::HmacSha256Stub);
        assert!(!verify_signature(b"modified", key, &sig));
    }
}
