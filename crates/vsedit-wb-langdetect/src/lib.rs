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
}
