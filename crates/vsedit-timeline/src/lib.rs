//! Git-based timeline provider.
//!
//! Parses `git log` output to produce timeline items for a given file path.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;

// ── Core Types ──

/// A single timeline entry derived from a git commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    pub timestamp: u64,
    pub message: String,
    pub author: String,
    pub sha: String,
}

impl fmt::Display for TimelineItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({}) - {}", self.sha, self.message, self.author, self.timestamp)
    }
}

// ── Errors ──

/// Errors that can occur when building a timeline.
#[derive(Debug)]
pub enum TimelineError {
    /// The git command failed to execute.
    GitExecFailed(io::Error),
    /// The git command returned a non-zero exit code.
    GitFailed(String),
    /// A line from git log could not be parsed.
    ParseError(String),
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimelineError::GitExecFailed(e) => write!(f, "failed to execute git: {e}"),
            TimelineError::GitFailed(msg) => write!(f, "git error: {msg}"),
            TimelineError::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

// ── Parsing ──

const GIT_LOG_SEP: &str = "\x1f";

/// Parse a single line of `git log` output formatted as `<timestamp>\x1f<sha>\x1f<author>\x1f<message>`.
pub fn parse_git_log_line(line: &str) -> Result<TimelineItem, TimelineError> {
    let parts: Vec<&str> = line.split(GIT_LOG_SEP).collect();
    if parts.len() < 4 {
        return Err(TimelineError::ParseError(format!(
            "expected 4 fields separated by \\x1f, got {}: {:?}",
            parts.len(),
            line
        )));
    }
    let timestamp: u64 = parts[0]
        .trim()
        .parse()
        .map_err(|e| TimelineError::ParseError(format!("invalid timestamp '{}': {e}", parts[0])))?;
    let sha = parts[1].trim().to_string();
    let author = parts[2].trim().to_string();
    let message = parts[3..].join(GIT_LOG_SEP).trim().to_string();
    Ok(TimelineItem {
        timestamp,
        message,
        author,
        sha,
    })
}

/// Parse multi-line `git log` output into timeline items, skipping blank lines.
pub fn parse_git_log_output(output: &str) -> Vec<Result<TimelineItem, TimelineError>> {
    output
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(parse_git_log_line)
        .collect()
}

// ── GitTimelineProvider ──

/// Provides timeline items for a file by running `git log`.
#[derive(Debug, Clone)]
pub struct GitTimelineProvider {
    /// Working directory (repository root).
    pub repo_dir: String,
}

impl GitTimelineProvider {
    pub fn new(repo_dir: impl Into<String>) -> Self {
        Self {
            repo_dir: repo_dir.into(),
        }
    }

