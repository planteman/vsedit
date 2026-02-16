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
}
