//! Resource label formatting – path manipulation and label templates.

use std::fmt;

/// Errors that can occur during label formatting and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelError {
    /// The format pattern contains no recognized placeholders.
    NoPlaceholders,
    /// The path string is empty.
    EmptyPath,
    /// The label name exceeds the maximum allowed length.
    NameTooLong { max: usize, actual: usize },
    /// An unknown placeholder was found in the pattern.
    UnknownPlaceholder(String),
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelError::NoPlaceholders => write!(f, "format pattern contains no placeholders"),
            LabelError::EmptyPath => write!(f, "path must not be empty"),
            LabelError::NameTooLong { max, actual } => {
                write!(f, "name length {actual} exceeds maximum {max}")
            }
            LabelError::UnknownPlaceholder(p) => write!(f, "unknown placeholder: {p}"),
        }
    }
}

impl std::error::Error for LabelError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelFormat {
    pub pattern: String,
}

/// Display detail level for labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelDetail {
    /// Filename only.
    Short,
    /// Filename + parent directory.
    Medium,
    /// Full path with filename.
    Long,
    /// Full path with icon and description.
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLabel {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

impl fmt::Display for ResourceLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref desc) = self.description {
            write!(f, "{} — {}", self.name, desc)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl fmt::Display for LabelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LabelFormat({})", self.pattern)
    }
}

impl fmt::Display for LabelDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LabelDetail::Short => write!(f, "short"),
            LabelDetail::Medium => write!(f, "medium"),
            LabelDetail::Long => write!(f, "long"),
            LabelDetail::Full => write!(f, "full"),
        }
    }
}

/// Builder for constructing a [`ResourceLabel`] with validation.
#[derive(Debug, Clone, Default)]
pub struct ResourceLabelBuilder {
    name: Option<String>,
    path: Option<String>,
    description: Option<String>,
    icon: Option<String>,
}

impl ResourceLabelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Build the label, deriving the name from the path filename if not set.
    pub fn build(self) -> Result<ResourceLabel, LabelError> {
        let path = self.path.ok_or(LabelError::EmptyPath)?;
        if path.is_empty() {
            return Err(LabelError::EmptyPath);
        }
        let name = self.name.unwrap_or_else(|| extract_filename(&path).to_string());
        const MAX_NAME_LEN: usize = 255;
        if name.len() > MAX_NAME_LEN {
            return Err(LabelError::NameTooLong {
                max: MAX_NAME_LEN,
                actual: name.len(),
            });
        }
        Ok(ResourceLabel {
            name,
            path,
            description: self.description,
            icon: self.icon,
        })
    }
}

impl ResourceLabel {
    /// Create a builder for this type.
    pub fn builder() -> ResourceLabelBuilder {
        ResourceLabelBuilder::new()
    }

    /// Format this label at the given detail level.
    pub fn format(&self, detail: LabelDetail) -> String {
        format_file_label(&self.path, detail)
    }

    /// Return the file extension of this label's path, if any.
    pub fn extension(&self) -> Option<&str> {
        extract_extension(&self.path)
    }

    /// Return the parent directory portion of this label's path.
    pub fn parent_dir(&self) -> &str {
        self.path.rfind('/').map(|i| &self.path[..i]).unwrap_or("")
    }

    /// Returns `true` if the path has a file extension, indicating it likely refers to a file.
    pub fn is_file(&self) -> bool {
        extract_extension(&self.path).is_some()
    }

    /// Return the full path including the filename.
    ///
    /// If the path already ends with the name, return the path as-is.
    /// Otherwise, join path and name with a `/` separator.
    pub fn full_path(&self) -> String {
        if self.path.ends_with(&self.name) {
            self.path.clone()
        } else {
            let sep = if self.path.ends_with('/') { "" } else { "/" };
            format!("{}{}{}", self.path, sep, self.name)
        }
    }

    /// Set the icon using builder-style chaining, consuming and returning self.
    pub fn with_icon(mut self, icon: String) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// A segment of a highlighted label (for fuzzy match rendering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSegment {
    pub text: String,
    pub highlighted: bool,
}

impl LabelSegment {
    /// Returns `true` if this segment is highlighted.
    pub fn is_highlight(&self) -> bool {
        self.highlighted
    }
}

/// Replace `${filename}`, `${dirname}`, and `${extname}` in the format pattern.
pub fn format_label(path: &str, format: &LabelFormat) -> String {
    let filename = extract_filename(path);
    let dirname = path
        .rfind('/')
        .map(|i| &path[..i])
        .unwrap_or("");
    let extname = extract_extension(path).unwrap_or("");

    format
        .pattern
        .replace("${filename}", filename)
        .replace("${dirname}", dirname)
        .replace("${extname}", extname)
}

/// Format a file path label at the requested detail level.
pub fn format_file_label(path: &str, detail: LabelDetail) -> String {
    match detail {
        LabelDetail::Short => extract_filename(path).to_string(),
        LabelDetail::Medium => {
            let filename = extract_filename(path);
            let parent = path
                .rfind('/')
                .and_then(|i| path[..i].rfind('/').map(|j| &path[j + 1..i]))
                .unwrap_or("");
            if parent.is_empty() {
                filename.to_string()
            } else {
                format!("{filename} — {parent}")
            }
        }
        LabelDetail::Long => path.to_string(),
        LabelDetail::Full => {
            let filename = extract_filename(path);
            let ext = extract_extension(path).unwrap_or("file");
            format!("[{ext}] {filename} — {path}")
        }
    }
}

/// Format a workspace name for display.
pub fn format_workspace_label(name: &str, root_path: Option<&str>) -> String {
    match root_path {
        Some(rp) => format!("{name} ({rp})"),
        None => name.to_string(),
    }
}

/// Highlight characters matching a fuzzy query in a label text.
/// Returns styled segments indicating which parts are highlighted.
pub fn highlight_label(label: &str, query: &str) -> Vec<LabelSegment> {
    if query.is_empty() {
        return vec![LabelSegment {
            text: label.to_string(),
            highlighted: false,
        }];
    }

    let label_lower = label.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut match_indices = Vec::new();
    let mut qi = 0;
    let query_chars: Vec<char> = query_lower.chars().collect();

    for (i, ch) in label_lower.chars().enumerate() {
        if qi < query_chars.len() && ch == query_chars[qi] {
            match_indices.push(i);
            qi += 1;
        }
    }

    if qi != query_chars.len() {
        // No full match; return unhighlighted
        return vec![LabelSegment {
            text: label.to_string(),
            highlighted: false,
        }];
    }

    let mut segments = Vec::new();
    let chars: Vec<char> = label.chars().collect();
    let mut i = 0;
    let mut mi = 0;

    while i < chars.len() {
        if mi < match_indices.len() && i == match_indices[mi] {
            let mut end = i;
            while mi < match_indices.len() && match_indices[mi] == end {
                mi += 1;
                end += 1;
            }
            segments.push(LabelSegment {
                text: chars[i..end].iter().collect(),
                highlighted: true,
            });
            i = end;
        } else {
            let start = i;
            while i < chars.len() && (mi >= match_indices.len() || i != match_indices[mi]) {
                i += 1;
            }
            segments.push(LabelSegment {
                text: chars[start..i].iter().collect(),
                highlighted: false,
            });
        }
    }

    segments
}

/// Shorten a path by replacing middle segments with `...` if it exceeds `max_len`.
pub fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        return path.to_string();
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    let shortened = format!("{first}/.../{last}");
    if shortened.len() >= path.len() {
        return path.to_string();
    }
    shortened
}

/// Extract the filename component from a path.
pub fn extract_filename(path: &str) -> &str {
    path.rfind('/').map(|i| &path[i + 1..]).unwrap_or(path)
}

/// Extract the file extension (without the dot) from a path.
pub fn extract_extension(path: &str) -> Option<&str> {
    let filename = extract_filename(path);
    filename.rfind('.').map(|i| &filename[i + 1..])
}

/// Validate a format pattern, returning an error if it contains no known placeholders.
pub fn validate_format(format: &LabelFormat) -> Result<(), LabelError> {
    let known = ["${filename}", "${dirname}", "${extname}"];
    if !known.iter().any(|p| format.pattern.contains(p)) {
        return Err(LabelError::NoPlaceholders);
    }
    Ok(())
}

/// Extract the stem (filename without extension) from a path.
pub fn extract_stem(path: &str) -> &str {
    let filename = extract_filename(path);
    match filename.rfind('.') {
        Some(i) if i > 0 => &filename[..i],
        _ => filename,
    }
}

/// Count the depth (number of path segments) of a path.
pub fn path_depth(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    path.split('/').filter(|s| !s.is_empty()).count()
}

/// Compute a common prefix shared by all given paths.
pub fn common_path_prefix<'a>(paths: &[&'a str]) -> &'a str {
    if paths.is_empty() {
        return "";
    }
    let first = paths[0];
    let mut end = first.len();
    for p in &paths[1..] {
        end = end.min(p.len());
        for (i, (a, b)) in first.bytes().zip(p.bytes()).enumerate() {
            if a != b {
                end = end.min(i);
                break;
            }
        }
    }
    // Snap back to the last '/' boundary so we don't split mid-segment.
    if let Some(pos) = first[..end].rfind('/') {
        &first[..=pos]
    } else {
        ""
    }
}

/// Strip a prefix from a path, returning the relative remainder.
pub fn strip_prefix<'a>(path: &'a str, prefix: &str) -> &'a str {
    path.strip_prefix(prefix).unwrap_or(path)
}

