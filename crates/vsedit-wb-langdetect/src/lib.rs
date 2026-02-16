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
}
