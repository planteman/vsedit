//! Content signing, integrity verification, and glyph margin decorations.

use std::collections::HashMap;

// ─── Content signing ──────────────────────────────────────────────

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

// ─── Glyph margin ─────────────────────────────────────────────────

/// Lane within the glyph margin column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphMarginLane {
    Left,
    Center,
    Right,
}

/// A decoration displayed in the glyph margin.
#[derive(Debug, Clone)]
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
}
