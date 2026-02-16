//! Problem matchers for parsing build output into diagnostics.

use regex::Regex;

// ── Types ───────────────────────────────────────────────────────────────

/// Severity of a problem matched from build output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for ProblemSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic problem parsed from build output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: ProblemSeverity,
    pub message: String,
    pub code: Option<String>,
    pub source: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.file, self.line, self.column, self.severity, self.message
        )
    }
}

/// A problem pattern with named capture group indices.
#[derive(Debug, Clone)]
pub struct ProblemPattern {
    pub regexp: Regex,
    pub file_group: usize,
    pub line_group: usize,
    pub column_group: Option<usize>,
    pub severity_group: Option<usize>,
    pub message_group: usize,
    pub code_group: Option<usize>,
}

impl ProblemPattern {
    pub fn new(
        regexp: &str,
        file_group: usize,
        line_group: usize,
        message_group: usize,
    ) -> Result<Self, regex::Error> {
        Ok(Self {
            regexp: Regex::new(regexp)?,
            file_group,
            line_group,
            column_group: None,
            severity_group: None,
            message_group,
            code_group: None,
        })
    }

    pub fn with_column(mut self, group: usize) -> Self {
        self.column_group = Some(group);
        self
    }

    pub fn with_severity(mut self, group: usize) -> Self {
        self.severity_group = Some(group);
        self
    }

    pub fn with_code(mut self, group: usize) -> Self {
        self.code_group = Some(group);
        self
    }
}

// ── Trait ────────────────────────────────────────────────────────────────

/// Trait for problem matchers that parse build output lines.
pub trait ProblemMatcher: Send + Sync {
    fn name(&self) -> &str;
    fn parse_line(&self, line: &str) -> Option<Diagnostic>;

    /// Parse multiple lines and return all matched diagnostics.
    fn parse_output(&self, output: &str) -> Vec<Diagnostic> {
        output.lines().filter_map(|l| self.parse_line(l)).collect()
    }
}

// ── Pattern-based matcher ───────────────────────────────────────────────

/// Generic problem matcher that uses a regex pattern.
pub struct PatternMatcher {
    name: String,
    pattern: ProblemPattern,
    default_severity: ProblemSeverity,
    source: String,
}

impl PatternMatcher {
    pub fn new(name: &str, pattern: ProblemPattern, source: &str) -> Self {
        Self {
            name: name.to_string(),
            pattern,
            default_severity: ProblemSeverity::Error,
            source: source.to_string(),
        }
    }

    pub fn with_default_severity(mut self, severity: ProblemSeverity) -> Self {
        self.default_severity = severity;
        self
    }
}

impl ProblemMatcher for PatternMatcher {
    fn name(&self) -> &str {
        &self.name
    }

    fn parse_line(&self, line: &str) -> Option<Diagnostic> {
        let caps = self.pattern.regexp.captures(line)?;
        let file = caps.get(self.pattern.file_group)?.as_str().to_string();
        let line_num: u32 = caps.get(self.pattern.line_group)?.as_str().parse().ok()?;
        let column: u32 = self
            .pattern
            .column_group
            .and_then(|g| caps.get(g))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(1);
        let severity = self
            .pattern
            .severity_group
            .and_then(|g| caps.get(g))
            .map(|m| parse_severity(m.as_str()))
            .unwrap_or(self.default_severity);
        let message = caps
            .get(self.pattern.message_group)?
            .as_str()
            .to_string();
        let code = self
            .pattern
            .code_group
            .and_then(|g| caps.get(g))
            .map(|m| m.as_str().to_string());

        Some(Diagnostic {
            file,
            line: line_num,
            column,
            severity,
            message,
            code,
            source: self.source.clone(),
        })
    }
}

fn parse_severity(s: &str) -> ProblemSeverity {
    match s.to_lowercase().as_str() {
        "error" => ProblemSeverity::Error,
        "warning" | "warn" => ProblemSeverity::Warning,
        "info" | "note" => ProblemSeverity::Info,
        "hint" => ProblemSeverity::Hint,
        _ => ProblemSeverity::Error,
    }
}

// ── Built-in matchers ───────────────────────────────────────────────────

/// Rust compiler problem matcher: parses `error[E0xxx]: message` and `warning: message` lines.
pub struct RustcProblemMatcher {
    re: Regex,
}

impl RustcProblemMatcher {
    pub fn new() -> Self {
        Self {
            // Matches: "error[E0308]: mismatched types" at " --> src/main.rs:10:5"
            // Also: "warning: unused variable" with location on next or same context
            // We parse the combined form: severity followed by location
            re: Regex::new(
                r"^\s*-->\s+(.+):(\d+):(\d+)$"
            )
            .expect("valid regex"),
        }
    }
}

impl Default for RustcProblemMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// State for multi-line rustc output parsing.
pub struct RustcParser {
    severity_re: Regex,
    location_re: Regex,
}

