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

impl TimelineItem {
    /// Compute the age of this item in seconds relative to `now`.
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    /// Check whether this item was authored by `author` (case-insensitive).
    pub fn is_by_author(&self, author: &str) -> bool {
        self.author.eq_ignore_ascii_case(author)
    }
}

impl fmt::Display for TimelineItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.sha, self.author, self.message)
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

/// A snapshot of the timeline at a point in time, used for diffing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineSnapshot {
    /// Label describing when/why this snapshot was taken.
    pub label: String,
    /// Timestamp when the snapshot was captured.
    pub captured_at: u64,
    /// The timeline items at the time of capture.
    pub items: Vec<TimelineItem>,
}

impl TimelineSnapshot {
    /// Create a new snapshot with the given label and items.
    pub fn new(label: impl Into<String>, captured_at: u64, items: Vec<TimelineItem>) -> Self {
        Self {
            label: label.into(),
            captured_at,
            items,
        }
    }

    /// Return unique authors across all items in this snapshot.
    pub fn authors(&self) -> Vec<&str> {
        let mut seen = Vec::new();
        for item in &self.items {
            let a = item.author.as_str();
            if !seen.contains(&a) {
                seen.push(a);
            }
        }
        seen
    }

    /// Return the most recent item by timestamp, or `None` if empty.
    pub fn latest_item(&self) -> Option<&TimelineItem> {
        self.items.iter().max_by_key(|i| i.timestamp)
    }

    /// Return the oldest item by timestamp, or `None` if empty.
    pub fn oldest_item(&self) -> Option<&TimelineItem> {
        self.items.iter().min_by_key(|i| i.timestamp)
    }

    /// Return items that match the given filter.
    pub fn filter(&self, f: &TimelineFilter) -> Vec<&TimelineItem> {
        self.items.iter().filter(|item| f.matches(item)).collect()
    }

    /// Compute the difference between this snapshot and another.
    pub fn diff(&self, other: &TimelineSnapshot) -> TimelineDiff {
        diff_timelines(&self.items, &other.items)
    }

    /// Number of items in this snapshot.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the snapshot contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl fmt::Display for TimelineSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TimelineSnapshot('{}', {} items, captured_at={})",
            self.label,
            self.items.len(),
            self.captured_at
        )
    }
}

/// Compute the difference between two timeline snapshots.
pub fn timeline_diff(old: &TimelineSnapshot, new: &TimelineSnapshot) -> TimelineDiff {
    diff_timelines(&old.items, &new.items)
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
    /// Only include items from the given source/provider name.
    pub source: Option<String>,
    /// Only include items whose message or author matches any of the given labels.
    pub labels: Vec<String>,
}

