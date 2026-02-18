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

// ─── Signature statistics ─────────────────────────────────────────

/// Summary statistics over a collection of signatures.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureStats {
    /// Number of signatures analysed.
    pub total_signatures: usize,
    /// Sum of all signature value byte-lengths (treated as parameter counts).
    pub total_parameters: usize,
    /// Average parameters (byte-length) per signature; 0.0 when empty.
    pub avg_params_per_signature: f64,
}

/// Compute aggregate statistics for a slice of [`Signature`] values.
///
/// Each signature's byte-length (`value.len()`) is treated as its parameter
/// count, which is a useful proxy when signatures encode typed parameter
/// information in their raw bytes.
pub fn compute_signature_stats(signatures: &[Signature]) -> SignatureStats {
    let total_signatures = signatures.len();
    let total_parameters: usize = signatures.iter().map(|s| s.value.len()).sum();
    let avg_params_per_signature = if total_signatures == 0 {
        0.0
    } else {
        total_parameters as f64 / total_signatures as f64
    };
    SignatureStats {
        total_signatures,
        total_parameters,
        avg_params_per_signature,
    }
}

/// Accumulated statistics for sign operations.
#[derive(Debug, Clone, PartialEq)]
pub struct SignStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl SignStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &SignStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for SignStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SignStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SignStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for sign.
#[derive(Debug, Clone)]
pub struct SignValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl SignValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for SignValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Signature parameter display helpers for tooltips
// ---------------------------------------------------------------------------

/// A parameter in a signature for tooltip rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParameter {
    pub name: String,
    pub param_type: String,
    pub documentation: Option<String>,
    pub is_optional: bool,
    pub default_value: Option<String>,
}

impl SignatureParameter {
    pub fn new(name: impl Into<String>, param_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
            documentation: None,
            is_optional: false,
            default_value: None,
        }
    }

    pub fn optional(mut self) -> Self {
        self.is_optional = true;
        self
    }

    pub fn with_default(mut self, default: impl Into<String>) -> Self {
        self.default_value = Some(default.into());
        self.is_optional = true;
        self
    }

    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    /// Format the parameter for inline display in a signature.
    /// Example: "name: string", "count?: number = 0"
    pub fn display_inline(&self) -> String {
        let mut s = self.name.clone();
        if self.is_optional {
            s.push('?');
        }
        s.push_str(": ");
        s.push_str(&self.param_type);
        if let Some(ref default) = self.default_value {
            s.push_str(" = ");
            s.push_str(default);
        }
        s
    }

    /// Format the parameter for a tooltip line.
    /// Example: "@param name — Description text"
    pub fn display_tooltip(&self) -> String {
        let mut s = format!("@param {} — ", self.name);
        if let Some(ref doc) = self.documentation {
            s.push_str(doc);
        } else {
            s.push_str(&self.param_type);
            if self.is_optional {
                s.push_str(" (optional)");
            }
        }
        s
    }
}

impl fmt::Display for SignatureParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_inline())
    }
}

/// Format a complete signature with parameters for tooltip display.
pub fn format_signature_tooltip(name: &str, params: &[SignatureParameter]) -> String {
    let param_strs: Vec<String> = params.iter().map(|p| p.display_inline()).collect();
    let sig = format!("{}({})", name, param_strs.join(", "));
    let mut tooltip = sig;
    for p in params {
        if p.documentation.is_some() {
            tooltip.push('\n');
            tooltip.push_str(&p.display_tooltip());
        }
    }
    tooltip
}

// ─── SignatureAlgorithm extensions ─────────────────────────────────

impl SignatureAlgorithm {
    pub fn is_asymmetric(&self) -> bool {
        matches!(self, SignatureAlgorithm::Ed25519Stub)
    }

    pub fn is_symmetric(&self) -> bool {
        !self.is_asymmetric()
    }

    pub fn key_size_bits(&self) -> usize {
        match self {
            SignatureAlgorithm::HmacSha256Stub => 256,
            SignatureAlgorithm::Ed25519Stub => 256,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SignatureAlgorithm::HmacSha256Stub => "HMAC-SHA256 (stub)",
            SignatureAlgorithm::Ed25519Stub => "Ed25519 (stub)",
        }
    }
}

impl fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ─── Signature extensions ─────────────────────────────────────────

impl Signature {
    pub fn parameter_count(&self) -> usize {
        self.value.len()
    }

    pub fn has_parameters(&self) -> bool {
        !self.value.is_empty()
    }

    pub fn matches_algorithm(&self, algo: SignatureAlgorithm) -> bool {
        self.algorithm == algo
    }
}

// ─── GlyphDecoration extensions ───────────────────────────────────

impl GlyphDecoration {
    pub fn is_visible(&self) -> bool {
        !self.color.is_empty() && self.glyph_char != ' '
    }

    pub fn overlaps_line(&self, line: u32) -> bool {
        self.line == line
    }

    pub fn has_tooltip(&self) -> bool {
        !self.tooltip.is_empty()
    }
}

// ─── GlyphMarginService iterator & extensions ─────────────────────

pub struct DecorationIter<'a> {
    inner: std::collections::hash_map::Values<'a, u64, GlyphDecoration>,
}

impl<'a> Iterator for DecorationIter<'a> {
    type Item = &'a GlyphDecoration;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl GlyphMarginService {
    pub fn iter(&self) -> DecorationIter<'_> {
        DecorationIter {
            inner: self.decorations.values(),
        }
    }

    pub fn decorations_on_line(&self, line: u32) -> Vec<&GlyphDecoration> {
        self.decorations
            .values()
            .filter(|d| d.overlaps_line(line))
            .collect()
    }

    pub fn has_decoration_on_line(&self, line: u32) -> bool {
        self.decorations.values().any(|d| d.line == line)
    }

    pub fn visible_decoration_count(&self) -> usize {
        self.decorations.values().filter(|d| d.is_visible()).count()
    }

    pub fn line_range(&self) -> Option<(u32, u32)> {
        let mut min = u32::MAX;
        let mut max = u32::MIN;
        for d in self.decorations.values() {
            if d.line < min {
                min = d.line;
            }
            if d.line > max {
                max = d.line;
            }
        }
        if min <= max { Some((min, max)) } else { None }
    }
}

impl<'a> IntoIterator for &'a GlyphMarginService {
    type Item = &'a GlyphDecoration;
    type IntoIter = DecorationIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ─── SignatureParameter extensions ────────────────────────────────

impl SignatureParameter {
    pub fn is_required(&self) -> bool {
        !self.is_optional
    }

    pub fn has_documentation(&self) -> bool {
        self.documentation.is_some()
    }

    pub fn has_default(&self) -> bool {
        self.default_value.is_some()
    }
}

// ─── SignatureStats extensions ────────────────────────────────────

impl SignatureStats {
    pub fn merge(&self, other: &SignatureStats) -> SignatureStats {
        let total_signatures = self.total_signatures + other.total_signatures;
        let total_parameters = self.total_parameters + other.total_parameters;
        let avg_params_per_signature = if total_signatures == 0 {
            0.0
        } else {
            total_parameters as f64 / total_signatures as f64
        };
        SignatureStats {
            total_signatures,
            total_parameters,
            avg_params_per_signature,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "{} signature(s), {} total parameter(s), {:.2} avg",
            self.total_signatures,
            self.total_parameters,
            self.avg_params_per_signature,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.total_signatures == 0
    }
}

impl fmt::Display for SignatureStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ─── Signature chain of trust ─────────────────────────────────────

/// A certificate in a chain of trust, linking a subject to an issuer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningCertificate {
    /// Unique identifier for this certificate.
    pub subject: String,
    /// The issuer that signed this certificate (None for self-signed roots).
    pub issuer: Option<String>,
    /// Algorithm used to sign this certificate.
    pub algorithm: SignatureAlgorithm,
    /// Validity window: not-valid-before (seconds since epoch).
    pub not_before: u64,
    /// Validity window: not-valid-after (seconds since epoch).
    pub not_after: u64,
    /// The raw signature bytes binding this certificate.
    pub signature: Vec<u8>,
}

impl SigningCertificate {
    /// Create a new self-signed root certificate.
    pub fn new_root(
        subject: impl Into<String>,
        algorithm: SignatureAlgorithm,
        not_before: u64,
        not_after: u64,
        key: &[u8],
    ) -> Self {
        let subject = subject.into();
        let sig = xor_fold(subject.as_bytes(), key);
        Self {
            subject,
            issuer: None,
            algorithm,
            not_before,
            not_after,
            signature: sig,
        }
    }

    /// Create a certificate issued by another entity.
    pub fn new_issued(
        subject: impl Into<String>,
        issuer: impl Into<String>,
        algorithm: SignatureAlgorithm,
        not_before: u64,
        not_after: u64,
        key: &[u8],
    ) -> Self {
        let subject = subject.into();
        let issuer = issuer.into();
        let payload: Vec<u8> = subject
            .as_bytes()
            .iter()
            .chain(issuer.as_bytes())
            .copied()
            .collect();
        let sig = xor_fold(&payload, key);
        Self {
            subject,
            issuer: Some(issuer),
            algorithm,
            not_before,
            not_after,
            signature: sig,
        }
    }