impl RustcParser {
    pub fn new() -> Self {
        Self {
            severity_re: Regex::new(
                r"^(error|warning)(?:\[([A-Z]\d+)\])?:\s+(.+)$"
            )
            .expect("valid regex"),
            location_re: Regex::new(
                r"^\s*-->\s+(.+):(\d+):(\d+)$"
            )
            .expect("valid regex"),
        }
    }

    /// Parse multi-line rustc output into diagnostics.
    pub fn parse(&self, output: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            if let Some(caps) = self.severity_re.captures(lines[i]) {
                let severity = parse_severity(caps.get(1).unwrap().as_str());
                let code = caps.get(2).map(|m| m.as_str().to_string());
                let message = caps.get(3).unwrap().as_str().to_string();

                // Look for location on the next few lines
                for j in (i + 1)..std::cmp::min(i + 5, lines.len()) {
                    if let Some(loc_caps) = self.location_re.captures(lines[j]) {
                        let file = loc_caps.get(1).unwrap().as_str().to_string();
                        let line: u32 = loc_caps.get(2).unwrap().as_str().parse().unwrap_or(1);
                        let column: u32 = loc_caps.get(3).unwrap().as_str().parse().unwrap_or(1);
                        diagnostics.push(Diagnostic {
                            file,
                            line,
                            column,
                            severity,
                            message: message.clone(),
                            code: code.clone(),
                            source: "rustc".to_string(),
                        });
                        break;
                    }
                }
            }
            i += 1;
        }

        diagnostics
    }
}

impl Default for RustcParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemMatcher for RustcProblemMatcher {
    fn name(&self) -> &str {
        "$rustc"
    }

    fn parse_line(&self, line: &str) -> Option<Diagnostic> {
        let caps = self.re.captures(line)?;
        let file = caps.get(1)?.as_str().to_string();
        let line_num: u32 = caps.get(2)?.as_str().parse().ok()?;
        let column: u32 = caps.get(3)?.as_str().parse().ok()?;
        Some(Diagnostic {
            file,
            line: line_num,
            column,
            severity: ProblemSeverity::Error,
            message: String::new(),
            code: None,
            source: "rustc".to_string(),
        })
    }
}

/// TypeScript compiler problem matcher.
pub struct TscProblemMatcher {
    re: Regex,
}

impl TscProblemMatcher {
    pub fn new() -> Self {
        Self {
            // Matches: "src/app.ts(10,5): error TS2322: Type 'string' is not assignable..."
            re: Regex::new(
                r"^(.+)\((\d+),(\d+)\):\s+(error|warning)\s+(TS\d+):\s+(.+)$"
            )
            .expect("valid regex"),
        }
    }
}

impl Default for TscProblemMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ProblemMatcher for TscProblemMatcher {
    fn name(&self) -> &str {
        "$tsc"
    }

    fn parse_line(&self, line: &str) -> Option<Diagnostic> {
        let caps = self.re.captures(line)?;
        Some(Diagnostic {
            file: caps.get(1)?.as_str().to_string(),
            line: caps.get(2)?.as_str().parse().ok()?,
            column: caps.get(3)?.as_str().parse().ok()?,
            severity: parse_severity(caps.get(4)?.as_str()),
            message: caps.get(6)?.as_str().to_string(),
            code: Some(caps.get(5)?.as_str().to_string()),
            source: "tsc".to_string(),
        })
    }
}

/// GCC-style problem matcher.
pub fn gcc_matcher() -> PatternMatcher {
    let pattern = ProblemPattern::new(
        r"^(.+):(\d+):(\d+):\s+(error|warning|note):\s+(.+)$",
        1,
        2,
        5,
    )
    .expect("valid pattern")
    .with_column(3)
    .with_severity(4);

    PatternMatcher::new("$gcc", pattern, "gcc")
}

/// ESLint problem matcher.
pub fn eslint_matcher() -> PatternMatcher {
    let pattern = ProblemPattern::new(
        r"^\s*(\d+):(\d+)\s+(error|warning)\s+(.+?)\s+(\S+)$",
        // ESLint outputs file on a separate line; this matches within a file context
        // We treat line as file placeholder for single-line matching
        1,
        1,
        4,
    )
    .expect("valid pattern")
    .with_column(2)
    .with_severity(3);

    PatternMatcher::new("$eslint", pattern, "eslint")
}