impl TimelineFilter {
    /// Create an empty filter that matches everything.
    pub fn new() -> Self {
        Self {
            labels: Vec::new(),
            ..Self::default()
        }
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

    /// Set the source filter.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Add a label to filter by.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
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
        if !self.labels.is_empty() {
            let msg_lower = item.message.to_lowercase();
            let author_lower = item.author.to_lowercase();
            let has_label = self.labels.iter().any(|l| {
                let ll = l.to_lowercase();
                msg_lower.contains(&ll) || author_lower.contains(&ll)
            });
            if !has_label {
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
        if let Some(ref source) = self.source {
            parts.push(format!("source={source}"));
        }
        if !self.labels.is_empty() {
            parts.push(format!("labels=[{}]", self.labels.join(",")));
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

// ---------------------------------------------------------------------------
// Group timeline entries by date
// ---------------------------------------------------------------------------

/// A group of timeline items sharing the same date label.
#[derive(Debug, Clone)]
pub struct TimelineDateGroup {
    /// The date label (e.g., "2024-01-15" or "Today").
    pub label: String,
    /// Items in this group, ordered by timestamp descending.
    pub items: Vec<TimelineItem>,
}

/// Group timeline items by calendar day (UTC).
///
/// Each item's `timestamp` is a Unix epoch in seconds. Items are grouped
/// by their date in the format "YYYY-MM-DD", and groups are ordered
/// most recent first. Within each group, items are sorted newest first.
pub fn timeline_group_by_date(items: &[TimelineItem]) -> Vec<TimelineDateGroup> {
    let mut groups: HashMap<String, Vec<TimelineItem>> = HashMap::new();

    for item in items {
        let date_label = unix_timestamp_to_date(item.timestamp);
        groups
            .entry(date_label)
            .or_default()
            .push(item.clone());
    }

    let mut result: Vec<TimelineDateGroup> = groups
        .into_iter()
        .map(|(label, mut items)| {
            items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            TimelineDateGroup { label, items }
        })
        .collect();

    // Sort groups by date descending (most recent first)
    result.sort_by(|a, b| b.label.cmp(&a.label));
    result
}

/// Convert a Unix timestamp (seconds since epoch) to a "YYYY-MM-DD" date string (UTC).
fn unix_timestamp_to_date(timestamp: u64) -> String {
    const SECS_PER_DAY: u64 = 86400;
    let days = timestamp / SECS_PER_DAY;

    // Algorithm to convert days since epoch to (year, month, day)
    // Based on Howard Hinnant's civil_from_days algorithm
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Format a timestamp relative to "now" for display (e.g., "Today", "Yesterday", "3 days ago").
pub fn format_relative_date(timestamp: u64, now: u64) -> String {
    if now < timestamp {
        return "in the future".to_string();
    }
    let diff_secs = now - timestamp;
    let diff_days = diff_secs / 86400;
    match diff_days {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        2..=6 => format!("{diff_days} days ago"),
        7..=13 => "Last week".to_string(),
        14..=29 => format!("{} weeks ago", diff_days / 7),
        30..=59 => "Last month".to_string(),
        _ => format!("{} months ago", diff_days / 30),
    }
}

// ── TimelineRange ──

/// A half-open timestamp range `[start, end)` for querying timeline items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineRange {
    pub start: u64,
    pub end: u64,
}

impl TimelineRange {
    /// Create a new range.  Panics if `start > end`.
    pub fn new(start: u64, end: u64) -> Self {
        assert!(start <= end, "start must be <= end");
        Self { start, end }
    }

    /// Return `true` when `timestamp` falls inside the range.
    pub fn contains(&self, timestamp: u64) -> bool {
        timestamp >= self.start && timestamp < self.end
    }

    /// Duration of the range in seconds.
    pub fn duration_secs(&self) -> u64 {
        self.end - self.start
    }

    /// Filter a slice of items, returning only those inside the range.
    pub fn filter<'a>(&self, items: &'a [TimelineItem]) -> Vec<&'a TimelineItem> {
        items.iter().filter(|i| self.contains(i.timestamp)).collect()
    }

    /// Return `true` when two ranges overlap.
    pub fn overlaps(&self, other: &TimelineRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl fmt::Display for TimelineRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{})", self.start, self.end)
    }
}

// ── Timeline aggregation ──

/// Granularity for timeline aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationPeriod {
    Day,
    Week,
}

/// A single bucket produced by aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregationBucket {
    /// The bucket key (e.g. `"2024-01-15"` for Day).
    pub key: String,
    /// Number of items in this bucket.
    pub count: usize,
}

/// Group timeline items by the given period and return counts per bucket.
pub fn timeline_aggregate(
    items: &[TimelineItem],
    period: AggregationPeriod,
) -> Vec<AggregationBucket> {
    let mut map: HashMap<String, usize> = HashMap::new();
    for item in items {
        let key = match period {
            AggregationPeriod::Day => unix_timestamp_to_date(item.timestamp),
            AggregationPeriod::Week => {
                // Use the Monday of the ISO week
                let secs_per_day: u64 = 86400;
                let day_index = item.timestamp / secs_per_day;
                // epoch (1970-01-01) was a Thursday (weekday index 3, Mon=0)
                let weekday = ((day_index + 3) % 7) as u64; // Mon=0 .. Sun=6
                let monday_ts = (day_index - weekday) * secs_per_day;
                format!("week-{}", unix_timestamp_to_date(monday_ts))
            }
        };
        *map.entry(key).or_insert(0) += 1;
    }
    let mut buckets: Vec<AggregationBucket> = map
        .into_iter()
        .map(|(key, count)| AggregationBucket { key, count })
        .collect();
    buckets.sort_by(|a, b| a.key.cmp(&b.key));
    buckets
}

// ── Timeline statistics ──

/// Summary statistics for a set of timeline items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineStatistics {
    /// Total number of items.
    pub total_items: usize,
    /// Number of unique authors.
    pub unique_authors: usize,
    /// The most active author (most commits).  `None` when items is empty.
    pub most_active_author: Option<String>,
    /// The busiest day (date string with most commits).
    pub busiest_day: Option<String>,
    /// Earliest timestamp.
    pub earliest: Option<u64>,
    /// Latest timestamp.
    pub latest: Option<u64>,
}

/// Compute statistics for a set of timeline items.
pub fn timeline_statistics(items: &[TimelineItem]) -> TimelineStatistics {
    if items.is_empty() {
        return TimelineStatistics {
            total_items: 0,
            unique_authors: 0,
            most_active_author: None,
            busiest_day: None,
            earliest: None,
            latest: None,
        };
    }

    let mut author_counts: HashMap<&str, usize> = HashMap::new();
    let mut day_counts: HashMap<String, usize> = HashMap::new();
    let mut earliest = u64::MAX;
    let mut latest = 0u64;

    for item in items {
        *author_counts.entry(item.author.as_str()).or_insert(0) += 1;
        let day = unix_timestamp_to_date(item.timestamp);
        *day_counts.entry(day).or_insert(0) += 1;
        if item.timestamp < earliest {
            earliest = item.timestamp;
        }
        if item.timestamp > latest {
            latest = item.timestamp;
        }
    }

    let most_active_author = author_counts
        .iter()
        .max_by_key(|(_, c)| **c)
        .map(|(a, _)| a.to_string());

    let busiest_day = day_counts
        .iter()
        .max_by_key(|(_, c)| **c)
        .map(|(d, _)| d.clone());

    TimelineStatistics {
        total_items: items.len(),
        unique_authors: author_counts.len(),
        most_active_author,
        busiest_day,
        earliest: Some(earliest),
        latest: Some(latest),
    }
}

