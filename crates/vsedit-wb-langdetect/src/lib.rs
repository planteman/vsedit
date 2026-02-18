//! Auto language detection.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur during language detection.
#[derive(Debug, Clone, PartialEq)]
pub enum DetectionError {
    /// The filename was empty or invalid.
    InvalidFilename(String),
    /// The confidence threshold was outside the valid range [0.0, 1.0].
    InvalidConfidence(f64),
    /// No language could be detected from the provided input.
    NoMatch,
}

impl fmt::Display for DetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectionError::InvalidFilename(name) => {
                write!(f, "invalid filename: '{}'", name)
            }
            DetectionError::InvalidConfidence(val) => {
                write!(f, "confidence {} is outside the valid range [0.0, 1.0]", val)
            }
            DetectionError::NoMatch => write!(f, "no language detected"),
        }
    }
}

impl std::error::Error for DetectionError {}

/// Result of a language detection heuristic.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionResult {
    pub language_id: String,
    pub confidence: f64,
}

impl DetectionResult {
    /// Create a new detection result, validating the confidence range.
    pub fn new(language_id: impl Into<String>, confidence: f64) -> Result<Self, DetectionError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(DetectionError::InvalidConfidence(confidence));
        }
        Ok(Self {
            language_id: language_id.into(),
            confidence,
        })
    }

    /// Returns `true` if the confidence exceeds the given threshold.
    pub fn exceeds_threshold(&self, threshold: f64) -> bool {
        self.confidence >= threshold
    }

    /// Returns the language identifier.
    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    /// Returns the confidence score.
    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Returns `true` if confidence is 0.8 or above.
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }
}

impl fmt::Display for DetectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:.0}%)", self.language_id, self.confidence * 100.0)
    }
}

/// Detect language by file extension.
pub fn detect_by_extension(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?;
    let lang = match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" => "cpp",
        "h" => "c",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "sh" => "shellscript",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        _ => return None,
    };
    Some(lang.to_string())
}

/// Detect language from a shebang line.
pub fn detect_by_shebang(first_line: &str) -> Option<String> {
    let line = first_line.trim();
    if !line.starts_with("#!") {
        return None;
    }
    let interpreter = line
        .strip_prefix("#!")
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap_or("");
    // Handle "env <prog>" form.
    let prog = if interpreter.starts_with("env ") {
        interpreter.strip_prefix("env ").unwrap().trim()
    } else {
        interpreter.trim()
    };
    let lang = match prog {
        p if p.starts_with("python") => "python",
        "bash" | "sh" | "zsh" => "shellscript",
        "node" => "javascript",
        "ruby" => "ruby",
        "perl" => "perl",
        "php" => "php",
        _ => return None,
    };
    Some(lang.to_string())
}

/// Detect language by simple content heuristics.
pub fn detect_by_content(content: &str) -> Vec<DetectionResult> {
    let mut results = Vec::new();

    let checks: &[(&str, &str, f64)] = &[
        ("fn main", "rust", 0.7),
        ("fn ", "rust", 0.3),
        ("let mut ", "rust", 0.4),
        ("def ", "python", 0.3),
        ("import ", "python", 0.2),
        ("function ", "javascript", 0.3),
        ("const ", "javascript", 0.2),
        ("class ", "java", 0.2),
        ("package ", "java", 0.2),
        ("func ", "go", 0.3),
        ("<html", "html", 0.6),
        ("<!DOCTYPE", "html", 0.7),
    ];

    for &(pattern, lang, confidence) in checks {
        if content.contains(pattern) {
            results.push(DetectionResult {
                language_id: lang.to_string(),
                confidence,
            });
        }
    }

    // Deduplicate: keep highest confidence per language.
    let mut best: HashMap<String, f64> = HashMap::new();
    for r in &results {
        let entry = best.entry(r.language_id.clone()).or_insert(0.0);
        if r.confidence > *entry {
            *entry = r.confidence;
        }
    }

    best.into_iter()
        .map(|(language_id, confidence)| DetectionResult {
            language_id,
            confidence,
        })
        .collect()
}

/// High-level language detection service.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageDetectionService {
    pub extension_map: HashMap<String, String>,
    /// Minimum confidence threshold for content-based detection.
    pub min_confidence: f64,
}

impl LanguageDetectionService {
    pub fn new() -> Self {
        let pairs = [
            ("rs", "rust"),
            ("py", "python"),
            ("js", "javascript"),
            ("ts", "typescript"),
            ("go", "go"),
            ("java", "java"),
            ("c", "c"),
            ("cpp", "cpp"),
            ("h", "c"),
            ("json", "json"),
            ("toml", "toml"),
            ("yaml", "yaml"),
            ("yml", "yaml"),
            ("md", "markdown"),
            ("html", "html"),
            ("css", "css"),
            ("sh", "shellscript"),
            ("rb", "ruby"),
            ("php", "php"),
            ("swift", "swift"),
            ("kt", "kotlin"),
        ];
        let extension_map = pairs
            .iter()
            .map(|&(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Self {
            extension_map,
            min_confidence: 0.0,
        }
    }

    /// Detect the language of a file using extension, shebang, and content heuristics.
    pub fn detect(&self, filename: &str, content: &str) -> Option<String> {
        // 1. Try extension via the service map.
        if let Some(ext) = filename.rsplit('.').next() {
            if let Some(lang) = self.extension_map.get(ext) {
                return Some(lang.clone());
            }
        }

        // 2. Try shebang.
        if let Some(first_line) = content.lines().next() {
            if let Some(lang) = detect_by_shebang(first_line) {
                return Some(lang);
            }
        }

        // 3. Fall back to content heuristics (pick highest confidence).
        let results = detect_by_content(content);
        results
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .map(|r| r.language_id)
    }
}

impl Default for LanguageDetectionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing a customised [`LanguageDetectionService`].
#[derive(Debug, Clone)]
pub struct ServiceBuilder {
    extra_extensions: Vec<(String, String)>,
    min_confidence: f64,
}

impl ServiceBuilder {
    /// Start building a new service with defaults.
    pub fn new() -> Self {
        Self {
            extra_extensions: Vec::new(),
            min_confidence: 0.0,
        }
    }

    /// Register an additional file-extension → language mapping.
    pub fn extension(mut self, ext: impl Into<String>, lang: impl Into<String>) -> Self {
        self.extra_extensions.push((ext.into(), lang.into()));
        self
    }

    /// Set the minimum confidence threshold for content-based detection.
    /// Values outside `[0.0, 1.0]` are clamped.
    pub fn min_confidence(mut self, threshold: f64) -> Self {
        self.min_confidence = threshold.clamp(0.0, 1.0);
        self
    }

    /// Consume the builder and produce a [`LanguageDetectionService`].
    pub fn build(self) -> LanguageDetectionService {
        let mut svc = LanguageDetectionService::new();
        svc.min_confidence = self.min_confidence;
        for (ext, lang) in self.extra_extensions {
            svc.extension_map.insert(ext, lang);
        }
        svc
    }
}

impl Default for ServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageDetectionService {
    /// Return a [`ServiceBuilder`] for step-by-step construction.
    pub fn builder() -> ServiceBuilder {
        ServiceBuilder::new()
    }

    /// Register a custom extension mapping at runtime.
    pub fn register_extension(&mut self, ext: impl Into<String>, lang: impl Into<String>) {
        self.extension_map.insert(ext.into(), lang.into());
    }

    /// Remove an extension mapping. Returns the previously mapped language, if any.
    pub fn unregister_extension(&mut self, ext: &str) -> Option<String> {
        self.extension_map.remove(ext)
    }

    /// Return how many extension mappings are registered.
    pub fn extension_count(&self) -> usize {
        self.extension_map.len()
    }

    /// Check whether a given extension is registered.
    pub fn supports_extension(&self, ext: &str) -> bool {
        self.extension_map.contains_key(ext)
    }

    /// Like [`detect`](Self::detect) but returns a [`DetectionResult`] with confidence,
    /// or a [`DetectionError::NoMatch`] when nothing matches.
    pub fn detect_with_confidence(
        &self,
        filename: &str,
        content: &str,
    ) -> Result<DetectionResult, DetectionError> {
        if filename.is_empty() {
            return Err(DetectionError::InvalidFilename(filename.to_string()));
        }

        // Extension match → full confidence.
        if let Some(ext) = filename.rsplit('.').next() {
            if let Some(lang) = self.extension_map.get(ext) {
                return Ok(DetectionResult {
                    language_id: lang.clone(),
                    confidence: 1.0,
                });
            }
        }

        // Shebang → high confidence.
        if let Some(first_line) = content.lines().next() {
            if let Some(lang) = detect_by_shebang(first_line) {
                return Ok(DetectionResult {
                    language_id: lang,
                    confidence: 0.9,
                });
            }
        }

        // Content heuristics – apply min_confidence filter.
        let results = detect_by_content(content);
        results
            .into_iter()
            .filter(|r| r.confidence >= self.min_confidence)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .ok_or(DetectionError::NoMatch)
    }

    /// Detect all plausible languages for the given content, sorted by descending confidence.
    pub fn detect_all(&self, content: &str) -> Vec<DetectionResult> {
        let mut results = detect_by_content(content);
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }

    /// Return a sorted list of all registered file extensions.
    pub fn supported_extensions(&self) -> Vec<&str> {
        let mut exts: Vec<&str> = self.extension_map.keys().map(|s| s.as_str()).collect();
        exts.sort_unstable();
        exts
    }

    /// Detect language using only the filename extension, without examining content.
    pub fn detect_by_filename_only(&self, filename: &str) -> Option<String> {
        let ext = filename.rsplit('.').next()?;
        self.extension_map.get(ext).cloned()
    }

    /// Return the number of unique languages in the extension map.
    pub fn language_count(&self) -> usize {
        let langs: std::collections::HashSet<&str> =
            self.extension_map.values().map(|v| v.as_str()).collect();
        langs.len()
    }
}

impl fmt::Display for LanguageDetectionService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LanguageDetectionService({} extensions, min_confidence={:.2})",
            self.extension_map.len(),
            self.min_confidence
        )
    }
}

/// Returns `true` if the given extension is a common binary file extension.
pub fn is_binary_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "exe" | "dll" | "so" | "bin" | "o" | "obj" | "png" | "jpg"
    )
}

/// Normalize a language identifier by trimming whitespace and lowercasing.
pub fn normalize_language_id(id: &str) -> String {
    id.trim().to_lowercase()
}