    /// Run `git log` for the given file path and return parsed timeline items.
    pub fn timeline_for_file(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
        let format_str = format!("%ct{sep}%H{sep}%an{sep}%s", sep = GIT_LOG_SEP);
        let output = Command::new("git")
            .args([
                "log",
                "--follow",
                &format!("--format={format_str}"),
                "--",
                path,
            ])
            .current_dir(&self.repo_dir)
            .output()
            .map_err(TimelineError::GitExecFailed)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(TimelineError::GitFailed(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let results = parse_git_log_output(&stdout);
        let mut items = Vec::with_capacity(results.len());
        for r in results {
            items.push(r?);
        }
        Ok(items)
    }
}

/// Convenience function to get timeline items for a file under the given repo directory.
pub fn timeline_for_file(repo_dir: impl AsRef<Path>, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
    let provider = GitTimelineProvider::new(repo_dir.as_ref().to_string_lossy().to_string());
    provider.timeline_for_file(path)
}

// ── Timeline Filtering ──

/// Filter criteria for timeline items.
#[derive(Debug, Clone, Default)]
pub struct TimelineFilter {
    /// Only include items within this timestamp range (inclusive).
    pub date_range: Option<(u64, u64)>,
    /// Only include items by this author (case-insensitive substring match).
    pub author: Option<String>,
    /// Only include items whose message contains this pattern.
    pub path_pattern: Option<String>,
}

impl TimelineFilter {
    /// Create an empty filter that matches everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the date range filter.
    pub fn with_date_range(mut self, min: u64, max: u64) -> Self {
        self.date_range = Some((min, max));
        self
    }

    /// Set the author filter.
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Set the path/message pattern filter.
    pub fn with_path_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.path_pattern = Some(pattern.into());
        self
    }

    /// Check whether a timeline item matches all active filter criteria.
    pub fn matches(&self, item: &TimelineItem) -> bool {
        if let Some((min, max)) = self.date_range {
            if item.timestamp < min || item.timestamp > max {
                return false;
            }
        }
        if let Some(ref author) = self.author {
            let item_author = item.author.to_lowercase();
            let filter_author = author.to_lowercase();
            if !item_author.contains(&filter_author) {
                return false;
            }
        }
        if let Some(ref pattern) = self.path_pattern {
            let msg = item.message.to_lowercase();
            let pat = pattern.to_lowercase();
            if !msg.contains(&pat) {
                return false;
            }
        }
        true
    }
}

impl fmt::Display for TimelineFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some((min, max)) = self.date_range {
            parts.push(format!("date=[{min}..{max}]"));
        }
        if let Some(ref author) = self.author {
            parts.push(format!("author={author}"));
        }
        if let Some(ref pattern) = self.path_pattern {
            parts.push(format!("pattern={pattern}"));
        }
        if parts.is_empty() {
            write!(f, "TimelineFilter(none)")
        } else {
            write!(f, "TimelineFilter({})", parts.join(", "))
        }
    }
}

/// Filter a slice of timeline items, returning only those that match the filter.
pub fn filter_items(items: &[TimelineItem], filter: &TimelineFilter) -> Vec<TimelineItem> {
    items.iter().filter(|item| filter.matches(item)).cloned().collect()
}

// ── Timeline Grouping ──

/// Grouping strategies for timeline items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineGrouping {
    /// Group by calendar day (86400-second buckets).
    Day,
    /// Group by calendar week (604800-second buckets).
    Week,
    /// Group by approximate month (2592000-second buckets).
    Month,
}

impl fmt::Display for TimelineGrouping {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Day => write!(f, "Day"),
            Self::Week => write!(f, "Week"),
            Self::Month => write!(f, "Month"),
        }
    }
}

/// Compute a group key for a timeline item based on the grouping strategy.
pub fn group_key(item: &TimelineItem, grouping: TimelineGrouping) -> String {
    match grouping {
        TimelineGrouping::Day => format!("day-{}", item.timestamp / 86400),
        TimelineGrouping::Week => format!("week-{}", item.timestamp / 604800),
        TimelineGrouping::Month => format!("month-{}", item.timestamp / 2592000),
    }
}

/// Group timeline items by the given strategy, returning (key, items) pairs sorted by key.
pub fn group_items(items: &[TimelineItem], grouping: TimelineGrouping) -> Vec<(String, Vec<TimelineItem>)> {
    let mut groups: HashMap<String, Vec<TimelineItem>> = HashMap::new();
    for item in items {
        let key = group_key(item, grouping);
        groups.entry(key).or_default().push(item.clone());
    }
    let mut result: Vec<(String, Vec<TimelineItem>)> = groups.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

// ── Timeline Diffing ──

/// Represents the difference between two timeline snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineDiff {
    /// Items present in `new` but not in `old`.
    pub added: Vec<TimelineItem>,
    /// Items present in `old` but not in `new`.
    pub removed: Vec<TimelineItem>,
    /// Items present in both but with different messages or authors.
    pub modified: Vec<TimelineItem>,
}

impl TimelineDiff {
    /// Returns true if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    /// Total number of changes.
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

impl fmt::Display for TimelineDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TimelineDiff(+{} added, -{} removed, ~{} modified)",
            self.added.len(),
            self.removed.len(),
            self.modified.len()
        )
    }
}