    /// Whether this is a self-signed root certificate.
    pub fn is_root(&self) -> bool {
        self.issuer.is_none()
    }

    /// Check if the certificate is valid at a given timestamp (seconds since epoch).
    pub fn is_valid_at(&self, now: u64) -> bool {
        now >= self.not_before && now <= self.not_after
    }

    /// Check if the certificate has expired relative to a given timestamp.
    pub fn is_expired_at(&self, now: u64) -> bool {
        now > self.not_after
    }

    /// Remaining validity in seconds, or 0 if expired.
    pub fn remaining_validity(&self, now: u64) -> u64 {
        self.not_after.saturating_sub(now)
    }
}

impl fmt::Display for SigningCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.issuer {
            Some(issuer) => write!(
                f,
                "Cert(subject={}, issuer={}, valid={}..{})",
                self.subject, issuer, self.not_before, self.not_after
            ),
            None => write!(
                f,
                "Cert(subject={} [root], valid={}..{})",
                self.subject, self.not_before, self.not_after
            ),
        }
    }
}

/// Errors from certificate chain validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// The chain is empty.
    EmptyChain,
    /// The chain's root is not self-signed.
    UntrustedRoot { subject: String },
    /// A link in the chain has a broken issuer reference.
    BrokenLink { child: String, expected_issuer: String },
    /// A certificate in the chain has expired.
    Expired { subject: String, expired_at: u64 },
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainError::EmptyChain => write!(f, "certificate chain is empty"),
            ChainError::UntrustedRoot { subject } => {
                write!(f, "root certificate '{}' is not self-signed", subject)
            }
            ChainError::BrokenLink {
                child,
                expected_issuer,
            } => write!(
                f,
                "broken chain link: '{}' expects issuer '{}' but it is not the next cert",
                child, expected_issuer
            ),
            ChainError::Expired { subject, expired_at } => {
                write!(f, "certificate '{}' expired at {}", subject, expired_at)
            }
        }
    }
}

/// Validate a certificate chain at a given point in time.
///
/// The chain must be ordered from leaf to root. The root must be self-signed,
/// each intermediate's issuer must match the subject of the next certificate,
/// and every certificate must be valid at `now`.
pub fn validate_certificate_chain(
    chain: &[SigningCertificate],
    now: u64,
) -> Result<(), ChainError> {
    if chain.is_empty() {
        return Err(ChainError::EmptyChain);
    }

    // Check each certificate is valid at `now`.
    for cert in chain {
        if cert.is_expired_at(now) || !cert.is_valid_at(now) {
            return Err(ChainError::Expired {
                subject: cert.subject.clone(),
                expired_at: cert.not_after,
            });
        }
    }

    // The last certificate in the chain must be a self-signed root.
    let root = &chain[chain.len() - 1];
    if !root.is_root() {
        return Err(ChainError::UntrustedRoot {
            subject: root.subject.clone(),
        });
    }

    // Walk from leaf toward root, ensuring each issuer matches the next subject.
    for window in chain.windows(2) {
        let child = &window[0];
        let parent = &window[1];
        if let Some(ref issuer) = child.issuer {
            if issuer != &parent.subject {
                return Err(ChainError::BrokenLink {
                    child: child.subject.clone(),
                    expected_issuer: issuer.clone(),
                });
            }
        }
    }

    Ok(())
}

// ─── Digest computation helpers ───────────────────────────────────

/// Supported digest algorithms for content hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestAlgorithm {
    /// XOR-fold to a single byte (fast, not cryptographic).
    Xor,
    /// DJB2 hash folded into 8 bytes.
    Djb2,
    /// FNV-1a 64-bit hash.
    Fnv1a64,
}

/// A computed content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDigest {
    pub algorithm: DigestAlgorithm,
    pub value: Vec<u8>,
}

impl ContentDigest {
    /// Return the digest as a hex string.
    pub fn to_hex(&self) -> String {
        self.value.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({:?}, {})", self.algorithm, self.to_hex())
    }
}

/// Compute a content digest using the specified algorithm.
pub fn compute_digest(content: &[u8], algorithm: DigestAlgorithm) -> ContentDigest {
    let value = match algorithm {
        DigestAlgorithm::Xor => {
            vec![content.iter().fold(0u8, |acc, &b| acc ^ b)]
        }
        DigestAlgorithm::Djb2 => {
            let mut hash: u64 = 5381;
            for &b in content {
                hash = hash.wrapping_mul(33).wrapping_add(b as u64);
            }
            hash.to_be_bytes().to_vec()
        }
        DigestAlgorithm::Fnv1a64 => {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            for &b in content {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0100_0000_01b3);
            }
            hash.to_be_bytes().to_vec()
        }
    };
    ContentDigest { algorithm, value }
}

/// Verify that a digest matches the content.
pub fn verify_digest(content: &[u8], digest: &ContentDigest) -> bool {
    let recomputed = compute_digest(content, digest.algorithm);
    recomputed.value == digest.value
}

// ─── Batch signature verification ─────────────────────────────────

/// Result of verifying a single entry in a batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchVerifyResult {
    /// Index into the original batch.
    pub index: usize,
    /// Label for this entry (e.g. file name).
    pub label: String,
    /// Whether verification succeeded.
    pub valid: bool,
    /// Optional reason on failure.
    pub reason: Option<String>,
}

/// An entry to be verified in a batch operation.
pub struct BatchEntry<'a> {
    pub label: String,
    pub content: &'a [u8],
    pub signature: &'a Signature,
}

/// Verify a batch of labelled entries against a key, collecting detailed results.
pub fn verify_batch_detailed<'a>(
    entries: &[BatchEntry<'a>],
    key: &[u8],
    expected_algorithm: SignatureAlgorithm,
) -> Vec<BatchVerifyResult> {
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            if entry.signature.algorithm != expected_algorithm {
                return BatchVerifyResult {
                    index: i,
                    label: entry.label.clone(),
                    valid: false,
                    reason: Some(format!(
                        "algorithm mismatch: expected {:?}, found {:?}",
                        expected_algorithm, entry.signature.algorithm
                    )),
                };
            }
            let ok = verify_signature(entry.content, key, entry.signature);
            BatchVerifyResult {
                index: i,
                label: entry.label.clone(),
                valid: ok,
                reason: if ok {
                    None
                } else {
                    Some("signature does not match content".to_string())
                },
            }
        })
        .collect()
}

/// Count the number of valid results in a batch verification.
pub fn batch_valid_count(results: &[BatchVerifyResult]) -> usize {
    results.iter().filter(|r| r.valid).count()
}

/// Check if all results in a batch are valid.
pub fn batch_all_valid(results: &[BatchVerifyResult]) -> bool {
    results.iter().all(|r| r.valid)
}

// ─── Signature metadata ──────────────────────────────────────────

/// Extracted metadata about a signature for display or auditing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureMetadata {
    pub algorithm: SignatureAlgorithm,
    pub algorithm_label: String,
    pub is_asymmetric: bool,
    pub key_size_bits: usize,
    pub signature_hex: String,
    pub signature_length: usize,
    pub signer: Option<String>,
}

/// Extract display-ready metadata from a [`Signature`].
pub fn extract_metadata(signature: &Signature) -> SignatureMetadata {
    SignatureMetadata {
        algorithm: signature.algorithm,
        algorithm_label: signature.algorithm.label().to_string(),
        is_asymmetric: signature.algorithm.is_asymmetric(),
        key_size_bits: signature.algorithm.key_size_bits(),
        signature_hex: signature.to_hex(),
        signature_length: signature.len(),
        signer: signature.signer.clone(),
    }
}

impl fmt::Display for SignatureMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SignatureMetadata(algo={}, key={}bit, len={}, signer={})",
            self.algorithm_label,
            self.key_size_bits,
            self.signature_length,
            self.signer.as_deref().unwrap_or("none"),
        )
    }
}

// ─── Package verification ─────────────────────────────────────────

/// A key trusted for extension package verification.
#[derive(Debug, Clone)]
pub struct TrustedKey {
    pub key_id: String,
    pub public_key: Vec<u8>,
    pub algorithm: SignatureAlgorithm,
    pub trusted: bool,
}

/// Result of a package signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    Valid { key_id: String },
    Invalid,
    NoMatchingKey,
    KeyNotTrusted(String),
}

impl VerifyResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, VerifyResult::Valid { .. })
    }
}

impl fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyResult::Valid { key_id } => write!(f, "Valid(key={key_id})"),
            VerifyResult::Invalid => write!(f, "Invalid"),
            VerifyResult::NoMatchingKey => write!(f, "NoMatchingKey"),
            VerifyResult::KeyNotTrusted(id) => write!(f, "KeyNotTrusted({id})"),
        }
    }
}