/// Compute a normalised similarity score between two sets of detection results.
/// Returns a value in `[0.0, 1.0]` representing the Jaccard index of detected language ids.
pub fn detection_similarity(a: &[DetectionResult], b: &[DetectionResult]) -> f64 {
    let set_a: std::collections::HashSet<&str> =
        a.iter().map(|r| r.language_id.as_str()).collect();
    let set_b: std::collections::HashSet<&str> =
        b.iter().map(|r| r.language_id.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Accumulated statistics for wb-langdetect operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbLangdetectStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbLangdetectStats {
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
    pub fn merge(&mut self, other: &WbLangdetectStats) {
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

impl Default for WbLangdetectStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbLangdetectStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbLangdetectStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-langdetect.
#[derive(Debug, Clone)]
pub struct WbLangdetectValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbLangdetectValidator {
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

impl Default for WbLangdetectValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Detects language from the first line via shebang or editor modelines.
pub struct FirstLineDetector;

impl FirstLineDetector {
    /// Detect language from first line. Checks shebang first, then vim/emacs modelines.
    pub fn detect(first_line: &str) -> Option<DetectionResult> {
        // Try shebang
        if let Some(lang) = detect_by_shebang(first_line) {
            return Some(DetectionResult {
                language_id: lang,
                confidence: 0.95,
            });
        }
        // Try vim modeline: "# vim: set ft=python :" or "// vim: filetype=rust"
        Self::detect_vim_modeline(first_line).or_else(|| Self::detect_emacs_modeline(first_line))
    }

    fn detect_vim_modeline(line: &str) -> Option<DetectionResult> {
        // Look for "vim:" followed by "ft=" or "filetype="
        if !line.contains("vim:") {
            return None;
        }
        for segment in line.split_whitespace() {
            if let Some(ft) = segment
                .strip_prefix("ft=")
                .or_else(|| segment.strip_prefix("filetype="))
            {
                let ft = ft.trim_end_matches(':');
                return Some(DetectionResult {
                    language_id: ft.to_string(),
                    confidence: 0.9,
                });
            }
        }
        None
    }

    fn detect_emacs_modeline(line: &str) -> Option<DetectionResult> {
        // Look for "-*- mode: python -*-" or "-*- python -*-"
        let start = line.find("-*-")?;
        let rest = &line[start + 3..];
        let end = rest.find("-*-")?;
        let content = rest[..end].trim();
        if let Some(mode_val) = content.strip_prefix("mode:") {
            return Some(DetectionResult {
                language_id: mode_val.trim().to_string(),
                confidence: 0.9,
            });
        }
        // Simple form: "-*- python -*-"
        if !content.contains(':') && !content.is_empty() {
            return Some(DetectionResult {
                language_id: content.to_string(),
                confidence: 0.85,
            });
        }
        None
    }
}

/// Heuristic content detector that examines structure, not just keywords.
pub struct ContentSniffDetector;

impl ContentSniffDetector {
    /// Detect language by sniffing content structure.
    pub fn detect(content: &str) -> Option<DetectionResult> {
        let trimmed = content.trim();
        // JSON detection
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            return Some(DetectionResult {
                language_id: "json".into(),
                confidence: 0.7,
            });
        }
        // XML/HTML detection
        if trimmed.starts_with("<?xml") {
            return Some(DetectionResult {
                language_id: "xml".into(),
                confidence: 0.9,
            });
        }
        if trimmed.starts_with("<!DOCTYPE html") || trimmed.starts_with("<html") {
            return Some(DetectionResult {
                language_id: "html".into(),
                confidence: 0.85,
            });
        }
        // YAML detection (key: value on first lines)
        if Self::looks_like_yaml(trimmed) {
            return Some(DetectionResult {
                language_id: "yaml".into(),
                confidence: 0.5,
            });
        }
        // Shell script detection
        if trimmed.starts_with("#!/") {
            return detect_by_shebang(trimmed.lines().next().unwrap_or("")).map(|lang| {
                DetectionResult {
                    language_id: lang,
                    confidence: 0.95,
                }
            });
        }
        None
    }

    fn looks_like_yaml(content: &str) -> bool {
        let mut kv_lines = 0;
        for line in content.lines().take(10) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.contains(": ") && !line.starts_with('{') {
                kv_lines += 1;
            }
        }
        kv_lines >= 2
    }
}

/// Compute an overall detection confidence by combining multiple detection results.
/// Uses the maximum confidence from extension, first-line, and content detectors.
pub fn detection_confidence(filename: &str, content: &str) -> f64 {
    let mut max_conf = 0.0_f64;
    if detect_by_extension(filename).is_some() {
        max_conf = max_conf.max(1.0);
    }
    if let Some(first_line) = content.lines().next() {
        if let Some(result) = FirstLineDetector::detect(first_line) {
            max_conf = max_conf.max(result.confidence);
        }
    }
    if let Some(result) = ContentSniffDetector::detect(content) {
        max_conf = max_conf.max(result.confidence);
    }
    max_conf
}

// ---------------------------------------------------------------------------
// LanguageConfidence – richer confidence scoring
// ---------------------------------------------------------------------------

/// Confidence level buckets for language detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfidenceLevel {
    None,
    Low,
    Medium,
    High,
    Certain,
}

impl ConfidenceLevel {
    /// Convert a raw 0.0–1.0 score into a bucket.
    pub fn from_score(score: f64) -> Self {
        if score <= 0.0 {
            Self::None
        } else if score < 0.3 {
            Self::Low
        } else if score < 0.6 {
            Self::Medium
        } else if score < 0.9 {
            Self::High
        } else {
            Self::Certain
        }
    }

    /// Minimum numeric threshold for this level.
    pub fn min_score(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.01,
            Self::Medium => 0.3,
            Self::High => 0.6,
            Self::Certain => 0.9,
        }
    }
}

impl std::fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Certain => "certain",
        };
        f.write_str(s)
    }
}

/// Aggregated confidence from multiple detection strategies.
#[derive(Debug, Clone)]
pub struct LanguageConfidence {
    pub language_id: String,
    pub scores: Vec<(&'static str, f64)>,
}

impl LanguageConfidence {
    pub fn new(language_id: impl Into<String>) -> Self {
        Self {
            language_id: language_id.into(),
            scores: Vec::new(),
        }
    }

    /// Record a score from a named strategy.
    pub fn add_score(&mut self, strategy: &'static str, score: f64) {
        self.scores.push((strategy, score.clamp(0.0, 1.0)));
    }

    /// Weighted average across all strategies.
    pub fn combined_score(&self) -> f64 {
        if self.scores.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.scores.iter().map(|(_, s)| s).sum();
        sum / self.scores.len() as f64
    }

    /// Best single strategy score.
    pub fn best_score(&self) -> f64 {
        self.scores
            .iter()
            .map(|(_, s)| *s)
            .fold(0.0_f64, f64::max)
    }

    pub fn level(&self) -> ConfidenceLevel {
        ConfidenceLevel::from_score(self.combined_score())
    }

    pub fn strategy_count(&self) -> usize {
        self.scores.len()
    }
}

// ---------------------------------------------------------------------------
// DetectionStrategy – pluggable multi-strategy detection
// ---------------------------------------------------------------------------

/// Which detection strategy produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionStrategy {
    Extension,
    Shebang,
    MagicBytes,
    ContentAnalysis,
}

impl std::fmt::Display for DetectionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Extension => "extension",
            Self::Shebang => "shebang",
            Self::MagicBytes => "magic_bytes",
            Self::ContentAnalysis => "content_analysis",
        };
        f.write_str(s)
    }
}

/// Result from a single detection strategy.
#[derive(Debug, Clone)]
pub struct StrategyResult {
    pub strategy: DetectionStrategy,
    pub language_id: String,
    pub confidence: f64,
}

impl StrategyResult {
    pub fn new(strategy: DetectionStrategy, lang: impl Into<String>, confidence: f64) -> Self {
        Self {
            strategy,
            language_id: lang.into(),
            confidence: confidence.clamp(0.0, 1.0),
        }
    }
}

/// Detect language by well-known magic byte signatures.
pub fn detect_by_magic_bytes(bytes: &[u8]) -> Option<StrategyResult> {
    if bytes.len() < 4 {
        return None;
    }
    let sig: &[u8] = &bytes[..std::cmp::min(bytes.len(), 8)];
    // PDF
    if sig.starts_with(b"%PDF") {
        return Some(StrategyResult::new(DetectionStrategy::MagicBytes, "pdf", 1.0));
    }
    // PNG
    if sig.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some(StrategyResult::new(DetectionStrategy::MagicBytes, "png", 1.0));
    }
    // ELF binary
    if sig.starts_with(&[0x7F, 0x45, 0x4C, 0x46]) {
        return Some(StrategyResult::new(DetectionStrategy::MagicBytes, "binary", 1.0));
    }
    // ZIP / DOCX / XLSX / JAR
    if sig.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return Some(StrategyResult::new(DetectionStrategy::MagicBytes, "zip", 0.8));
    }
    // GIF
    if sig.starts_with(b"GIF8") {
        return Some(StrategyResult::new(DetectionStrategy::MagicBytes, "gif", 1.0));
    }
    None
}

/// Run all strategies and merge into a single `LanguageConfidence`.
pub fn detect_multi_strategy(filename: &str, content: &str) -> LanguageConfidence {
    let mut results: std::collections::HashMap<String, LanguageConfidence> =
        std::collections::HashMap::new();

    // Strategy 1 – extension
    if let Some(lang) = detect_by_extension(filename) {
        let entry = results
            .entry(lang.clone())
            .or_insert_with(|| LanguageConfidence::new(&lang));
        entry.add_score("extension", 0.85);
    }

    // Strategy 2 – shebang
    if let Some(first_line) = content.lines().next() {
        if let Some(lang) = detect_by_shebang(first_line) {
            let entry = results
                .entry(lang.clone())
                .or_insert_with(|| LanguageConfidence::new(&lang));
            entry.add_score("shebang", 0.95);
        }
    }

    // Strategy 3 – magic bytes
    if let Some(sr) = detect_by_magic_bytes(content.as_bytes()) {
        let entry = results
            .entry(sr.language_id.clone())
            .or_insert_with(|| LanguageConfidence::new(&sr.language_id));
        entry.add_score("magic_bytes", sr.confidence);
    }

    // Strategy 4 – content analysis
    let content_results = detect_by_content(content);
    for dr in &content_results {
        let entry = results
            .entry(dr.language_id().to_string())
            .or_insert_with(|| LanguageConfidence::new(dr.language_id()));
        entry.add_score("content_analysis", dr.confidence());
    }

    // Return the language with the best combined score
    results
        .into_values()
        .max_by(|a, b| a.combined_score().partial_cmp(&b.combined_score()).unwrap())
        .unwrap_or_else(|| LanguageConfidence::new("plaintext"))
}

// ---------------------------------------------------------------------------
// Language family grouping
// ---------------------------------------------------------------------------

/// Broad language families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageFamily {
    CFamily,
    ScriptingDynamic,
    Markup,
    Functional,
    Systems,
    Data,
    Shell,
    Other,
}

impl std::fmt::Display for LanguageFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::CFamily => "C-family",
            Self::ScriptingDynamic => "scripting/dynamic",
            Self::Markup => "markup",
            Self::Functional => "functional",
            Self::Systems => "systems",
            Self::Data => "data",
            Self::Shell => "shell",
            Self::Other => "other",
        };
        f.write_str(s)
    }
}

/// Map a language id to its family.
pub fn language_family(lang: &str) -> LanguageFamily {
    match lang.to_lowercase().as_str() {
        "c" | "cpp" | "csharp" | "java" | "objective-c" | "d" => LanguageFamily::CFamily,
        "python" | "ruby" | "javascript" | "typescript" | "lua" | "perl" | "php" => {
            LanguageFamily::ScriptingDynamic
        }
        "html" | "xml" | "markdown" | "latex" | "css" | "scss" => LanguageFamily::Markup,
        "haskell" | "ocaml" | "erlang" | "elixir" | "clojure" | "lisp" | "scheme" => {
            LanguageFamily::Functional
        }
        "rust" | "go" | "zig" | "nim" => LanguageFamily::Systems,
        "json" | "yaml" | "toml" | "csv" | "sql" => LanguageFamily::Data,
        "bash" | "zsh" | "fish" | "powershell" | "shellscript" => LanguageFamily::Shell,
        _ => LanguageFamily::Other,
    }
}