/// Accumulated statistics for label operations.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl LabelStats {
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
    pub fn merge(&mut self, other: &LabelStats) {
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

impl Default for LabelStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LabelStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LabelStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for label.
#[derive(Debug, Clone)]
pub struct LabelValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl LabelValidator {
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

impl Default for LabelValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A label that optionally includes an icon identifier (e.g. codicon name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconLabel {
    pub text: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

impl IconLabel {
    /// Create a plain text label with no icon.
    pub fn text_only(text: impl Into<String>) -> Self {
        Self { text: text.into(), icon: None, description: None }
    }

    /// Create a label with an icon.
    pub fn with_icon(text: impl Into<String>, icon: impl Into<String>) -> Self {
        Self { text: text.into(), icon: Some(icon.into()), description: None }
    }

    /// Set the description, consuming self.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Return the display string, prepending the icon if present.
    pub fn display_string(&self) -> String {
        match (&self.icon, &self.description) {
            (Some(icon), Some(desc)) => format!("$({}) {} — {}", icon, self.text, desc),
            (Some(icon), None) => format!("$({}) {}", icon, self.text),
            (None, Some(desc)) => format!("{} — {}", self.text, desc),
            (None, None) => self.text.clone(),
        }
    }

    /// Returns `true` if the label has an icon.
    pub fn has_icon(&self) -> bool {
        self.icon.is_some()
    }
}

impl fmt::Display for IconLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_string())
    }
}

/// A highlighted label, splitting text into highlighted and non-highlighted ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelHighlight {
    pub segments: Vec<LabelSegment>,
}

impl LabelHighlight {
    /// Create a highlight result from a label and a search query using fuzzy matching.
    pub fn from_query(label: &str, query: &str) -> Self {
        Self { segments: highlight_label(label, query) }
    }

    /// Return the fully concatenated text (without highlight info).
    pub fn plain_text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }

    /// Return only the highlighted portions concatenated.
    pub fn highlighted_text(&self) -> String {
        self.segments.iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.as_str())
            .collect()
    }

    /// Returns `true` if any part of the label is highlighted.
    pub fn has_match(&self) -> bool {
        self.segments.iter().any(|s| s.highlighted)
    }

    /// Count the number of highlighted characters.
    pub fn highlight_count(&self) -> usize {
        self.segments.iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.chars().count())
            .sum()
    }
}

impl fmt::Display for LabelHighlight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for seg in &self.segments {
            if seg.highlighted {
                write!(f, "[{}]", seg.text)?;
            } else {
                write!(f, "{}", seg.text)?;
            }
        }
        Ok(())
    }
}

/// Truncate a label to `max_chars` characters, appending "..." if it was truncated.
///
/// Unlike `LabelValidator::truncate` which uses the Unicode ellipsis character,
/// this uses the ASCII "..." which is 3 characters wide.
pub fn label_ellipsis(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }
    let take = max_chars - 3;
    let truncated: String = text.chars().take(take).collect();
    format!("{}...", truncated)
}

/// Truncate from the middle, keeping start and end visible with "..." in between.
pub fn label_ellipsis_middle(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return text.chars().take(max_chars).collect();
    }
    let available = max_chars - 3;
    let start_len = (available + 1) / 2;
    let end_len = available / 2;
    let start: String = text.chars().take(start_len).collect();
    let end: String = text.chars().rev().take(end_len).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{}...{}", start, end)
}

/// Truncate a label to `max_len` characters, appending a custom ellipsis string if truncated.
///
/// If the label already fits within `max_len`, it is returned unchanged.
/// Otherwise the label is truncated so that the result (text + ellipsis) is at most `max_len`.
pub fn truncate_label(label: &str, max_len: usize, ellipsis: &str) -> String {
    let label_chars: usize = label.chars().count();
    if label_chars <= max_len {
        return label.to_string();
    }
    let ellipsis_chars = ellipsis.chars().count();
    if max_len <= ellipsis_chars {
        return label.chars().take(max_len).collect();
    }
    let keep = max_len - ellipsis_chars;
    let truncated: String = label.chars().take(keep).collect();
    format!("{truncated}{ellipsis}")
}

/// Format a byte count as a human-readable size label.
///
/// Uses binary units (1 KB = 1024 bytes) and returns strings like
/// `"512 B"`, `"1.2 KB"`, `"3.4 MB"`, `"1.0 GB"`, or `"2.5 TB"`.
pub fn format_size_label(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else if b < TB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.1} TB", b / TB)
    }
}

/// Format a count with the appropriate singular or plural noun.
///
/// Returns e.g. `"1 file"` or `"3 files"`.
pub fn format_count_label(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

/// Remove control characters (Unicode category Cc) from a label string.
///
/// Tabs, newlines, null bytes, and other control codes are stripped.
/// Normal whitespace (space, U+0020) is preserved.
pub fn sanitize_label(label: &str) -> String {
    label.chars().filter(|c| !c.is_control()).collect()
}

// ---------------------------------------------------------------------------
// LabelTemplate — template-based label generation
// ---------------------------------------------------------------------------

/// A reusable label template with named placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelTemplate {
    pattern: String,
    placeholders: Vec<String>,
}

impl LabelTemplate {
    /// Create a new template. Placeholders are `{name}` style.
    pub fn new(pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        let mut placeholders = Vec::new();
        let mut rest = pattern.as_str();
        while let Some(start) = rest.find('{') {
            if let Some(end) = rest[start..].find('}') {
                let name = &rest[start + 1..start + end];
                if !name.is_empty() && !placeholders.contains(&name.to_string()) {
                    placeholders.push(name.to_string());
                }
                rest = &rest[start + end + 1..];
            } else {
                break;
            }
        }
        Self { pattern, placeholders }
    }

    /// Return the list of placeholder names found in the template.
    pub fn placeholder_names(&self) -> &[String] {
        &self.placeholders
    }

    /// Render the template by replacing placeholders with values from the map.
    /// Missing keys are left as-is.
    pub fn render(&self, values: &std::collections::HashMap<String, String>) -> String {
        let mut result = self.pattern.clone();
        for key in &self.placeholders {
            if let Some(val) = values.get(key) {
                result = result.replace(&format!("{{{key}}}"), val);
            }
        }
        result
    }

    /// Return true if all placeholders have corresponding values.
    pub fn is_complete(&self, values: &std::collections::HashMap<String, String>) -> bool {
        self.placeholders.iter().all(|p| values.contains_key(p))
    }
}

// ---------------------------------------------------------------------------
// LabelLocalizer — localization support for labels
// ---------------------------------------------------------------------------

/// Simple localization map from key to translated string.
#[derive(Debug, Clone, Default)]
pub struct LabelLocalizer {
    translations: std::collections::HashMap<String, String>,
    fallback_locale: String,
}

impl LabelLocalizer {
    pub fn new(fallback_locale: impl Into<String>) -> Self {
        Self {
            translations: std::collections::HashMap::new(),
            fallback_locale: fallback_locale.into(),
        }
    }

    /// Register a translation for a key.
    pub fn add(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.translations.insert(key.into(), value.into());
    }

    /// Look up a translation, falling back to the key itself.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.translations.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    /// Return how many translations are registered.
    pub fn count(&self) -> usize {
        self.translations.len()
    }

    /// Return the fallback locale identifier.
    pub fn fallback_locale(&self) -> &str {
        &self.fallback_locale
    }
}

// ---------------------------------------------------------------------------
// LabelCache — cache computed labels
// ---------------------------------------------------------------------------

/// Caches computed label strings keyed by path.
#[derive(Debug, Clone, Default)]
pub struct LabelCache {
    entries: std::collections::HashMap<String, String>,
}

impl LabelCache {
    pub fn new() -> Self {
        Self { entries: std::collections::HashMap::new() }
    }

    /// Get a cached label for a path, or compute and cache it.
    pub fn get_or_insert(&mut self, path: &str, detail: LabelDetail) -> &str {
        self.entries.entry(path.to_string())
            .or_insert_with(|| format_file_label(path, detail))
    }

    /// Invalidate a cached entry.
    pub fn invalidate(&mut self, path: &str) {
        self.entries.remove(path);
    }

    /// Clear all cached labels.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Comparison and sorting for ResourceLabel
// ---------------------------------------------------------------------------

impl PartialOrd for ResourceLabel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResourceLabel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
            .then_with(|| self.path.cmp(&other.path))
    }
}

impl ResourceLabel {
    /// Compare two labels by extension, then name.
    pub fn cmp_by_extension(&self, other: &ResourceLabel) -> std::cmp::Ordering {
        let ext_a = self.extension().unwrap_or("");
        let ext_b = other.extension().unwrap_or("");
        ext_a.cmp(ext_b).then_with(|| self.name.cmp(&other.name))
    }

    /// Return true if the label name contains the given query (case-insensitive).
    pub fn matches_query(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }
}

/// Sort a slice of ResourceLabels by name.
pub fn sort_labels_by_name(labels: &mut [ResourceLabel]) {
    labels.sort();
}

/// Sort a slice of ResourceLabels by extension, then name.
pub fn sort_labels_by_extension(labels: &mut [ResourceLabel]) {
    labels.sort_by(|a, b| a.cmp_by_extension(b));
}

/// Filter resource labels that match a given extension (case-insensitive).
pub fn filter_labels_by_extension<'a>(labels: &'a [ResourceLabel], ext: &str) -> Vec<&'a ResourceLabel> {
    let ext_lower = ext.to_lowercase();
    labels.iter().filter(|l| {
        l.extension().map(|e| e.to_lowercase()) == Some(ext_lower.clone())
    }).collect()
}