/// Compare two timeline snapshots, matching items by SHA.
pub fn diff_timelines(old: &[TimelineItem], new: &[TimelineItem]) -> TimelineDiff {
    let old_map: HashMap<&str, &TimelineItem> = old.iter().map(|i| (i.sha.as_str(), i)).collect();
    let new_map: HashMap<&str, &TimelineItem> = new.iter().map(|i| (i.sha.as_str(), i)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (sha, new_item) in &new_map {
        match old_map.get(sha) {
            None => added.push((*new_item).clone()),
            Some(old_item) => {
                if old_item.message != new_item.message || old_item.author != new_item.author {
                    modified.push((*new_item).clone());
                }
            }
        }
    }

    for (sha, old_item) in &old_map {
        if !new_map.contains_key(sha) {
            removed.push((*old_item).clone());
        }
    }

    TimelineDiff { added, removed, modified }
}

// ── Timeline Provider Trait ──

/// Trait for components that can provide timeline items for a file.
pub trait TimelineProvider {
    /// Human-readable name for this provider.
    fn name(&self) -> &str;

    /// Retrieve timeline items for the given file path.
    fn timeline_for_path(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError>;
}

impl TimelineProvider for GitTimelineProvider {
    fn name(&self) -> &str {
        "git"
    }

    fn timeline_for_path(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
        self.timeline_for_file(path)
    }
}

// ── FileChangeTimelineProvider ──

/// A timeline provider that tracks file modification timestamps via manual registration.
#[derive(Debug, Clone)]
pub struct FileChangeTimelineProvider {
    entries: HashMap<String, Vec<TimelineItem>>,
}

impl FileChangeTimelineProvider {
    /// Create a new empty provider.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a timeline item for a given file path.
    pub fn add_entry(&mut self, path: &str, item: TimelineItem) {
        self.entries.entry(path.to_string()).or_default().push(item);
    }

    /// Return the number of tracked file paths.
    pub fn tracked_paths(&self) -> usize {
        self.entries.len()
    }

    /// Return all tracked file path keys.
    pub fn paths(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for FileChangeTimelineProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FileChangeTimelineProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileChangeTimelineProvider({} paths)", self.entries.len())
    }
}

impl TimelineProvider for FileChangeTimelineProvider {
    fn name(&self) -> &str {
        "file-changes"
    }

    fn timeline_for_path(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
        Ok(self.entries.get(path).cloned().unwrap_or_default())
    }
}

// ── TimelineService ──

/// Aggregates timeline items from multiple providers.
///
/// The service collects items from all registered providers and merges
/// them into a single sorted timeline for a requested file path.
pub struct TimelineService {
    providers: Vec<Box<dyn TimelineProvider>>,
}

impl TimelineService {
    /// Create a new service with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a provider to this service.
    pub fn add_provider(&mut self, provider: Box<dyn TimelineProvider>) {
        self.providers.push(provider);
    }

    /// Return the names of all registered providers.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    /// Return the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Collect timeline items from all providers for the given file path,
    /// sorted by timestamp descending.
    pub fn all_items_for_file(&self, path: &str) -> Result<Vec<TimelineItem>, TimelineError> {
        let mut all_items = Vec::new();
        for provider in &self.providers {
            let items = provider.timeline_for_path(path)?;
            all_items.extend(items);
        }
        all_items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(all_items)
    }
}

impl Default for TimelineService {
    fn default() -> Self {
        Self::new()
    }
}

// ── Serialization ──

/// Serialize timeline items into a tab-separated string (one line per item).
///
/// Format: `<timestamp>\t<sha>\t<author>\t<message>`
pub fn serialize_items(items: &[TimelineItem]) -> String {
    let mut out = String::new();
    for item in items {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            item.timestamp, item.sha, item.author, item.message
        ));
    }
    out
}

