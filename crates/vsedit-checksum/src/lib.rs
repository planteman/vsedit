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

/// Decode a hex string into bytes. Returns `None` if the input has odd length
/// or contains non-hex characters.
pub fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
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
    /// CRC-32 (IEEE 802.3).
    Crc32,
}

/// Return the human-readable algorithm name for a [`ChecksumKind`].
pub fn algorithm_name(kind: ChecksumKind) -> &'static str {
    match kind {
        ChecksumKind::Sha256 => "SHA-256",
        ChecksumKind::Md5Stub => "MD5 (stub)",
        ChecksumKind::Sha1Stub => "SHA-1 (stub)",
        ChecksumKind::Fnv64 => "FNV-1a 64-bit",
        ChecksumKind::Crc32 => "CRC-32",
    }
}

/// Result of a checksum computation with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumResult {
    /// Which algorithm was used.
    pub kind: ChecksumKind,
    /// The hex-encoded digest string.
    pub hex_digest: String,
    /// Length of the input data in bytes.
    pub byte_length: usize,
}

/// Compute a checksum and return a [`ChecksumResult`] with metadata.
pub fn compute_checksum_result(data: &[u8], kind: ChecksumKind) -> ChecksumResult {
    ChecksumResult {
        kind,
        hex_digest: compute_checksum(data, kind),
        byte_length: data.len(),
    }
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
        ChecksumKind::Crc32 => {
            let c = crc32(data);
            hex_encode(&c.to_be_bytes())
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

/// CRC-32 lookup table (IEEE 802.3, reflected polynomial 0xEDB88320).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute the CRC-32 (IEEE 802.3) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        let idx = ((crc as u8) ^ b) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF
}

/// Compute a checksum over multiple data chunks as if they were concatenated.
pub fn compute_streaming_checksum(chunks: &[&[u8]], kind: ChecksumKind) -> String {
    match kind {
        ChecksumKind::Sha256 => {
            let mut hasher = Sha256::new();
            for chunk in chunks {
                hasher.update(chunk);
            }
            hex_encode(&hasher.finalize())
        }
        _ => {
            let combined: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
            compute_checksum(&combined, kind)
        }
    }
}

/// Constant-time comparison of two hex checksum strings.
///
/// Returns `false` immediately if lengths differ; otherwise compares every
/// byte to avoid timing side-channels.
pub fn compare_checksums(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Accumulates data incrementally and verifies its checksum against an
/// expected value at the end, useful for streaming or chunked I/O where
/// the full payload is not available at once.
pub struct ChecksumVerifier {
    kind: ChecksumKind,
    expected: String,
    buffer: Vec<u8>,
}

impl ChecksumVerifier {
    /// Create a new verifier for the given algorithm and expected hex digest.
    pub fn new(kind: ChecksumKind, expected: &str) -> Self {
        Self {
            kind,
            expected: expected.to_string(),
            buffer: Vec::new(),
        }
    }

    /// Feed additional data into the verifier.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Verify that the accumulated data matches the expected checksum.
    pub fn verify(&self) -> bool {
        let actual = compute_checksum(&self.buffer, self.kind);
        compare_checksums(&actual, &self.expected)
    }
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

    #[test]
    fn hex_decode_roundtrip() {
        let original = b"\x00\x0f\x10\xff";
        let encoded = hex_encode(original);
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn hex_decode_invalid() {
        assert!(hex_decode("0g").is_none());
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
    }

    #[test]
    fn hex_decode_uppercase() {
        let decoded = hex_decode("DEADBEEF").unwrap();
        assert_eq!(decoded, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn checksum_result_fields() {
        let data = b"metadata test";
        let result = compute_checksum_result(data, ChecksumKind::Sha256);
        assert_eq!(result.kind, ChecksumKind::Sha256);
        assert_eq!(result.byte_length, data.len());
        assert_eq!(result.hex_digest, compute_sha256(data));
    }

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(b""), 0x0000_0000);
    }

    #[test]
    fn crc32_via_compute_checksum() {
        let hex = compute_checksum(b"123456789", ChecksumKind::Crc32);
        assert_eq!(hex, "cbf43926");
    }

    #[test]
    fn streaming_checksum_matches_whole() {
        let data = b"hello world";
        let whole = compute_checksum(data, ChecksumKind::Sha256);
        let streamed =
            compute_streaming_checksum(&[b"hello", b" ", b"world"], ChecksumKind::Sha256);
        assert_eq!(whole, streamed);
    }

    #[test]
    fn streaming_checksum_fnv() {
        let whole = compute_checksum(b"abcdef", ChecksumKind::Fnv64);
        let streamed = compute_streaming_checksum(&[b"abc", b"def"], ChecksumKind::Fnv64);
        assert_eq!(whole, streamed);
    }

    #[test]
    fn compare_checksums_equal() {
        assert!(compare_checksums("abcdef1234", "abcdef1234"));
    }

    #[test]
    fn compare_checksums_different() {
        assert!(!compare_checksums("abcdef1234", "abcdef1235"));
        assert!(!compare_checksums("short", "longer_string"));
    }

    #[test]
    fn checksum_verifier_pass() {
        let data = b"verified payload";
        let expected = compute_checksum(data, ChecksumKind::Sha256);
        let mut v = ChecksumVerifier::new(ChecksumKind::Sha256, &expected);
        v.feed(b"verified ");
        v.feed(b"payload");
        assert!(v.verify());
    }

    #[test]
    fn checksum_verifier_fail() {
        let mut v = ChecksumVerifier::new(ChecksumKind::Fnv64, "0000000000000000");
        v.feed(b"some data");
        assert!(!v.verify());
    }

    #[test]
    fn algorithm_name_values() {
        assert_eq!(algorithm_name(ChecksumKind::Sha256), "SHA-256");
        assert_eq!(algorithm_name(ChecksumKind::Md5Stub), "MD5 (stub)");
        assert_eq!(algorithm_name(ChecksumKind::Sha1Stub), "SHA-1 (stub)");
        assert_eq!(algorithm_name(ChecksumKind::Fnv64), "FNV-1a 64-bit");
        assert_eq!(algorithm_name(ChecksumKind::Crc32), "CRC-32");
    }
}