/// Return all language ids that belong to the given family from a list.
pub fn filter_by_family<'a>(languages: &[&'a str], family: LanguageFamily) -> Vec<&'a str> {
    languages
        .iter()
        .copied()
        .filter(|l| language_family(l) == family)
        .collect()
}

/// Group a list of language ids by family.
pub fn group_by_family<'a>(
    languages: &[&'a str],
) -> std::collections::HashMap<LanguageFamily, Vec<&'a str>> {
    let mut map: std::collections::HashMap<LanguageFamily, Vec<&'a str>> =
        std::collections::HashMap::new();
    for &lang in languages {
        map.entry(language_family(lang)).or_default().push(lang);
    }
    map
}

// ---------------------------------------------------------------------------
// Language alias resolution
// ---------------------------------------------------------------------------

/// Map common language aliases and alternative names to canonical identifiers.
pub fn resolve_alias(alias: &str) -> &str {
    match alias.to_lowercase().as_str() {
        "c++" | "cplusplus" => "cpp",
        "c#" | "csharp" => "csharp",
        "js" => "javascript",
        "ts" => "typescript",
        "py" | "python3" => "python",
        "rb" => "ruby",
        "sh" | "bash" | "zsh" => "shellscript",
        "yml" => "yaml",
        "rs" => "rust",
        "md" => "markdown",
        "tex" => "latex",
        "htm" => "html",
        "golang" => "go",
        other => {
            // Can't return a borrow of a temporary — return the input as-is
            // when no alias matches (the input already has 'static-compatible lifetime
            // through the match).
            // We leak nothing because all arms return string literals or the input.
            let _ = other;
            alias
        }
    }
}

/// Return `true` if two language identifiers are equivalent after alias resolution.
pub fn languages_equivalent(a: &str, b: &str) -> bool {
    resolve_alias(a).eq_ignore_ascii_case(resolve_alias(b))
}

// ---------------------------------------------------------------------------
// Language feature hints
// ---------------------------------------------------------------------------

/// Rough feature set hints for a detected language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageFeatures {
    pub language_id: String,
    pub has_types: bool,
    pub has_classes: bool,
    pub has_pattern_matching: bool,
    pub is_compiled: bool,
}

impl LanguageFeatures {
    /// Infer feature hints from a language id.
    pub fn from_language(lang: &str) -> Self {
        let lower = lang.to_lowercase();
        let has_types = matches!(
            lower.as_str(),
            "rust" | "typescript" | "java" | "go" | "cpp" | "c" | "csharp" | "haskell"
        );
        let has_classes = matches!(
            lower.as_str(),
            "java" | "python" | "ruby" | "typescript" | "javascript" | "cpp" | "csharp" | "php"
        );
        let has_pattern_matching = matches!(
            lower.as_str(),
            "rust" | "haskell" | "ocaml" | "erlang" | "elixir" | "scala"
        );
        let is_compiled = matches!(
            lower.as_str(),
            "rust" | "go" | "c" | "cpp" | "java" | "csharp" | "haskell"
        );
        Self {
            language_id: lang.to_string(),
            has_types,
            has_classes,
            has_pattern_matching,
            is_compiled,
        }
    }
}

impl fmt::Display for LanguageFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut feats = Vec::new();
        if self.has_types {
            feats.push("typed");
        }
        if self.has_classes {
            feats.push("OOP");
        }
        if self.has_pattern_matching {
            feats.push("pattern-matching");
        }
        if self.is_compiled {
            feats.push("compiled");
        }
        write!(f, "{}: [{}]", self.language_id, feats.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Detection priority ordering
// ---------------------------------------------------------------------------

/// Compare two detection results, preferring higher confidence, then shorter
/// language id (as a tiebreaker for determinism).
pub fn compare_detections(a: &DetectionResult, b: &DetectionResult) -> std::cmp::Ordering {
    b.confidence
        .partial_cmp(&a.confidence)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.language_id.len().cmp(&b.language_id.len()))
        .then_with(|| a.language_id.cmp(&b.language_id))
}

/// Sort a list of detection results by confidence descending.
pub fn sort_detections(results: &mut [DetectionResult]) {
    results.sort_by(compare_detections);
}

/// Merge multiple detection results for the same language by keeping the highest confidence.
pub fn merge_detections(results: &[DetectionResult]) -> Vec<DetectionResult> {
    let mut best: HashMap<String, f64> = HashMap::new();
    for r in results {
        let entry = best.entry(r.language_id.clone()).or_insert(0.0);
        if r.confidence > *entry {
            *entry = r.confidence;
        }
    }
    let mut merged: Vec<DetectionResult> = best
        .into_iter()
        .map(|(lang, conf)| DetectionResult {
            language_id: lang,
            confidence: conf,
        })
        .collect();
    sort_detections(&mut merged);
    merged
}

// ---------------------------------------------------------------------------
// DetectionCache – cache detection results with a time-to-live
// ---------------------------------------------------------------------------

/// A simple TTL-based cache for detection results.
///
/// Each entry is stamped with the time it was inserted (in milliseconds) and
/// is considered expired once `current_ms - inserted_ms >= ttl_ms`.
pub struct DetectionCache {
    /// Time-to-live in milliseconds.
    ttl_ms: u64,
    /// Map from cache key to (result, insertion timestamp).
    entries: HashMap<String, (DetectionResult, u64)>,
}

impl DetectionCache {
    /// Create a new cache with the given TTL (in milliseconds).
    pub fn new(ttl_ms: u64) -> Self {
        Self {
            ttl_ms,
            entries: HashMap::new(),
        }
    }

    /// Insert a detection result into the cache.
    pub fn insert(&mut self, key: &str, result: DetectionResult, timestamp_ms: u64) {
        self.entries
            .insert(key.to_string(), (result, timestamp_ms));
    }

    /// Retrieve a cached result if it has not expired.
    pub fn get(&self, key: &str, current_ms: u64) -> Option<&DetectionResult> {
        self.entries.get(key).and_then(|(res, ts)| {
            if current_ms.saturating_sub(*ts) < self.ttl_ms {
                Some(res)
            } else {
                None
            }
        })
    }