/// Verifier for extension package signatures.
#[derive(Debug, Clone)]
pub struct SignatureVerifier {
    pub trusted_keys: Vec<TrustedKey>,
}

impl SignatureVerifier {
    pub fn new() -> Self {
        Self {
            trusted_keys: Vec::new(),
        }
    }

    pub fn add_key(&mut self, key_id: &str, key: &[u8], algorithm: SignatureAlgorithm) {
        self.trusted_keys.push(TrustedKey {
            key_id: key_id.to_string(),
            public_key: key.to_vec(),
            algorithm,
            trusted: true,
        });
    }

    pub fn verify_package(&self, content: &[u8], signature: &Signature) -> VerifyResult {
        for tk in &self.trusted_keys {
            if tk.algorithm == signature.algorithm {
                if !tk.trusted {
                    return VerifyResult::KeyNotTrusted(tk.key_id.clone());
                }
                if verify_signature(content, &tk.public_key, signature) {
                    return VerifyResult::Valid {
                        key_id: tk.key_id.clone(),
                    };
                }
                return VerifyResult::Invalid;
            }
        }
        VerifyResult::NoMatchingKey
    }
}

// ─── Trust store ──────────────────────────────────────────────────

/// Level of trust assigned to a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustLevel {
    Full,
    Partial,
    Untrusted,
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustLevel::Full => write!(f, "Full"),
            TrustLevel::Partial => write!(f, "Partial"),
            TrustLevel::Untrusted => write!(f, "Untrusted"),
        }
    }
}

/// An entry in the trust store.
#[derive(Debug, Clone)]
pub struct TrustEntry {
    pub key_id: String,
    pub trust_level: TrustLevel,
    pub added_at: u64,
    pub last_used: Option<u64>,
}

/// Persistent store of key trust information.
#[derive(Debug, Clone)]
pub struct SignatureTrustStore {
    store: HashMap<String, TrustEntry>,
}

impl SignatureTrustStore {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }

    pub fn add_trusted(&mut self, key_id: &str, level: TrustLevel, timestamp: u64) {
        self.store.insert(
            key_id.to_string(),
            TrustEntry {
                key_id: key_id.to_string(),
                trust_level: level,
                added_at: timestamp,
                last_used: None,
            },
        );
    }

    pub fn is_trusted(&self, key_id: &str) -> bool {
        self.store
            .get(key_id)
            .map(|e| matches!(e.trust_level, TrustLevel::Full | TrustLevel::Partial))
            .unwrap_or(false)
    }

    pub fn get_trust_level(&self, key_id: &str) -> Option<&TrustLevel> {
        self.store.get(key_id).map(|e| &e.trust_level)
    }

    pub fn revoke(&mut self, key_id: &str) -> bool {
        if let Some(entry) = self.store.get_mut(key_id) {
            entry.trust_level = TrustLevel::Untrusted;
            true
        } else {
            false
        }
    }

    pub fn mark_used(&mut self, key_id: &str, timestamp: u64) {
        if let Some(entry) = self.store.get_mut(key_id) {
            entry.last_used = Some(timestamp);
        }
    }

    pub fn trusted_count(&self) -> usize {
        self.store
            .values()
            .filter(|e| matches!(e.trust_level, TrustLevel::Full | TrustLevel::Partial))
            .count()
    }

    pub fn all_entries(&self) -> Vec<&TrustEntry> {
        self.store.values().collect()
    }
}

// ─── Signature chain ──────────────────────────────────────────────

/// A single link in a signature validation chain.
#[derive(Debug, Clone)]
pub struct ChainLink {
    pub signer: String,
    pub signature: Vec<u8>,
    pub algorithm: SignatureAlgorithm,
    pub valid: bool,
}

/// An ordered chain of signatures used for multi-party validation.
#[derive(Debug, Clone)]
pub struct SignatureChain {
    chain: Vec<ChainLink>,
}

impl SignatureChain {
    pub fn new() -> Self {
        Self { chain: Vec::new() }
    }

    pub fn add_link(&mut self, signer: &str, sig: &[u8], algo: SignatureAlgorithm) {
        self.chain.push(ChainLink {
            signer: signer.to_string(),
            signature: sig.to_vec(),
            algorithm: algo,
            valid: false,
        });
    }

    /// Simplified chain validation: first link is always valid,
    /// subsequent links are valid only if the previous link is valid.
    pub fn validate_chain(&mut self) -> bool {
        for i in 0..self.chain.len() {
            if i == 0 {
                self.chain[i].valid = true;
            } else {
                self.chain[i].valid = self.chain[i - 1].valid;
            }
        }
        self.is_valid()
    }

    pub fn is_valid(&self) -> bool {
        !self.chain.is_empty() && self.chain.iter().all(|l| l.valid)
    }

    pub fn chain_length(&self) -> usize {
        self.chain.len()
    }

    pub fn root_signer(&self) -> Option<&str> {
        self.chain.first().map(|l| l.signer.as_str())
    }

    pub fn leaf_signer(&self) -> Option<&str> {
        self.chain.last().map(|l| l.signer.as_str())
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, link) in self.chain.iter().enumerate() {
            let status = if link.valid { "✓" } else { "✗" };
            let hex: String = link.signature.iter().map(|b| format!("{b:02x}")).collect();
            out.push_str(&format!(
                "[{i}] {status} {} ({:?}) {hex}\n",
                link.signer, link.algorithm,
            ));
        }
        out
    }
}

// ─── Revocation checker ──────────────────────────────────────────

/// Record of a revoked key.
#[derive(Debug, Clone)]
pub struct RevocationEntry {
    pub key_id: String,
    pub reason: String,
    pub revoked_at: u64,
}

/// Checks whether keys have been revoked.
#[derive(Debug, Clone)]
pub struct RevocationChecker {
    revoked: HashMap<String, RevocationEntry>,
}

impl RevocationChecker {
    pub fn new() -> Self {
        Self {
            revoked: HashMap::new(),
        }
    }

    pub fn revoke(&mut self, key_id: &str, reason: &str, timestamp: u64) {
        self.revoked.insert(
            key_id.to_string(),
            RevocationEntry {
                key_id: key_id.to_string(),
                reason: reason.to_string(),
                revoked_at: timestamp,
            },
        );
    }

    pub fn is_revoked(&self, key_id: &str) -> bool {
        self.revoked.contains_key(key_id)
    }

    pub fn revocation_reason(&self, key_id: &str) -> Option<&str> {
        self.revoked.get(key_id).map(|e| e.reason.as_str())
    }

    pub fn revoked_count(&self) -> usize {
        self.revoked.len()
    }

    pub fn revoked_since(&self, since: u64) -> Vec<&RevocationEntry> {
        self.revoked
            .values()
            .filter(|e| e.revoked_at >= since)
            .collect()
    }

    /// Returns `true` if the signature's signer has not been revoked.
    pub fn check_signature(&self, signature: &Signature) -> bool {
        match &signature.signer {
            Some(s) => !self.is_revoked(s),
            None => true,
        }
    }
}


// ---------------------------------------------------------------------------
// SignatureCacheManager
// ---------------------------------------------------------------------------

/// Result of a cached signature verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedVerification {
    pub content_hash: u64,
    pub algorithm: SignatureAlgorithm,
    pub verified: bool,
    pub cached_at_epoch: u64,
    pub ttl_secs: u64,
}

impl CachedVerification {
    /// Whether this cache entry has expired given the current epoch time.
    pub fn is_expired(&self, current_epoch: u64) -> bool {
        current_epoch.saturating_sub(self.cached_at_epoch) > self.ttl_secs
    }
}

/// Caches signature verification results to avoid repeated computation.
#[derive(Debug)]
pub struct SignatureCacheManager {
    cache: HashMap<u64, CachedVerification>,
    default_ttl: u64,
    max_entries: usize,
    hits: u64,
    misses: u64,
}