/// Deduplicate resource labels by path, keeping the first occurrence.
pub fn dedup_labels_by_path(labels: &mut Vec<ResourceLabel>) {
    let mut seen = std::collections::HashSet::new();
    labels.retain(|l| seen.insert(l.path.clone()));
}

/// Return the longest label name length from a slice.
pub fn max_label_name_length(labels: &[ResourceLabel]) -> usize {
    labels.iter().map(|l| l.name.chars().count()).max().unwrap_or(0)
}

/// Build a mapping from extension to count of labels with that extension.
pub fn extension_histogram(labels: &[ResourceLabel]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for label in labels {
        let ext = label.extension().unwrap_or("(none)").to_lowercase();
        *map.entry(ext).or_insert(0) += 1;
    }
    map
}

/// Group resource labels by their parent directory.
pub fn group_labels_by_dir(labels: &[ResourceLabel]) -> std::collections::HashMap<String, Vec<&ResourceLabel>> {
    let mut map: std::collections::HashMap<String, Vec<&ResourceLabel>> = std::collections::HashMap::new();
    for label in labels {
        let dir = label.parent_dir().to_string();
        map.entry(dir).or_default().push(label);
    }
    map
}

impl ResourceLabel {
    /// Return true if the label's path has the given extension (case-insensitive).
    pub fn has_extension(&self, ext: &str) -> bool {
        self.extension()
            .map(|e| e.eq_ignore_ascii_case(ext))
            .unwrap_or(false)
    }

    /// Return the file name without extension (stem).
    pub fn stem(&self) -> &str {
        extract_stem(&self.name)
    }

    /// Return a compact display string: "name (dir)".
    pub fn compact_display(&self) -> String {
        format!("{} ({})", self.name, self.parent_dir())
    }
}

// ---------------------------------------------------------------------------
// LabelFormat – additional methods
// ---------------------------------------------------------------------------

impl LabelFormat {
    /// Create a new `LabelFormat` from a pattern string.
    pub fn new(pattern: impl Into<String>) -> Self {
        Self { pattern: pattern.into() }
    }

    /// Return the set of known placeholder names present in this pattern.
    pub fn used_placeholders(&self) -> Vec<&'static str> {
        let known: &[&str] = &["${filename}", "${dirname}", "${extname}"];
        known.iter().copied().filter(|p| self.pattern.contains(p)).collect()
    }

    /// Return `true` if the pattern references the given placeholder token.
    pub fn uses(&self, placeholder: &str) -> bool {
        self.pattern.contains(placeholder)
    }
}

// ---------------------------------------------------------------------------
// LabelDetail – ordering & conversion
// ---------------------------------------------------------------------------

impl LabelDetail {
    /// Return an integer verbosity rank (0 = least verbose, 3 = most verbose).
    pub fn verbosity(self) -> u8 {
        match self {
            LabelDetail::Short => 0,
            LabelDetail::Medium => 1,
            LabelDetail::Long => 2,
            LabelDetail::Full => 3,
        }
    }

    /// Parse a detail level from a string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "short" => Some(LabelDetail::Short),
            "medium" | "med" => Some(LabelDetail::Medium),
            "long" => Some(LabelDetail::Long),
            "full" => Some(LabelDetail::Full),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// LabelSegment – additional helpers
// ---------------------------------------------------------------------------

impl LabelSegment {
    /// Create a plain (non-highlighted) segment.
    pub fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), highlighted: false }
    }

    /// Create a highlighted segment.
    pub fn highlight(text: impl Into<String>) -> Self {
        Self { text: text.into(), highlighted: true }
    }

    /// Return the number of characters in this segment.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }
}

// ---------------------------------------------------------------------------
// ResourceLabel – batch & path utilities
// ---------------------------------------------------------------------------

impl ResourceLabel {
    /// Return the path depth (number of `/`-separated segments).
    pub fn depth(&self) -> usize {
        path_depth(&self.path)
    }

    /// Return `true` if this label's path starts with the given prefix.
    pub fn is_under(&self, prefix: &str) -> bool {
        self.path.starts_with(prefix)
    }

    /// Strip a common prefix from the path, returning the relative remainder.
    pub fn relative_path(&self, prefix: &str) -> &str {
        strip_prefix(&self.path, prefix)
    }

    /// Produce a `LabelHighlight` by fuzzy-matching the name against `query`.
    pub fn highlight_name(&self, query: &str) -> LabelHighlight {
        LabelHighlight::from_query(&self.name, query)
    }
}

/// Filter resource labels whose path starts with the given prefix.
pub fn filter_labels_under<'a>(labels: &'a [ResourceLabel], prefix: &str) -> Vec<&'a ResourceLabel> {
    labels.iter().filter(|l| l.is_under(prefix)).collect()
}

/// Join an iterator of labels into a single comma-separated string of names.
pub fn join_label_names(labels: &[ResourceLabel], separator: &str) -> String {
    labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(separator)
}

/// Normalize a path by collapsing consecutive separators and removing trailing slashes.
pub fn normalize_path_separators(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut last_was_slash = false;
    for ch in path.chars() {
        if ch == '/' {
            if !last_was_slash {
                result.push('/');
            }
            last_was_slash = true;
        } else {
            result.push(ch);
            last_was_slash = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// LabelFormatter – template variable substitution
// ---------------------------------------------------------------------------

/// Position of the ellipsis when truncating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EllipsisPosition {
    Start,
    Middle,
    End,
}

/// Simple template formatter that replaces `${key}` placeholders with values.
#[derive(Debug, Clone)]
pub struct LabelFormatter {
    template: String,
    variables: std::collections::HashMap<String, String>,
}

impl LabelFormatter {
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            variables: std::collections::HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.variables.insert(key.into(), value.into());
        self
    }

    /// Replaces every `${key}` occurrence in the template with its value.
    /// Unresolved placeholders are left as-is.
    pub fn format(&self) -> String {
        let mut out = self.template.clone();
        for (k, v) in &self.variables {
            let placeholder = format!("${{{}}}", k);
            out = out.replace(&placeholder, v);
        }
        out
    }

    pub fn has_variable(&self, key: &str) -> bool {
        self.variables.contains_key(key)
    }

    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    pub fn clear(&mut self) {
        self.variables.clear();
    }
}

// ---------------------------------------------------------------------------
// LabelTruncator – configurable ellipsis position
// ---------------------------------------------------------------------------

/// Truncates text to a maximum character count with a configurable ellipsis
/// position.
#[derive(Debug, Clone)]
pub struct LabelTruncator {
    max_chars: usize,
    position: EllipsisPosition,
}

impl LabelTruncator {
    pub fn new(max_chars: usize, position: EllipsisPosition) -> Self {
        Self { max_chars, position }
    }

    pub fn needs_truncation(&self, text: &str) -> bool {
        text.chars().count() > self.max_chars
    }

    pub fn truncate(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() <= self.max_chars {
            return text.to_string();
        }
        match self.position {
            EllipsisPosition::End => {
                let keep = self.max_chars.saturating_sub(1);
                let mut s: String = chars[..keep].iter().collect();
                s.push('…');
                s
            }
            EllipsisPosition::Start => {
                let keep = self.max_chars.saturating_sub(1);
                let start = chars.len() - keep;
                let mut s = String::from('…');
                s.extend(&chars[start..]);
                s
            }
            EllipsisPosition::Middle => {
                let half = self.max_chars.saturating_sub(1) / 2;
                let tail = self.max_chars.saturating_sub(1) - half;
                let mut s: String = chars[..half].iter().collect();
                s.push('…');
                s.extend(&chars[chars.len() - tail..]);
                s
            }
        }
    }
}

// ---------------------------------------------------------------------------
// LabelHighlighter – highlight matching portions of text
// ---------------------------------------------------------------------------

/// A span within highlighted text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub matched: bool,
}

/// Finds and highlights all occurrences of a query within text.
#[derive(Debug, Clone)]
pub struct LabelHighlighter {
    case_sensitive: bool,
}

impl LabelHighlighter {
    pub fn new(case_sensitive: bool) -> Self {
        Self { case_sensitive }
    }

    pub fn has_match(&self, text: &str, query: &str) -> bool {
        if query.is_empty() {
            return false;
        }
        if self.case_sensitive {
            text.contains(query)
        } else {
            text.to_lowercase().contains(&query.to_lowercase())
        }
    }

    pub fn match_count(&self, text: &str, query: &str) -> usize {
        if query.is_empty() {
            return 0;
        }
        if self.case_sensitive {
            text.matches(query).count()
        } else {
            text.to_lowercase().matches(&query.to_lowercase()).count()
        }
    }

    /// Returns a list of spans covering the entire text, with `matched` set to
    /// `true` for portions that match `query`.
    pub fn highlight(&self, text: &str, query: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        if query.is_empty() {
            if !text.is_empty() {
                spans.push(HighlightSpan { start: 0, end: text.len(), matched: false });
            }
            return spans;
        }
        let (haystack, needle);
        let (h, n);
        if self.case_sensitive {
            haystack = text;
            needle = query;
        } else {
            h = text.to_lowercase();
            n = query.to_lowercase();
            haystack = &h;
            needle = &n;
        }
        let mut cursor = 0usize;
        for (idx, _) in haystack.match_indices(needle) {
            if idx > cursor {
                spans.push(HighlightSpan { start: cursor, end: idx, matched: false });
            }
            spans.push(HighlightSpan { start: idx, end: idx + query.len(), matched: true });
            cursor = idx + query.len();
        }
        if cursor < text.len() {
            spans.push(HighlightSpan { start: cursor, end: text.len(), matched: false });
        }
        spans
    }
}