/// Look up a built-in matcher by name.
pub fn get_builtin_matcher(name: &str) -> Option<Box<dyn ProblemMatcher>> {
    match name {
        "$rustc" => Some(Box::new(RustcProblemMatcher::new())),
        "$tsc" => Some(Box::new(TscProblemMatcher::new())),
        "$gcc" => Some(Box::new(gcc_matcher())),
        "$eslint" => Some(Box::new(eslint_matcher())),
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsc_matcher_parses_error() {
        let matcher = TscProblemMatcher::new();
        let line = "src/app.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.file, "src/app.ts");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.column, 5);
        assert_eq!(diag.severity, ProblemSeverity::Error);
        assert_eq!(diag.code.as_deref(), Some("TS2322"));
        assert!(diag.message.contains("not assignable"));
    }

    #[test]
    fn tsc_matcher_parses_warning() {
        let matcher = TscProblemMatcher::new();
        let line = "lib/util.ts(3,1): warning TS6133: 'x' is declared but its value is never read";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.severity, ProblemSeverity::Warning);
        assert_eq!(diag.code.as_deref(), Some("TS6133"));
    }

    #[test]
    fn tsc_matcher_no_match() {
        let matcher = TscProblemMatcher::new();
        assert!(matcher.parse_line("Compilation complete.").is_none());
    }

    #[test]
    fn gcc_matcher_parses_error() {
        let matcher = gcc_matcher();
        let line = "main.c:10:5: error: expected ';' before '}' token";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.file, "main.c");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.column, 5);
        assert_eq!(diag.severity, ProblemSeverity::Error);
    }

    #[test]
    fn gcc_matcher_parses_warning() {
        let matcher = gcc_matcher();
        let line = "src/util.c:20:3: warning: unused variable 'x'";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.severity, ProblemSeverity::Warning);
    }

    #[test]
    fn gcc_matcher_parses_note() {
        let matcher = gcc_matcher();
        let line = "src/main.c:5:1: note: declared here";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.severity, ProblemSeverity::Info);
    }

    #[test]
    fn rustc_parser_multi_line() {
        let output = "\
error[E0308]: mismatched types
 --> src/main.rs:10:5
  |
10|     let x: i32 = \"hello\";
  |                  ^^^^^^^ expected `i32`, found `&str`

warning: unused variable `y`
 --> src/lib.rs:20:9
  |
20|     let y = 42;
  |         ^ help: if this is intentional, prefix it with an underscore: `_y`
";
        let parser = RustcParser::new();
        let diagnostics = parser.parse(output);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].file, "src/main.rs");
        assert_eq!(diagnostics[0].line, 10);
        assert_eq!(diagnostics[0].severity, ProblemSeverity::Error);
        assert_eq!(diagnostics[0].code.as_deref(), Some("E0308"));
        assert!(diagnostics[0].message.contains("mismatched types"));

        assert_eq!(diagnostics[1].file, "src/lib.rs");
        assert_eq!(diagnostics[1].line, 20);
        assert_eq!(diagnostics[1].severity, ProblemSeverity::Warning);
    }

    #[test]
    fn rustc_single_line_matcher() {
        let matcher = RustcProblemMatcher::new();
        let line = " --> src/main.rs:42:13";
        let diag = matcher.parse_line(line).unwrap();
        assert_eq!(diag.file, "src/main.rs");
        assert_eq!(diag.line, 42);
        assert_eq!(diag.column, 13);
    }

    #[test]
    fn parse_output_multiple_lines() {
        let matcher = gcc_matcher();
        let output = "main.c:1:1: error: unknown type\nmain.c:2:1: warning: unused\nsome other line\n";
        let diags = matcher.parse_output(output);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn get_builtin_matcher_known() {
        assert!(get_builtin_matcher("$rustc").is_some());
        assert!(get_builtin_matcher("$tsc").is_some());
        assert!(get_builtin_matcher("$gcc").is_some());
        assert!(get_builtin_matcher("$eslint").is_some());
    }

    #[test]
    fn get_builtin_matcher_unknown() {
        assert!(get_builtin_matcher("$unknown").is_none());
    }

    #[test]
    fn diagnostic_display() {
        let d = Diagnostic {
            file: "src/main.rs".to_string(),
            line: 10,
            column: 5,
            severity: ProblemSeverity::Error,
            message: "type mismatch".to_string(),
            code: None,
            source: "rustc".to_string(),
        };
        assert_eq!(d.to_string(), "src/main.rs:10:5: error: type mismatch");
    }

    #[test]
    fn problem_severity_display() {
        assert_eq!(ProblemSeverity::Error.to_string(), "error");
        assert_eq!(ProblemSeverity::Warning.to_string(), "warning");
        assert_eq!(ProblemSeverity::Info.to_string(), "info");
        assert_eq!(ProblemSeverity::Hint.to_string(), "hint");
    }

    #[test]
    fn custom_pattern_matcher() {
        let pattern = ProblemPattern::new(
            r"^ERROR:\s+(.+):(\d+):\s+(.+)$",
            1,
            2,
            3,
        )
        .unwrap();
        let matcher = PatternMatcher::new("custom", pattern, "custom-tool");
        let diag = matcher.parse_line("ERROR: build.py:42: syntax error").unwrap();
        assert_eq!(diag.file, "build.py");
        assert_eq!(diag.line, 42);
        assert_eq!(diag.message, "syntax error");
        assert_eq!(diag.source, "custom-tool");
    }

    #[test]
    fn pattern_matcher_with_default_severity() {
        let pattern = ProblemPattern::new(r"^(.+):(\d+):\s+(.+)$", 1, 2, 3).unwrap();
        let matcher =
            PatternMatcher::new("test", pattern, "test").with_default_severity(ProblemSeverity::Warning);
        let diag = matcher.parse_line("file.txt:1: something").unwrap();
        assert_eq!(diag.severity, ProblemSeverity::Warning);
    }
}
