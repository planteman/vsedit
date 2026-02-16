//! Content signing, integrity verification, and glyph margin decorations.

use std::collections::HashMap;
use std::fmt;

// ─── Error types ────────────────────────────────────────────────────

/// Errors that can occur during signing and verification operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    /// The signing key is empty.
    EmptyKey,
    /// The content to sign is empty.
    EmptyContent,
    /// The signature value has an unexpected length.
    InvalidSignatureLength { expected: usize, actual: usize },
    /// The algorithm in the signature does not match the expected one.
    AlgorithmMismatch {
        expected: SignatureAlgorithm,
        found: SignatureAlgorithm,
    },
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignError::EmptyKey => write!(f, "signing key must not be empty"),
            SignError::EmptyContent => write!(f, "content to sign must not be empty"),
            SignError::InvalidSignatureLength { expected, actual } => {
                write!(f, "signature length mismatch: expected {expected}, got {actual}")
            }
            SignError::AlgorithmMismatch { expected, found } => {
                write!(f, "algorithm mismatch: expected {expected:?}, found {found:?}")
            }
        }
    }
}

// ─── Content signing ──────────────────────────────────────────────

/// Stub signature algorithm identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    HmacSha256Stub,
    Ed25519Stub,
}

/// A computed signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub algorithm: SignatureAlgorithm,
    pub value: Vec<u8>,
    pub signer: Option<String>,
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex: String = self.value.iter().map(|b| format!("{b:02x}")).collect();
        write!(f, "Signature({:?}, {})", self.algorithm, hex)
    }
}

impl Signature {
    /// Return the signature value as a hex-encoded string.
    pub fn to_hex(&self) -> String {
        self.value.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Length of the raw signature value in bytes.
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Whether the signature value is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Create a signature with a signer identity attached.
    pub fn with_signer(mut self, signer: impl Into<String>) -> Self {
        self.signer = Some(signer.into());
        self
    }
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

/// Sign content with validation — returns an error if key or content is empty.
pub fn sign_content_checked(
    content: &[u8],
    key: &[u8],
    algorithm: SignatureAlgorithm,
) -> Result<Signature, SignError> {
    if key.is_empty() {
        return Err(SignError::EmptyKey);
    }
    if content.is_empty() {
        return Err(SignError::EmptyContent);
    }
    Ok(sign_content(content, key, algorithm))
}

/// Verify a signature, also checking that the algorithm matches.
pub fn verify_signature_checked(
    content: &[u8],
    key: &[u8],
    signature: &Signature,
    expected_algorithm: SignatureAlgorithm,
) -> Result<bool, SignError> {
    if key.is_empty() {
        return Err(SignError::EmptyKey);
    }
    if signature.algorithm != expected_algorithm {
        return Err(SignError::AlgorithmMismatch {
            expected: expected_algorithm,
            found: signature.algorithm,
        });
    }
    Ok(verify_signature(content, key, signature))
}

/// Verify multiple content/signature pairs against the same key.
/// Returns a `Vec<bool>` with one result per pair.
pub fn verify_batch(
    pairs: &[(&[u8], &Signature)],
    key: &[u8],
) -> Vec<bool> {
    pairs
        .iter()
        .map(|(content, sig)| verify_signature(content, key, sig))
        .collect()
}

/// Compute a simple checksum (XOR of all bytes) for quick integrity checks.
pub fn content_checksum(content: &[u8]) -> u8 {
    content.iter().fold(0u8, |acc, &b| acc ^ b)
}

// ─── Glyph margin ─────────────────────────────────────────────────

/// Lane within the glyph margin column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphMarginLane {
    Left,
    Center,
    Right,
}

impl fmt::Display for GlyphMarginLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlyphMarginLane::Left => write!(f, "left"),
            GlyphMarginLane::Center => write!(f, "center"),
            GlyphMarginLane::Right => write!(f, "right"),
        }
    }
}

