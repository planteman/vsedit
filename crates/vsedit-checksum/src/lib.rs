//! File integrity verification.

use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;

/// Compute a 64-bit FNV-1a hash of the input bytes.
pub fn simple_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Encode bytes as a lowercase hex string.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Supported checksum algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    /// SHA-256 (real, via sha2 crate).
    Sha256,
    /// MD5-style stub (uses FNV internally, not real MD5).
    Md5Stub,
    /// SHA-1–style stub (uses FNV internally, not real SHA-1).
    Sha1Stub,
    /// Real FNV-1a 64-bit hash.
    Fnv64,
}

/// Compute SHA-256 of `data`, returning a lowercase hex string.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(&hasher.finalize())
}

/// Compute an MD5-style stub hash (uses FNV-1a internally).
pub fn compute_md5(data: &[u8]) -> String {
    // Produce a 128-bit-style output by hashing two halves
    let h1 = simple_hash(data);
    let h2 = simple_hash(&[data, &[0xff]].concat());
    hex_encode(&[h1.to_be_bytes(), h2.to_be_bytes()].concat())
}

/// Compute a SHA-1–style stub hash (uses FNV-1a internally).
pub fn compute_sha1(data: &[u8]) -> String {
    // Produce a 160-bit-style output
    let h1 = simple_hash(data);
    let h2 = simple_hash(&[data, &[0xfe]].concat());
    let h3_data = [data, &[0xfd]].concat();
    let h3 = simple_hash(&h3_data) & 0xFFFF_FFFF;
    let mut out = Vec::with_capacity(20);
    out.extend_from_slice(&h1.to_be_bytes());
    out.extend_from_slice(&h2.to_be_bytes());
    out.extend_from_slice(&(h3 as u32).to_be_bytes());
    hex_encode(&out)
}

/// Compute a checksum of `data` using the given algorithm.
pub fn compute_checksum(data: &[u8], kind: ChecksumKind) -> String {
    match kind {
        ChecksumKind::Sha256 => compute_sha256(data),
        ChecksumKind::Md5Stub => compute_md5(data),
        ChecksumKind::Sha1Stub => compute_sha1(data),
        ChecksumKind::Fnv64 => {
            let h = simple_hash(data);
            hex_encode(&h.to_be_bytes())
        }
    }
}

/// Compute a checksum for a file on disk.
pub fn compute_file_checksum(path: &Path, kind: ChecksumKind) -> io::Result<String> {
    let data = std::fs::read(path)?;
    Ok(compute_checksum(&data, kind))
}

/// Verify that `data` produces the `expected` checksum for the given algorithm.
pub fn verify_checksum(data: &[u8], expected: &str, kind: ChecksumKind) -> bool {
    compute_checksum(data, kind) == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn deterministic_output() {
        let data = b"hello world";
        let a = compute_checksum(data, ChecksumKind::Fnv64);
        let b = compute_checksum(data, ChecksumKind::Fnv64);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn verification() {
        let data = b"test data";
        let checksum = compute_checksum(data, ChecksumKind::Fnv64);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Fnv64));
        assert!(!verify_checksum(data, "0000000000000000", ChecksumKind::Fnv64));
    }

    #[test]
    fn different_data_different_hash() {
        let a = compute_checksum(b"alpha", ChecksumKind::Fnv64);
        let b = compute_checksum(b"beta", ChecksumKind::Fnv64);
        assert_ne!(a, b);
    }

    #[test]
    fn sha256_known_value() {
        // SHA-256 of empty string is well-known
        let hash = compute_sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_deterministic() {
        let a = compute_sha256(b"hello");
        let b = compute_sha256(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // 256 bits = 32 bytes = 64 hex chars
    }

    #[test]
    fn md5_stub_deterministic_and_length() {
        let a = compute_md5(b"data");
        let b = compute_md5(b"data");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32); // 128-bit output = 16 bytes = 32 hex
    }

    #[test]
    fn sha1_stub_deterministic_and_length() {
        let a = compute_sha1(b"data");
        let b = compute_sha1(b"data");
        assert_eq!(a, b);
        assert_eq!(a.len(), 40); // 160-bit output = 20 bytes = 40 hex
    }

    #[test]
    fn verify_sha256() {
        let data = b"verify me";
        let checksum = compute_checksum(data, ChecksumKind::Sha256);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Sha256));
        assert!(!verify_checksum(b"other", &checksum, ChecksumKind::Sha256));
    }

    #[test]
    fn compute_file_checksum_works() {
        let dir = std::env::temp_dir().join("vsedit_checksum_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(b"file content").unwrap();
        drop(f);

        let checksum = compute_file_checksum(&file, ChecksumKind::Sha256).unwrap();
        let expected = compute_sha256(b"file content");
        assert_eq!(checksum, expected);

        std::fs::remove_file(&file).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn compute_file_checksum_not_found() {
        let result = compute_file_checksum(Path::new("/nonexistent/path"), ChecksumKind::Sha256);
        assert!(result.is_err());
    }

    #[test]
    fn different_algorithms_different_output() {
        let data = b"same data";
        let sha256 = compute_checksum(data, ChecksumKind::Sha256);
        let fnv = compute_checksum(data, ChecksumKind::Fnv64);
        let md5 = compute_checksum(data, ChecksumKind::Md5Stub);
        assert_ne!(sha256, fnv);
        assert_ne!(sha256, md5);
    }
}