// ── Timeline Entry Types ──

/// Classification of timeline entries by their source action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    /// A git commit.
    GitCommit,
    /// A manual file save in the editor.
    FileSave,
    /// A debug session event (breakpoint hit, step, etc.).
    DebugEvent,
    /// An automated build or CI event.
    BuildEvent,
    /// A code review comment or approval.
    ReviewEvent,
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitCommit => write!(f, "git-commit"),
            Self::FileSave => write!(f, "file-save"),
            Self::DebugEvent => write!(f, "debug-event"),
            Self::BuildEvent => write!(f, "build-event"),
            Self::ReviewEvent => write!(f, "review-event"),
        }
    }
}

/// A timeline entry enriched with a kind classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedTimelineEntry {
    pub kind: EntryKind,
    pub item: TimelineItem,
}

impl TypedTimelineEntry {
    pub fn new(kind: EntryKind, item: TimelineItem) -> Self {
        Self { kind, item }
    }
}

/// Classify a `TimelineItem` into an `EntryKind` by inspecting the message
/// and author fields with simple heuristics.
pub fn classify_entry(item: &TimelineItem) -> EntryKind {
    let msg = item.message.to_lowercase();
    let author = item.author.to_lowercase();
    if msg.contains("[debug]") || msg.contains("breakpoint") || msg.contains("step into") {
        EntryKind::DebugEvent
    } else if msg.contains("[build]") || msg.contains("[ci]") || author.contains("ci-bot") {
        EntryKind::BuildEvent
    } else if msg.contains("[review]") || msg.contains("lgtm") || msg.contains("approved") {
        EntryKind::ReviewEvent
    } else if msg.contains("[save]") || author == "editor" || author == "autosave" {
        EntryKind::FileSave
    } else {
        EntryKind::GitCommit
    }
}

/// Filter a slice of typed entries, keeping only those of the specified kinds.
pub fn filter_by_kind(entries: &[TypedTimelineEntry], kinds: &[EntryKind]) -> Vec<TypedTimelineEntry> {
    entries
        .iter()
        .filter(|e| kinds.contains(&e.kind))
        .cloned()
        .collect()
}

/// Group typed entries by their kind, returning a map from kind to entries.
pub fn group_by_kind(entries: &[TypedTimelineEntry]) -> HashMap<EntryKind, Vec<TypedTimelineEntry>> {
    let mut map: HashMap<EntryKind, Vec<TypedTimelineEntry>> = HashMap::new();
    for entry in entries {
        map.entry(entry.kind).or_default().push(entry.clone());
    }
    map
}

// ── Cursor-based Pagination ──

/// A page of timeline items produced by cursor-based pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelinePage {
    /// The items on this page.
    pub items: Vec<TimelineItem>,
    /// Cursor pointing to the next page, or `None` if this is the last page.
    pub next_cursor: Option<String>,
    /// Cursor pointing to the previous page, or `None` if this is the first page.
    pub prev_cursor: Option<String>,
    /// Total number of items across all pages.
    pub total: usize,
}

/// A cursor-based paginator over a pre-sorted list of timeline items.
///
/// Items must be sorted by timestamp descending (newest first) before
/// constructing the paginator.
#[derive(Debug, Clone)]
pub struct TimelinePaginator {
    items: Vec<TimelineItem>,
    page_size: usize,
}

impl TimelinePaginator {
    /// Create a paginator.  `items` should already be sorted newest-first.
    /// `page_size` must be >= 1.
    pub fn new(items: Vec<TimelineItem>, page_size: usize) -> Self {
        assert!(page_size >= 1, "page_size must be >= 1");
        Self { items, page_size }
    }

    /// Total number of items.
    pub fn total(&self) -> usize {
        self.items.len()
    }

    /// Total number of pages.
    pub fn page_count(&self) -> usize {
        if self.items.is_empty() {
            0
        } else {
            (self.items.len() + self.page_size - 1) / self.page_size
        }
    }

    /// Fetch the first page.
    pub fn first_page(&self) -> TimelinePage {
        self.page_at_offset(0)
    }

    /// Fetch a page using a cursor string previously returned from a `TimelinePage`.
    ///
    /// The cursor encodes the byte offset into the item list as a decimal string.
    pub fn page_for_cursor(&self, cursor: &str) -> Option<TimelinePage> {
        let offset: usize = cursor.parse().ok()?;
        if offset > self.items.len() {
            return None;
        }
        Some(self.page_at_offset(offset))
    }