/// Deserialize timeline items from the tab-separated format produced by [`serialize_items`].
pub fn deserialize_items(s: &str) -> Vec<Result<TimelineItem, TimelineError>> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 4 {
                return Err(TimelineError::ParseError(format!(
                    "expected 4 tab-separated fields, got {}: {:?}",
                    parts.len(),
                    line
                )));
            }
            let timestamp: u64 = parts[0].trim().parse().map_err(|e| {
                TimelineError::ParseError(format!("invalid timestamp '{}': {e}", parts[0]))
            })?;
            Ok(TimelineItem {
                timestamp,
                sha: parts[1].trim().to_string(),
                author: parts[2].trim().to_string(),
                message: parts[3..].join("\t").trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_line() {
        let line = "1700000000\x1fabc123\x1fAlice\x1fFix bug";
        let item = parse_git_log_line(line).unwrap();
        assert_eq!(item.timestamp, 1700000000);
        assert_eq!(item.sha, "abc123");
        assert_eq!(item.author, "Alice");
        assert_eq!(item.message, "Fix bug");
    }

    #[test]
    fn parse_line_with_separator_in_message() {
        let line = "1700000000\x1fabc123\x1fAlice\x1fFix\x1fbug";
        let item = parse_git_log_line(line).unwrap();
        assert_eq!(item.message, "Fix\x1fbug");
    }

    #[test]
    fn parse_invalid_line_too_few_fields() {
        let line = "1700000000\x1fabc123";
        assert!(parse_git_log_line(line).is_err());
    }

    #[test]
    fn parse_invalid_timestamp() {
        let line = "notanumber\x1fabc123\x1fAlice\x1fFix bug";
        assert!(parse_git_log_line(line).is_err());
    }

    #[test]
    fn parse_multi_line_output() {
        let output = "1700000000\x1fabc123\x1fAlice\x1fFirst commit\n\
                       1700000100\x1fdef456\x1fBob\x1fSecond commit\n";
        let results = parse_git_log_output(output);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[test]
    fn parse_multi_line_skips_blank() {
        let output = "\n1700000000\x1fabc123\x1fAlice\x1fCommit\n\n";
        let results = parse_git_log_output(output);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn timeline_item_display() {
        let item = TimelineItem {
            timestamp: 1700000000,
            message: "Fix bug".into(),
            author: "Alice".into(),
            sha: "abc123".into(),
        };
        let s = format!("{item}");
        assert!(s.contains("abc123"));
        assert!(s.contains("Fix bug"));
        assert!(s.contains("Alice"));
    }

    #[test]
    fn timeline_error_display() {
        let e = TimelineError::ParseError("bad line".into());
        assert!(format!("{e}").contains("parse error"));
        let e2 = TimelineError::GitFailed("not a repo".into());
        assert!(format!("{e2}").contains("git error"));
    }

    #[test]
    fn git_timeline_provider_new() {
        let provider = GitTimelineProvider::new("/tmp/repo");
        assert_eq!(provider.repo_dir, "/tmp/repo");
    }

    #[test]
    fn timeline_for_file_in_current_repo() {
        // Use the workspace root (two levels up from the crate dir) so we hit
        // a file that has git history.
        let crate_dir = std::env::current_dir().unwrap();
        let repo_root = crate_dir.join("../..").canonicalize().unwrap_or(crate_dir);
        let result = timeline_for_file(&repo_root, "Cargo.toml");
        match result {
            Ok(items) => {
                // The repo should have at least one commit touching Cargo.toml
                assert!(!items.is_empty());
                for item in &items {
                    assert!(!item.sha.is_empty());
                    assert!(!item.author.is_empty());
                    assert!(item.timestamp > 0);
                }
            }
            Err(TimelineError::GitExecFailed(_)) => {
                // git not available in CI – acceptable
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn timeline_for_nonexistent_file() {
        let crate_dir = std::env::current_dir().unwrap();
        let repo_root = crate_dir.join("../..").canonicalize().unwrap_or(crate_dir);
        let result = timeline_for_file(&repo_root, "this_file_does_not_exist_xyz.txt");
        match result {
            Ok(items) => assert!(items.is_empty()),
            Err(TimelineError::GitExecFailed(_)) => {} // git not available
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    // ── Filter tests ──

    fn sample_items() -> Vec<TimelineItem> {
        vec![
            TimelineItem { timestamp: 1700000000, message: "Fix bug in parser".into(), author: "Alice".into(), sha: "aaa111".into() },
            TimelineItem { timestamp: 1700100000, message: "Add tests".into(), author: "Bob".into(), sha: "bbb222".into() },
            TimelineItem { timestamp: 1700200000, message: "Refactor parser".into(), author: "Alice".into(), sha: "ccc333".into() },
            TimelineItem { timestamp: 1700300000, message: "Update docs".into(), author: "Charlie".into(), sha: "ddd444".into() },
            TimelineItem { timestamp: 1700400000, message: "Fix typo".into(), author: "Bob".into(), sha: "eee555".into() },
        ]
    }

    #[test]
    fn filter_by_date_range() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_date_range(1700100000, 1700300000);
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|i| i.timestamp >= 1700100000 && i.timestamp <= 1700300000));
    }

    #[test]
    fn filter_by_author() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_author("alice");
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|i| i.author == "Alice"));
    }

    #[test]
    fn filter_by_path_pattern() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_path_pattern("parser");
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_combined() {
        let items = sample_items();
        let filter = TimelineFilter::new()
            .with_author("alice")
            .with_path_pattern("parser");
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_empty_matches_all() {
        let items = sample_items();
        let filter = TimelineFilter::new();
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), items.len());
    }

    #[test]
    fn filter_no_matches() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_author("nobody");
        let result = filter_items(&items, &filter);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_display() {
        let filter = TimelineFilter::new().with_author("Alice").with_date_range(100, 200);
        let s = format!("{filter}");
        assert!(s.contains("author=Alice"));
        assert!(s.contains("date="));
    }

    #[test]
    fn filter_display_empty() {
        let filter = TimelineFilter::new();
        assert_eq!(format!("{filter}"), "TimelineFilter(none)");
    }

    // ── Grouping tests ──

    #[test]
    fn group_by_day() {
        let items = sample_items();
        let groups = group_items(&items, TimelineGrouping::Day);
        assert!(!groups.is_empty());
        for (key, _) in &groups {
            assert!(key.starts_with("day-"));
        }
    }

    #[test]
    fn group_by_week() {
        let items = sample_items();
        let groups = group_items(&items, TimelineGrouping::Week);
        assert!(!groups.is_empty());
        for (key, _) in &groups {
            assert!(key.starts_with("week-"));
        }
    }

    #[test]
    fn group_by_month() {
        let items = sample_items();
        let groups = group_items(&items, TimelineGrouping::Month);
        assert!(!groups.is_empty());
        for (key, _) in &groups {
            assert!(key.starts_with("month-"));
        }
    }

    #[test]
    fn group_key_day_value() {
        let item = TimelineItem { timestamp: 86400 * 5, message: "m".into(), author: "a".into(), sha: "s".into() };
        assert_eq!(group_key(&item, TimelineGrouping::Day), "day-5");
    }

    #[test]
    fn group_items_sorted_by_key() {
        let items = sample_items();
        let groups = group_items(&items, TimelineGrouping::Day);
        let keys: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys);
    }

    #[test]
    fn grouping_display() {
        assert_eq!(format!("{}", TimelineGrouping::Day), "Day");
        assert_eq!(format!("{}", TimelineGrouping::Week), "Week");
        assert_eq!(format!("{}", TimelineGrouping::Month), "Month");
    }

    // ── Diff tests ──

    #[test]
    fn diff_identical_timelines() {
        let items = sample_items();
        let diff = diff_timelines(&items, &items);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn diff_additions() {
        let old = vec![sample_items()[0].clone()];
        let new = sample_items();
        let diff = diff_timelines(&old, &new);
        assert_eq!(diff.added.len(), 4);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn diff_removals() {
        let old = sample_items();
        let new = vec![sample_items()[0].clone()];
        let diff = diff_timelines(&old, &new);
        assert_eq!(diff.removed.len(), 4);
        assert!(diff.added.is_empty());
    }

    #[test]
    fn diff_modifications() {
        let old = sample_items();
        let mut new = sample_items();
        new[0].message = "Modified message".into();
        let diff = diff_timelines(&old, &new);
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].sha, "aaa111");
    }

    #[test]
    fn diff_display() {
        let diff = TimelineDiff { added: vec![], removed: vec![], modified: vec![] };
        assert!(format!("{diff}").contains("+0 added"));
    }

    // ── FileChangeTimelineProvider tests ──

    #[test]
    fn file_change_provider_empty() {
        let provider = FileChangeTimelineProvider::new();
        assert_eq!(provider.tracked_paths(), 0);
        let items = provider.timeline_for_path("any").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn file_change_provider_add_and_retrieve() {
        let mut provider = FileChangeTimelineProvider::new();
        let item = TimelineItem { timestamp: 100, message: "saved".into(), author: "editor".into(), sha: "local-1".into() };
        provider.add_entry("src/main.rs", item.clone());
        assert_eq!(provider.tracked_paths(), 1);
        let items = provider.timeline_for_path("src/main.rs").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], item);
    }

    #[test]
    fn file_change_provider_display() {
        let provider = FileChangeTimelineProvider::new();
        let s = format!("{provider}");
        assert!(s.contains("FileChangeTimelineProvider"));
    }

    #[test]
    fn file_change_provider_name() {
        let provider = FileChangeTimelineProvider::new();
        assert_eq!(provider.name(), "file-changes");
    }

    // ── TimelineService tests ──

    #[test]
    fn service_empty() {
        let service = TimelineService::new();
        assert_eq!(service.provider_count(), 0);
        let items = service.all_items_for_file("any").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn service_with_file_change_provider() {
        let mut provider = FileChangeTimelineProvider::new();
        provider.add_entry("f.rs", TimelineItem { timestamp: 200, message: "m2".into(), author: "a".into(), sha: "s2".into() });
        provider.add_entry("f.rs", TimelineItem { timestamp: 100, message: "m1".into(), author: "a".into(), sha: "s1".into() });
        let mut service = TimelineService::new();
        service.add_provider(Box::new(provider));
        assert_eq!(service.provider_count(), 1);
        assert_eq!(service.provider_names(), vec!["file-changes"]);
        let items = service.all_items_for_file("f.rs").unwrap();
        assert_eq!(items.len(), 2);
        // Should be sorted desc by timestamp
        assert!(items[0].timestamp >= items[1].timestamp);
    }

    // ── Serialization tests ──

    #[test]
    fn serialize_roundtrip() {
        let items = sample_items();
        let serialized = serialize_items(&items);
        let deserialized: Vec<TimelineItem> = deserialize_items(&serialized)
            .into_iter()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(items, deserialized);
    }

    #[test]
    fn serialize_empty() {
        let serialized = serialize_items(&[]);
        assert!(serialized.is_empty());
        let deserialized = deserialize_items(&serialized);
        assert!(deserialized.is_empty());
    }

    #[test]
    fn deserialize_invalid_line() {
        let bad = "not\tenough";
        let results = deserialize_items(bad);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn deserialize_invalid_timestamp() {
        let bad = "abc\tsha\tauthor\tmsg";
        let results = deserialize_items(bad);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn deserialize_skips_blank_lines() {
        let input = "\n100\ts1\ta1\tm1\n\n200\ts2\ta2\tm2\n\n";
        let results = deserialize_items(input);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}
