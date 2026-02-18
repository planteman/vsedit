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

}