    fn page_at_offset(&self, offset: usize) -> TimelinePage {
        let end = (offset + self.page_size).min(self.items.len());
        let page_items = self.items[offset..end].to_vec();

        let next_cursor = if end < self.items.len() {
            Some(end.to_string())
        } else {
            None
        };
        let prev_cursor = if offset > 0 {
            Some(offset.saturating_sub(self.page_size).to_string())
        } else {
            None
        };

        TimelinePage {
            items: page_items,
            next_cursor,
            prev_cursor,
            total: self.items.len(),
        }
    }
}

// ── Expand/Collapse Tracking ──

/// Tracks which timeline entries are expanded (showing full detail) vs collapsed.
#[derive(Debug, Clone)]
pub struct ExpansionState {
    expanded: HashMap<String, bool>,
}

impl ExpansionState {
    /// Create a new state with all entries collapsed.
    pub fn new() -> Self {
        Self {
            expanded: HashMap::new(),
        }
    }

    /// Returns `true` if the entry identified by `sha` is expanded.
    pub fn is_expanded(&self, sha: &str) -> bool {
        self.expanded.get(sha).copied().unwrap_or(false)
    }

    /// Expand an entry.
    pub fn expand(&mut self, sha: &str) {
        self.expanded.insert(sha.to_string(), true);
    }

    /// Collapse an entry.
    pub fn collapse(&mut self, sha: &str) {
        self.expanded.insert(sha.to_string(), false);
    }

    /// Toggle the expanded state for an entry, returning the new state.
    pub fn toggle(&mut self, sha: &str) -> bool {
        let new_state = !self.is_expanded(sha);
        self.expanded.insert(sha.to_string(), new_state);
        new_state
    }

    /// Return the number of currently expanded entries.
    pub fn expanded_count(&self) -> usize {
        self.expanded.values().filter(|&&v| v).count()
    }

    /// Collapse all entries.
    pub fn collapse_all(&mut self) {
        for v in self.expanded.values_mut() {
            *v = false;
        }
    }

    /// Expand all entries whose SHA appears in the given list.
    pub fn expand_all(&mut self, shas: &[&str]) {
        for sha in shas {
            self.expand(sha);
        }
    }
}

impl Default for ExpansionState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Time-range Comparison ──

/// Result of comparing timeline activity across two time ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeComparison {
    /// Items that fall in range A only.
    pub only_a: Vec<TimelineItem>,
    /// Items that fall in range B only.
    pub only_b: Vec<TimelineItem>,
    /// Items that fall in both ranges (overlapping region).
    pub overlap: Vec<TimelineItem>,
    /// Commit count in range A.
    pub count_a: usize,
    /// Commit count in range B.
    pub count_b: usize,
    /// Unique authors across both ranges.
    pub unique_authors: usize,
}

/// Compare timeline activity between two `TimelineRange`s over a common item set.
///
/// Each item is placed into `only_a`, `only_b`, or `overlap` depending on which
/// range(s) contain its timestamp.
pub fn compare_ranges(
    items: &[TimelineItem],
    range_a: &TimelineRange,
    range_b: &TimelineRange,
) -> RangeComparison {
    let mut only_a = Vec::new();
    let mut only_b = Vec::new();
    let mut overlap = Vec::new();
    let mut authors: Vec<String> = Vec::new();

    for item in items {
        let in_a = range_a.contains(item.timestamp);
        let in_b = range_b.contains(item.timestamp);
        match (in_a, in_b) {
            (true, true) => overlap.push(item.clone()),
            (true, false) => only_a.push(item.clone()),
            (false, true) => only_b.push(item.clone()),
            (false, false) => {}
        }
        if (in_a || in_b) && !authors.iter().any(|a| a.eq_ignore_ascii_case(&item.author)) {
            authors.push(item.author.clone());
        }
    }

    let count_a = only_a.len() + overlap.len();
    let count_b = only_b.len() + overlap.len();

    RangeComparison {
        only_a,
        only_b,
        overlap,
        count_a,
        count_b,
        unique_authors: authors.len(),
    }
}

// ── Relative Date Grouping ──

/// Human-friendly date bucket relative to a reference "now" timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelativeDateBucket {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    Older,
}

impl fmt::Display for RelativeDateBucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Today => write!(f, "Today"),
            Self::Yesterday => write!(f, "Yesterday"),
            Self::ThisWeek => write!(f, "This Week"),
            Self::LastWeek => write!(f, "Last Week"),
            Self::ThisMonth => write!(f, "This Month"),
            Self::Older => write!(f, "Older"),
        }
    }
}