// ---------------------------------------------------------------------------
// LabelIconResolver – map file extension to icon name
// ---------------------------------------------------------------------------

/// Resolves a file extension to an icon identifier.
#[derive(Debug, Clone)]
pub struct LabelIconResolver {
    icon_map: std::collections::HashMap<String, String>,
    default_icon: String,
}

impl LabelIconResolver {
    pub fn new(default_icon: impl Into<String>) -> Self {
        Self {
            icon_map: std::collections::HashMap::new(),
            default_icon: default_icon.into(),
        }
    }

    pub fn register(&mut self, extension: &str, icon: &str) -> &mut Self {
        self.icon_map.insert(extension.to_lowercase(), icon.to_string());
        self
    }

    /// Returns the icon for the extension extracted from `filename`, or the
    /// default icon when no mapping exists.
    pub fn resolve(&self, filename: &str) -> &str {
        if let Some(dot) = filename.rfind('.') {
            let ext = &filename[dot + 1..];
            if let Some(icon) = self.icon_map.get(&ext.to_lowercase()) {
                return icon.as_str();
            }
        }
        &self.default_icon
    }

    pub fn icon_count(&self) -> usize {
        self.icon_map.len()
    }

    pub fn has_icon(&self, extension: &str) -> bool {
        self.icon_map.contains_key(&extension.to_lowercase())
    }
}


// ── Label Template Engine ──

/// A parsed template token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateToken {
    Literal(String),
    Variable(String),
    Conditional { variable: String, if_true: String, if_false: String },
}

/// A compiled template ready for rendering.
#[derive(Debug, Clone)]
pub struct CompiledTemplate {
    tokens: Vec<TemplateToken>,
    source: String,
}

/// Template engine with variable substitution and conditionals.
///
/// Supports `${var}` for variable substitution and `${var?then:else}` for conditionals.
pub struct LabelTemplateEngine {
    variables: Vec<(String, String)>,
}

impl LabelTemplateEngine {
    pub fn new() -> Self {
        Self {
            variables: Vec::new(),
        }
    }

    /// Register a variable and its value.
    pub fn set_var(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let name = name.into();
        if let Some(entry) = self.variables.iter_mut().find(|(k, _)| *k == name) {
            entry.1 = value.into();
        } else {
            self.variables.push((name, value.into()));
        }
        self
    }

    /// Remove a variable.
    pub fn remove_var(&mut self, name: &str) -> bool {
        let len = self.variables.len();
        self.variables.retain(|(k, _)| k != name);
        self.variables.len() != len
    }

    /// Get a variable value.
    pub fn get_var(&self, name: &str) -> Option<&str> {
        self.variables.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Compile a template string into tokens.
    pub fn compile(&self, template: &str) -> CompiledTemplate {
        let mut tokens = Vec::new();
        let mut rest = template;
        while let Some(start) = rest.find("${") {
            if start > 0 {
                tokens.push(TemplateToken::Literal(rest[..start].to_string()));
            }
            let after_start = &rest[start + 2..];
            if let Some(end) = after_start.find('}') {
                let expr = &after_start[..end];
                if let Some(q_pos) = expr.find('?') {
                    let var_name = expr[..q_pos].to_string();
                    let branches = &expr[q_pos + 1..];
                    let (if_true, if_false) = if let Some(colon) = branches.find(':') {
                        (branches[..colon].to_string(), branches[colon + 1..].to_string())
                    } else {
                        (branches.to_string(), String::new())
                    };
                    tokens.push(TemplateToken::Conditional { variable: var_name, if_true, if_false });
                } else {
                    tokens.push(TemplateToken::Variable(expr.to_string()));
                }
                rest = &after_start[end + 1..];
            } else {
                tokens.push(TemplateToken::Literal(rest[start..].to_string()));
                rest = "";
            }
        }
        if !rest.is_empty() {
            tokens.push(TemplateToken::Literal(rest.to_string()));
        }
        CompiledTemplate {
            tokens,
            source: template.to_string(),
        }
    }

    /// Render a compiled template with the current variables.
    pub fn render_compiled(&self, template: &CompiledTemplate) -> String {
        let mut result = String::new();
        for token in &template.tokens {
            match token {
                TemplateToken::Literal(text) => result.push_str(text),
                TemplateToken::Variable(name) => {
                    if let Some(val) = self.get_var(name) {
                        result.push_str(val);
                    }
                }
                TemplateToken::Conditional { variable, if_true, if_false } => {
                    let val = self.get_var(variable).unwrap_or("");
                    if !val.is_empty() {
                        result.push_str(if_true);
                    } else {
                        result.push_str(if_false);
                    }
                }
            }
        }
        result
    }

    /// Compile and render in one step.
    pub fn render(&self, template: &str) -> String {
        let compiled = self.compile(template);
        self.render_compiled(&compiled)
    }

    /// Count the number of registered variables.
    pub fn var_count(&self) -> usize {
        self.variables.len()
    }

    /// Clear all variables.
    pub fn clear_vars(&mut self) {
        self.variables.clear();
    }

    /// List all variable names.
    pub fn var_names(&self) -> Vec<&str> {
        self.variables.iter().map(|(k, _)| k.as_str()).collect()
    }
}

// ── Label Accessibility Text Builder ──

/// Builds accessible text descriptions from labels.
pub struct LabelAccessibilityTextBuilder {
    parts: Vec<String>,
    separator: String,
    prefix: Option<String>,
    suffix: Option<String>,
}

impl LabelAccessibilityTextBuilder {
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            separator: ", ".to_string(),
            prefix: None,
            suffix: None,
        }
    }

    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Add a text part.
    pub fn add(&mut self, text: impl Into<String>) -> &mut Self {
        let text = text.into();
        if !text.is_empty() {
            self.parts.push(text);
        }
        self
    }

    /// Add a label with a role description.
    pub fn add_with_role(&mut self, text: impl Into<String>, role: impl Into<String>) -> &mut Self {
        let text = text.into();
        let role = role.into();
        if !text.is_empty() {
            self.parts.push(format!("{} ({})", text, role));
        }
        self
    }

    /// Add a key-value pair.
    pub fn add_property(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.parts.push(format!("{}: {}", key.into(), value.into()));
        self
    }

    /// Build the final accessible text string.
    pub fn build(&self) -> String {
        let mut result = String::new();
        if let Some(ref prefix) = self.prefix {
            result.push_str(prefix);
            if !self.parts.is_empty() {
                result.push_str(&self.separator);
            }
        }
        result.push_str(&self.parts.join(&self.separator));
        if let Some(ref suffix) = self.suffix {
            if !result.is_empty() {
                result.push_str(&self.separator);
            }
            result.push_str(suffix);
        }
        result
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn clear(&mut self) {
        self.parts.clear();
    }

    /// Build and wrap in an ARIA label attribute string.
    pub fn to_aria_label(&self) -> String {
        format!("aria-label=\"{}\"", self.build())
    }
}



// ---------------------------------------------------------------------------
// vsedit-label: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl LabelXConfig {
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