impl SignatureCacheManager {
    /// Create a new cache manager with a given TTL (in seconds) and max capacity.
    pub fn new(default_ttl: u64, max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            default_ttl,
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// Simple hash function for content bytes.
    pub fn hash_content(content: &[u8]) -> u64 {
        let mut h: u64 = 5381;
        for &b in content {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    /// Store a verification result in the cache.
    pub fn store(&mut self, content_hash: u64, algorithm: SignatureAlgorithm, verified: bool, current_epoch: u64) {
        if self.cache.len() >= self.max_entries {
            // Evict the oldest entry
            if let Some(&oldest_key) = self.cache.values()
                .min_by_key(|v| v.cached_at_epoch)
                .map(|v| v.content_hash)
                .as_ref()
            {
                // Find and remove by content_hash match
                let key_to_remove = self.cache.iter()
                    .find(|(_, v)| v.content_hash == oldest_key)
                    .map(|(&k, _)| k);
                if let Some(k) = key_to_remove {
                    self.cache.remove(&k);
                }
            }
        }
        self.cache.insert(content_hash, CachedVerification {
            content_hash,
            algorithm,
            verified,
            cached_at_epoch: current_epoch,
            ttl_secs: self.default_ttl,
        });
    }

    /// Look up a cached result, returning `None` if not found or expired.
    pub fn lookup(&mut self, content_hash: u64, current_epoch: u64) -> Option<&CachedVerification> {
        let expired = self.cache.get(&content_hash)
            .map(|v| v.is_expired(current_epoch))
            .unwrap_or(false);
        if expired {
            self.cache.remove(&content_hash);
            self.misses += 1;
            return None;
        }
        if self.cache.contains_key(&content_hash) {
            self.hits += 1;
            self.cache.get(&content_hash)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Invalidate a specific entry.
    pub fn invalidate(&mut self, content_hash: u64) -> bool {
        self.cache.remove(&content_hash).is_some()
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Hit rate as a fraction (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    /// Remove all expired entries.
    pub fn evict_expired(&mut self, current_epoch: u64) -> usize {
        let before = self.cache.len();
        self.cache.retain(|_, v| !v.is_expired(current_epoch));
        before - self.cache.len()
    }
}

// ---------------------------------------------------------------------------
// SignatureTrustChainBuilder
// ---------------------------------------------------------------------------

/// A link in a trust chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustChainLink {
    pub key_id: String,
    pub issuer_key_id: Option<String>,
    pub trust_level: u8,
    pub description: String,
}

/// Errors specific to trust chain operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustChainError {
    MissingRootKey,
    CircularChain(String),
    KeyNotFound(String),
    ChainTooLong(usize),
}

impl fmt::Display for TrustChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRootKey => write!(f, "no root key found in chain"),
            Self::CircularChain(id) => write!(f, "circular chain detected at key {id}"),
            Self::KeyNotFound(id) => write!(f, "key not found: {id}"),
            Self::ChainTooLong(len) => write!(f, "chain exceeds maximum length: {len}"),
        }
    }
}

/// Builds and validates trust chains for signatures.
#[derive(Debug)]
pub struct SignatureTrustChainBuilder {
    links: Vec<TrustChainLink>,
    max_chain_length: usize,
}

impl SignatureTrustChainBuilder {
    pub fn new(max_chain_length: usize) -> Self {
        Self {
            links: Vec::new(),
            max_chain_length,
        }
    }

    /// Add a trust chain link.
    pub fn add_link(&mut self, key_id: &str, issuer: Option<&str>, trust_level: u8, description: &str) {
        self.links.push(TrustChainLink {
            key_id: key_id.to_string(),
            issuer_key_id: issuer.map(|s| s.to_string()),
            trust_level,
            description: description.to_string(),
        });
    }

    /// Find the root key (the link with no issuer).
    pub fn find_root(&self) -> Option<&TrustChainLink> {
        self.links.iter().find(|l| l.issuer_key_id.is_none())
    }

    /// Build the chain from root to a given leaf key.
    pub fn build_chain(&self, leaf_key_id: &str) -> Result<Vec<&TrustChainLink>, TrustChainError> {
        let mut chain = Vec::new();
        let mut current_id = leaf_key_id;
        let mut visited = std::collections::HashSet::new();

        loop {
            if !visited.insert(current_id.to_string()) {
                return Err(TrustChainError::CircularChain(current_id.to_string()));
            }
            let link = self.links.iter().find(|l| l.key_id == current_id)
                .ok_or_else(|| TrustChainError::KeyNotFound(current_id.to_string()))?;
            chain.push(link);
            if chain.len() > self.max_chain_length {
                return Err(TrustChainError::ChainTooLong(chain.len()));
            }
            match &link.issuer_key_id {
                Some(issuer) => current_id = issuer,
                None => break,
            }
        }
        chain.reverse();
        Ok(chain)
    }

    /// Validate the chain: ensure a root exists and trust levels are non-decreasing from root.
    pub fn validate_chain(&self, chain: &[&TrustChainLink]) -> bool {
        if chain.is_empty() {
            return false;
        }
        if chain[0].issuer_key_id.is_some() {
            return false;
        }
        for window in chain.windows(2) {
            if window[1].trust_level > window[0].trust_level {
                return false;
            }
        }
        true
    }

    /// Minimum trust level in a chain.
    pub fn min_trust_level(&self, chain: &[&TrustChainLink]) -> u8 {
        chain.iter().map(|l| l.trust_level).min().unwrap_or(0)
    }

    /// Total number of links registered.
    pub fn link_count(&self) -> usize {
        self.links.len()
    }
}



// ─── SignBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for signature audit.
#[derive(Debug, Clone)]
pub struct SignBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> SignBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for SignBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SignBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── SignBld Builder & Validator ─────────────────────────────

/// Builder for constructing signing configurations.
#[derive(Debug, Clone)]
pub struct SignBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl SignBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<SignBldCfg, SignBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(SignBldBuildErr { errors }); }
        Ok(SignBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated signing configuration.
#[derive(Debug, Clone)]
pub struct SignBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl SignBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &SignBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for SignBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SignBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct SignBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for SignBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SignBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for SignBldBuildErr {}



// ---------------------------------------------------------------------------
// sign – Extended signature chain helpers
// ---------------------------------------------------------------------------

/// Priority levels for signature chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZSignPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZSignPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZSignPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZSignPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks signature chain data.
#[derive(Debug, Clone)]
pub struct ZSignSignatureChain {
    pub signers: Vec<(String, u64)>,
    pub root_signer: String,
    pub chain_valid: bool,
}

impl ZSignSignatureChain {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            signers: Vec::new(),
            root_signer: String::new(),
            chain_valid: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.signers.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.signers.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.signers.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZSignSignatureChain[root_signer={:?}, chain_valid={:?}]", self.root_signer, self.chain_valid)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.chain_valid = !c.chain_valid;
        c
    }
}

/// Compute a simple rolling hash for signature chain.
pub fn z_sign_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_sign_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_sign_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_sign_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_sign_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_sign_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_sign_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 159
// ---------------------------------------------------------------------------

/// Generic object pool `Xc159Pool<T>`.
pub struct Xc159Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc159Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc159PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc159Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc159PoolStats {
        Xc159PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc159Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc159Scheduler`.
pub struct Xc159Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc159Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc159Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_159 hash for the given byte slice.
pub fn xc_159_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_159 convention.
pub fn xc_159_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_18 deepening: state machine + event bus ---

/// States for the Xd18 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd18State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd18State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd18Transition {
    pub from: Xd18State,
    pub to: Xd18State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd18StateMachine {
    current: Xd18State,
    history: Vec<Xd18Transition>,
    step_counter: usize,
}

impl Xd18StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd18State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd18State {
        self.current
    }

    pub fn history(&self) -> &[Xd18Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd18State) -> Result<Xd18State, String> {
        let allowed = match (self.current, target) {
            (Xd18State::Idle, Xd18State::Running) => true,
            (Xd18State::Running, Xd18State::Paused) => true,
            (Xd18State::Running, Xd18State::Done) => true,
            (Xd18State::Paused, Xd18State::Running) => true,
            (Xd18State::Paused, Xd18State::Done) => true,
            (Xd18State::Done, Xd18State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_18: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd18Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd18SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd18State> {
        let prefix = "Xd18SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd18State::Idle),
            "Running" => Some(Xd18State::Running),
            "Paused" => Some(Xd18State::Paused),
            "Done" => Some(Xd18State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd18State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd18 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd18Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd18Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd18HandlerFn = Box<dyn Fn(&Xd18Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd18EventBus {
    handlers: Vec<(usize, Option<String>, Xd18HandlerFn)>,
    next_id: usize,
    published: Vec<Xd18Event>,
}

impl Xd18EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd18Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd18Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd18Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd18Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #16
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf16Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf16TrieNode {
    children: std::collections::HashMap<char, Xf16TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf16Trie {
    root: Xf16TrieNode,
    count: usize,
}

impl Xf16Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf16TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf16TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf16TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf16BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf16BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 158).
pub struct Xh158SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh158SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 200 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 158).
pub struct Xh158BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh158BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
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

    // ─── Signature statistics tests ─────────────────────────────

    #[test]
    fn stats_empty_collection() {
        let stats = compute_signature_stats(&[]);
        assert_eq!(stats.total_signatures, 0);
        assert_eq!(stats.total_parameters, 0);
        assert!((stats.avg_params_per_signature - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_single_signature() {
        let sig = sign_content(b"hello", b"key", SignatureAlgorithm::HmacSha256Stub);
        let stats = compute_signature_stats(&[sig]);
        assert_eq!(stats.total_signatures, 1);
        assert_eq!(stats.total_parameters, 5);
        assert!((stats.avg_params_per_signature - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_multiple_signatures() {
        let s1 = sign_content(b"ab", b"k", SignatureAlgorithm::HmacSha256Stub);
        let s2 = sign_content(b"cdef", b"k", SignatureAlgorithm::Ed25519Stub);
        let s3 = sign_content(b"ghijkl", b"k", SignatureAlgorithm::HmacSha256Stub);
        let stats = compute_signature_stats(&[s1, s2, s3]);
        assert_eq!(stats.total_signatures, 3);
        assert_eq!(stats.total_parameters, 2 + 4 + 6);
        assert!((stats.avg_params_per_signature - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_with_empty_value_signature() {
        let empty_sig = Signature {
            algorithm: SignatureAlgorithm::HmacSha256Stub,
            value: vec![],
            signer: None,
        };
        let normal = sign_content(b"abc", b"k", SignatureAlgorithm::Ed25519Stub);
        let stats = compute_signature_stats(&[empty_sig, normal]);
        assert_eq!(stats.total_signatures, 2);
        assert_eq!(stats.total_parameters, 3);
        assert!((stats.avg_params_per_signature - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_struct_debug_and_clone() {
        let stats = compute_signature_stats(&[]);
        let cloned = stats.clone();
        assert_eq!(stats, cloned);
        let debug = format!("{stats:?}");
        assert!(debug.contains("SignatureStats"));
    }

    #[test]
    fn sign_stats_new_defaults() {
        let stats = SignStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn sign_stats_record_success() {
        let mut stats = SignStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sign_stats_record_failure() {
        let mut stats = SignStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn sign_stats_reset() {
        let mut stats = SignStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn sign_stats_merge() {
        let mut a = SignStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = SignStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn sign_stats_display() {
        let mut stats = SignStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn sign_stats_default() {
        let stats = SignStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn sign_validator_accepts_valid_name() {
        let v = SignValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn sign_validator_rejects_empty() {
        let v = SignValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn sign_validator_rejects_too_long() {
        let v = SignValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn sign_validator_forbidden_prefix() {
        let v = SignValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn sign_validator_allowed_chars() {
        let v = SignValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn sign_validator_range() {
        let v = SignValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn sign_sanitize_removes_control() {
        let result = SignValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn sign_truncate_short_string() {
        assert_eq!(SignValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn sign_truncate_long_string() {
        let result = SignValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn sign_is_ascii_printable() {
        assert!(SignValidator::is_ascii_printable("Hello World 123"));
        assert!(!SignValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn signature_param_display_inline_basic() {
        let p = SignatureParameter::new("name", "string");
        assert_eq!(p.display_inline(), "name: string");
    }

    #[test]
    fn signature_param_display_inline_optional() {
        let p = SignatureParameter::new("count", "number").optional();
        assert_eq!(p.display_inline(), "count?: number");
    }

    #[test]
    fn signature_param_display_inline_with_default() {
        let p = SignatureParameter::new("count", "number").with_default("0");
        assert_eq!(p.display_inline(), "count?: number = 0");
    }

    #[test]
    fn signature_param_tooltip_with_doc() {
        let p = SignatureParameter::new("path", "string")
            .with_doc("The file path to open");
        let tooltip = p.display_tooltip();
        assert!(tooltip.contains("@param path"));
        assert!(tooltip.contains("The file path to open"));
    }

    #[test]
    fn signature_param_tooltip_without_doc() {
        let p = SignatureParameter::new("x", "i32").optional();
        let tooltip = p.display_tooltip();
        assert!(tooltip.contains("i32"));
        assert!(tooltip.contains("(optional)"));
    }

    #[test]
    fn format_signature_tooltip_multiple_params() {
        let params = vec![
            SignatureParameter::new("a", "i32").with_doc("First value"),
            SignatureParameter::new("b", "i32").with_doc("Second value"),
        ];
        let tooltip = format_signature_tooltip("add", &params);
        assert!(tooltip.starts_with("add(a: i32, b: i32)"));
        assert!(tooltip.contains("@param a"));
        assert!(tooltip.contains("@param b"));
    }

    #[test]
    fn signature_param_display_trait() {
        let p = SignatureParameter::new("x", "f64");
        assert_eq!(format!("{p}"), "x: f64");
    }

    // ─── New functionality tests ────────────────────────────────

    #[test]
    fn algorithm_is_asymmetric() {
        assert!(SignatureAlgorithm::Ed25519Stub.is_asymmetric());
        assert!(!SignatureAlgorithm::HmacSha256Stub.is_asymmetric());
        assert!(SignatureAlgorithm::HmacSha256Stub.is_symmetric());
        assert!(!SignatureAlgorithm::Ed25519Stub.is_symmetric());
    }

    #[test]
    fn algorithm_key_size_and_label() {
        assert_eq!(SignatureAlgorithm::HmacSha256Stub.key_size_bits(), 256);
        assert_eq!(SignatureAlgorithm::Ed25519Stub.key_size_bits(), 256);
        assert_eq!(
            format!("{}", SignatureAlgorithm::HmacSha256Stub),
            "HMAC-SHA256 (stub)"
        );
        assert_eq!(
            format!("{}", SignatureAlgorithm::Ed25519Stub),
            "Ed25519 (stub)"
        );
    }

    #[test]
    fn signature_parameter_count_and_has_parameters() {
        let sig = sign_content(b"abc", b"k", SignatureAlgorithm::HmacSha256Stub);
        assert_eq!(sig.parameter_count(), 3);
        assert!(sig.has_parameters());

        let empty = Signature {
            algorithm: SignatureAlgorithm::Ed25519Stub,
            value: vec![],
            signer: None,
        };
        assert_eq!(empty.parameter_count(), 0);
        assert!(!empty.has_parameters());
    }

    #[test]
    fn signature_matches_algorithm() {
        let sig = sign_content(b"x", b"k", SignatureAlgorithm::HmacSha256Stub);
        assert!(sig.matches_algorithm(SignatureAlgorithm::HmacSha256Stub));
        assert!(!sig.matches_algorithm(SignatureAlgorithm::Ed25519Stub));
    }

    #[test]
    fn glyph_decoration_visibility_and_overlap() {
        let visible = GlyphDecoration::new(1, 5, '●', "red");
        assert!(visible.is_visible());
        assert!(visible.overlaps_line(5));
        assert!(!visible.overlaps_line(6));
        assert!(!visible.has_tooltip());

        let with_tip = visible.with_tooltip("info");
        assert!(with_tip.has_tooltip());

        let invisible_color = GlyphDecoration::new(2, 1, '●', "");
        assert!(!invisible_color.is_visible());

        let invisible_char = GlyphDecoration::new(3, 1, ' ', "red");
        assert!(!invisible_char.is_visible());
    }

    #[test]
    fn glyph_margin_service_iter_and_into_iter() {
        let mut svc = GlyphMarginService::new();
        svc.register_decoration(1, 'A', "r");
        svc.register_decoration(2, 'B', "g");
        svc.register_decoration(3, 'C', "b");

        let count = svc.iter().count();
        assert_eq!(count, 3);

        let count2 = (&svc).into_iter().count();
        assert_eq!(count2, 3);
    }

    #[test]
    fn glyph_margin_service_line_range_and_visible() {
        let mut svc = GlyphMarginService::new();
        assert!(svc.line_range().is_none());

        svc.register_decoration(10, '●', "red");
        svc.register_decoration(3, '▶', "green");
        svc.register_decoration(7, ' ', "blue");

        assert_eq!(svc.line_range(), Some((3, 10)));
        assert_eq!(svc.visible_decoration_count(), 2);
        assert!(svc.has_decoration_on_line(10));
        assert!(!svc.has_decoration_on_line(99));
    }

    #[test]
    fn signature_stats_merge_and_summary() {
        let s1 = sign_content(b"ab", b"k", SignatureAlgorithm::HmacSha256Stub);
        let s2 = sign_content(b"cdef", b"k", SignatureAlgorithm::Ed25519Stub);
        let stats_a = compute_signature_stats(&[s1]);
        let stats_b = compute_signature_stats(&[s2]);

        let merged = stats_a.merge(&stats_b);
        assert_eq!(merged.total_signatures, 2);
        assert_eq!(merged.total_parameters, 6);
        assert!((merged.avg_params_per_signature - 3.0).abs() < f64::EPSILON);
        assert!(!merged.is_empty());

        let summary = merged.summary();
        assert!(summary.contains("2 signature(s)"));
        assert!(summary.contains("6 total parameter(s)"));

        let empty = compute_signature_stats(&[]);
        assert!(empty.is_empty());

        let display = format!("{}", merged);
        assert!(display.contains("3.00 avg"));
    }

    #[test]
    fn signature_parameter_required_and_has_flags() {
        let required = SignatureParameter::new("x", "i32");
        assert!(required.is_required());
        assert!(!required.has_documentation());
        assert!(!required.has_default());

        let optional_with_all = SignatureParameter::new("y", "string")
            .optional()
            .with_doc("A value")
            .with_default("foo");
        assert!(!optional_with_all.is_required());
        assert!(optional_with_all.has_documentation());
        assert!(optional_with_all.has_default());
    }

    // ─── Certificate chain tests ────────────────────────────────

    #[test]
    fn certificate_chain_valid() {
        let root = SigningCertificate::new_root(
            "RootCA", SignatureAlgorithm::Ed25519Stub, 1000, 9999, b"rootkey",
        );
        let intermediate = SigningCertificate::new_issued(
            "IntermediateCA", "RootCA", SignatureAlgorithm::Ed25519Stub, 1000, 9999, b"intkey",
        );
        let leaf = SigningCertificate::new_issued(
            "LeafCert", "IntermediateCA", SignatureAlgorithm::HmacSha256Stub, 1000, 9999, b"leafkey",
        );
        assert!(validate_certificate_chain(&[leaf, intermediate, root], 5000).is_ok());
    }

    #[test]
    fn certificate_chain_expired() {
        let root = SigningCertificate::new_root(
            "RootCA", SignatureAlgorithm::Ed25519Stub, 1000, 2000, b"key",
        );
        let leaf = SigningCertificate::new_issued(
            "Leaf", "RootCA", SignatureAlgorithm::HmacSha256Stub, 1000, 2000, b"key2",
        );
        let result = validate_certificate_chain(&[leaf, root], 3000);
        assert!(matches!(result, Err(ChainError::Expired { .. })));
    }

    #[test]
    fn certificate_chain_broken_link() {
        let root = SigningCertificate::new_root(
            "RootCA", SignatureAlgorithm::Ed25519Stub, 0, 99999, b"k",
        );
        let leaf = SigningCertificate::new_issued(
            "Leaf", "WrongIssuer", SignatureAlgorithm::HmacSha256Stub, 0, 99999, b"k2",
        );
        let result = validate_certificate_chain(&[leaf, root], 500);
        assert!(matches!(result, Err(ChainError::BrokenLink { .. })));
    }

    #[test]
    fn certificate_chain_untrusted_root() {
        let not_root = SigningCertificate::new_issued(
            "NotRoot", "SomeIssuer", SignatureAlgorithm::Ed25519Stub, 0, 99999, b"k",
        );
        let result = validate_certificate_chain(&[not_root], 500);
        assert!(matches!(result, Err(ChainError::UntrustedRoot { .. })));
    }

    #[test]
    fn certificate_chain_empty() {
        assert_eq!(validate_certificate_chain(&[], 0), Err(ChainError::EmptyChain));
    }

    #[test]
    fn certificate_validity_helpers() {
        let cert = SigningCertificate::new_root(
            "Test", SignatureAlgorithm::Ed25519Stub, 100, 200, b"k",
        );
        assert!(cert.is_root());
        assert!(cert.is_valid_at(150));
        assert!(!cert.is_valid_at(50));
        assert!(!cert.is_valid_at(250));
        assert!(cert.is_expired_at(201));
        assert!(!cert.is_expired_at(200));
        assert_eq!(cert.remaining_validity(150), 50);
        assert_eq!(cert.remaining_validity(300), 0);
        let display = format!("{cert}");
        assert!(display.contains("root"));
    }

    // ─── Digest computation tests ───────────────────────────────

    #[test]
    fn digest_xor_matches_checksum() {
        let data = b"hello";
        let digest = compute_digest(data, DigestAlgorithm::Xor);
        assert_eq!(digest.value.len(), 1);
        assert_eq!(digest.value[0], content_checksum(data));
        assert!(verify_digest(data, &digest));
    }

    #[test]
    fn digest_djb2_deterministic() {
        let d1 = compute_digest(b"test data", DigestAlgorithm::Djb2);
        let d2 = compute_digest(b"test data", DigestAlgorithm::Djb2);
        assert_eq!(d1, d2);
        assert_eq!(d1.value.len(), 8);
        assert!(verify_digest(b"test data", &d1));
        assert!(!verify_digest(b"other data", &d1));
    }

    #[test]
    fn digest_fnv1a_deterministic_and_different_from_djb2() {
        let fnv = compute_digest(b"sample", DigestAlgorithm::Fnv1a64);
        let djb = compute_digest(b"sample", DigestAlgorithm::Djb2);
        assert_eq!(fnv.value.len(), 8);
        assert_ne!(fnv.value, djb.value);
        assert!(verify_digest(b"sample", &fnv));
        let display = format!("{fnv}");
        assert!(display.contains("Fnv1a64"));
    }

    // ─── Batch verification tests ───────────────────────────────

    #[test]
    fn batch_verify_detailed_all_valid() {
        let key = b"batchkey";
        let algo = SignatureAlgorithm::HmacSha256Stub;
        let s1 = sign_content(b"file1", key, algo);
        let s2 = sign_content(b"file2", key, algo);
        let entries = vec![
            BatchEntry { label: "file1.txt".into(), content: b"file1", signature: &s1 },
            BatchEntry { label: "file2.txt".into(), content: b"file2", signature: &s2 },
        ];
        let results = verify_batch_detailed(&entries, key, algo);
        assert_eq!(results.len(), 2);
        assert!(batch_all_valid(&results));
        assert_eq!(batch_valid_count(&results), 2);
        assert_eq!(results[0].label, "file1.txt");
    }

    #[test]
    fn batch_verify_detailed_with_failures() {
        let key = b"k";
        let algo = SignatureAlgorithm::HmacSha256Stub;
        let good_sig = sign_content(b"good", key, algo);
        let bad_sig = sign_content(b"original", key, algo);
        let wrong_algo_sig = sign_content(b"data", key, SignatureAlgorithm::Ed25519Stub);
        let entries = vec![
            BatchEntry { label: "good.rs".into(), content: b"good", signature: &good_sig },
            BatchEntry { label: "tampered.rs".into(), content: b"tampered", signature: &bad_sig },
            BatchEntry { label: "wrong_algo.rs".into(), content: b"data", signature: &wrong_algo_sig },
        ];
        let results = verify_batch_detailed(&entries, key, algo);
        assert!(results[0].valid);
        assert!(!results[1].valid);
        assert!(results[1].reason.as_ref().unwrap().contains("does not match"));
        assert!(!results[2].valid);
        assert!(results[2].reason.as_ref().unwrap().contains("algorithm mismatch"));
        assert!(!batch_all_valid(&results));
        assert_eq!(batch_valid_count(&results), 1);
    }

    // ─── Metadata extraction tests ──────────────────────────────

    #[test]
    fn extract_metadata_fields() {
        let sig = sign_content(b"hello", b"key", SignatureAlgorithm::Ed25519Stub)
            .with_signer("bob");
        let meta = extract_metadata(&sig);
        assert_eq!(meta.algorithm, SignatureAlgorithm::Ed25519Stub);
        assert!(meta.is_asymmetric);
        assert_eq!(meta.key_size_bits, 256);
        assert_eq!(meta.signature_length, 5);
        assert_eq!(meta.signer, Some("bob".to_string()));
        assert_eq!(meta.algorithm_label, "Ed25519 (stub)");
        assert_eq!(meta.signature_hex.len(), 10); // 5 bytes * 2 hex chars
        let display = format!("{meta}");
        assert!(display.contains("Ed25519"));
        assert!(display.contains("bob"));
    }

    // ─── SignatureVerifier tests ──────────────────────────────────

    #[test]
    fn verifier_valid_package() {
        let key = b"trusted-key";
        let content = b"package data";
        let sig = sign_content(content, key, SignatureAlgorithm::HmacSha256Stub);
        let mut v = SignatureVerifier::new();
        v.add_key("k1", key, SignatureAlgorithm::HmacSha256Stub);
        let result = v.verify_package(content, &sig);
        assert!(result.is_valid());
        assert_eq!(format!("{result}"), "Valid(key=k1)");
    }

    #[test]
    fn verifier_invalid_signature() {
        let mut v = SignatureVerifier::new();
        v.add_key("k1", b"wrong-key", SignatureAlgorithm::HmacSha256Stub);
        let sig = sign_content(b"data", b"real-key", SignatureAlgorithm::HmacSha256Stub);
        let result = v.verify_package(b"data", &sig);
        assert_eq!(result, VerifyResult::Invalid);
        assert!(!result.is_valid());
    }

    #[test]
    fn verifier_no_matching_key() {
        let v = SignatureVerifier::new();
        let sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub);
        assert_eq!(v.verify_package(b"data", &sig), VerifyResult::NoMatchingKey);
    }

    #[test]
    fn verifier_key_not_trusted() {
        let mut v = SignatureVerifier::new();
        v.add_key("k1", b"key", SignatureAlgorithm::HmacSha256Stub);
        v.trusted_keys[0].trusted = false;
        let sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub);
        let result = v.verify_package(b"data", &sig);
        assert_eq!(result, VerifyResult::KeyNotTrusted("k1".into()));
        assert!(format!("{result}").contains("k1"));
    }

    // ─── SignatureTrustStore tests ───────────────────────────────

    #[test]
    fn trust_store_operations() {
        let mut ts = SignatureTrustStore::new();
        ts.add_trusted("a", TrustLevel::Full, 100);
        ts.add_trusted("b", TrustLevel::Partial, 200);
        ts.add_trusted("c", TrustLevel::Untrusted, 300);
        assert!(ts.is_trusted("a"));
        assert!(ts.is_trusted("b"));
        assert!(!ts.is_trusted("c"));
        assert!(!ts.is_trusted("missing"));
        assert_eq!(ts.trusted_count(), 2);
        assert_eq!(*ts.get_trust_level("a").unwrap(), TrustLevel::Full);
        assert!(ts.get_trust_level("missing").is_none());
    }

    #[test]
    fn trust_store_revoke_and_mark_used() {
        let mut ts = SignatureTrustStore::new();
        ts.add_trusted("k", TrustLevel::Full, 10);
        assert!(ts.is_trusted("k"));
        assert!(ts.revoke("k"));
        assert!(!ts.is_trusted("k"));
        assert!(!ts.revoke("nonexistent"));
        ts.add_trusted("k2", TrustLevel::Partial, 20);
        ts.mark_used("k2", 50);
        let entries = ts.all_entries();
        let k2 = entries.iter().find(|e| e.key_id == "k2").unwrap();
        assert_eq!(k2.last_used, Some(50));
    }

    #[test]
    fn trust_level_display() {
        assert_eq!(format!("{}", TrustLevel::Full), "Full");
        assert_eq!(format!("{}", TrustLevel::Partial), "Partial");
        assert_eq!(format!("{}", TrustLevel::Untrusted), "Untrusted");
    }

    // ─── SignatureChain tests ────────────────────────────────────

    #[test]
    fn chain_validate_and_render() {
        let mut chain = SignatureChain::new();
        chain.add_link("root-ca", &[0xAA], SignatureAlgorithm::Ed25519Stub);
        chain.add_link("intermediate", &[0xBB], SignatureAlgorithm::Ed25519Stub);
        chain.add_link("leaf", &[0xCC], SignatureAlgorithm::HmacSha256Stub);
        assert!(!chain.is_valid());
        assert!(chain.validate_chain());
        assert!(chain.is_valid());
        assert_eq!(chain.chain_length(), 3);
        assert_eq!(chain.root_signer(), Some("root-ca"));
        assert_eq!(chain.leaf_signer(), Some("leaf"));
        let rendered = chain.render();
        assert!(rendered.contains("root-ca"));
        assert!(rendered.contains("leaf"));
        assert!(rendered.contains("✓"));
    }

    #[test]
    fn chain_empty_is_invalid() {
        let chain = SignatureChain::new();
        assert!(!chain.is_valid());
        assert_eq!(chain.chain_length(), 0);
        assert!(chain.root_signer().is_none());
        assert!(chain.leaf_signer().is_none());
    }

    // ─── RevocationChecker tests ─────────────────────────────────

    #[test]
    fn revocation_basic_operations() {
        let mut rc = RevocationChecker::new();
        rc.revoke("bad-key", "compromised", 1000);
        assert!(rc.is_revoked("bad-key"));
        assert!(!rc.is_revoked("good-key"));
        assert_eq!(rc.revocation_reason("bad-key"), Some("compromised"));
        assert!(rc.revocation_reason("good-key").is_none());
        assert_eq!(rc.revoked_count(), 1);
    }

    #[test]
    fn revocation_since_filter() {
        let mut rc = RevocationChecker::new();
        rc.revoke("old", "expired", 100);
        rc.revoke("new", "leaked", 500);
        let since_300 = rc.revoked_since(300);
        assert_eq!(since_300.len(), 1);
        assert_eq!(since_300[0].key_id, "new");
        assert_eq!(rc.revoked_since(0).len(), 2);
    }

    #[test]
    fn revocation_check_signature() {
        let mut rc = RevocationChecker::new();
        rc.revoke("alice", "compromised", 1000);
        let good_sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub)
            .with_signer("bob");
        let bad_sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub)
            .with_signer("alice");
        let anon_sig = sign_content(b"data", b"key", SignatureAlgorithm::HmacSha256Stub);
        assert!(rc.check_signature(&good_sig));
        assert!(!rc.check_signature(&bad_sig));
        assert!(rc.check_signature(&anon_sig));
    }

    // -- SignatureCacheManager tests ------------------------------------------

    #[test]
    fn cache_store_and_lookup() {
        let mut c = SignatureCacheManager::new(300, 100);
        c.store(42, SignatureAlgorithm::HmacSha256Stub, true, 1000);
        let result = c.lookup(42, 1100);
        assert!(result.is_some());
        assert!(result.unwrap().verified);
    }

    #[test]
    fn cache_expired_entry() {
        let mut c = SignatureCacheManager::new(100, 100);
        c.store(42, SignatureAlgorithm::HmacSha256Stub, true, 1000);
        let result = c.lookup(42, 1200);
        assert!(result.is_none());
    }

    #[test]
    fn cache_invalidate() {
        let mut c = SignatureCacheManager::new(300, 100);
        c.store(42, SignatureAlgorithm::HmacSha256Stub, true, 1000);
        assert!(c.invalidate(42));
        assert!(c.is_empty());
    }

    #[test]
    fn cache_hit_rate() {
        let mut c = SignatureCacheManager::new(300, 100);
        c.store(1, SignatureAlgorithm::Ed25519Stub, true, 0);
        let _ = c.lookup(1, 0); // hit
        let _ = c.lookup(2, 0); // miss
        assert!((c.hit_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn cache_evict_expired() {
        let mut c = SignatureCacheManager::new(50, 100);
        c.store(1, SignatureAlgorithm::HmacSha256Stub, true, 100);
        c.store(2, SignatureAlgorithm::HmacSha256Stub, false, 200);
        let evicted = c.evict_expired(160);
        assert_eq!(evicted, 1);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_hash_content_deterministic() {
        let h1 = SignatureCacheManager::hash_content(b"hello");
        let h2 = SignatureCacheManager::hash_content(b"hello");
        assert_eq!(h1, h2);
        let h3 = SignatureCacheManager::hash_content(b"world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn cache_clear() {
        let mut c = SignatureCacheManager::new(300, 100);
        c.store(1, SignatureAlgorithm::HmacSha256Stub, true, 0);
        c.store(2, SignatureAlgorithm::Ed25519Stub, false, 0);
        c.clear();
        assert!(c.is_empty());
    }

    // -- SignatureTrustChainBuilder tests -------------------------------------

    #[test]
    fn trust_chain_build_simple() {
        let mut b = SignatureTrustChainBuilder::new(10);
        b.add_link("root", None, 100, "Root CA");
        b.add_link("intermediate", Some("root"), 80, "Intermediate");
        b.add_link("leaf", Some("intermediate"), 60, "Leaf");
        let chain = b.build_chain("leaf").unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].key_id, "root");
        assert_eq!(chain[2].key_id, "leaf");
    }

    #[test]
    fn trust_chain_validate_valid() {
        let mut b = SignatureTrustChainBuilder::new(10);
        b.add_link("root", None, 100, "Root");
        b.add_link("leaf", Some("root"), 80, "Leaf");
        let chain = b.build_chain("leaf").unwrap();
        assert!(b.validate_chain(&chain));
    }

    #[test]
    fn trust_chain_circular_detection() {
        let mut b = SignatureTrustChainBuilder::new(10);
        b.add_link("a", Some("b"), 100, "A");
        b.add_link("b", Some("a"), 100, "B");
        let result = b.build_chain("a");
        assert!(matches!(result, Err(TrustChainError::CircularChain(_))));
    }

    #[test]
    fn trust_chain_key_not_found() {
        let b = SignatureTrustChainBuilder::new(10);
        let result = b.build_chain("nonexistent");
        assert!(matches!(result, Err(TrustChainError::KeyNotFound(_))));
    }

    #[test]
    fn trust_chain_min_trust_level() {
        let mut b = SignatureTrustChainBuilder::new(10);
        b.add_link("root", None, 100, "Root");
        b.add_link("mid", Some("root"), 50, "Mid");
        b.add_link("leaf", Some("mid"), 70, "Leaf");
        let chain = b.build_chain("leaf").unwrap();
        assert_eq!(b.min_trust_level(&chain), 50);
    }

    #[test]
    fn trust_chain_error_display() {
        let e = TrustChainError::MissingRootKey;
        assert_eq!(e.to_string(), "no root key found in chain");
        let e2 = TrustChainError::ChainTooLong(5);
        assert!(e2.to_string().contains("5"));
    }



    #[test]
    fn signbuf_ringbuf_push_get() {
        let mut rb = SignBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn signbuf_ringbuf_overflow() {
        let mut rb = SignBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn signbuf_ringbuf_clear() {
        let mut rb = SignBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn signbuf_ringbuf_newest_oldest() {
        let mut rb = SignBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn signbuf_ringbuf_to_vec() {
        let mut rb = SignBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn signbuf_ringbuf_is_full() {
        let mut rb = SignBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn signbld_builder_valid() {
        let cfg = SignBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn signbld_builder_empty_name() {
        let r = SignBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn signbld_builder_bad_priority() {
        assert!(SignBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn signbld_builder_zero_max() {
        assert!(SignBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn signbld_cfg_merge() {
        let mut a = SignBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = SignBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn signbld_cfg_display() {
        let cfg = SignBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    // -- sign Z-extended tests -----------------------------------------------

    #[test]
    fn z_sign_priority_weight() {
        assert_eq!(ZSignPriority::Idle.weight(), 0);
        assert_eq!(ZSignPriority::Normal.weight(), 2);
        assert_eq!(ZSignPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_sign_priority_label() {
        assert_eq!(ZSignPriority::Low.label(), "low");
        assert_eq!(ZSignPriority::High.label(), "high");
    }

    #[test]
    fn z_sign_priority_is_elevated() {
        assert!(!ZSignPriority::Normal.is_elevated());
        assert!(ZSignPriority::High.is_elevated());
        assert!(ZSignPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_sign_priority_display() {
        assert_eq!(format!("{}", ZSignPriority::Idle), "idle");
    }

    #[test]
    fn z_sign_priority_all_asc() {
        let all = ZSignPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZSignPriority::Idle);
        assert_eq!(all[4], ZSignPriority::Realtime);
    }

    #[test]
    fn z_sign_struct_new() {
        let s = ZSignSignatureChain::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_sign_struct_toggled_clone() {
        let s = ZSignSignatureChain::new();
        let t = s.toggled_clone();
        assert_ne!(s.chain_valid, t.chain_valid);
    }

    #[test]
    fn z_sign_rolling_hash_deterministic() {
        let h1 = z_sign_rolling_hash(b"test");
        let h2 = z_sign_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_sign_rolling_hash(b"a"), z_sign_rolling_hash(b"b"));
    }

    #[test]
    fn z_sign_pad_to_basic() {
        assert_eq!(z_sign_pad_to("hi", 5), "hi   ");
        assert_eq!(z_sign_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_sign_is_identifier_basic() {
        assert!(z_sign_is_identifier("foo_bar"));
        assert!(z_sign_is_identifier("abc123"));
        assert!(!z_sign_is_identifier(""));
        assert!(!z_sign_is_identifier("has space"));
    }

    #[test]
    fn z_sign_levenshtein_basic() {
        assert_eq!(z_sign_levenshtein("", ""), 0);
        assert_eq!(z_sign_levenshtein("abc", "abc"), 0);
        assert_eq!(z_sign_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_sign_unique_words_basic() {
        let w = z_sign_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_sign_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_sign_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_sign_common_prefix_basic() {
        assert_eq!(z_sign_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_sign_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_sign_struct_clear() {
        let mut s = ZSignSignatureChain::new();
        s.signers.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_sign_rolling_hash_empty() {
        let h = z_sign_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // ---- xc_ pool / scheduler tests – block 159 ----

    #[test]
    fn xc_159_pool_new_empty() {
        let pool: super::Xc159Pool<i32> = super::Xc159Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_159_pool_release_acquire() {
        let mut pool = super::Xc159Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_159_pool_acquire_empty() {
        let mut pool: super::Xc159Pool<i32> = super::Xc159Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_159_pool_full() {
        let mut pool = super::Xc159Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_159_pool_drain() {
        let mut pool = super::Xc159Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_159_pool_stats() {
        let mut pool = super::Xc159Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_159_pool_clear() {
        let mut pool = super::Xc159Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_159_pool_shrink() {
        let mut pool = super::Xc159Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_159_pool_default() {
        let pool: super::Xc159Pool<String> = super::Xc159Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_159_pool_extend() {
        let mut pool = super::Xc159Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_159_pool_retain() {
        let mut pool = super::Xc159Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_159_scheduler_round_robin() {
        let mut sched = super::Xc159Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_159_scheduler_empty() {
        let mut sched = super::Xc159Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_159_scheduler_reset() {
        let mut sched = super::Xc159Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_159_scheduler_add_remove() {
        let mut sched = super::Xc159Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_159_scheduler_targets() {
        let sched = super::Xc159Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_159_hash_empty() {
        assert_eq!(super::xc_159_hash(b""), 5381);
    }

    #[test]
    fn xc_159_hash_data() {
        let h = super::xc_159_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_159_hash(b"hello"), h);
    }

    #[test]
    fn xc_159_reverse_str() {
        assert_eq!(super::xc_159_reverse("abc"), "cba");
        assert_eq!(super::xc_159_reverse(""), "");
    }


    // --- xd_18 deepening tests ---

    #[test]
    fn xd_18_sm_initial_state() {
        let sm = Xd18StateMachine::new();
        assert_eq!(sm.current_state(), Xd18State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_18_sm_valid_idle_to_running() {
        let mut sm = Xd18StateMachine::new();
        assert!(sm.transition(Xd18State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd18State::Running);
    }

    #[test]
    fn xd_18_sm_valid_running_to_paused() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        assert!(sm.transition(Xd18State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd18State::Paused);
    }

    #[test]
    fn xd_18_sm_valid_running_to_done() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        assert!(sm.transition(Xd18State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd18State::Done);
    }

    #[test]
    fn xd_18_sm_valid_paused_to_running() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        sm.transition(Xd18State::Paused).unwrap();
        assert!(sm.transition(Xd18State::Running).is_ok());
    }

    #[test]
    fn xd_18_sm_valid_done_to_idle() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        sm.transition(Xd18State::Done).unwrap();
        assert!(sm.transition(Xd18State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd18State::Idle);
    }

    #[test]
    fn xd_18_sm_invalid_idle_to_done() {
        let mut sm = Xd18StateMachine::new();
        assert!(sm.transition(Xd18State::Done).is_err());
    }

    #[test]
    fn xd_18_sm_invalid_idle_to_paused() {
        let mut sm = Xd18StateMachine::new();
        assert!(sm.transition(Xd18State::Paused).is_err());
    }

    #[test]
    fn xd_18_sm_history_tracking() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        sm.transition(Xd18State::Paused).unwrap();
        sm.transition(Xd18State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd18State::Idle);
        assert_eq!(sm.history()[0].to, Xd18State::Running);
        assert_eq!(sm.history()[1].from, Xd18State::Running);
        assert_eq!(sm.history()[2].to, Xd18State::Done);
    }

    #[test]
    fn xd_18_sm_serialize_deserialize() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd18StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd18State::Running));
    }

    #[test]
    fn xd_18_sm_deserialize_invalid() {
        assert_eq!(Xd18StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_18_sm_reset() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd18State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_18_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd18EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd18Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_18_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd18EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd18Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd18Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_18_bus_unsubscribe() {
        let mut bus = Xd18EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_18_event_kind_and_payload() {
        let e = Xd18Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd18Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_18_bus_clear_history() {
        let mut bus = Xd18EventBus::new();
        bus.publish(Xd18Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_18_sm_step_counter_increments() {
        let mut sm = Xd18StateMachine::new();
        sm.transition(Xd18State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd18State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #16 --

    #[test]
    fn xf16_trie_insert_search() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf16_trie_starts_with() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf16_trie_remove() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf16_trie_word_count() {
        let mut t = Xf16Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf16_trie_longest_prefix() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf16_trie_all_words() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf16_trie_autocomplete() {
        let mut t = Xf16Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf16_trie_empty_search() {
        let t = Xf16Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf16_bloom_add_contains() {
        let mut bf = Xf16BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf16_bloom_probably_absent() {
        let bf = Xf16BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf16_bloom_false_positive_rate() {
        let mut bf = Xf16BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf16_bloom_clear() {
        let mut bf = Xf16BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf16_bloom_union() {
        let mut a = Xf16BloomFilter::xf_new(512, 2);
        let mut b = Xf16BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf16_bloom_intersection_estimate() {
        let mut a = Xf16BloomFilter::xf_new(512, 2);
        let mut b = Xf16BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf16_bloom_union_size_mismatch() {
        let a = Xf16BloomFilter::xf_new(256, 2);
        let b = Xf16BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh158_skip_insert_contains() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh158_skip_remove() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh158_skip_len() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh158_skip_range_query() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh158_skip_floor_ceiling() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh158_skip_rank() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh158_skip_empty() {
        let sl = super::Xh158SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh158_skip_duplicates() {
        let mut sl = super::Xh158SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh158_bitset_set_test() {
        let mut bs = super::Xh158BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh158_bitset_clear_count() {
        let mut bs = super::Xh158BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh158_bitset_and_or_xor() {
        let mut a = super::Xh158BitSet::xh_new(128);
        let mut b = super::Xh158BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh158_bitset_iter_ones() {
        let mut bs = super::Xh158BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh158_bitset_first_last() {
        let mut bs = super::Xh158BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh158_bitset_empty() {
        let bs = super::Xh158BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}