/// A decoration displayed in the glyph margin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphDecoration {
    /// Line number (1-based) where the glyph appears.
    pub line: u32,
    /// Character to render in the margin.
    pub glyph_char: char,
    /// Color name or code for the glyph.
    pub color: String,
    /// Tooltip text shown on hover.
    pub tooltip: String,
    /// Which lane within the margin to use.
    pub lane: GlyphMarginLane,
    /// Unique identifier for this decoration.
    pub id: u64,
}

impl GlyphDecoration {
    pub fn new(id: u64, line: u32, glyph_char: char, color: impl Into<String>) -> Self {
        Self {
            id,
            line,
            glyph_char,
            color: color.into(),
            tooltip: String::new(),
            lane: GlyphMarginLane::Center,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = tooltip.into();
        self
    }

    pub fn with_lane(mut self, lane: GlyphMarginLane) -> Self {
        self.lane = lane;
        self
    }
}

impl fmt::Display for GlyphDecoration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] line {} '{}' ({})",
            self.lane, self.line, self.glyph_char, self.color,
        )
    }
}

/// Service that manages glyph margin decorations.
pub struct GlyphMarginService {
    decorations: HashMap<u64, GlyphDecoration>,
    next_id: u64,
}

impl GlyphMarginService {
    pub fn new() -> Self {
        Self {
            decorations: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a new decoration, returning its assigned ID.
    pub fn register_decoration(
        &mut self,
        line: u32,
        glyph_char: char,
        color: impl Into<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let dec = GlyphDecoration::new(id, line, glyph_char, color);
        self.decorations.insert(id, dec);
        id
    }

    /// Register a decoration with full configuration.
    pub fn register_full(&mut self, mut decoration: GlyphDecoration) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        decoration.id = id;
        self.decorations.insert(id, decoration);
        id
    }

    /// Remove a decoration by ID. Returns true if it existed.
    pub fn remove_decoration(&mut self, id: u64) -> bool {
        self.decorations.remove(&id).is_some()
    }

    /// Get all decorations for a specific line, sorted by lane.
    pub fn get_decorations_for_line(&self, line: u32) -> Vec<&GlyphDecoration> {
        let mut decs: Vec<&GlyphDecoration> = self
            .decorations
            .values()
            .filter(|d| d.line == line)
            .collect();
        decs.sort_by_key(|d| match d.lane {
            GlyphMarginLane::Left => 0,
            GlyphMarginLane::Center => 1,
            GlyphMarginLane::Right => 2,
        });
        decs
    }

    /// Total number of registered decorations.
    pub fn decoration_count(&self) -> usize {
        self.decorations.len()
    }

    /// Remove all decorations for a given line.
    pub fn clear_line(&mut self, line: u32) {
        self.decorations.retain(|_, d| d.line != line);
    }

    /// Remove all decorations.
    pub fn clear_all(&mut self) {
        self.decorations.clear();
    }

    /// Retrieve a decoration by its ID.
    pub fn get_decoration(&self, id: u64) -> Option<&GlyphDecoration> {
        self.decorations.get(&id)
    }

    /// Return sorted list of unique line numbers that have decorations.
    pub fn lines_with_decorations(&self) -> Vec<u32> {
        let mut lines: Vec<u32> = self.decorations.values().map(|d| d.line).collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Move all decorations on `old_line` to `new_line`.
    pub fn move_line(&mut self, old_line: u32, new_line: u32) {
        for dec in self.decorations.values_mut() {
            if dec.line == old_line {
                dec.line = new_line;
            }
        }
    }

    /// Shift decorations on lines >= `from_line` by `delta` lines.
    /// Useful when lines are inserted or deleted in the editor.
    pub fn shift_lines(&mut self, from_line: u32, delta: i32) {
        for dec in self.decorations.values_mut() {
            if dec.line >= from_line {
                dec.line = (dec.line as i64 + delta as i64).max(1) as u32;
            }
        }
    }
}

impl Default for GlyphMarginService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Signing tests ────────────────────────────────────────────

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

    // ─── Glyph margin tests ──────────────────────────────────────

    #[test]
    fn register_and_get_decoration() {
        let mut svc = GlyphMarginService::new();
        let id = svc.register_decoration(5, '●', "red");
        assert_eq!(svc.decoration_count(), 1);
        let decs = svc.get_decorations_for_line(5);
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].id, id);
        assert_eq!(decs[0].glyph_char, '●');
    }

    #[test]
    fn remove_decoration() {
        let mut svc = GlyphMarginService::new();
        let id = svc.register_decoration(1, '▶', "green");
        assert!(svc.remove_decoration(id));
        assert!(!svc.remove_decoration(id));
        assert_eq!(svc.decoration_count(), 0);
    }

    #[test]
    fn get_decorations_empty_line() {
        let svc = GlyphMarginService::new();
        assert!(svc.get_decorations_for_line(42).is_empty());
    }

    #[test]
    fn decorations_sorted_by_lane() {
        let mut svc = GlyphMarginService::new();
        let d1 = GlyphDecoration::new(0, 1, 'R', "red").with_lane(GlyphMarginLane::Right);
        let d2 = GlyphDecoration::new(0, 1, 'L', "blue").with_lane(GlyphMarginLane::Left);
        let d3 = GlyphDecoration::new(0, 1, 'C', "green").with_lane(GlyphMarginLane::Center);
        svc.register_full(d1);
        svc.register_full(d2);
        svc.register_full(d3);
        let decs = svc.get_decorations_for_line(1);
        assert_eq!(decs[0].glyph_char, 'L');
        assert_eq!(decs[1].glyph_char, 'C');
        assert_eq!(decs[2].glyph_char, 'R');
    }

    #[test]
    fn clear_line() {
        let mut svc = GlyphMarginService::new();
        svc.register_decoration(1, 'A', "red");
        svc.register_decoration(1, 'B', "red");
        svc.register_decoration(2, 'C', "red");
        svc.clear_line(1);
        assert_eq!(svc.decoration_count(), 1);
        assert!(svc.get_decorations_for_line(1).is_empty());
    }

    #[test]
    fn clear_all() {
        let mut svc = GlyphMarginService::new();
        svc.register_decoration(1, 'A', "red");
        svc.register_decoration(2, 'B', "blue");
        svc.clear_all();
        assert_eq!(svc.decoration_count(), 0);
    }

    #[test]
    fn decoration_with_tooltip() {
        let d = GlyphDecoration::new(1, 10, '!', "yellow")
            .with_tooltip("Breakpoint");
        assert_eq!(d.tooltip, "Breakpoint");
    }

    // ─── Validated signing tests ─────────────────────────────────

    #[test]
    fn sign_checked_rejects_empty_key() {
        let err = sign_content_checked(b"data", b"", SignatureAlgorithm::HmacSha256Stub);
        assert_eq!(err.unwrap_err(), SignError::EmptyKey);
    }

    #[test]
    fn sign_checked_rejects_empty_content() {
        let err = sign_content_checked(b"", b"key", SignatureAlgorithm::Ed25519Stub);
        assert_eq!(err.unwrap_err(), SignError::EmptyContent);
    }

    #[test]
    fn sign_checked_ok() {
        let sig = sign_content_checked(b"data", b"key", SignatureAlgorithm::HmacSha256Stub)
            .expect("should succeed");
        assert_eq!(sig.algorithm, SignatureAlgorithm::HmacSha256Stub);
        assert!(!sig.is_empty());
    }

    #[test]
    fn verify_checked_algorithm_mismatch() {
        let sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub);
        let result = verify_signature_checked(b"data", b"key", &sig, SignatureAlgorithm::Ed25519Stub);
        assert!(matches!(result, Err(SignError::AlgorithmMismatch { .. })));
    }

    #[test]
    fn verify_checked_empty_key() {
        let sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub);
        let result = verify_signature_checked(b"data", b"", &sig, SignatureAlgorithm::HmacSha256Stub);
        assert_eq!(result.unwrap_err(), SignError::EmptyKey);
    }

    #[test]
    fn verify_batch_mixed() {
        let key = b"mykey";
        let s1 = sign_content(b"aaa", key, SignatureAlgorithm::HmacSha256Stub);
        let s2 = sign_content(b"bbb", key, SignatureAlgorithm::Ed25519Stub);
        let results = verify_batch(
            &[(b"aaa".as_slice(), &s1), (b"ccc".as_slice(), &s2)],
            key,
        );
        assert_eq!(results, vec![true, false]);
    }

    #[test]
    fn signature_hex_and_display() {
        let sig = sign_content(b"AB", b"\x01", SignatureAlgorithm::HmacSha256Stub);
        assert_eq!(sig.to_hex().len(), sig.len() * 2);
        let display = format!("{sig}");
        assert!(display.starts_with("Signature("));
    }

    #[test]
    fn signature_with_signer() {
        let sig = sign_content(b"x", b"k", SignatureAlgorithm::Ed25519Stub)
            .with_signer("alice");
        assert_eq!(sig.signer.as_deref(), Some("alice"));
    }

    #[test]
    fn content_checksum_basic() {
        assert_eq!(content_checksum(b""), 0);
        assert_eq!(content_checksum(&[0xFF, 0xFF]), 0);
        assert_eq!(content_checksum(&[0x01, 0x02]), 0x03);
    }

    // ─── Additional glyph margin tests ──────────────────────────

    #[test]
    fn get_decoration_by_id() {
        let mut svc = GlyphMarginService::new();
        let id = svc.register_decoration(7, '⬤', "blue");
        let dec = svc.get_decoration(id).expect("should exist");
        assert_eq!(dec.line, 7);
        assert!(svc.get_decoration(999).is_none());
    }

    #[test]
    fn lines_with_decorations_sorted() {
        let mut svc = GlyphMarginService::new();
        svc.register_decoration(10, 'A', "r");
        svc.register_decoration(3, 'B', "g");
        svc.register_decoration(10, 'C', "b");
        svc.register_decoration(7, 'D', "y");
        assert_eq!(svc.lines_with_decorations(), vec![3, 7, 10]);
    }

    #[test]
    fn move_line_decorations() {
        let mut svc = GlyphMarginService::new();
        svc.register_decoration(5, 'X', "red");
        svc.register_decoration(5, 'Y', "red");
        svc.register_decoration(6, 'Z', "red");
        svc.move_line(5, 20);
        assert!(svc.get_decorations_for_line(5).is_empty());
        assert_eq!(svc.get_decorations_for_line(20).len(), 2);
        assert_eq!(svc.get_decorations_for_line(6).len(), 1);
    }

    #[test]
    fn shift_lines_positive() {
        let mut svc = GlyphMarginService::new();
        let id_a = svc.register_decoration(3, 'A', "r");
        let id_b = svc.register_decoration(5, 'B', "g");
        let id_c = svc.register_decoration(1, 'C', "b");
        svc.shift_lines(3, 2);
        assert_eq!(svc.get_decoration(id_a).unwrap().line, 5);
        assert_eq!(svc.get_decoration(id_b).unwrap().line, 7);
        assert_eq!(svc.get_decoration(id_c).unwrap().line, 1); // below threshold, unchanged
    }

    #[test]
    fn shift_lines_negative_clamps_to_one() {
        let mut svc = GlyphMarginService::new();
        let id = svc.register_decoration(2, 'A', "r");
        svc.shift_lines(1, -10);
        assert_eq!(svc.get_decoration(id).unwrap().line, 1);
    }

    #[test]
    fn glyph_margin_lane_display() {
        assert_eq!(format!("{}", GlyphMarginLane::Left), "left");
        assert_eq!(format!("{}", GlyphMarginLane::Center), "center");
        assert_eq!(format!("{}", GlyphMarginLane::Right), "right");
    }

    #[test]
    fn glyph_decoration_display() {
        let d = GlyphDecoration::new(1, 42, '●', "red");
        let s = format!("{d}");
        assert!(s.contains("42"));
        assert!(s.contains("●"));
        assert!(s.contains("red"));
    }

    #[test]
    fn sign_error_display() {
        let e = SignError::EmptyKey;
        assert!(format!("{e}").contains("empty"));
        let e2 = SignError::AlgorithmMismatch {
            expected: SignatureAlgorithm::HmacSha256Stub,
            found: SignatureAlgorithm::Ed25519Stub,
        };
        assert!(format!("{e2}").contains("mismatch"));
    }
}