/// Assign a `RelativeDateBucket` to a timestamp given a reference `now`.
pub fn relative_bucket(timestamp: u64, now: u64) -> RelativeDateBucket {
    if now < timestamp {
        return RelativeDateBucket::Today;
    }
    let diff_secs = now - timestamp;
    let diff_days = diff_secs / 86400;
    match diff_days {
        0 => RelativeDateBucket::Today,
        1 => RelativeDateBucket::Yesterday,
        2..=6 => RelativeDateBucket::ThisWeek,
        7..=13 => RelativeDateBucket::LastWeek,
        14..=29 => RelativeDateBucket::ThisMonth,
        _ => RelativeDateBucket::Older,
    }
}

/// Group timeline items into relative-date buckets.
///
/// Returns groups in display order: Today → Yesterday → This Week → … → Older.
/// Within each group items are sorted newest-first.
pub fn group_by_relative_date(
    items: &[TimelineItem],
    now: u64,
) -> Vec<(RelativeDateBucket, Vec<TimelineItem>)> {
    let bucket_order = [
        RelativeDateBucket::Today,
        RelativeDateBucket::Yesterday,
        RelativeDateBucket::ThisWeek,
        RelativeDateBucket::LastWeek,
        RelativeDateBucket::ThisMonth,
        RelativeDateBucket::Older,
    ];

    let mut map: HashMap<RelativeDateBucket, Vec<TimelineItem>> = HashMap::new();
    for item in items {
        let bucket = relative_bucket(item.timestamp, now);
        map.entry(bucket).or_default().push(item.clone());
    }

    // Sort items within each bucket newest-first
    for group in map.values_mut() {
        group.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    bucket_order
        .iter()
        .filter_map(|b| map.remove(b).map(|items| (*b, items)))
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
        assert_eq!(s, "abc123 Alice: Fix bug");
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

    // ── Snapshot & timeline_diff tests ──

    #[test]
    fn snapshot_new_and_accessors() {
        let items = sample_items();
        let snap = TimelineSnapshot::new("before-merge", 1700500000, items.clone());
        assert_eq!(snap.label, "before-merge");
        assert_eq!(snap.captured_at, 1700500000);
        assert_eq!(snap.len(), items.len());
        assert!(!snap.is_empty());
    }

    #[test]
    fn snapshot_empty() {
        let snap = TimelineSnapshot::new("empty", 0, vec![]);
        assert!(snap.is_empty());
        assert_eq!(snap.len(), 0);
    }

    #[test]
    fn snapshot_display() {
        let snap = TimelineSnapshot::new("v1", 100, sample_items());
        let s = format!("{snap}");
        assert!(s.contains("v1"));
        assert!(s.contains("5 items"));
    }

    #[test]
    fn snapshot_diff_method() {
        let old_snap = TimelineSnapshot::new("old", 100, vec![sample_items()[0].clone()]);
        let new_snap = TimelineSnapshot::new("new", 200, sample_items());
        let diff = old_snap.diff(&new_snap);
        assert_eq!(diff.added.len(), 4);
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn timeline_diff_function() {
        let old_snap = TimelineSnapshot::new("old", 100, sample_items());
        let new_snap = TimelineSnapshot::new("new", 200, sample_items());
        let diff = timeline_diff(&old_snap, &new_snap);
        assert!(diff.is_empty());
    }

    #[test]
    fn filter_by_label() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_label("Fix");
        let result = filter_items(&items, &filter);
        assert_eq!(result.len(), 2); // "Fix bug in parser" and "Fix typo"
    }

    #[test]
    fn filter_by_label_no_match() {
        let items = sample_items();
        let filter = TimelineFilter::new().with_label("nonexistent-label");
        let result = filter_items(&items, &filter);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_with_source_display() {
        let filter = TimelineFilter::new().with_source("git").with_label("bug");
        let s = format!("{filter}");
        assert!(s.contains("source=git"));
        assert!(s.contains("labels=[bug]"));
    }

    #[test]
    fn group_by_date_same_day() {
        let items = vec![
            TimelineItem { timestamp: 1000, message: "A".into(), author: "x".into(), sha: "a".into() },
            TimelineItem { timestamp: 1500, message: "B".into(), author: "x".into(), sha: "b".into() },
        ];
        let groups = timeline_group_by_date(&items);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].items.len(), 2);
        // Should be sorted newest first
        assert_eq!(groups[0].items[0].timestamp, 1500);
    }

    #[test]
    fn group_by_date_different_days() {
        let items = vec![
            TimelineItem { timestamp: 86400 * 0 + 100, message: "Day0".into(), author: "x".into(), sha: "a".into() },
            TimelineItem { timestamp: 86400 * 1 + 100, message: "Day1".into(), author: "x".into(), sha: "b".into() },
            TimelineItem { timestamp: 86400 * 2 + 100, message: "Day2".into(), author: "x".into(), sha: "c".into() },
        ];
        let groups = timeline_group_by_date(&items);
        assert_eq!(groups.len(), 3);
        // Most recent first
        assert!(groups[0].label > groups[1].label);
    }

    #[test]
    fn group_by_date_empty() {
        let groups = timeline_group_by_date(&[]);
        assert!(groups.is_empty());
    }

    #[test]
    fn format_relative_date_today() {
        let now = 1700000000;
        assert_eq!(format_relative_date(now - 3600, now), "Today");
    }

    #[test]
    fn format_relative_date_yesterday() {
        let now = 1700000000;
        assert_eq!(format_relative_date(now - 86400 - 100, now), "Yesterday");
    }

    #[test]
    fn format_relative_date_days_ago() {
        let now = 1700000000;
        let result = format_relative_date(now - 86400 * 5, now);
        assert!(result.contains("5 days ago"));
    }

    #[test]
    fn unix_timestamp_to_date_epoch() {
        // Unix epoch is 1970-01-01
        let date = unix_timestamp_to_date(0);
        assert_eq!(date, "1970-01-01");
    }

    // ── New method tests ──

    #[test]
    fn age_seconds_basic() {
        let item = TimelineItem { timestamp: 1000, message: "m".into(), author: "a".into(), sha: "s".into() };
        assert_eq!(item.age_seconds(1500), 500);
        // now < timestamp saturates to 0
        assert_eq!(item.age_seconds(500), 0);
    }

    #[test]
    fn is_by_author_case_insensitive() {
        let item = TimelineItem { timestamp: 1, message: "m".into(), author: "Alice".into(), sha: "s".into() };
        assert!(item.is_by_author("alice"));
        assert!(item.is_by_author("ALICE"));
        assert!(item.is_by_author("Alice"));
        assert!(!item.is_by_author("Bob"));
    }

    #[test]
    fn snapshot_authors_unique() {
        let snap = TimelineSnapshot::new("t", 0, sample_items());
        let authors = snap.authors();
        assert_eq!(authors.len(), 3);
        assert!(authors.contains(&"Alice"));
        assert!(authors.contains(&"Bob"));
        assert!(authors.contains(&"Charlie"));
    }

    #[test]
    fn snapshot_latest_and_oldest() {
        let snap = TimelineSnapshot::new("t", 0, sample_items());
        let latest = snap.latest_item().unwrap();
        assert_eq!(latest.sha, "eee555");
        let oldest = snap.oldest_item().unwrap();
        assert_eq!(oldest.sha, "aaa111");
    }

    #[test]
    fn snapshot_latest_oldest_empty() {
        let snap = TimelineSnapshot::new("empty", 0, vec![]);
        assert!(snap.latest_item().is_none());
        assert!(snap.oldest_item().is_none());
    }

    #[test]
    fn snapshot_filter_method() {
        let snap = TimelineSnapshot::new("t", 0, sample_items());
        let filter = TimelineFilter::new().with_author("bob");
        let filtered = snap.filter(&filter);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|i| i.author == "Bob"));
    }

    #[test]
    fn snapshot_filter_with_date_range() {
        let snap = TimelineSnapshot::new("t", 0, sample_items());
        let filter = TimelineFilter::new().with_date_range(1700100000, 1700200000);
        let filtered = snap.filter(&filter);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|i| i.timestamp >= 1700100000 && i.timestamp <= 1700200000));
    }

    // ── TimelineRange tests ──

    #[test]
    fn timeline_range_contains_and_filter() {
        let range = TimelineRange::new(1700000000, 1700200000);
        assert!(range.contains(1700000000));
        assert!(range.contains(1700100000));
        assert!(!range.contains(1700200000)); // half-open
        let items = sample_items();
        let filtered = range.filter(&items);
        assert!(filtered.iter().all(|i| range.contains(i.timestamp)));
    }

    #[test]
    fn timeline_range_overlaps() {
        let r1 = TimelineRange::new(100, 200);
        let r2 = TimelineRange::new(150, 250);
        let r3 = TimelineRange::new(200, 300);
        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
    }

    // ── Timeline diff tests ──

    #[test]
    fn timeline_diff_via_snapshots() {
        let a_items = vec![
            TimelineItem { timestamp: 1, message: "m1".into(), author: "A".into(), sha: "s1".into() },
            TimelineItem { timestamp: 2, message: "m2".into(), author: "B".into(), sha: "s2".into() },
        ];
        let b_items = vec![
            TimelineItem { timestamp: 2, message: "m2".into(), author: "B".into(), sha: "s2".into() },
            TimelineItem { timestamp: 3, message: "m3".into(), author: "C".into(), sha: "s3".into() },
        ];
        let snap_a = TimelineSnapshot::new("old", 0, a_items);
        let snap_b = TimelineSnapshot::new("new", 1, b_items);
        let diff = timeline_diff(&snap_a, &snap_b);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].sha, "s3");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].sha, "s1");
    }

    // ── Timeline aggregation tests ──

    #[test]
    fn timeline_aggregate_by_day() {
        let items = sample_items();
        let buckets = timeline_aggregate(&items, AggregationPeriod::Day);
        assert!(!buckets.is_empty());
        let total: usize = buckets.iter().map(|b| b.count).sum();
        assert_eq!(total, items.len());
    }

    // ── Timeline statistics tests ──

    #[test]
    fn timeline_statistics_basic() {
        let items = sample_items();
        let stats = timeline_statistics(&items);
        assert_eq!(stats.total_items, items.len());
        assert!(stats.unique_authors > 0);
        assert!(stats.most_active_author.is_some());
        assert!(stats.busiest_day.is_some());
        assert!(stats.earliest.unwrap() <= stats.latest.unwrap());
    }

    #[test]
    fn timeline_statistics_empty() {
        let stats = timeline_statistics(&[]);
        assert_eq!(stats.total_items, 0);
        assert!(stats.most_active_author.is_none());
    }

    // ── EntryKind & classification tests ──

    #[test]
    fn classify_git_commit() {
        let item = TimelineItem { timestamp: 1, message: "Fix parser".into(), author: "Alice".into(), sha: "a1".into() };
        assert_eq!(classify_entry(&item), EntryKind::GitCommit);
    }

    #[test]
    fn classify_debug_event() {
        let item = TimelineItem { timestamp: 1, message: "[debug] breakpoint hit".into(), author: "Alice".into(), sha: "d1".into() };
        assert_eq!(classify_entry(&item), EntryKind::DebugEvent);
    }

    #[test]
    fn classify_build_event() {
        let item = TimelineItem { timestamp: 1, message: "[CI] pipeline passed".into(), author: "ci-bot".into(), sha: "b1".into() };
        assert_eq!(classify_entry(&item), EntryKind::BuildEvent);
    }

    #[test]
    fn classify_review_event() {
        let item = TimelineItem { timestamp: 1, message: "LGTM, looks good".into(), author: "Bob".into(), sha: "r1".into() };
        assert_eq!(classify_entry(&item), EntryKind::ReviewEvent);
    }

    #[test]
    fn classify_file_save() {
        let item = TimelineItem { timestamp: 1, message: "[save] buffer written".into(), author: "editor".into(), sha: "s1".into() };
        assert_eq!(classify_entry(&item), EntryKind::FileSave);
    }

    #[test]
    fn filter_by_kind_filters_correctly() {
        let entries = vec![
            TypedTimelineEntry::new(EntryKind::GitCommit, sample_items()[0].clone()),
            TypedTimelineEntry::new(EntryKind::DebugEvent, sample_items()[1].clone()),
            TypedTimelineEntry::new(EntryKind::GitCommit, sample_items()[2].clone()),
            TypedTimelineEntry::new(EntryKind::FileSave, sample_items()[3].clone()),
        ];
        let commits = filter_by_kind(&entries, &[EntryKind::GitCommit]);
        assert_eq!(commits.len(), 2);
        let debug_and_save = filter_by_kind(&entries, &[EntryKind::DebugEvent, EntryKind::FileSave]);
        assert_eq!(debug_and_save.len(), 2);
    }

    #[test]
    fn group_by_kind_groups_correctly() {
        let entries = vec![
            TypedTimelineEntry::new(EntryKind::GitCommit, sample_items()[0].clone()),
            TypedTimelineEntry::new(EntryKind::GitCommit, sample_items()[1].clone()),
            TypedTimelineEntry::new(EntryKind::DebugEvent, sample_items()[2].clone()),
        ];
        let groups = group_by_kind(&entries);
        assert_eq!(groups.get(&EntryKind::GitCommit).unwrap().len(), 2);
        assert_eq!(groups.get(&EntryKind::DebugEvent).unwrap().len(), 1);
        assert!(groups.get(&EntryKind::FileSave).is_none());
    }

    #[test]
    fn entry_kind_display() {
        assert_eq!(format!("{}", EntryKind::GitCommit), "git-commit");
        assert_eq!(format!("{}", EntryKind::FileSave), "file-save");
        assert_eq!(format!("{}", EntryKind::DebugEvent), "debug-event");
    }

    // ── Pagination tests ──

    #[test]
    fn paginator_first_page() {
        let items = sample_items();
        let paginator = TimelinePaginator::new(items.clone(), 2);
        let page = paginator.first_page();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 5);
        assert!(page.prev_cursor.is_none());
        assert!(page.next_cursor.is_some());
    }

    #[test]
    fn paginator_walk_all_pages() {
        let items = sample_items();
        let paginator = TimelinePaginator::new(items.clone(), 2);
        assert_eq!(paginator.page_count(), 3);

        let mut collected = Vec::new();
        let mut page = paginator.first_page();
        collected.extend(page.items.clone());
        while let Some(cursor) = page.next_cursor {
            page = paginator.page_for_cursor(&cursor).unwrap();
            collected.extend(page.items.clone());
        }
        assert_eq!(collected.len(), 5);
        assert_eq!(collected, items);
    }

    #[test]
    fn paginator_empty() {
        let paginator = TimelinePaginator::new(vec![], 10);
        assert_eq!(paginator.page_count(), 0);
        let page = paginator.first_page();
        assert!(page.items.is_empty());
        assert!(page.next_cursor.is_none());
        assert!(page.prev_cursor.is_none());
    }

    #[test]
    fn paginator_invalid_cursor_returns_none() {
        let paginator = TimelinePaginator::new(sample_items(), 2);
        assert!(paginator.page_for_cursor("not_a_number").is_none());
        assert!(paginator.page_for_cursor("999").is_none());
    }

    #[test]
    fn paginator_prev_cursor() {
        let paginator = TimelinePaginator::new(sample_items(), 2);
        let page1 = paginator.first_page();
        let page2 = paginator.page_for_cursor(page1.next_cursor.as_ref().unwrap()).unwrap();
        assert!(page2.prev_cursor.is_some());
        let back = paginator.page_for_cursor(page2.prev_cursor.as_ref().unwrap()).unwrap();
        assert_eq!(back.items, page1.items);
    }

    // ── Expansion state tests ──

    #[test]
    fn expansion_state_toggle() {
        let mut state = ExpansionState::new();
        assert!(!state.is_expanded("abc"));
        assert!(state.toggle("abc"));
        assert!(state.is_expanded("abc"));
        assert!(!state.toggle("abc"));
        assert!(!state.is_expanded("abc"));
    }

    #[test]
    fn expansion_state_expand_collapse_all() {
        let mut state = ExpansionState::new();
        state.expand_all(&["a", "b", "c"]);
        assert_eq!(state.expanded_count(), 3);
        state.collapse_all();
        assert_eq!(state.expanded_count(), 0);
    }

    // ── Range comparison tests ──

    #[test]
    fn compare_ranges_non_overlapping() {
        let items = sample_items();
        let range_a = TimelineRange::new(1700000000, 1700150000);
        let range_b = TimelineRange::new(1700300000, 1700500000);
        let cmp = compare_ranges(&items, &range_a, &range_b);
        assert_eq!(cmp.only_a.len(), 2); // timestamps 1700000000, 1700100000
        assert_eq!(cmp.only_b.len(), 2); // timestamps 1700300000, 1700400000
        assert!(cmp.overlap.is_empty());
        assert_eq!(cmp.count_a, 2);
        assert_eq!(cmp.count_b, 2);
    }

    #[test]
    fn compare_ranges_with_overlap() {
        let items = sample_items();
        let range_a = TimelineRange::new(1700000000, 1700250000);
        let range_b = TimelineRange::new(1700100000, 1700350000);
        let cmp = compare_ranges(&items, &range_a, &range_b);
        // overlap: 1700100000, 1700200000
        assert_eq!(cmp.overlap.len(), 2);
        assert_eq!(cmp.only_a.len(), 1); // 1700000000
        assert_eq!(cmp.only_b.len(), 1); // 1700300000
        assert_eq!(cmp.count_a, 3);
        assert_eq!(cmp.count_b, 3);
    }

    #[test]
    fn compare_ranges_unique_authors() {
        let items = sample_items();
        let range_a = TimelineRange::new(1700000000, 1700500000);
        let range_b = TimelineRange::new(1700000000, 1700500000);
        let cmp = compare_ranges(&items, &range_a, &range_b);
        assert_eq!(cmp.unique_authors, 3); // Alice, Bob, Charlie
    }

    // ── Relative date grouping tests ──

    #[test]
    fn relative_bucket_assignment() {
        let now = 1_700_000_000u64;
        assert_eq!(relative_bucket(now - 100, now), RelativeDateBucket::Today);
        assert_eq!(relative_bucket(now - 86400, now), RelativeDateBucket::Yesterday);
        assert_eq!(relative_bucket(now - 86400 * 4, now), RelativeDateBucket::ThisWeek);
        assert_eq!(relative_bucket(now - 86400 * 10, now), RelativeDateBucket::LastWeek);
        assert_eq!(relative_bucket(now - 86400 * 20, now), RelativeDateBucket::ThisMonth);
        assert_eq!(relative_bucket(now - 86400 * 60, now), RelativeDateBucket::Older);
    }

    #[test]
    fn group_by_relative_date_ordering() {
        let now = 1_700_000_000u64;
        let items = vec![
            TimelineItem { timestamp: now - 100, message: "today".into(), author: "A".into(), sha: "t1".into() },
            TimelineItem { timestamp: now - 86400 * 2, message: "this week".into(), author: "A".into(), sha: "t2".into() },
            TimelineItem { timestamp: now - 86400 * 50, message: "older".into(), author: "A".into(), sha: "t3".into() },
        ];
        let groups = group_by_relative_date(&items, now);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, RelativeDateBucket::Today);
        assert_eq!(groups[1].0, RelativeDateBucket::ThisWeek);
        assert_eq!(groups[2].0, RelativeDateBucket::Older);
    }

    #[test]
    fn relative_date_bucket_display() {
        assert_eq!(format!("{}", RelativeDateBucket::Today), "Today");
        assert_eq!(format!("{}", RelativeDateBucket::Older), "Older");
    }
}