impl std::fmt::Display for LabelXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct LabelXRegistry {
    entries: Vec<LabelXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl LabelXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: LabelXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&LabelXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut LabelXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<LabelXConfig> {
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

    pub fn active_entries(&self) -> Vec<&LabelXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&LabelXConfig> {
        let mut sorted: Vec<&LabelXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&LabelXConfig> {
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

    pub fn iter(&self) -> LabelXIterator<'_> {
        LabelXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct LabelXIterator<'a> {
    inner: std::slice::Iter<'a, LabelXConfig>,
}

impl<'a> Iterator for LabelXIterator<'a> {
    type Item = &'a LabelXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct LabelXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl LabelXCache {
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
pub struct LabelXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl LabelXFormatter {
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

    pub fn format_entry(&self, entry: &LabelXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &LabelXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &LabelXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for LabelXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct LabelXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl LabelXValidator {
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

    pub fn validate(&self, entry: &LabelXConfig) -> Result<(), Vec<String>> {
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

    pub fn validate_all(&self, registry: &LabelXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for LabelXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// Label rendering and icon resolution — extended utilities (xn)
// ---------------------------------------------------------------------------

/// Metric accumulator for label operations.
#[derive(Debug, Clone)]
pub struct XnMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XnMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for label.
#[derive(Debug, Clone)]
pub struct XnRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XnRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for label lookups.
#[derive(Debug, Clone)]
pub struct XnLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XnLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 33
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer33 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer33 {
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
pub fn xb_fnv1a_33(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_33<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_33<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_33(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_33(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 106
// ---------------------------------------------------------------------------

/// Generic object pool `Xc106Pool<T>`.
pub struct Xc106Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc106Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc106PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc106Pool<T> {
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
    pub fn stats(&self) -> Xc106PoolStats {
        Xc106PoolStats {
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

impl<T> Default for Xc106Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc106Scheduler`.
pub struct Xc106Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc106Scheduler {
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

impl Default for Xc106Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_106 hash for the given byte slice.
pub fn xc_106_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_106 convention.
pub fn xc_106_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe45 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe45Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe45PipelineError {
    pub stage: Xe45Stage,
    pub message: String,
}

impl std::fmt::Display for Xe45PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe45Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe45Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError>>>,
    stage_names: Vec<Xe45Stage>,
}

impl Xe45Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe45Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe45Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe45Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe45Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
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

    pub fn compose(mut self, other: Xe45Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe45CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe45CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe45Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe45CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe45CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe45Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe45CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_45_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe45CacheEntry {
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

    fn xe_45_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe45CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_45_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
    Ok(data)
}

pub fn xe_45_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_45_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_45_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_45_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe45PipelineError> {
    Err(Xe45PipelineError {
        stage: Xe45Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_14: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg14Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg14Graph {
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

impl Default for Xg14Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_14: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg14Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg14Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg14Heap<T>) {
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

impl<T: Ord> Default for Xg14Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 105).
pub struct Xh105SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh105SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 147 as u64,
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

/// A compact bit set supporting boolean operations (variant 105).
pub struct Xh105BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh105BitSet {
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

    #[test]
    fn format_label_replaces_variables() {
        let fmt = LabelFormat {
            pattern: "${filename} — ${dirname}".to_string(),
        };
        let result = format_label("/home/user/project/main.rs", &fmt);
        assert_eq!(result, "main.rs — /home/user/project");
    }

    #[test]
    fn extract_filename_and_extension() {
        assert_eq!(extract_filename("/foo/bar/baz.txt"), "baz.txt");
        assert_eq!(extract_extension("/foo/bar/baz.txt"), Some("txt"));
        assert_eq!(extract_filename("nopath"), "nopath");
        assert_eq!(extract_extension("noext"), None);
    }

    #[test]
    fn shorten_path_long() {
        let path = "/home/user/projects/deeply/nested/file.rs";
        let short = shorten_path(path, 20);
        assert!(short.contains("..."));
        assert!(short.len() < path.len());
    }

    #[test]
    fn shorten_path_already_short() {
        assert_eq!(shorten_path("a/b.rs", 50), "a/b.rs");
    }

    #[test]
    fn format_file_label_short() {
        assert_eq!(format_file_label("/a/b/c.rs", LabelDetail::Short), "c.rs");
    }

    #[test]
    fn format_file_label_medium() {
        let label = format_file_label("/home/user/project/main.rs", LabelDetail::Medium);
        assert!(label.contains("main.rs"));
        assert!(label.contains("project"));
    }

    #[test]
    fn format_file_label_full() {
        let label = format_file_label("/a/b.rs", LabelDetail::Full);
        assert!(label.contains("[rs]"));
        assert!(label.contains("b.rs"));
    }

    #[test]
    fn format_workspace_label_with_path() {
        assert_eq!(
            format_workspace_label("my-project", Some("/home/user/project")),
            "my-project (/home/user/project)"
        );
    }

    #[test]
    fn format_workspace_label_without_path() {
        assert_eq!(format_workspace_label("my-project", None), "my-project");
    }

    #[test]
    fn highlight_label_basic() {
        let segments = highlight_label("main.rs", "mn");
        let highlighted: String = segments
            .iter()
            .filter(|s| s.highlighted)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(highlighted, "mn");
    }

    #[test]
    fn highlight_label_no_match() {
        let segments = highlight_label("hello", "xyz");
        assert_eq!(segments.len(), 1);
        assert!(!segments[0].highlighted);
    }

    #[test]
    fn highlight_label_empty_query() {
        let segments = highlight_label("hello", "");
        assert_eq!(segments.len(), 1);
        assert!(!segments[0].highlighted);
    }

    // --- New tests ---

    #[test]
    fn validate_format_ok() {
        let fmt = LabelFormat {
            pattern: "${filename}".to_string(),
        };
        assert!(validate_format(&fmt).is_ok());
    }

    #[test]
    fn validate_format_no_placeholders() {
        let fmt = LabelFormat {
            pattern: "just text".to_string(),
        };
        assert_eq!(validate_format(&fmt), Err(LabelError::NoPlaceholders));
    }

    #[test]
    fn extract_stem_basic() {
        assert_eq!(extract_stem("/foo/bar/baz.txt"), "baz");
        assert_eq!(extract_stem("noext"), "noext");
        assert_eq!(extract_stem("/a/.hidden"), ".hidden");
    }

    #[test]
    fn path_depth_counts_segments() {
        assert_eq!(path_depth(""), 0);
        assert_eq!(path_depth("file.rs"), 1);
        assert_eq!(path_depth("/home/user/file.rs"), 3);
        assert_eq!(path_depth("/a/b/c/d/"), 4);
    }

    #[test]
    fn common_path_prefix_basic() {
        let paths = vec!["/home/user/a/foo.rs", "/home/user/b/bar.rs"];
        assert_eq!(common_path_prefix(&paths), "/home/user/");
    }

    #[test]
    fn common_path_prefix_empty() {
        assert_eq!(common_path_prefix(&[]), "");
    }

    #[test]
    fn common_path_prefix_no_shared() {
        let paths = vec!["alpha/one.rs", "beta/two.rs"];
        assert_eq!(common_path_prefix(&paths), "");
    }

    #[test]
    fn strip_prefix_removes_prefix() {
        assert_eq!(strip_prefix("/home/user/file.rs", "/home/user/"), "file.rs");
        assert_eq!(strip_prefix("file.rs", "/nope/"), "file.rs");
    }

    #[test]
    fn resource_label_builder_success() {
        let label = ResourceLabel::builder()
            .path("/src/main.rs")
            .description("Entry point")
            .icon("rs-icon")
            .build()
            .unwrap();
        assert_eq!(label.name, "main.rs");
        assert_eq!(label.description.as_deref(), Some("Entry point"));
        assert_eq!(label.icon.as_deref(), Some("rs-icon"));
    }

    #[test]
    fn resource_label_builder_empty_path_error() {
        let result = ResourceLabel::builder().path("").build();
        assert_eq!(result, Err(LabelError::EmptyPath));
    }

    #[test]
    fn resource_label_builder_no_path_error() {
        let result = ResourceLabel::builder().name("test").build();
        assert_eq!(result, Err(LabelError::EmptyPath));
    }

    #[test]
    fn resource_label_display() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/src/main.rs".into(),
            description: Some("Rust source".into()),
            icon: None,
        };
        assert_eq!(format!("{label}"), "main.rs — Rust source");

        let no_desc = ResourceLabel {
            name: "lib.rs".into(),
            path: "/src/lib.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(format!("{no_desc}"), "lib.rs");
    }

    #[test]
    fn resource_label_helpers() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/home/user/src/main.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(label.extension(), Some("rs"));
        assert_eq!(label.parent_dir(), "/home/user/src");
        assert_eq!(label.format(LabelDetail::Short), "main.rs");
    }

    #[test]
    fn label_error_display() {
        assert_eq!(
            LabelError::EmptyPath.to_string(),
            "path must not be empty"
        );
        assert_eq!(
            LabelError::NameTooLong { max: 10, actual: 20 }.to_string(),
            "name length 20 exceeds maximum 10"
        );
        assert_eq!(
            LabelError::UnknownPlaceholder("${bad}".into()).to_string(),
            "unknown placeholder: ${bad}"
        );
    }

    #[test]
    fn label_detail_display() {
        assert_eq!(LabelDetail::Short.to_string(), "short");
        assert_eq!(LabelDetail::Full.to_string(), "full");
    }

    #[test]
    fn label_format_display() {
        let fmt = LabelFormat {
            pattern: "${filename}".to_string(),
        };
        assert_eq!(format!("{fmt}"), "LabelFormat(${filename})");
    }

    #[test]
    fn label_stats_new_defaults() {
        let stats = LabelStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn label_stats_record_success() {
        let mut stats = LabelStats::new();
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
    fn label_stats_record_failure() {
        let mut stats = LabelStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn label_stats_reset() {
        let mut stats = LabelStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn label_stats_merge() {
        let mut a = LabelStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = LabelStats::new();
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
    fn label_stats_display() {
        let mut stats = LabelStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn label_stats_default() {
        let stats = LabelStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn label_validator_accepts_valid_name() {
        let v = LabelValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn label_validator_rejects_empty() {
        let v = LabelValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn label_validator_rejects_too_long() {
        let v = LabelValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn label_validator_forbidden_prefix() {
        let v = LabelValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn label_validator_allowed_chars() {
        let v = LabelValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn label_validator_range() {
        let v = LabelValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn label_sanitize_removes_control() {
        let result = LabelValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn label_truncate_short_string() {
        assert_eq!(LabelValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn label_truncate_long_string() {
        let result = LabelValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn label_is_ascii_printable() {
        assert!(LabelValidator::is_ascii_printable("Hello World 123"));
        assert!(!LabelValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn icon_label_text_only() {
        let label = IconLabel::text_only("main.rs");
        assert_eq!(label.display_string(), "main.rs");
        assert!(!label.has_icon());
    }

    #[test]
    fn icon_label_with_icon() {
        let label = IconLabel::with_icon("main.rs", "file-code");
        assert_eq!(label.display_string(), "$(file-code) main.rs");
        assert!(label.has_icon());
    }

    #[test]
    fn icon_label_with_description() {
        let label = IconLabel::with_icon("main.rs", "file-code")
            .with_description("Rust source");
        assert_eq!(label.display_string(), "$(file-code) main.rs — Rust source");
    }

    #[test]
    fn label_highlight_from_query() {
        let hl = LabelHighlight::from_query("main.rs", "mn");
        assert!(hl.has_match());
        assert_eq!(hl.highlighted_text(), "mn");
        assert_eq!(hl.plain_text(), "main.rs");
    }

    #[test]
    fn label_highlight_no_match() {
        let hl = LabelHighlight::from_query("hello", "xyz");
        assert!(!hl.has_match());
        assert_eq!(hl.highlight_count(), 0);
    }

    #[test]
    fn label_ellipsis_short() {
        assert_eq!(label_ellipsis("hello", 10), "hello");
    }

    #[test]
    fn label_ellipsis_truncates() {
        let result = label_ellipsis("hello world foo bar", 10);
        assert_eq!(result, "hello w...");
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn label_ellipsis_middle_truncates() {
        let result = label_ellipsis_middle("abcdefghij", 7);
        assert_eq!(result, "ab...ij");
    }

    #[test]
    fn resource_label_is_file() {
        let file_label = ResourceLabel {
            name: "main.rs".into(),
            path: "/src/main.rs".into(),
            description: None,
            icon: None,
        };
        assert!(file_label.is_file());

        let dir_label = ResourceLabel {
            name: "src".into(),
            path: "/home/user/src".into(),
            description: None,
            icon: None,
        };
        assert!(!dir_label.is_file());
    }

    #[test]
    fn resource_label_full_path() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/src/main.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(label.full_path(), "/src/main.rs");

        let label2 = ResourceLabel {
            name: "lib.rs".into(),
            path: "/src".into(),
            description: None,
            icon: None,
        };
        assert_eq!(label2.full_path(), "/src/lib.rs");
    }

    #[test]
    fn resource_label_with_icon_builder() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/src/main.rs".into(),
            description: None,
            icon: None,
        }
        .with_icon("file-code".into());
        assert_eq!(label.icon.as_deref(), Some("file-code"));
    }

    #[test]
    fn label_segment_is_highlight() {
        let hl = LabelSegment { text: "foo".into(), highlighted: true };
        assert!(hl.is_highlight());
        let plain = LabelSegment { text: "bar".into(), highlighted: false };
        assert!(!plain.is_highlight());
    }

    #[test]
    fn truncate_label_with_custom_ellipsis() {
        assert_eq!(truncate_label("hello", 10, ".."), "hello");
        assert_eq!(truncate_label("hello world", 7, ".."), "hello..");
        assert_eq!(truncate_label("abcdef", 3, ">>>"), "abc");
        assert_eq!(truncate_label("abcdefghij", 6, "---"), "abc---");
    }

    #[test]
    fn format_size_label_various() {
        assert_eq!(format_size_label(0), "0 B");
        assert_eq!(format_size_label(512), "512 B");
        assert_eq!(format_size_label(1024), "1.0 KB");
        assert_eq!(format_size_label(1536), "1.5 KB");
        assert_eq!(format_size_label(1048576), "1.0 MB");
        assert_eq!(format_size_label(1073741824), "1.0 GB");
        assert_eq!(format_size_label(1099511627776), "1.0 TB");
    }

    #[test]
    fn format_count_label_singular_and_plural() {
        assert_eq!(format_count_label(0, "file", "files"), "0 files");
        assert_eq!(format_count_label(1, "file", "files"), "1 file");
        assert_eq!(format_count_label(5, "error", "errors"), "5 errors");
    }

    #[test]
    fn sanitize_label_removes_control_chars() {
        assert_eq!(sanitize_label("hello\x00world"), "helloworld");
        assert_eq!(sanitize_label("line1\nline2\ttab"), "line1line2tab");
        assert_eq!(sanitize_label("clean string"), "clean string");
        assert_eq!(sanitize_label("\x07bell\x1b[31m"), "bell[31m");
    }

    // -- LabelTemplate tests ------------------------------------------------

    #[test]
    fn template_renders_placeholders() {
        let tmpl = LabelTemplate::new("{name} — {dir}");
        assert_eq!(tmpl.placeholder_names(), &["name", "dir"]);

        let mut vals = std::collections::HashMap::new();
        vals.insert("name".into(), "main.rs".into());
        vals.insert("dir".into(), "src".into());
        assert_eq!(tmpl.render(&vals), "main.rs — src");
        assert!(tmpl.is_complete(&vals));
    }

    #[test]
    fn template_missing_values_left_as_is() {
        let tmpl = LabelTemplate::new("{x} + {y}");
        let mut vals = std::collections::HashMap::new();
        vals.insert("x".into(), "hello".into());
        assert_eq!(tmpl.render(&vals), "hello + {y}");
        assert!(!tmpl.is_complete(&vals));
    }

    // -- LabelLocalizer tests -----------------------------------------------

    #[test]
    fn localizer_lookup_and_fallback() {
        let mut loc = LabelLocalizer::new("en");
        loc.add("file", "File");
        loc.add("edit", "Edit");
        assert_eq!(loc.get("file"), "File");
        assert_eq!(loc.get("unknown"), "unknown"); // fallback to key
        assert_eq!(loc.count(), 2);
        assert_eq!(loc.fallback_locale(), "en");
    }

    // -- LabelCache tests ---------------------------------------------------

    #[test]
    fn cache_stores_and_invalidates() {
        let mut cache = LabelCache::new();
        assert!(cache.is_empty());
        let label = cache.get_or_insert("/home/user/main.rs", LabelDetail::Short).to_string();
        assert_eq!(label, "main.rs");
        assert_eq!(cache.len(), 1);

        cache.invalidate("/home/user/main.rs");
        assert!(cache.is_empty());
    }

    // -- ResourceLabel comparison tests -------------------------------------

    #[test]
    fn resource_label_sorting_by_name() {
        let mut labels = vec![
            ResourceLabel { name: "zebra.rs".into(), path: "/z".into(), description: None, icon: None },
            ResourceLabel { name: "alpha.rs".into(), path: "/a".into(), description: None, icon: None },
            ResourceLabel { name: "middle.rs".into(), path: "/m".into(), description: None, icon: None },
        ];
        sort_labels_by_name(&mut labels);
        assert_eq!(labels[0].name, "alpha.rs");
        assert_eq!(labels[2].name, "zebra.rs");
    }

    #[test]
    fn resource_label_matches_query() {
        let label = ResourceLabel {
            name: "MyComponent.tsx".into(),
            path: "/src/MyComponent.tsx".into(),
            description: None,
            icon: None,
        };
        assert!(label.matches_query("mycomp"));
        assert!(label.matches_query("COMPONENT"));
        assert!(!label.matches_query("xyz"));
    }

    #[test]
    fn filter_labels_by_ext() {
        let labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.ts".into(), path: "/b.ts".into(), description: None, icon: None },
            ResourceLabel { name: "c.RS".into(), path: "/c.RS".into(), description: None, icon: None },
        ];
        let rs = filter_labels_by_extension(&labels, "rs");
        assert_eq!(rs.len(), 2);
    }

    #[test]
    fn dedup_labels_removes_duplicates() {
        let mut labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "a.rs".into(), path: "/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.rs".into(), path: "/b.rs".into(), description: None, icon: None },
        ];
        dedup_labels_by_path(&mut labels);
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn max_label_name_length_works() {
        let labels = vec![
            ResourceLabel { name: "ab".into(), path: "/ab".into(), description: None, icon: None },
            ResourceLabel { name: "abcde".into(), path: "/abcde".into(), description: None, icon: None },
        ];
        assert_eq!(max_label_name_length(&labels), 5);
        assert_eq!(max_label_name_length(&[]), 0);
    }

    #[test]
    fn extension_histogram_counts() {
        let labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.rs".into(), path: "/b.rs".into(), description: None, icon: None },
            ResourceLabel { name: "c.ts".into(), path: "/c.ts".into(), description: None, icon: None },
        ];
        let hist = extension_histogram(&labels);
        assert_eq!(hist.get("rs"), Some(&2));
        assert_eq!(hist.get("ts"), Some(&1));
    }

    #[test]
    fn group_labels_by_dir_groups() {
        let labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/src/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.rs".into(), path: "/src/b.rs".into(), description: None, icon: None },
            ResourceLabel { name: "c.rs".into(), path: "/lib/c.rs".into(), description: None, icon: None },
        ];
        let grouped = group_labels_by_dir(&labels);
        assert_eq!(grouped.get("/src").unwrap().len(), 2);
        assert_eq!(grouped.get("/lib").unwrap().len(), 1);
    }

    #[test]
    fn resource_label_has_extension() {
        let label = ResourceLabel { name: "test.RS".into(), path: "/test.RS".into(), description: None, icon: None };
        assert!(label.has_extension("rs"));
        assert!(!label.has_extension("ts"));
    }

    #[test]
    fn resource_label_stem_and_compact() {
        let label = ResourceLabel { name: "file.txt".into(), path: "/home/file.txt".into(), description: None, icon: None };
        assert_eq!(label.stem(), "file");
        assert_eq!(label.compact_display(), "file.txt (/home)");
    }

    #[test]
    fn normalize_path_separators_basic() {
        assert_eq!(normalize_path_separators("//a///b//"), "/a/b");
        assert_eq!(normalize_path_separators("/a/b/c"), "/a/b/c");
        assert_eq!(normalize_path_separators("/"), "/");
        assert_eq!(normalize_path_separators(""), "");
    }

    // -- LabelFormat additional methods ------------------------------------

    #[test]
    fn label_format_new_and_used_placeholders() {
        let fmt = LabelFormat::new("${filename} in ${dirname}");
        assert_eq!(fmt.used_placeholders(), vec!["${filename}", "${dirname}"]);
        assert!(fmt.uses("${filename}"));
        assert!(!fmt.uses("${extname}"));
    }

    // -- LabelDetail verbosity & parsing -----------------------------------

    #[test]
    fn label_detail_verbosity_ordering() {
        assert!(LabelDetail::Short.verbosity() < LabelDetail::Medium.verbosity());
        assert!(LabelDetail::Medium.verbosity() < LabelDetail::Long.verbosity());
        assert!(LabelDetail::Long.verbosity() < LabelDetail::Full.verbosity());
    }

    #[test]
    fn label_detail_from_str_loose() {
        assert_eq!(LabelDetail::from_str_loose("short"), Some(LabelDetail::Short));
        assert_eq!(LabelDetail::from_str_loose("MED"), Some(LabelDetail::Medium));
        assert_eq!(LabelDetail::from_str_loose("FULL"), Some(LabelDetail::Full));
        assert_eq!(LabelDetail::from_str_loose("bogus"), None);
    }

    // -- LabelSegment constructors & char_count ----------------------------

    #[test]
    fn label_segment_constructors_and_char_count() {
        let p = LabelSegment::plain("hello");
        assert!(!p.highlighted);
        assert_eq!(p.char_count(), 5);

        let h = LabelSegment::highlight("wörld");
        assert!(h.highlighted);
        assert_eq!(h.char_count(), 5);
    }

    // -- ResourceLabel depth, is_under, relative_path ----------------------

    #[test]
    fn resource_label_depth_and_under() {
        let label = ResourceLabel {
            name: "main.rs".into(),
            path: "/home/user/src/main.rs".into(),
            description: None,
            icon: None,
        };
        assert_eq!(label.depth(), 4);
        assert!(label.is_under("/home/user/"));
        assert!(!label.is_under("/tmp/"));
        assert_eq!(label.relative_path("/home/user/"), "src/main.rs");
    }

    #[test]
    fn resource_label_highlight_name() {
        let label = ResourceLabel {
            name: "Cargo.toml".into(),
            path: "/project/Cargo.toml".into(),
            description: None,
            icon: None,
        };
        let hl = label.highlight_name("carg");
        assert!(hl.has_match());
        assert_eq!(hl.highlighted_text(), "Carg");
    }

    // -- filter_labels_under & join_label_names ----------------------------

    #[test]
    fn filter_labels_under_prefix() {
        let labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/src/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.rs".into(), path: "/lib/b.rs".into(), description: None, icon: None },
            ResourceLabel { name: "c.rs".into(), path: "/src/sub/c.rs".into(), description: None, icon: None },
        ];
        let under_src = filter_labels_under(&labels, "/src/");
        assert_eq!(under_src.len(), 2);
    }

    #[test]
    fn join_label_names_produces_csv() {
        let labels = vec![
            ResourceLabel { name: "a.rs".into(), path: "/a.rs".into(), description: None, icon: None },
            ResourceLabel { name: "b.rs".into(), path: "/b.rs".into(), description: None, icon: None },
        ];
        assert_eq!(join_label_names(&labels, ", "), "a.rs, b.rs");
        assert_eq!(join_label_names(&[], "; "), "");
    }

    // -- LabelFormatter tests -----------------------------------------------

    #[test]
    fn test_formatter_basic() {
        let mut f = LabelFormatter::new("Hello, ${name}!");
        f.set("name", "World");
        assert_eq!(f.format(), "Hello, World!");
        assert!(f.has_variable("name"));
        assert_eq!(f.variable_count(), 1);
    }

    #[test]
    fn test_formatter_missing_var() {
        let f = LabelFormatter::new("${greeting}, ${name}!");
        assert_eq!(f.format(), "${greeting}, ${name}!");
    }

    #[test]
    fn test_formatter_clear() {
        let mut f = LabelFormatter::new("${a}");
        f.set("a", "1");
        assert_eq!(f.variable_count(), 1);
        f.clear();
        assert_eq!(f.variable_count(), 0);
        assert_eq!(f.format(), "${a}");
    }

    // -- LabelTruncator tests -----------------------------------------------

    #[test]
    fn test_truncator_end() {
        let t = LabelTruncator::new(5, EllipsisPosition::End);
        assert_eq!(t.truncate("abcdefgh"), "abcd…");
    }

    #[test]
    fn test_truncator_start() {
        let t = LabelTruncator::new(5, EllipsisPosition::Start);
        assert_eq!(t.truncate("abcdefgh"), "…efgh");
    }

    #[test]
    fn test_truncator_middle() {
        let t = LabelTruncator::new(5, EllipsisPosition::Middle);
        let result = t.truncate("abcdefgh");
        assert!(result.contains('…'));
        assert_eq!(result.chars().count(), 5);
    }

    #[test]
    fn test_truncator_no_truncation() {
        let t = LabelTruncator::new(10, EllipsisPosition::End);
        assert_eq!(t.truncate("short"), "short");
        assert!(!t.needs_truncation("short"));
    }

    // -- LabelHighlighter tests ---------------------------------------------

    #[test]
    fn test_highlighter_case_sensitive() {
        let h = LabelHighlighter::new(true);
        assert!(h.has_match("Hello World", "World"));
        assert!(!h.has_match("Hello World", "world"));
        assert_eq!(h.match_count("aaa", "a"), 3);
    }

    #[test]
    fn test_highlighter_case_insensitive() {
        let h = LabelHighlighter::new(false);
        assert!(h.has_match("Hello World", "world"));
        let spans = h.highlight("Hello World", "world");
        assert_eq!(spans.len(), 2);
        assert!(!spans[0].matched);
        assert!(spans[1].matched);
    }

    #[test]
    fn test_highlighter_no_match() {
        let h = LabelHighlighter::new(true);
        assert!(!h.has_match("abc", "xyz"));
        let spans = h.highlight("abc", "xyz");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].matched);
    }

    // -- LabelIconResolver tests --------------------------------------------

    #[test]
    fn test_icon_resolver_known() {
        let mut r = LabelIconResolver::new("file");
        r.register("rs", "rust-icon");
        assert_eq!(r.resolve("main.rs"), "rust-icon");
    }

    #[test]
    fn test_icon_resolver_default() {
        let r = LabelIconResolver::new("file");
        assert_eq!(r.resolve("readme"), "file");
        assert_eq!(r.resolve("data.xyz"), "file");
    }

    #[test]
    fn test_icon_resolver_register() {
        let mut r = LabelIconResolver::new("file");
        r.register("py", "python").register("js", "javascript");
        assert_eq!(r.icon_count(), 2);
        assert!(r.has_icon("py"));
        assert!(!r.has_icon("go"));
    }

    #[test]
    fn template_engine_variable_substitution() {
        let mut engine = LabelTemplateEngine::new();
        engine.set_var("name", "file.rs");
        let result = engine.render("Opening ${name}");
        assert_eq!(result, "Opening file.rs");
    }

    #[test]
    fn template_engine_missing_variable() {
        let engine = LabelTemplateEngine::new();
        let result = engine.render("Hello ${name}!");
        assert_eq!(result, "Hello !");
    }

    #[test]
    fn template_engine_conditional_true() {
        let mut engine = LabelTemplateEngine::new();
        engine.set_var("modified", "yes");
        let result = engine.render("File ${modified?[modified]:[clean]}");
        assert_eq!(result, "File [modified]");
    }

    #[test]
    fn template_engine_conditional_false() {
        let engine = LabelTemplateEngine::new();
        let result = engine.render("Status: ${modified?dirty:clean}");
        assert_eq!(result, "Status: clean");
    }

    #[test]
    fn template_engine_multiple_vars() {
        let mut engine = LabelTemplateEngine::new();
        engine.set_var("dir", "src").set_var("file", "main.rs");
        let result = engine.render("${dir}/${file}");
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn template_engine_set_overwrites() {
        let mut engine = LabelTemplateEngine::new();
        engine.set_var("x", "1");
        engine.set_var("x", "2");
        assert_eq!(engine.get_var("x"), Some("2"));
        assert_eq!(engine.var_count(), 1);
    }

    #[test]
    fn template_engine_remove_var() {
        let mut engine = LabelTemplateEngine::new();
        engine.set_var("x", "1");
        assert!(engine.remove_var("x"));
        assert!(!engine.remove_var("x"));
        assert_eq!(engine.var_count(), 0);
    }

    #[test]
    fn template_engine_compile_and_rerender() {
        let mut engine = LabelTemplateEngine::new();
        let compiled = engine.compile("Hello ${name}!");
        engine.set_var("name", "Alice");
        assert_eq!(engine.render_compiled(&compiled), "Hello Alice!");
        engine.set_var("name", "Bob");
        assert_eq!(engine.render_compiled(&compiled), "Hello Bob!");
    }

    #[test]
    fn accessibility_builder_basic() {
        let mut builder = LabelAccessibilityTextBuilder::new();
        builder.add("File").add("main.rs");
        assert_eq!(builder.build(), "File, main.rs");
    }

    #[test]
    fn accessibility_builder_with_role() {
        let mut builder = LabelAccessibilityTextBuilder::new();
        builder.add_with_role("Save", "button");
        assert_eq!(builder.build(), "Save (button)");
    }

    #[test]
    fn accessibility_builder_prefix_suffix() {
        let mut builder = LabelAccessibilityTextBuilder::new()
            .with_prefix("Start")
            .with_suffix("End");
        builder.add("Middle");
        let result = builder.build();
        assert!(result.starts_with("Start"));
        assert!(result.ends_with("End"));
        assert!(result.contains("Middle"));
    }

    #[test]
    fn accessibility_builder_aria() {
        let mut builder = LabelAccessibilityTextBuilder::new();
        builder.add("Close");
        assert_eq!(builder.to_aria_label(), "aria-label=\"Close\"");
    }

    #[test]
    fn accessibility_builder_property() {
        let mut builder = LabelAccessibilityTextBuilder::new();
        builder.add_property("Type", "File");
        assert_eq!(builder.build(), "Type: File");
        assert_eq!(builder.part_count(), 1);
    }



    #[test]
    fn label_x_config_new() {
        let c = LabelXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn label_x_config_builder() {
        let c = LabelXConfig::new("k")
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
    fn label_x_config_display() {
        let c = LabelXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn label_x_registry_insert_get() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn label_x_registry_duplicate() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a")).unwrap();
        assert!(reg.insert(LabelXConfig::new("a")).is_err());
    }

    #[test]
    fn label_x_registry_remove() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a")).unwrap();
        reg.insert(LabelXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn label_x_registry_active_entries() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a")).unwrap();
        reg.insert(LabelXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn label_x_registry_by_weight() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(LabelXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn label_x_registry_tags() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(LabelXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn label_x_registry_total_weight() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(LabelXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn label_x_registry_iterator() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a")).unwrap();
        reg.insert(LabelXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn label_x_cache_put_get() {
        let mut cache = LabelXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn label_x_cache_eviction() {
        let mut cache = LabelXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn label_x_cache_lru_order() {
        let mut cache = LabelXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn label_x_cache_most_least_recent() {
        let mut cache = LabelXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn label_x_formatter_entry() {
        let e = LabelXConfig::new("k").with_value("v");
        let fmt = LabelXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn label_x_formatter_summary() {
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("a").with_weight(5)).unwrap();
        let fmt = LabelXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn label_x_validator_valid() {
        let v = LabelXValidator::new();
        let c = LabelXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn label_x_validator_empty_key() {
        let v = LabelXValidator::new();
        let c = LabelXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn label_x_validator_require_value() {
        let v = LabelXValidator::new().require_value(true);
        let c = LabelXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn label_x_validator_allowed_tags() {
        let v = LabelXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = LabelXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn label_x_validator_validate_all() {
        let v = LabelXValidator::new();
        let mut reg = LabelXRegistry::new();
        reg.insert(LabelXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn xn_metrics_empty() {
        let m = XnMetrics::new("label");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xn_metrics_record_and_mean() {
        let mut m = XnMetrics::new("label");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xn_metrics_min_max() {
        let mut m = XnMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xn_metrics_variance_and_std() {
        let mut m = XnMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xn_metrics_percentile() {
        let mut m = XnMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xn_metrics_merge() {
        let mut a = XnMetrics::new("a");
        a.record(1.0);
        let mut b = XnMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xn_metrics_reset() {
        let mut m = XnMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xn_rate_window_empty() {
        let rw = XnRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xn_rate_window_tick_and_rate() {
        let mut rw = XnRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xn_lru_cache_basic() {
        let mut c = XnLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xn_lru_cache_contains_and_keys() {
        let mut c = XnLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xn_lru_cache_remove() {
        let mut c = XnLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xn_metrics_sum() {
        let mut m = XnMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xn_metrics_label() {
        let m = XnMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xn_lru_cache_clear() {
        let mut c = XnLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_33_push_and_len() {
        let mut rb = super::XbRingBuffer33::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_33_overwrite() {
        let mut rb = super::XbRingBuffer33::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_33_get_out_of_bounds() {
        let rb = super::XbRingBuffer33::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_33_drain_all() {
        let mut rb = super::XbRingBuffer33::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_33_peek_front_back() {
        let mut rb = super::XbRingBuffer33::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_33_clear() {
        let mut rb = super::XbRingBuffer33::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_33_capacity() {
        let rb = super::XbRingBuffer33::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_33_basic() {
        let h = super::xb_fnv1a_33(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_33(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_33_different_inputs() {
        let h1 = super::xb_fnv1a_33(b"abc");
        let h2 = super::xb_fnv1a_33(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_33_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_33(&data);
        let dec = super::xb_rle_decode_33(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_33_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_33(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_33(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_33_values() {
        assert!((super::xb_clamp_33(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_33(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_33(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_33_values() {
        assert!((super::xb_lerp_33(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_33(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_33(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_33_wrap_around_twice() {
        let mut rb = super::XbRingBuffer33::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 106 ----

    #[test]
    fn xc_106_pool_new_empty() {
        let pool: super::Xc106Pool<i32> = super::Xc106Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_106_pool_release_acquire() {
        let mut pool = super::Xc106Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_106_pool_acquire_empty() {
        let mut pool: super::Xc106Pool<i32> = super::Xc106Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_106_pool_full() {
        let mut pool = super::Xc106Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_106_pool_drain() {
        let mut pool = super::Xc106Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_106_pool_stats() {
        let mut pool = super::Xc106Pool::new(8);
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
    fn xc_106_pool_clear() {
        let mut pool = super::Xc106Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_106_pool_shrink() {
        let mut pool = super::Xc106Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_106_pool_default() {
        let pool: super::Xc106Pool<String> = super::Xc106Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_106_pool_extend() {
        let mut pool = super::Xc106Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_106_pool_retain() {
        let mut pool = super::Xc106Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_106_scheduler_round_robin() {
        let mut sched = super::Xc106Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_106_scheduler_empty() {
        let mut sched = super::Xc106Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_106_scheduler_reset() {
        let mut sched = super::Xc106Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_106_scheduler_add_remove() {
        let mut sched = super::Xc106Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_106_scheduler_targets() {
        let sched = super::Xc106Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_106_hash_empty() {
        assert_eq!(super::xc_106_hash(b""), 5381);
    }

    #[test]
    fn xc_106_hash_data() {
        let h = super::xc_106_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_106_hash(b"hello"), h);
    }

    #[test]
    fn xc_106_reverse_str() {
        assert_eq!(super::xc_106_reverse("abc"), "cba");
        assert_eq!(super::xc_106_reverse(""), "");
    }


    #[test]
    fn xe_45_pipeline_empty() {
        let p = super::Xe45Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_45_pipeline_parse_stage() {
        let p = super::Xe45Pipeline::new()
            .add_parse(super::xe_45_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_45_pipeline_transform_double() {
        let p = super::Xe45Pipeline::new()
            .add_transform(super::xe_45_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_45_pipeline_validate_reverse() {
        let p = super::Xe45Pipeline::new()
            .add_validate(super::xe_45_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_45_pipeline_emit_filter() {
        let p = super::Xe45Pipeline::new()
            .add_emit(super::xe_45_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_45_pipeline_multi_stage() {
        let p = super::Xe45Pipeline::new()
            .add_parse(super::xe_45_pipeline_identity)
            .add_transform(super::xe_45_pipeline_double)
            .add_validate(super::xe_45_pipeline_reverse)
            .add_emit(super::xe_45_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_45_pipeline_error_propagation() {
        let p = super::Xe45Pipeline::new()
            .add_parse(super::xe_45_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe45Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_45_pipeline_compose() {
        let p1 = super::Xe45Pipeline::new()
            .add_parse(super::xe_45_pipeline_identity);
        let p2 = super::Xe45Pipeline::new()
            .add_transform(super::xe_45_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_45_pipeline_error_display() {
        let e = super::Xe45PipelineError {
            stage: super::Xe45Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_45_cache_put_get() {
        let mut c = super::Xe45Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_45_cache_miss() {
        let mut c: super::Xe45Cache<&str, i32> = super::Xe45Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_45_cache_ttl_expiry() {
        let mut c = super::Xe45Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_45_cache_evict() {
        let mut c = super::Xe45Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_45_cache_capacity() {
        let mut c = super::Xe45Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_45_cache_stats() {
        let mut c = super::Xe45Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_45_cache_clear() {
        let mut c = super::Xe45Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_14 graph tests ------------------------------------------------

    #[test]
    fn xg_14_graph_empty() {
        let g = super::Xg14Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_14_graph_add_node() {
        let mut g = super::Xg14Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_14_graph_add_edge() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_14_graph_neighbors() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_14_graph_has_path() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_14_graph_self_path() {
        let g = super::Xg14Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_14_graph_topo_sort() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_14_graph_cycle_detect_false() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_14_graph_cycle_detect_true() {
        let mut g = super::Xg14Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_14 heap tests -------------------------------------------------

    #[test]
    fn xg_14_heap_empty() {
        let h: super::Xg14Heap<i32> = super::Xg14Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_14_heap_push_pop() {
        let mut h = super::Xg14Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_14_heap_peek() {
        let mut h = super::Xg14Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_14_heap_drain_sorted() {
        let mut h = super::Xg14Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_14_heap_merge() {
        let mut a = super::Xg14Heap::new();
        let mut b = super::Xg14Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_14_heap_default() {
        let h: super::Xg14Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_14_graph_default() {
        let g: super::Xg14Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh105_skip_insert_contains() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh105_skip_remove() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh105_skip_len() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh105_skip_range_query() {
        let mut sl = super::Xh105SkipList::xh_new(4);
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
    fn xh105_skip_floor_ceiling() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh105_skip_rank() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh105_skip_empty() {
        let sl = super::Xh105SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh105_skip_duplicates() {
        let mut sl = super::Xh105SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh105_bitset_set_test() {
        let mut bs = super::Xh105BitSet::xh_new(256);
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
    fn xh105_bitset_clear_count() {
        let mut bs = super::Xh105BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh105_bitset_and_or_xor() {
        let mut a = super::Xh105BitSet::xh_new(128);
        let mut b = super::Xh105BitSet::xh_new(128);
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
    fn xh105_bitset_iter_ones() {
        let mut bs = super::Xh105BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh105_bitset_first_last() {
        let mut bs = super::Xh105BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh105_bitset_empty() {
        let bs = super::Xh105BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