    /// Remove a single entry from the cache.
    pub fn invalidate(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Remove all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the number of entries currently stored (including expired).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ContentPatternDetector – pattern-based language detection
// ---------------------------------------------------------------------------

/// A single pattern rule used by [`ContentPatternDetector`].
struct PatternRule {
    /// Substring to search for in the content.
    pattern: String,
    /// Language identifier to return on a match.
    language: String,
    /// Base score assigned when the pattern matches.
    score: f64,
}

/// Detects languages by scanning content for characteristic substrings.
///
/// Ships with a set of built-in patterns and allows users to register
/// additional ones via [`add_pattern`](ContentPatternDetector::add_pattern).
pub struct ContentPatternDetector {
    rules: Vec<PatternRule>,
}

impl ContentPatternDetector {
    /// Create a detector pre-loaded with common patterns.
    pub fn new() -> Self {
        let rules = vec![
            PatternRule { pattern: "fn main(".into(), language: "rust".into(), score: 0.85 },
            PatternRule { pattern: "impl ".into(), language: "rust".into(), score: 0.60 },
            PatternRule { pattern: "pub fn ".into(), language: "rust".into(), score: 0.55 },
            PatternRule { pattern: "def ".into(), language: "python".into(), score: 0.50 },
            PatternRule { pattern: "import ".into(), language: "python".into(), score: 0.30 },
            PatternRule { pattern: "function ".into(), language: "javascript".into(), score: 0.50 },
            PatternRule { pattern: "const ".into(), language: "javascript".into(), score: 0.25 },
            PatternRule { pattern: "func ".into(), language: "go".into(), score: 0.55 },
            PatternRule { pattern: "package ".into(), language: "go".into(), score: 0.35 },
            PatternRule { pattern: "public class ".into(), language: "java".into(), score: 0.65 },
            PatternRule { pattern: "System.out.".into(), language: "java".into(), score: 0.50 },
            PatternRule { pattern: "#include ".into(), language: "c".into(), score: 0.55 },
            PatternRule { pattern: "int main(".into(), language: "c".into(), score: 0.70 },
            PatternRule { pattern: "class ".into(), language: "python".into(), score: 0.30 },
            PatternRule { pattern: "require ".into(), language: "ruby".into(), score: 0.35 },
            PatternRule { pattern: "puts ".into(), language: "ruby".into(), score: 0.30 },
        ];
        Self { rules }
    }

    /// Register an additional pattern rule.
    pub fn add_pattern(&mut self, pattern: &str, language: &str, score: f64) {
        self.rules.push(PatternRule {
            pattern: pattern.to_string(),
            language: language.to_string(),
            score: score.clamp(0.0, 1.0),
        });
    }

    /// Scan `content` and return scored detection results.
    ///
    /// Each matching pattern contributes its score; when multiple patterns
    /// map to the same language the highest score wins.
    pub fn detect(&self, content: &str) -> Vec<DetectionResult> {
        let mut scores: HashMap<String, f64> = HashMap::new();
        for rule in &self.rules {
            if content.contains(&rule.pattern) {
                let entry = scores.entry(rule.language.clone()).or_insert(0.0);
                if rule.score > *entry {
                    *entry = rule.score;
                }
            }
        }
        let mut results: Vec<DetectionResult> = scores
            .into_iter()
            .map(|(lang, conf)| DetectionResult {
                language_id: lang,
                confidence: conf,
            })
            .collect();
        sort_detections(&mut results);
        results
    }
}

impl Default for ContentPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ShebangParser – structured shebang parsing
// ---------------------------------------------------------------------------

/// Structured information extracted from a shebang line.
#[derive(Debug, Clone, PartialEq)]
pub struct ShebangInfo {
    /// The interpreter name (e.g. `python3`, `bash`).
    pub interpreter: String,
    /// Any arguments that follow the interpreter on the shebang line.
    pub args: Vec<String>,
}

/// Parses shebang (`#!`) lines into structured information.
pub struct ShebangParser;

impl ShebangParser {
    /// Parse a shebang line and return structured info.
    ///
    /// Returns `None` if the line does not start with `#!`.
    pub fn parse(line: &str) -> Option<ShebangInfo> {
        let trimmed = line.trim();
        if !trimmed.starts_with("#!") {
            return None;
        }
        let after_hash = trimmed[2..].trim();
        let parts: Vec<&str> = after_hash.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        // Handle `/usr/bin/env <interpreter> [args...]`
        if parts[0].ends_with("/env") && parts.len() > 1 {
            let interpreter = parts[1]
                .rsplit('/')
                .next()
                .unwrap_or(parts[1])
                .to_string();
            let args = parts[2..].iter().map(|s| s.to_string()).collect();
            return Some(ShebangInfo { interpreter, args });
        }

        // Direct path: `/usr/bin/python3 [args...]`
        let interpreter = parts[0]
            .rsplit('/')
            .next()
            .unwrap_or(parts[0])
            .to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();
        Some(ShebangInfo { interpreter, args })
    }

    /// Map an interpreter name to a language identifier.
    pub fn language_from_interpreter(interpreter: &str) -> Option<String> {
        // Strip version suffixes like "python3.11" → "python"
        let base = interpreter
            .trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        match base {
            "python" => Some("python".into()),
            "ruby" => Some("ruby".into()),
            "node" => Some("javascript".into()),
            "bash" | "sh" | "zsh" | "fish" | "dash" => Some("shellscript".into()),
            "perl" => Some("perl".into()),
            "lua" => Some("lua".into()),
            "Rscript" => Some("r".into()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// DetectionScorer – combine multiple signals into a final score
// ---------------------------------------------------------------------------

/// A single signal fed into the scorer.
#[allow(dead_code)]
struct ScorerSignal {
    source: String,
    language: String,
    confidence: f64,
}

/// Combines detection signals from different sources to produce a ranked
/// list of candidate languages.
///
/// Signals are weighted equally; for each language the maximum confidence
/// across all sources is used.
pub struct DetectionScorer {
    signals: Vec<ScorerSignal>,
}

impl DetectionScorer {
    /// Create an empty scorer.
    pub fn new() -> Self {
        Self {
            signals: Vec::new(),
        }
    }

    /// Record a detection signal from the named `source`.
    pub fn add_signal(&mut self, source: &str, language: &str, confidence: f64) {
        self.signals.push(ScorerSignal {
            source: source.to_string(),
            language: language.to_string(),
            confidence: confidence.clamp(0.0, 1.0),
        });
    }

    /// Return the single best candidate, or `None` if no signals have been
    /// recorded.
    pub fn best_match(&self) -> Option<DetectionResult> {
        self.all_candidates().into_iter().next()
    }

    /// Return all candidate languages sorted by descending confidence.
    pub fn all_candidates(&self) -> Vec<DetectionResult> {
        let mut best: HashMap<String, f64> = HashMap::new();
        for sig in &self.signals {
            let entry = best.entry(sig.language.clone()).or_insert(0.0);
            if sig.confidence > *entry {
                *entry = sig.confidence;
            }
        }
        let mut results: Vec<DetectionResult> = best
            .into_iter()
            .map(|(lang, conf)| DetectionResult {
                language_id: lang,
                confidence: conf,
            })
            .collect();
        sort_detections(&mut results);
        results
    }

    /// Return the number of signals that have been added.
    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }
}

impl Default for DetectionScorer {
    fn default() -> Self {
        Self::new()
    }
}


// === Language Detection Ensemble ===

/// Language Detection Ensemble implementation.
#[derive(Debug, Clone)]
pub struct LanguageDetectionEnsemble {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: LanguageDetectionEnsembleStats,
}

/// Statistics for LanguageDetectionEnsemble.
#[derive(Debug, Clone, Default)]
pub struct LanguageDetectionEnsembleStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl LanguageDetectionEnsembleStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl LanguageDetectionEnsemble {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: LanguageDetectionEnsembleStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &LanguageDetectionEnsembleStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for LanguageDetectionEnsemble {
    fn default() -> Self {
        Self::new()
    }
}

// === Detection Accuracy Scorer ===

/// Priority level for DetectionAccuracyScorer items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetectionAccuracyScorerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl DetectionAccuracyScorerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for DetectionAccuracyScorerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Detection Accuracy Scorer implementation.
#[derive(Debug, Clone)]
pub struct DetectionAccuracyScorer {
    items: Vec<DetectionAccuracyScorerItem>,
    max_items: usize,
    default_priority: DetectionAccuracyScorerPriority,
}

/// A single item in DetectionAccuracyScorer.
#[derive(Debug, Clone)]
pub struct DetectionAccuracyScorerItem {
    pub id: String,
    pub label: String,
    pub priority: DetectionAccuracyScorerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl DetectionAccuracyScorerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: DetectionAccuracyScorerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: DetectionAccuracyScorerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl DetectionAccuracyScorer {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: DetectionAccuracyScorerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: DetectionAccuracyScorerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<DetectionAccuracyScorerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&DetectionAccuracyScorerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: DetectionAccuracyScorerPriority) -> Vec<&DetectionAccuracyScorerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DetectionAccuracyScorerItem> {
        let mut sorted: Vec<&DetectionAccuracyScorerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&DetectionAccuracyScorerItem> {
        let mut sorted: Vec<&DetectionAccuracyScorerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&DetectionAccuracyScorerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: DetectionAccuracyScorerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> DetectionAccuracyScorerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &DetectionAccuracyScorerItem> {
        self.items.iter()
    }
}

impl Default for DetectionAccuracyScorer {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-wb-langdetect: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbLangdetectXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl WbLangdetectXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for WbLangdetectXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct WbLangdetectXRegistry {
    entries: Vec<WbLangdetectXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl WbLangdetectXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: WbLangdetectXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&WbLangdetectXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut WbLangdetectXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<WbLangdetectXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&WbLangdetectXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&WbLangdetectXConfig> {
        let mut sorted: Vec<&WbLangdetectXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&WbLangdetectXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> WbLangdetectXIterator<'_> {
        WbLangdetectXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct WbLangdetectXIterator<'a> {
    inner: std::slice::Iter<'a, WbLangdetectXConfig>,
}

impl<'a> Iterator for WbLangdetectXIterator<'a> {
    type Item = &'a WbLangdetectXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct WbLangdetectXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl WbLangdetectXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct WbLangdetectXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl WbLangdetectXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &WbLangdetectXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &WbLangdetectXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &WbLangdetectXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for WbLangdetectXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct WbLangdetectXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl WbLangdetectXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &WbLangdetectXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &WbLangdetectXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for WbLangdetectXValidator {
    fn default() -> Self {
        Self::new()
    }
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
// xb_ utilities – batch 94
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer94 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer94 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_94(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_94<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_94<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_94(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_94(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 211
// ---------------------------------------------------------------------------

/// Generic object pool `Xc211Pool<T>`.
pub struct Xc211Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc211Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc211PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc211Pool<T> {
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
    pub fn stats(&self) -> Xc211PoolStats {
        Xc211PoolStats {
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

impl<T> Default for Xc211Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc211Scheduler`.
pub struct Xc211Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc211Scheduler {
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

impl Default for Xc211Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_211 hash for the given byte slice.
pub fn xc_211_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_211 convention.
pub fn xc_211_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe107 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe107Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe107PipelineError {
    pub stage: Xe107Stage,
    pub message: String,
}

impl std::fmt::Display for Xe107PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe107Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe107Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError>>>,
    stage_names: Vec<Xe107Stage>,
}

impl Xe107Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe107Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe107Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe107Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe107Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe107Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe107CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe107CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe107Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe107CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe107CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe107Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe107CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_107_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe107CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_107_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe107CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_107_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
    Ok(data)
}

pub fn xe_107_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_107_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_107_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_107_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe107PipelineError> {
    Err(Xe107PipelineError {
        stage: Xe107Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_105: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg105Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg105Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg105Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_105: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg105Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg105Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg105Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg105Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 210).
pub struct Xh210SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh210SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 252 as u64,
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

/// A compact bit set supporting boolean operations (variant 210).
pub struct Xh210BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh210BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 210).
pub struct Xi210Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi210Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi210Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi210Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 210).
pub struct Xi210IntervalTree {
    xi_intervals: Vec<Xi210Interval>,
}

impl Xi210IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi210Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi210Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi210Interval) -> Vec<&Xi210Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi210Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi210Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi210Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi210Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi210Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi210Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 210) ---

/// Disjoint set / union-find for crate 210.
pub struct Xj210UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj210UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ210_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 210.
pub struct Xj210BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj210BTreeNode<K, V>>>,
    len: usize,
}

struct Xj210BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj210BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj210BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ210_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ210_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj210BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj210BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj210BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj210BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_210 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk210SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk210SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk210DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk210DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_210).
#[derive(Debug, Clone)]
pub struct Xl210Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl210Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_210).
#[derive(Debug, Clone)]
pub struct Xl210SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl210SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm210MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm210MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm210Tokenizer {
    text: String,
}

impl Xm210Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 210.
pub struct Xn210Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn210Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 210 -----

#[derive(Debug, Clone)]
struct Xn210AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn210AvlNode<K, V>>>,
    right: Option<Box<Xn210AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 210.
#[derive(Debug, Clone)]
pub struct Xn210AVL<K, V> {
    root: Option<Box<Xn210AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn210AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn210AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn210AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn210AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn210AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn210AvlNode<K, V>>) -> Box<Xn210AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn210AvlNode<K, V>>) -> Box<Xn210AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn210AvlNode<K, V>>) -> Box<Xn210AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn210AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn210AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn210AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn210AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn210AvlNode<K, V>>) -> &Xn210AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn210AvlNode<K, V>>) -> (Box<Xn210AvlNode<K, V>>, Option<Box<Xn210AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn210AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn210AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn210AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn210AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn210AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn210AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn210AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo210RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo210Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo210RBNode<K, V> {
    key: K,
    value: V,
    color: Xo210Color,
    left: Option<Box<Xo210RBNode<K, V>>>,
    right: Option<Box<Xo210RBNode<K, V>>>,
}

/// A red-black tree map for crate 210.
#[derive(Debug, Clone)]
pub struct Xo210RedBlack<K, V> {
    root: Option<Box<Xo210RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo210RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo210Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo210RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo210RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo210RBNode {
                    key, value, color: Xo210Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo210RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo210Color::Red)
    }

    fn xo_balance(mut h: Box<Xo210RBNode<K, V>>) -> Box<Xo210RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo210Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo210RBNode<K, V>>) -> Box<Xo210RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo210Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo210RBNode<K, V>>) -> Box<Xo210RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo210Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo210RBNode<K, V>>) {
        h.color = Xo210Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo210Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo210Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo210Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo210RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo210RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo210RBNode<K, V>) -> (K, V, Option<Box<Xo210RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo210RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo210Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo210RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo210ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 210.
#[derive(Debug, Clone)]
pub struct Xo210ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo210ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo210#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo210#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 210).
#[derive(Debug)]
pub struct Xp210SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp210Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp210Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp210Node<K, V>>>,
    xp_right: Option<Box<Xp210Node<K, V>>>,
}

impl<K: Ord, V> Xp210Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp210SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp210SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp210Node<K, V>>>, key: &K) -> Option<Box<Xp210Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp210Node<K, V>>) -> Box<Xp210Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp210Node<K, V>>) -> Box<Xp210Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp210Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp210Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp210Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq210Treap ---------------

use std::cmp::Ordering as Xq210Ord;

struct Xq210TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq210TreapNode<K, V>>>,
    right: Option<Box<Xq210TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq210Treap<K, V> {
    root: Option<Box<Xq210TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq210TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_210_size<K, V>(node: &Option<Box<Xq210TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_210_update_size<K, V>(node: &mut Xq210TreapNode<K, V>) {
    node.size = 1 + xq_210_size(&node.left) + xq_210_size(&node.right);
}

fn xq_210_rotate_right<K, V>(mut node: Box<Xq210TreapNode<K, V>>) -> Box<Xq210TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_210_update_size(&mut node);
    left.right = Some(node);
    xq_210_update_size(&mut left);
    left
}

fn xq_210_rotate_left<K, V>(mut node: Box<Xq210TreapNode<K, V>>) -> Box<Xq210TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_210_update_size(&mut node);
    right.left = Some(node);
    xq_210_update_size(&mut right);
    right
}

fn xq_210_insert_node<K: Ord, V>(
    node: Option<Box<Xq210TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq210TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq210TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq210Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq210Ord::Less => {
                let (new_left, old) = xq_210_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_210_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_210_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq210Ord::Greater => {
                let (new_right, old) = xq_210_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_210_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_210_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_210_remove_node<K: Ord, V>(
    node: Option<Box<Xq210TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq210TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq210Ord::Less => {
                let (new_left, old) = xq_210_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_210_update_size(&mut n);
                (Some(n), old)
            }
            Xq210Ord::Greater => {
                let (new_right, old) = xq_210_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_210_update_size(&mut n);
                (Some(n), old)
            }
            Xq210Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_210_rotate_right(n);
                    let (new_right, old) = xq_210_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_210_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_210_rotate_left(n);
                    let (new_left, old) = xq_210_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_210_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_210_find_min<K, V>(node: &Option<Box<Xq210TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_210_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_210_find_max<K, V>(node: &Option<Box<Xq210TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_210_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_210_rank<K: Ord, V>(node: &Option<Box<Xq210TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq210Ord::Less => xq_210_rank(&n.left, key),
            Xq210Ord::Equal => xq_210_size(&n.left),
            Xq210Ord::Greater => 1 + xq_210_size(&n.left) + xq_210_rank(&n.right, key),
        },
    }
}

fn xq_210_kth<K, V>(node: &Option<Box<Xq210TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_210_size(&n.left);
        if k < left_size {
            xq_210_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_210_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_210_in_order<K: Clone, V>(node: &Option<Box<Xq210TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_210_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_210_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq210Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 210 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_210_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq210Ord::Equal => return Some(&n.value),
                Xq210Ord::Less => cur = &n.left,
                Xq210Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_210_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_210_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_210_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_210_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_210_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_210_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_210_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq210VEBTree ---------------

pub struct Xq210VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq210VEBTree>>,
    clusters: Vec<Option<Box<Xq210VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq210VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq210VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq210VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr210KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr210KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr210BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr210KDNode {
    xr_point: Xr210KDPoint,
    xr_left: Option<Box<Xr210KDNode>>,
    xr_right: Option<Box<Xr210KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr210KDTree {
    xr_root: Option<Box<Xr210KDNode>>,
    xr_size: usize,
}

impl Xr210KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr210KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr210KDNode>>,
        point: Xr210KDPoint,
        depth: usize,
    ) -> Box<Xr210KDNode> {
        match node {
            None => Box::new(Xr210KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr210KDPoint) -> Option<Xr210KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr210KDNode>,
        query: &Xr210KDPoint,
        depth: usize,
        best: &mut Xr210KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr210KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr210KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr210KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr210KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr210KDNode>>, pts: &mut Vec<Xr210KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr210KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr210BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr210BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_by_extension_known() {
        assert_eq!(detect_by_extension("main.rs"), Some("rust".to_string()));
        assert_eq!(detect_by_extension("app.py"), Some("python".to_string()));
        assert_eq!(
            detect_by_extension("index.js"),
            Some("javascript".to_string())
        );
        assert_eq!(detect_by_extension("unknown.xyz"), None);
    }

    #[test]
    fn detect_by_shebang_variants() {
        assert_eq!(
            detect_by_shebang("#!/usr/bin/env python3"),
            Some("python".to_string())
        );
        assert_eq!(
            detect_by_shebang("#!/bin/bash"),
            Some("shellscript".to_string())
        );
        assert_eq!(
            detect_by_shebang("#!/usr/bin/env node"),
            Some("javascript".to_string())
        );
        assert_eq!(detect_by_shebang("no shebang here"), None);
    }

    #[test]
    fn service_detect_uses_extension_first() {
        let svc = LanguageDetectionService::new();
        // Even though content looks like Python, extension wins.
        let lang = svc.detect("main.rs", "def foo():\n    pass");
        assert_eq!(lang, Some("rust".to_string()));
    }

    #[test]
    fn service_detect_falls_back_to_shebang() {
        let svc = LanguageDetectionService::new();
        let lang = svc.detect("script", "#!/usr/bin/env python3\nimport os");
        assert_eq!(lang, Some("python".to_string()));
    }

    #[test]
    fn detect_by_content_returns_results() {
        let results = detect_by_content("fn main() {\n    let mut x = 1;\n}");
        let langs: Vec<&str> = results.iter().map(|r| r.language_id.as_str()).collect();
        assert!(langs.contains(&"rust"));
    }

    // ── additional tests ──

    #[test]
    fn detection_result_new_validates_confidence() {
        assert!(DetectionResult::new("rust", 0.5).is_ok());
        assert!(DetectionResult::new("rust", 0.0).is_ok());
        assert!(DetectionResult::new("rust", 1.0).is_ok());
        assert_eq!(
            DetectionResult::new("rust", 1.5),
            Err(DetectionError::InvalidConfidence(1.5))
        );
        assert_eq!(
            DetectionResult::new("rust", -0.1),
            Err(DetectionError::InvalidConfidence(-0.1))
        );
    }

    #[test]
    fn detection_result_display() {
        let r = DetectionResult {
            language_id: "rust".into(),
            confidence: 0.75,
        };
        assert_eq!(format!("{r}"), "rust (75%)");
    }

    #[test]
    fn detection_result_exceeds_threshold() {
        let r = DetectionResult {
            language_id: "go".into(),
            confidence: 0.6,
        };
        assert!(r.exceeds_threshold(0.5));
        assert!(r.exceeds_threshold(0.6));
        assert!(!r.exceeds_threshold(0.61));
    }

    #[test]
    fn detection_error_display() {
        assert_eq!(
            DetectionError::NoMatch.to_string(),
            "no language detected"
        );
        assert!(DetectionError::InvalidFilename("".into())
            .to_string()
            .contains("invalid filename"));
        assert!(DetectionError::InvalidConfidence(2.0)
            .to_string()
            .contains("2"));
    }

    #[test]
    fn builder_adds_custom_extension() {
        let svc = LanguageDetectionService::builder()
            .extension("zig", "zig")
            .extension("v", "vlang")
            .build();
        assert!(svc.supports_extension("zig"));
        assert!(svc.supports_extension("v"));
        // built-in extensions are still present
        assert!(svc.supports_extension("rs"));
    }

    #[test]
    fn builder_min_confidence_clamps() {
        let svc = LanguageDetectionService::builder()
            .min_confidence(5.0)
            .build();
        assert!((svc.min_confidence - 1.0).abs() < f64::EPSILON);

        let svc2 = LanguageDetectionService::builder()
            .min_confidence(-3.0)
            .build();
        assert!((svc2.min_confidence).abs() < f64::EPSILON);
    }

    #[test]
    fn service_register_and_unregister() {
        let mut svc = LanguageDetectionService::new();
        let initial = svc.extension_count();
        svc.register_extension("zig", "zig");
        assert_eq!(svc.extension_count(), initial + 1);
        assert_eq!(svc.unregister_extension("zig"), Some("zig".into()));
        assert_eq!(svc.extension_count(), initial);
        assert_eq!(svc.unregister_extension("nope"), None);
    }

    #[test]
    fn detect_with_confidence_extension() {
        let svc = LanguageDetectionService::new();
        let res = svc.detect_with_confidence("lib.rs", "").unwrap();
        assert_eq!(res.language_id, "rust");
        assert!((res.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn detect_with_confidence_shebang() {
        let svc = LanguageDetectionService::new();
        let res = svc
            .detect_with_confidence("script", "#!/usr/bin/env ruby\nputs 'hi'")
            .unwrap();
        assert_eq!(res.language_id, "ruby");
        assert!(res.confidence > 0.8);
    }

    #[test]
    fn detect_with_confidence_no_match() {
        let svc = LanguageDetectionService::new();
        let err = svc.detect_with_confidence("data", "just some random bytes 12345");
        assert_eq!(err, Err(DetectionError::NoMatch));
    }

    #[test]
    fn detect_with_confidence_empty_filename() {
        let svc = LanguageDetectionService::new();
        assert_eq!(
            svc.detect_with_confidence("", "fn main(){}"),
            Err(DetectionError::InvalidFilename(String::new()))
        );
    }

    #[test]
    fn detect_all_sorted_by_confidence() {
        let svc = LanguageDetectionService::new();
        let results = svc.detect_all("fn main() { let mut x = 1; }");
        assert!(!results.is_empty());
        for w in results.windows(2) {
            assert!(w[0].confidence >= w[1].confidence);
        }
    }

    #[test]
    fn detection_similarity_identical() {
        let a = detect_by_content("fn main() {}");
        let sim = detection_similarity(&a, &a);
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn detection_similarity_disjoint() {
        let a = vec![DetectionResult {
            language_id: "rust".into(),
            confidence: 0.5,
        }];
        let b = vec![DetectionResult {
            language_id: "python".into(),
            confidence: 0.5,
        }];
        assert!((detection_similarity(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn service_display() {
        let svc = LanguageDetectionService::new();
        let s = format!("{svc}");
        assert!(s.contains("extensions"));
        assert!(s.contains("min_confidence"));
    }

    #[test]
    fn wb_langdetect_stats_new_defaults() {
        let stats = WbLangdetectStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_langdetect_stats_record_success() {
        let mut stats = WbLangdetectStats::new();
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
    fn wb_langdetect_stats_record_failure() {
        let mut stats = WbLangdetectStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_langdetect_stats_reset() {
        let mut stats = WbLangdetectStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_langdetect_stats_merge() {
        let mut a = WbLangdetectStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbLangdetectStats::new();
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
    fn wb_langdetect_stats_display() {
        let mut stats = WbLangdetectStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_langdetect_stats_default() {
        let stats = WbLangdetectStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_langdetect_validator_accepts_valid_name() {
        let v = WbLangdetectValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_langdetect_validator_rejects_empty() {
        let v = WbLangdetectValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_langdetect_validator_rejects_too_long() {
        let v = WbLangdetectValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_langdetect_validator_forbidden_prefix() {
        let v = WbLangdetectValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_langdetect_validator_allowed_chars() {
        let v = WbLangdetectValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_langdetect_validator_range() {
        let v = WbLangdetectValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_langdetect_sanitize_removes_control() {
        let result = WbLangdetectValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_langdetect_truncate_short_string() {
        assert_eq!(WbLangdetectValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_langdetect_truncate_long_string() {
        let result = WbLangdetectValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_langdetect_is_ascii_printable() {
        assert!(WbLangdetectValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbLangdetectValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn first_line_detector_shebang() {
        let result = FirstLineDetector::detect("#!/usr/bin/env python3").unwrap();
        assert_eq!(result.language_id, "python");
        assert!(result.confidence >= 0.9);
    }

    #[test]
    fn first_line_detector_vim_modeline() {
        let result = FirstLineDetector::detect("# vim: set ft=ruby :").unwrap();
        assert_eq!(result.language_id, "ruby");
    }

    #[test]
    fn first_line_detector_emacs_modeline() {
        let result = FirstLineDetector::detect("# -*- mode: python -*-").unwrap();
        assert_eq!(result.language_id, "python");
    }

    #[test]
    fn first_line_detector_emacs_simple() {
        let result = FirstLineDetector::detect("# -*- rust -*-").unwrap();
        assert_eq!(result.language_id, "rust");
    }

    #[test]
    fn content_sniff_json() {
        let result = ContentSniffDetector::detect("{ \"key\": \"value\" }").unwrap();
        assert_eq!(result.language_id, "json");
    }

    #[test]
    fn content_sniff_xml() {
        let result =
            ContentSniffDetector::detect("<?xml version=\"1.0\"?>\n<root/>").unwrap();
        assert_eq!(result.language_id, "xml");
    }

    #[test]
    fn content_sniff_html() {
        let result =
            ContentSniffDetector::detect("<!DOCTYPE html>\n<html></html>").unwrap();
        assert_eq!(result.language_id, "html");
    }

    #[test]
    fn content_sniff_yaml() {
        let yaml = "name: test\nversion: 1.0\ndescription: a thing";
        let result = ContentSniffDetector::detect(yaml).unwrap();
        assert_eq!(result.language_id, "yaml");
    }

    #[test]
    fn detection_confidence_known_extension() {
        assert!((detection_confidence("main.rs", "") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn detection_confidence_content_only() {
        let conf = detection_confidence("unknown_file", "<?xml version=\"1.0\"?>");
        assert!(conf >= 0.9);
    }

    // ── tests for new functionality ──

    #[test]
    fn detection_result_accessors() {
        let r = DetectionResult {
            language_id: "rust".into(),
            confidence: 0.85,
        };
        assert_eq!(r.language_id(), "rust");
        assert!((r.confidence() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn detection_result_is_high_confidence() {
        let high = DetectionResult { language_id: "go".into(), confidence: 0.8 };
        let low = DetectionResult { language_id: "go".into(), confidence: 0.79 };
        assert!(high.is_high_confidence());
        assert!(!low.is_high_confidence());
    }

    #[test]
    fn is_binary_extension_known() {
        assert!(is_binary_extension("exe"));
        assert!(is_binary_extension("dll"));
        assert!(is_binary_extension("so"));
        assert!(is_binary_extension("bin"));
        assert!(is_binary_extension("o"));
        assert!(is_binary_extension("obj"));
        assert!(is_binary_extension("png"));
        assert!(is_binary_extension("jpg"));
        assert!(is_binary_extension("EXE")); // case-insensitive
        assert!(!is_binary_extension("rs"));
        assert!(!is_binary_extension("txt"));
    }

    #[test]
    fn normalize_language_id_trims_and_lowercases() {
        assert_eq!(normalize_language_id("  Rust  "), "rust");
        assert_eq!(normalize_language_id("JavaScript"), "javascript");
        assert_eq!(normalize_language_id("go"), "go");
        assert_eq!(normalize_language_id("  CPP "), "cpp");
    }

    #[test]
    fn service_supported_extensions() {
        let svc = LanguageDetectionService::new();
        let exts = svc.supported_extensions();
        assert!(exts.contains(&"rs"));
        assert!(exts.contains(&"py"));
        assert!(exts.contains(&"js"));
        // verify sorted
        let mut sorted = exts.clone();
        sorted.sort_unstable();
        assert_eq!(exts, sorted);
    }

    #[test]
    fn service_detect_by_filename_only() {
        let svc = LanguageDetectionService::new();
        assert_eq!(svc.detect_by_filename_only("main.rs"), Some("rust".to_string()));
        assert_eq!(svc.detect_by_filename_only("app.py"), Some("python".to_string()));
        assert_eq!(svc.detect_by_filename_only("unknown.xyz"), None);
        assert_eq!(svc.detect_by_filename_only("noext"), None);
    }

    #[test]
    fn service_language_count() {
        let svc = LanguageDetectionService::new();
        // "yaml" and "yml" map to same language, "c" and "h" map to same language
        let count = svc.language_count();
        assert!(count < svc.extension_count());
        assert!(count >= 15); // at least 15 unique languages
    }

    // --- New tests for LanguageConfidence, multi-strategy, families ---

    #[test]
    fn confidence_level_from_score() {
        assert_eq!(ConfidenceLevel::from_score(0.0), ConfidenceLevel::None);
        assert_eq!(ConfidenceLevel::from_score(0.1), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.5), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.75), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(1.0), ConfidenceLevel::Certain);
    }

    #[test]
    fn language_confidence_combined() {
        let mut lc = LanguageConfidence::new("rust");
        lc.add_score("ext", 0.8);
        lc.add_score("content", 0.6);
        let combined = lc.combined_score();
        assert!((combined - 0.7).abs() < 1e-9);
        assert_eq!(lc.best_score(), 0.8);
        assert_eq!(lc.strategy_count(), 2);
        assert_eq!(lc.level(), ConfidenceLevel::High);
    }

    #[test]
    fn detect_magic_bytes_pdf() {
        let data = b"%PDF-1.4 rest of file";
        let result = detect_by_magic_bytes(data).unwrap();
        assert_eq!(result.language_id, "pdf");
        assert_eq!(result.strategy, DetectionStrategy::MagicBytes);
        assert!((result.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn detect_magic_bytes_png_and_none() {
        let png_header: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(detect_by_magic_bytes(png_header).unwrap().language_id, "png");
        assert!(detect_by_magic_bytes(b"hi").is_none());
    }

    #[test]
    fn multi_strategy_with_extension_and_shebang() {
        let lc = detect_multi_strategy("script.py", "#!/usr/bin/env python3\nprint('hi')");
        assert_eq!(lc.language_id, "python");
        assert!(lc.combined_score() > 0.5);
        assert!(lc.strategy_count() >= 2);
    }

    #[test]
    fn language_family_mapping() {
        assert_eq!(language_family("rust"), LanguageFamily::Systems);
        assert_eq!(language_family("python"), LanguageFamily::ScriptingDynamic);
        assert_eq!(language_family("html"), LanguageFamily::Markup);
        assert_eq!(language_family("json"), LanguageFamily::Data);
        assert_eq!(language_family("unknown_lang"), LanguageFamily::Other);
    }

    #[test]
    fn group_by_family_works() {
        let langs = vec!["rust", "python", "json", "html", "go"];
        let groups = group_by_family(&langs);
        assert_eq!(groups[&LanguageFamily::Systems], vec!["rust", "go"]);
        assert_eq!(groups[&LanguageFamily::Data], vec!["json"]);
    }

    #[test]
    fn filter_by_family_works() {
        let langs = vec!["c", "cpp", "python", "java"];
        let c_fam = filter_by_family(&langs, LanguageFamily::CFamily);
        assert_eq!(c_fam, vec!["c", "cpp", "java"]);
    }

    #[test]
    fn resolve_alias_common() {
        assert_eq!(resolve_alias("c++"), "cpp");
        assert_eq!(resolve_alias("js"), "javascript");
        assert_eq!(resolve_alias("py"), "python");
        assert_eq!(resolve_alias("golang"), "go");
        assert_eq!(resolve_alias("unknown"), "unknown");
    }

    #[test]
    fn languages_equivalent_with_aliases() {
        assert!(languages_equivalent("js", "javascript"));
        assert!(languages_equivalent("py", "python3"));
        assert!(languages_equivalent("c++", "cplusplus"));
        assert!(!languages_equivalent("rust", "python"));
    }

    #[test]
    fn language_features_rust() {
        let feats = LanguageFeatures::from_language("rust");
        assert!(feats.has_types);
        assert!(!feats.has_classes);
        assert!(feats.has_pattern_matching);
        assert!(feats.is_compiled);
        let display = format!("{}", feats);
        assert!(display.contains("typed"));
        assert!(display.contains("compiled"));
    }

    #[test]
    fn language_features_python() {
        let feats = LanguageFeatures::from_language("python");
        assert!(!feats.has_types);
        assert!(feats.has_classes);
        assert!(!feats.is_compiled);
    }

    #[test]
    fn sort_detections_by_confidence() {
        let mut results = vec![
            DetectionResult { language_id: "python".into(), confidence: 0.5 },
            DetectionResult { language_id: "rust".into(), confidence: 0.9 },
            DetectionResult { language_id: "go".into(), confidence: 0.7 },
        ];
        sort_detections(&mut results);
        assert_eq!(results[0].language_id, "rust");
        assert_eq!(results[1].language_id, "go");
        assert_eq!(results[2].language_id, "python");
    }

    #[test]
    fn merge_detections_keeps_best() {
        let results = vec![
            DetectionResult { language_id: "rust".into(), confidence: 0.5 },
            DetectionResult { language_id: "rust".into(), confidence: 0.9 },
            DetectionResult { language_id: "python".into(), confidence: 0.7 },
        ];
        let merged = merge_detections(&results);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].language_id, "rust");
        assert!((merged[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn compare_detections_ordering() {
        let a = DetectionResult { language_id: "rust".into(), confidence: 0.8 };
        let b = DetectionResult { language_id: "python".into(), confidence: 0.8 };
        // Same confidence — shorter name first
        let ord = compare_detections(&a, &b);
        assert_eq!(ord, std::cmp::Ordering::Less);
    }

    // -----------------------------------------------------------------------
    // DetectionCache tests
    // -----------------------------------------------------------------------

    #[test]
    fn cache_insert_and_get() {
        let mut cache = DetectionCache::new(1000);
        let result = DetectionResult { language_id: "rust".into(), confidence: 0.9 };
        cache.insert("main.rs", result, 100);
        assert_eq!(cache.len(), 1);
        let got = cache.get("main.rs", 500);
        assert!(got.is_some());
        assert_eq!(got.unwrap().language_id, "rust");
    }

    #[test]
    fn cache_entry_expires() {
        let mut cache = DetectionCache::new(500);
        let result = DetectionResult { language_id: "python".into(), confidence: 0.8 };
        cache.insert("app.py", result, 100);
        // Within TTL
        assert!(cache.get("app.py", 599).is_some());
        // Expired
        assert!(cache.get("app.py", 600).is_none());
    }

    #[test]
    fn cache_invalidate_and_clear() {
        let mut cache = DetectionCache::new(5000);
        cache.insert("a", DetectionResult { language_id: "go".into(), confidence: 0.7 }, 0);
        cache.insert("b", DetectionResult { language_id: "c".into(), confidence: 0.6 }, 0);
        assert_eq!(cache.len(), 2);
        cache.invalidate("a");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("a", 0).is_none());
        cache.clear();
        assert!(cache.is_empty());
    }

    // -----------------------------------------------------------------------
    // ContentPatternDetector tests
    // -----------------------------------------------------------------------

    #[test]
    fn pattern_detector_rust() {
        let det = ContentPatternDetector::new();
        let results = det.detect("fn main() {\n    println!(\"hello\");\n}");
        assert!(!results.is_empty());
        assert_eq!(results[0].language_id, "rust");
    }

    #[test]
    fn pattern_detector_python() {
        let det = ContentPatternDetector::new();
        let results = det.detect("def hello():\n    print('hi')");
        assert!(results.iter().any(|r| r.language_id == "python"));
    }

    #[test]
    fn pattern_detector_add_custom() {
        let mut det = ContentPatternDetector::new();
        det.add_pattern("SELECT ", "sql", 0.75);
        let results = det.detect("SELECT * FROM users;");
        assert!(results.iter().any(|r| r.language_id == "sql"));
    }

    // -----------------------------------------------------------------------
    // ShebangParser tests
    // -----------------------------------------------------------------------

    #[test]
    fn shebang_parse_env_python() {
        let info = ShebangParser::parse("#!/usr/bin/env python3 -u").unwrap();
        assert_eq!(info.interpreter, "python3");
        assert_eq!(info.args, vec!["-u"]);
    }

    #[test]
    fn shebang_parse_direct_path() {
        let info = ShebangParser::parse("#!/bin/bash").unwrap();
        assert_eq!(info.interpreter, "bash");
        assert!(info.args.is_empty());
    }

    #[test]
    fn shebang_no_shebang() {
        assert!(ShebangParser::parse("just a comment").is_none());
    }

    #[test]
    fn shebang_language_mapping() {
        assert_eq!(ShebangParser::language_from_interpreter("python3"), Some("python".into()));
        assert_eq!(ShebangParser::language_from_interpreter("node"), Some("javascript".into()));
        assert_eq!(ShebangParser::language_from_interpreter("bash"), Some("shellscript".into()));
        assert_eq!(ShebangParser::language_from_interpreter("unknown_interp"), None);
    }

    // -----------------------------------------------------------------------
    // DetectionScorer tests
    // -----------------------------------------------------------------------

    #[test]
    fn scorer_best_match() {
        let mut scorer = DetectionScorer::new();
        scorer.add_signal("extension", "rust", 0.9);
        scorer.add_signal("content", "python", 0.5);
        scorer.add_signal("shebang", "python", 0.7);
        let best = scorer.best_match().unwrap();
        assert_eq!(best.language_id, "rust");
        assert!((best.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn scorer_all_candidates_sorted() {
        let mut scorer = DetectionScorer::new();
        scorer.add_signal("ext", "go", 0.4);
        scorer.add_signal("content", "rust", 0.8);
        scorer.add_signal("content", "go", 0.6);
        let candidates = scorer.all_candidates();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].language_id, "rust");
        // go should get the max of its two signals
        assert!((candidates[1].confidence - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn scorer_empty_returns_none() {
        let scorer = DetectionScorer::new();
        assert!(scorer.best_match().is_none());
        assert_eq!(scorer.signal_count(), 0);
    }

    #[test]
    fn languageDetectionEnsemble_new() {
        let s = LanguageDetectionEnsemble::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn languageDetectionEnsemble_add_contains() {
        let mut s = LanguageDetectionEnsemble::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn languageDetectionEnsemble_add_duplicate() {
        let mut s = LanguageDetectionEnsemble::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn languageDetectionEnsemble_remove() {
        let mut s = LanguageDetectionEnsemble::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn languageDetectionEnsemble_capacity() {
        let s = LanguageDetectionEnsemble::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn languageDetectionEnsemble_search() {
        let mut s = LanguageDetectionEnsemble::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn languageDetectionEnsemble_stats() {
        let mut s = LanguageDetectionEnsemble::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn detectionAccuracyScorer_new() {
        let m = DetectionAccuracyScorer::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn detectionAccuracyScorer_add_find() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn detectionAccuracyScorer_priority_filter() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("a", "A").with_priority(DetectionAccuracyScorerPriority::High));
        m.add(DetectionAccuracyScorerItem::new("b", "B").with_priority(DetectionAccuracyScorerPriority::Low));
        m.add(DetectionAccuracyScorerItem::new("c", "C").with_priority(DetectionAccuracyScorerPriority::High));
        assert_eq!(m.by_priority(DetectionAccuracyScorerPriority::High).len(), 2);
    }

    #[test]
    fn detectionAccuracyScorer_remove() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn detectionAccuracyScorer_search() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("id1", "Hello World"));
        m.add(DetectionAccuracyScorerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn detectionAccuracyScorer_total_weight() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("a", "A").with_priority(DetectionAccuracyScorerPriority::Critical));
        m.add(DetectionAccuracyScorerItem::new("b", "B").with_priority(DetectionAccuracyScorerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn detectionAccuracyScorer_capacity_limit() {
        let mut m = DetectionAccuracyScorer::new().with_max_items(2);
        m.add(DetectionAccuracyScorerItem::new("1", "one"));
        m.add(DetectionAccuracyScorerItem::new("2", "two"));
        assert!(!m.add(DetectionAccuracyScorerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn detectionAccuracyScorer_sorted_by_priority() {
        let mut m = DetectionAccuracyScorer::new();
        m.add(DetectionAccuracyScorerItem::new("lo", "Low").with_priority(DetectionAccuracyScorerPriority::Low));
        m.add(DetectionAccuracyScorerItem::new("hi", "High").with_priority(DetectionAccuracyScorerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn detectionAccuracyScorer_item_metadata() {
        let mut item = DetectionAccuracyScorerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn languageDetectionEnsemble_enabled_toggle() {
        let mut s = LanguageDetectionEnsemble::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn detectionAccuracyScorer_priority_display() {
        assert_eq!(format!("{}", DetectionAccuracyScorerPriority::High), "high");
        assert_eq!(format!("{}", DetectionAccuracyScorerPriority::Low), "low");
    }


    #[test]
    fn wbLangdetect_x_config_new() {
        let c = WbLangdetectXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn wbLangdetect_x_config_builder() {
        let c = WbLangdetectXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn wbLangdetect_x_config_display() {
        let c = WbLangdetectXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn wbLangdetect_x_registry_insert_get() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbLangdetect_x_registry_duplicate() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a")).unwrap();
        assert!(reg.insert(WbLangdetectXConfig::new("a")).is_err());
    }

    #[test]
    fn wbLangdetect_x_registry_remove() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a")).unwrap();
        reg.insert(WbLangdetectXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbLangdetect_x_registry_active_entries() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a")).unwrap();
        reg.insert(WbLangdetectXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn wbLangdetect_x_registry_by_weight() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(WbLangdetectXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn wbLangdetect_x_registry_tags() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(WbLangdetectXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn wbLangdetect_x_registry_total_weight() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(WbLangdetectXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn wbLangdetect_x_registry_iterator() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a")).unwrap();
        reg.insert(WbLangdetectXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wbLangdetect_x_cache_put_get() {
        let mut cache = WbLangdetectXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn wbLangdetect_x_cache_eviction() {
        let mut cache = WbLangdetectXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn wbLangdetect_x_cache_lru_order() {
        let mut cache = WbLangdetectXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn wbLangdetect_x_cache_most_least_recent() {
        let mut cache = WbLangdetectXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn wbLangdetect_x_formatter_entry() {
        let e = WbLangdetectXConfig::new("k").with_value("v");
        let fmt = WbLangdetectXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn wbLangdetect_x_formatter_summary() {
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("a").with_weight(5)).unwrap();
        let fmt = WbLangdetectXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn wbLangdetect_x_validator_valid() {
        let v = WbLangdetectXValidator::new();
        let c = WbLangdetectXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn wbLangdetect_x_validator_empty_key() {
        let v = WbLangdetectXValidator::new();
        let c = WbLangdetectXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLangdetect_x_validator_require_value() {
        let v = WbLangdetectXValidator::new().require_value(true);
        let c = WbLangdetectXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLangdetect_x_validator_allowed_tags() {
        let v = WbLangdetectXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = WbLangdetectXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLangdetect_x_validator_validate_all() {
        let v = WbLangdetectXValidator::new();
        let mut reg = WbLangdetectXRegistry::new();
        reg.insert(WbLangdetectXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
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


    #[test]
    fn xb_ring_buffer_94_push_and_len() {
        let mut rb = super::XbRingBuffer94::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_94_overwrite() {
        let mut rb = super::XbRingBuffer94::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_94_get_out_of_bounds() {
        let rb = super::XbRingBuffer94::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_94_drain_all() {
        let mut rb = super::XbRingBuffer94::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_94_peek_front_back() {
        let mut rb = super::XbRingBuffer94::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_94_clear() {
        let mut rb = super::XbRingBuffer94::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_94_capacity() {
        let rb = super::XbRingBuffer94::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_94_basic() {
        let h = super::xb_fnv1a_94(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_94(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_94_different_inputs() {
        let h1 = super::xb_fnv1a_94(b"abc");
        let h2 = super::xb_fnv1a_94(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_94_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_94(&data);
        let dec = super::xb_rle_decode_94(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_94_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_94(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_94(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_94_values() {
        assert!((super::xb_clamp_94(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_94(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_94(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_94_values() {
        assert!((super::xb_lerp_94(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_94(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_94(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_94_wrap_around_twice() {
        let mut rb = super::XbRingBuffer94::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 211 ----

    #[test]
    fn xc_211_pool_new_empty() {
        let pool: super::Xc211Pool<i32> = super::Xc211Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_211_pool_release_acquire() {
        let mut pool = super::Xc211Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_211_pool_acquire_empty() {
        let mut pool: super::Xc211Pool<i32> = super::Xc211Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_211_pool_full() {
        let mut pool = super::Xc211Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_211_pool_drain() {
        let mut pool = super::Xc211Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_211_pool_stats() {
        let mut pool = super::Xc211Pool::new(8);
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
    fn xc_211_pool_clear() {
        let mut pool = super::Xc211Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_211_pool_shrink() {
        let mut pool = super::Xc211Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_211_pool_default() {
        let pool: super::Xc211Pool<String> = super::Xc211Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_211_pool_extend() {
        let mut pool = super::Xc211Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_211_pool_retain() {
        let mut pool = super::Xc211Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_211_scheduler_round_robin() {
        let mut sched = super::Xc211Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_211_scheduler_empty() {
        let mut sched = super::Xc211Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_211_scheduler_reset() {
        let mut sched = super::Xc211Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_211_scheduler_add_remove() {
        let mut sched = super::Xc211Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_211_scheduler_targets() {
        let sched = super::Xc211Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_211_hash_empty() {
        assert_eq!(super::xc_211_hash(b""), 5381);
    }

    #[test]
    fn xc_211_hash_data() {
        let h = super::xc_211_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_211_hash(b"hello"), h);
    }

    #[test]
    fn xc_211_reverse_str() {
        assert_eq!(super::xc_211_reverse("abc"), "cba");
        assert_eq!(super::xc_211_reverse(""), "");
    }


    #[test]
    fn xe_107_pipeline_empty() {
        let p = super::Xe107Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_107_pipeline_parse_stage() {
        let p = super::Xe107Pipeline::new()
            .add_parse(super::xe_107_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_107_pipeline_transform_double() {
        let p = super::Xe107Pipeline::new()
            .add_transform(super::xe_107_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_107_pipeline_validate_reverse() {
        let p = super::Xe107Pipeline::new()
            .add_validate(super::xe_107_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_107_pipeline_emit_filter() {
        let p = super::Xe107Pipeline::new()
            .add_emit(super::xe_107_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_107_pipeline_multi_stage() {
        let p = super::Xe107Pipeline::new()
            .add_parse(super::xe_107_pipeline_identity)
            .add_transform(super::xe_107_pipeline_double)
            .add_validate(super::xe_107_pipeline_reverse)
            .add_emit(super::xe_107_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_107_pipeline_error_propagation() {
        let p = super::Xe107Pipeline::new()
            .add_parse(super::xe_107_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe107Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_107_pipeline_compose() {
        let p1 = super::Xe107Pipeline::new()
            .add_parse(super::xe_107_pipeline_identity);
        let p2 = super::Xe107Pipeline::new()
            .add_transform(super::xe_107_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_107_pipeline_error_display() {
        let e = super::Xe107PipelineError {
            stage: super::Xe107Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_107_cache_put_get() {
        let mut c = super::Xe107Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_107_cache_miss() {
        let mut c: super::Xe107Cache<&str, i32> = super::Xe107Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_107_cache_ttl_expiry() {
        let mut c = super::Xe107Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_107_cache_evict() {
        let mut c = super::Xe107Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_107_cache_capacity() {
        let mut c = super::Xe107Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_107_cache_stats() {
        let mut c = super::Xe107Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_107_cache_clear() {
        let mut c = super::Xe107Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_105 graph tests ------------------------------------------------

    #[test]
    fn xg_105_graph_empty() {
        let g = super::Xg105Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_105_graph_add_node() {
        let mut g = super::Xg105Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_105_graph_add_edge() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_105_graph_neighbors() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_105_graph_has_path() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_105_graph_self_path() {
        let g = super::Xg105Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_105_graph_topo_sort() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_105_graph_cycle_detect_false() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_105_graph_cycle_detect_true() {
        let mut g = super::Xg105Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_105 heap tests -------------------------------------------------

    #[test]
    fn xg_105_heap_empty() {
        let h: super::Xg105Heap<i32> = super::Xg105Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_105_heap_push_pop() {
        let mut h = super::Xg105Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_105_heap_peek() {
        let mut h = super::Xg105Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_105_heap_drain_sorted() {
        let mut h = super::Xg105Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_105_heap_merge() {
        let mut a = super::Xg105Heap::new();
        let mut b = super::Xg105Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_105_heap_default() {
        let h: super::Xg105Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_105_graph_default() {
        let g: super::Xg105Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh210_skip_insert_contains() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh210_skip_remove() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh210_skip_len() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh210_skip_range_query() {
        let mut sl = super::Xh210SkipList::xh_new(4);
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
    fn xh210_skip_floor_ceiling() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh210_skip_rank() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh210_skip_empty() {
        let sl = super::Xh210SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh210_skip_duplicates() {
        let mut sl = super::Xh210SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh210_bitset_set_test() {
        let mut bs = super::Xh210BitSet::xh_new(256);
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
    fn xh210_bitset_clear_count() {
        let mut bs = super::Xh210BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh210_bitset_and_or_xor() {
        let mut a = super::Xh210BitSet::xh_new(128);
        let mut b = super::Xh210BitSet::xh_new(128);
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
    fn xh210_bitset_iter_ones() {
        let mut bs = super::Xh210BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh210_bitset_first_last() {
        let mut bs = super::Xh210BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh210_bitset_empty() {
        let bs = super::Xh210BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi210_deque_push_pop_back() {
        let mut dq = super::Xi210Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi210_deque_push_pop_front() {
        let mut dq = super::Xi210Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi210_deque_mixed_ops() {
        let mut dq = super::Xi210Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi210_deque_get_and_split() {
        let mut dq = super::Xi210Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi210_deque_rotate_left() {
        let mut dq = super::Xi210Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi210_deque_rotate_right() {
        let mut dq = super::Xi210Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi210_deque_grow() {
        let mut dq = super::Xi210Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi210_deque_empty() {
        let dq = super::Xi210Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi210_interval_tree_insert_query() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi210Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi210Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi210_interval_tree_overlap() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi210Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi210Interval::xi_new(12, 20));
        let q = super::Xi210Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi210_interval_tree_remove() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi210Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi210_interval_tree_gaps() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi210Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi210Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi210Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi210Interval::xi_new(8, 10));
    }

    #[test]
    fn xi210_interval_tree_merge() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi210Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi210Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi210Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi210Interval::xi_new(10, 15));
    }

    #[test]
    fn xi210_interval_tree_all() {
        let mut tree = super::Xi210IntervalTree::xi_new();
        tree.xi_insert(super::Xi210Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi210Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi210_interval_tree_empty() {
        let tree = super::Xi210IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi210_interval_tree_contains_point() {
        let iv = super::Xi210Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 210) ---

    #[test]
    fn xj_210_uf_make_and_find() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_210_uf_union_connected() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_210_uf_component_count() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_210_uf_component_size() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_210_uf_largest_component() {
        let mut uf = super::Xj210UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_210_uf_many_elements() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_210_uf_separate_components() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_210_uf_path_compression() {
        let mut uf = super::Xj210UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_210_bt_insert_get() {
        let mut bt = super::Xj210BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_210_bt_contains_len() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_210_bt_replace() {
        let mut bt = super::Xj210BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_210_bt_remove() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_210_bt_keys_values() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_210_bt_range() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_210_bt_min_max() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_210_bt_many_inserts() {
        let mut bt = super::Xj210BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_210 segment tree tests ---

    #[test]
    fn xk_210_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_210_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk210SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_210_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_210_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_210_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_210_st_single_element() {
        let data = vec![42];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_210_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk210SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_210_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk210SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_210 disjoint intervals tests ---

    #[test]
    fn xk_210_di_add_and_count() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_210_di_merge_overlap() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_210_di_contains() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_210_di_remove() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_210_di_covered_length() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_210_di_gaps() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_210_di_merge_adjacent() {
        let mut di = super::Xk210DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_210_di_empty() {
        let di = super::Xk210DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_210_rope_new_empty() {
        let rope = super::Xl210Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_210_rope_from_str() {
        let rope = super::Xl210Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_210_rope_insert_at() {
        let mut rope = super::Xl210Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_210_rope_delete_range() {
        let mut rope = super::Xl210Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_210_rope_char_at() {
        let rope = super::Xl210Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_210_rope_split_concat() {
        let rope = super::Xl210Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_210_rope_line_count() {
        let rope = super::Xl210Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_210_rope_line_at() {
        let rope = super::Xl210Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_210_sa_build_and_search() {
        let sa = super::Xl210SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_210_sa_count() {
        let sa = super::Xl210SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_210_sa_longest_repeated() {
        let sa = super::Xl210SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_210_sa_all_positions() {
        let sa = super::Xl210SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_210_sa_len() {
        let sa = super::Xl210SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_210_sa_empty() {
        let sa = super::Xl210SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_210_rope_slice() {
        let rope = super::Xl210Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_210_sa_search_start() {
        let sa = super::Xl210SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_210_sparse_set_get() {
        let mut m = super::Xm210MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_210_sparse_row_col() {
        let mut m = super::Xm210MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_210_sparse_transpose() {
        let mut m = super::Xm210MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_210_sparse_multiply_vec() {
        let mut m = super::Xm210MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_210_sparse_nnz_density() {
        let mut m = super::Xm210MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_210_sparse_clear() {
        let mut m = super::Xm210MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_210_sparse_overwrite_zero() {
        let mut m = super::Xm210MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_210_tokenizer_basic() {
        let t = super::Xm210Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_210_tokenizer_count() {
        let t = super::Xm210Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_210_tokenizer_unique() {
        let t = super::Xm210Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_210_tokenizer_frequency() {
        let t = super::Xm210Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_210_tokenizer_delimiter() {
        let t = super::Xm210Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_210_tokenizer_whitespace() {
        let t = super::Xm210Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_210_tokenizer_empty() {
        let t = super::Xm210Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 210 ----

    #[test]
    fn xn_210_fenwick_prefix_sum() {
        let mut ft = super::Xn210Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_210_fenwick_range_sum() {
        let mut ft = super::Xn210Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_210_fenwick_point_query() {
        let mut ft = super::Xn210Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_210_fenwick_len() {
        let ft = super::Xn210Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_210_fenwick_multiple_updates() {
        let mut ft = super::Xn210Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_210_fenwick_single_element() {
        let mut ft = super::Xn210Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_210_fenwick_find_kth() {
        let mut ft = super::Xn210Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_210_fenwick_negative_delta() {
        let mut ft = super::Xn210Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 210 ----

    #[test]
    fn xn_210_avl_insert_get() {
        let mut m = super::Xn210AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_210_avl_remove() {
        let mut m = super::Xn210AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_210_avl_in_order() {
        let mut m = super::Xn210AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_210_avl_min_max() {
        let mut m = super::Xn210AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_210_avl_floor_ceiling() {
        let mut m = super::Xn210AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_210_avl_height_balanced() {
        let mut m = super::Xn210AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_210_avl_overwrite() {
        let mut m = super::Xn210AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_210_avl_empty() {
        let m: super::Xn210AVL<i32, i32> = super::Xn210AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo210RedBlack tests ---

    #[test]
    fn xo_210_rb_insert_and_get() {
        let mut tree = super::Xo210RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_210_rb_len_and_empty() {
        let mut tree = super::Xo210RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_210_rb_min_max() {
        let mut tree = super::Xo210RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_210_rb_contains() {
        let mut tree = super::Xo210RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_210_rb_remove() {
        let mut tree = super::Xo210RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_210_rb_in_order() {
        let mut tree = super::Xo210RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_210_rb_black_height() {
        let mut tree = super::Xo210RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_210_rb_overwrite() {
        let mut tree = super::Xo210RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo210ConsistentHash tests ---

    #[test]
    fn xo_210_ch_add_and_count() {
        let mut ring = super::Xo210ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_210_ch_remove_node() {
        let mut ring = super::Xo210ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_210_ch_get_node() {
        let mut ring = super::Xo210ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_210_ch_empty_ring() {
        let ring = super::Xo210ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_210_ch_distribution() {
        let mut ring = super::Xo210ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_210_ch_rebalance() {
        let mut ring = super::Xo210ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_210_ch_virtual_nodes() {
        let mut ring = super::Xo210ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_210_ch_consistent_lookup() {
        let mut ring = super::Xo210ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_210_splay_insert_get() {
        let mut t = super::Xp210SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_210_splay_remove() {
        let mut t = super::Xp210SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_210_splay_count_increases() {
        let mut t = super::Xp210SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_210_splay_depth() {
        let mut t = super::Xp210SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_210_splay_len_empty() {
        let t = super::Xp210SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_210_splay_min_max() {
        let mut t = super::Xp210SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_210_splay_overwrite() {
        let mut t = super::Xp210SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_210_splay_remove_missing() {
        let mut t = super::Xp210SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_210 treap tests ----
    #[test]
    fn xq_210_treap_empty() {
        let t = super::Xq210Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_210_treap_insert_get() {
        let mut t = super::Xq210Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_210_treap_overwrite() {
        let mut t = super::Xq210Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_210_treap_remove() {
        let mut t = super::Xq210Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_210_treap_min_max() {
        let mut t = super::Xq210Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_210_treap_rank() {
        let mut t = super::Xq210Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_210_treap_kth() {
        let mut t = super::Xq210Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_210_treap_in_order() {
        let mut t = super::Xq210Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_210 VEB tree tests ----
    #[test]
    fn xq_210_veb_empty() {
        let v = super::Xq210VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_210_veb_insert_contains() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_210_veb_min_max() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_210_veb_delete() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_210_veb_successor() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_210_veb_predecessor() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_210_veb_count() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_210_veb_duplicate_insert() {
        let mut v = super::Xq210VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_210_kdtree_empty() {
        let tree = super::Xr210KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_210_kdtree_insert_one() {
        let mut tree = super::Xr210KDTree::xr_new();
        tree.xr_insert(super::Xr210KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_210_kdtree_insert_multiple() {
        let mut tree = super::Xr210KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr210KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_210_kdtree_nearest_neighbor() {
        let mut tree = super::Xr210KDTree::xr_new();
        tree.xr_insert(super::Xr210KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr210KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr210KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_210_kdtree_nn_empty() {
        let tree = super::Xr210KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr210KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_210_kdtree_range_search() {
        let mut tree = super::Xr210KDTree::xr_new();
        tree.xr_insert(super::Xr210KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr210KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr210KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_210_kdtree_range_empty() {
        let mut tree = super::Xr210KDTree::xr_new();
        tree.xr_insert(super::Xr210KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_210_kdtree_all_points() {
        let mut tree = super::Xr210KDTree::xr_new();
        tree.xr_insert(super::Xr210KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr210KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_210_kdtree_depth() {
        let mut tree = super::Xr210KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr210KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_210_kdtree_bounding_box() {
        let mut tree = super::Xr210KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr210KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr210KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
