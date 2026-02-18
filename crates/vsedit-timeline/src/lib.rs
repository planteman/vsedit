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


// ---------------------------------------------------------------------------
// TimelineProviderRegistry
// ---------------------------------------------------------------------------

pub struct TimelineProviderRegistry {
    providers: Vec<(String, Box<dyn TimelineProvider>)>,
}

impl TimelineProviderRegistry {
    pub fn new() -> Self { Self { providers: Vec::new() } }

    pub fn register(&mut self, id: impl Into<String>, provider: Box<dyn TimelineProvider>) {
        self.providers.push((id.into(), provider));
    }

    pub fn get(&self, id: &str) -> Option<&dyn TimelineProvider> {
        self.providers.iter().find(|(pid, _)| pid == id).map(|(_, p)| p.as_ref())
    }

    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.iter().map(|(id, _)| id.as_str()).collect()
    }

    pub fn len(&self) -> usize { self.providers.len() }
    pub fn is_empty(&self) -> bool { self.providers.is_empty() }

    pub fn unregister(&mut self, id: &str) -> bool {
        if let Some(i) = self.providers.iter().position(|(pid, _)| pid == id) {
            self.providers.remove(i);
            true
        } else {
            false
        }
    }
}

impl Default for TimelineProviderRegistry {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// TimelineEntryCompare
// ---------------------------------------------------------------------------

pub struct TimelineEntryCompare;

impl TimelineEntryCompare {
    /// Compare two timeline entries by timestamp.
    pub fn by_timestamp(a: &TimelineItem, b: &TimelineItem) -> std::cmp::Ordering {
        a.timestamp.cmp(&b.timestamp)
    }

    /// Compare by author then timestamp.
    pub fn by_author_then_time(a: &TimelineItem, b: &TimelineItem) -> std::cmp::Ordering {
        a.author.cmp(&b.author).then(a.timestamp.cmp(&b.timestamp))
    }

    /// Find items that appear in new but not old (by commit hash).
    pub fn new_items<'a>(old: &[TimelineItem], new: &'a [TimelineItem]) -> Vec<&'a TimelineItem> {
        let old_hashes: std::collections::HashSet<&str> = old.iter().map(|i| i.sha.as_str()).collect();
        new.iter().filter(|i| !old_hashes.contains(i.sha.as_str())).collect()
    }

    /// Find items removed (in old but not new).
    pub fn removed_items<'a>(old: &'a [TimelineItem], new: &[TimelineItem]) -> Vec<&'a TimelineItem> {
        let new_hashes: std::collections::HashSet<&str> = new.iter().map(|i| i.sha.as_str()).collect();
        old.iter().filter(|i| !new_hashes.contains(i.sha.as_str())).collect()
    }
}

// ---------------------------------------------------------------------------
// TimelineRefreshDebouncer
// ---------------------------------------------------------------------------

pub struct TimelineRefreshDebouncer {
    last_refresh_ms: u64,
    debounce_ms: u64,
    pending: bool,
}

impl TimelineRefreshDebouncer {
    pub fn new(debounce_ms: u64) -> Self {
        Self { last_refresh_ms: 0, debounce_ms, pending: false }
    }

    pub fn request_refresh(&mut self, now_ms: u64) -> bool {
        if now_ms >= self.last_refresh_ms + self.debounce_ms {
            self.last_refresh_ms = now_ms;
            self.pending = false;
            true
        } else {
            self.pending = true;
            false
        }
    }

    pub fn is_pending(&self) -> bool { self.pending }

    pub fn check_pending(&mut self, now_ms: u64) -> bool {
        if self.pending && now_ms >= self.last_refresh_ms + self.debounce_ms {
            self.last_refresh_ms = now_ms;
            self.pending = false;
            true
        } else {
            false
        }
    }

    pub fn debounce_ms(&self) -> u64 { self.debounce_ms }
}

impl Default for TimelineRefreshDebouncer {
    fn default() -> Self { Self::new(500) }
}

// ---------------------------------------------------------------------------
// TimelineExportFormatter
// ---------------------------------------------------------------------------

pub struct TimelineExportFormatter;

impl TimelineExportFormatter {
    /// Format items as CSV.
    pub fn to_csv(items: &[TimelineItem]) -> String {
        let mut out = String::from("sha,author,message,timestamp\n");
        for item in items {
            out.push_str(&format!("{},{},{},{}\n",
                item.sha, item.author, item.message.replace(',', ";"), item.timestamp));
        }
        out
    }

    /// Format items as JSON-like text.
    pub fn to_json_text(items: &[TimelineItem]) -> String {
        let mut out = String::from("[\n");
        for (i, item) in items.iter().enumerate() {
            out.push_str(&format!("  {{\"sha\":\"{}\",\"author\":\"{}\",\"message\":\"{}\",\"timestamp\":{}}}",
                item.sha, item.author, item.message, item.timestamp));
            if i < items.len() - 1 { out.push_str(",\n"); }
        }
        out.push_str("\n]");
        out
    }

    /// Format as plain text summary.
    pub fn to_plain_text(items: &[TimelineItem]) -> String {
        items.iter().map(|i| format!("{}: {} ({})", i.sha, i.message, i.author)).collect::<Vec<_>>().join("\n")
    }
}


// === Timeline Event Filter ===

/// Timeline Event Filter implementation.
#[derive(Debug, Clone)]
pub struct TimelineEventFilter {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TimelineEventFilterStats,
}

/// Statistics for TimelineEventFilter.
#[derive(Debug, Clone, Default)]
pub struct TimelineEventFilterStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TimelineEventFilterStats {
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

impl TimelineEventFilter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TimelineEventFilterStats::default(),
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

    pub fn stats(&self) -> &TimelineEventFilterStats {
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

impl Default for TimelineEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

// === Timeline Range Selector ===

/// Priority level for TimelineRangeSelector items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimelineRangeSelectorPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TimelineRangeSelectorPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TimelineRangeSelectorPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Timeline Range Selector implementation.
#[derive(Debug, Clone)]
pub struct TimelineRangeSelector {
    items: Vec<TimelineRangeSelectorItem>,
    max_items: usize,
    default_priority: TimelineRangeSelectorPriority,
}

/// A single item in TimelineRangeSelector.
#[derive(Debug, Clone)]
pub struct TimelineRangeSelectorItem {
    pub id: String,
    pub label: String,
    pub priority: TimelineRangeSelectorPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TimelineRangeSelectorItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TimelineRangeSelectorPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TimelineRangeSelectorPriority) -> Self {
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

impl TimelineRangeSelector {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TimelineRangeSelectorPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TimelineRangeSelectorItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TimelineRangeSelectorItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TimelineRangeSelectorItem> {
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

    pub fn by_priority(&self, priority: TimelineRangeSelectorPriority) -> Vec<&TimelineRangeSelectorItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TimelineRangeSelectorItem> {
        let mut sorted: Vec<&TimelineRangeSelectorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TimelineRangeSelectorItem> {
        let mut sorted: Vec<&TimelineRangeSelectorItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TimelineRangeSelectorItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TimelineRangeSelectorPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TimelineRangeSelectorPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TimelineRangeSelectorItem> {
        self.items.iter()
    }
}

impl Default for TimelineRangeSelector {
    fn default() -> Self {
        Self::new()
    }
}


// ─── TlBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for timeline events.
#[derive(Debug, Clone)]
pub struct TlBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> TlBufRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for TlBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TlBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── TlFmt Formatter ───────────────────────────────────────

/// Formatting options for timeline output.
#[derive(Debug, Clone)]
pub struct TlFmtFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for TlFmtFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl TlFmtFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for timeline data.
pub struct TlFmtFmt {
    options: TlFmtFmtOpts,
}

impl TlFmtFmt {
    pub fn new(options: TlFmtFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: TlFmtFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}



// ---------------------------------------------------------------------------
// timeline – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for timeline view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YTimelineTimelineItemKind {
    Commit,
    Save,
    Rename,
    Branch,
}

impl YTimelineTimelineItemKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Commit => 0,
            Self::Save => 1,
            Self::Rename => 2,
            Self::Branch => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Commit => "Commit",
            Self::Save => "Save",
            Self::Rename => "Rename",
            Self::Branch => "Branch",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YTimelineTimelineItemKind] {
        &[
            YTimelineTimelineItemKind::Commit,
            YTimelineTimelineItemKind::Save,
            YTimelineTimelineItemKind::Rename,
            YTimelineTimelineItemKind::Branch,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YTimelineTimelineItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks timeline range data.
#[derive(Debug, Clone)]
pub struct YTimelineTimelineRange {
    pub start_ms: u64,
    pub end_ms: u64,
    pub label: String,
}

impl YTimelineTimelineRange {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            start_ms: 0,
            end_ms: 0,
            label: String::new(),
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YTimelineTimelineRange({}: {:?})", "start_ms", self.start_ms)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_timeline_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_timeline_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_timeline_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_timeline_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_timeline_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_timeline_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_timeline_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_timeline_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// timeline – Extended timeline cursor helpers
// ---------------------------------------------------------------------------

/// Priority levels for timeline cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZTimelinePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZTimelinePriority {
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
    pub fn all_asc() -> [ZTimelinePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZTimelinePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks timeline cursor data.
#[derive(Debug, Clone)]
pub struct ZTimelineTimelineCursor {
    pub positions_ms: Vec<u64>,
    pub current_idx: usize,
    pub looping: bool,
}

impl ZTimelineTimelineCursor {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            positions_ms: Vec::new(),
            current_idx: 0,
            looping: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.positions_ms.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.positions_ms.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.positions_ms.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZTimelineTimelineCursor[current_idx={:?}, looping={:?}]", self.current_idx, self.looping)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.looping = !c.looping;
        c
    }
}

/// Compute a simple rolling hash for timeline cursor.
pub fn z_timeline_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_timeline_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_timeline_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_timeline_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_timeline_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_timeline_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_timeline_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 183
// ---------------------------------------------------------------------------

/// Generic object pool `Xc183Pool<T>`.
pub struct Xc183Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc183Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc183PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc183Pool<T> {
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
    pub fn stats(&self) -> Xc183PoolStats {
        Xc183PoolStats {
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

impl<T> Default for Xc183Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc183Scheduler`.
pub struct Xc183Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc183Scheduler {
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

impl Default for Xc183Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_183 hash for the given byte slice.
pub fn xc_183_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_183 convention.
pub fn xc_183_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_3 deepening: state machine + event bus ---

/// States for the Xd3 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd3State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd3State {
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
pub struct Xd3Transition {
    pub from: Xd3State,
    pub to: Xd3State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd3StateMachine {
    current: Xd3State,
    history: Vec<Xd3Transition>,
    step_counter: usize,
}

impl Xd3StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd3State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd3State {
        self.current
    }

    pub fn history(&self) -> &[Xd3Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd3State) -> Result<Xd3State, String> {
        let allowed = match (self.current, target) {
            (Xd3State::Idle, Xd3State::Running) => true,
            (Xd3State::Running, Xd3State::Paused) => true,
            (Xd3State::Running, Xd3State::Done) => true,
            (Xd3State::Paused, Xd3State::Running) => true,
            (Xd3State::Paused, Xd3State::Done) => true,
            (Xd3State::Done, Xd3State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_3: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd3Transition {
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
            "Xd3SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd3State> {
        let prefix = "Xd3SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd3State::Idle),
            "Running" => Some(Xd3State::Running),
            "Paused" => Some(Xd3State::Paused),
            "Done" => Some(Xd3State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd3State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd3 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd3Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd3Event {
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

type Xd3HandlerFn = Box<dyn Fn(&Xd3Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd3EventBus {
    handlers: Vec<(usize, Option<String>, Xd3HandlerFn)>,
    next_id: usize,
    published: Vec<Xd3Event>,
}

impl Xd3EventBus {
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
        F: Fn(&Xd3Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd3Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd3Event) {
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

    pub fn published_events(&self) -> &[Xd3Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #1
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf1Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf1TrieNode {
    children: std::collections::HashMap<char, Xf1TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf1Trie {
    root: Xf1TrieNode,
    count: usize,
}

impl Xf1Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf1TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf1TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf1TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf1BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf1BloomFilter {
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


// ---------------------------------------------------------------------------
// xg_120: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg120Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg120Graph {
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

impl Default for Xg120Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_120: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg120Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg120Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg120Heap<T>) {
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

impl<T: Ord> Default for Xg120Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 182).
pub struct Xh182SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh182SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 224 as u64,
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

/// A compact bit set supporting boolean operations (variant 182).
pub struct Xh182BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh182BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 182).
pub struct Xi182Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi182Deque<T> {
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
pub struct Xi182Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi182Interval {
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

/// A simple interval tree (variant 182).
pub struct Xi182IntervalTree {
    xi_intervals: Vec<Xi182Interval>,
}

impl Xi182IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi182Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi182Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi182Interval) -> Vec<&Xi182Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi182Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi182Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi182Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi182Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi182Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi182Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 182) ---

/// Disjoint set / union-find for crate 182.
pub struct Xj182UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj182UnionFind {
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

const XJ182_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 182.
pub struct Xj182BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj182BTreeNode<K, V>>>,
    len: usize,
}

struct Xj182BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj182BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj182BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ182_BTREE_ORDER - 1
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
        let mid = XJ182_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj182BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj182BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj182BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj182BTreeNode::xj_new_leaf();
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


// --- xk_182 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk182SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk182SegmentTree {
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
pub struct Xk182DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk182DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_182).
#[derive(Debug, Clone)]
pub struct Xl182Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl182Rope {
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

/// Suffix array for efficient string searching (xl_182).
#[derive(Debug, Clone)]
pub struct Xl182SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl182SuffixArray {
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
pub struct Xm182MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm182MatrixSparse {
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
pub struct Xm182Tokenizer {
    text: String,
}

impl Xm182Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 182.
pub struct Xn182Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn182Fenwick {
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

// ----- AVL tree map — crate 182 -----

#[derive(Debug, Clone)]
struct Xn182AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn182AvlNode<K, V>>>,
    right: Option<Box<Xn182AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 182.
#[derive(Debug, Clone)]
pub struct Xn182AVL<K, V> {
    root: Option<Box<Xn182AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn182AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn182AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn182AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn182AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn182AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn182AvlNode<K, V>>) -> Box<Xn182AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn182AvlNode<K, V>>) -> Box<Xn182AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn182AvlNode<K, V>>) -> Box<Xn182AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn182AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn182AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn182AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn182AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn182AvlNode<K, V>>) -> &Xn182AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn182AvlNode<K, V>>) -> (Box<Xn182AvlNode<K, V>>, Option<Box<Xn182AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn182AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn182AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn182AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn182AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn182AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn182AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn182AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo182RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo182Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo182RBNode<K, V> {
    key: K,
    value: V,
    color: Xo182Color,
    left: Option<Box<Xo182RBNode<K, V>>>,
    right: Option<Box<Xo182RBNode<K, V>>>,
}

/// A red-black tree map for crate 182.
#[derive(Debug, Clone)]
pub struct Xo182RedBlack<K, V> {
    root: Option<Box<Xo182RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo182RedBlack<K, V> {
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
            r.color = Xo182Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo182RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo182RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo182RBNode {
                    key, value, color: Xo182Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo182RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo182Color::Red)
    }

    fn xo_balance(mut h: Box<Xo182RBNode<K, V>>) -> Box<Xo182RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo182Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo182RBNode<K, V>>) -> Box<Xo182RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo182Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo182RBNode<K, V>>) -> Box<Xo182RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo182Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo182RBNode<K, V>>) {
        h.color = Xo182Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo182Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo182Color::Black; }
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
            r.color = Xo182Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo182RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo182RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo182RBNode<K, V>) -> (K, V, Option<Box<Xo182RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo182RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo182Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo182RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo182ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 182.
#[derive(Debug, Clone)]
pub struct Xo182ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo182ConsistentHash {
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
            let vkey = format!("{}#xo182#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo182#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 182).
#[derive(Debug)]
pub struct Xp182SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp182Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp182Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp182Node<K, V>>>,
    xp_right: Option<Box<Xp182Node<K, V>>>,
}

impl<K: Ord, V> Xp182Node<K, V> {
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

impl<K: Ord, V> Default for Xp182SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp182SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp182Node<K, V>>>, key: &K) -> Option<Box<Xp182Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp182Node<K, V>>) -> Box<Xp182Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp182Node<K, V>>) -> Box<Xp182Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp182Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp182Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp182Node::xp_new(key, val));
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


    #[test]
    fn provider_registry_basic() {
        let mut reg = TimelineProviderRegistry::new();
        reg.register("git", Box::new(GitTimelineProvider::new("/tmp")));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("git").is_some());
    }

    #[test]
    fn provider_registry_unregister() {
        let mut reg = TimelineProviderRegistry::new();
        reg.register("git", Box::new(GitTimelineProvider::new("/tmp")));
        assert!(reg.unregister("git"));
        assert!(reg.is_empty());
    }

    #[test]
    fn entry_compare_by_timestamp() {
        let a = TimelineItem { timestamp: 100, message: "a".into(), author: "x".into(), sha: "1".into() };
        let b = TimelineItem { timestamp: 200, message: "b".into(), author: "x".into(), sha: "2".into() };
        assert_eq!(TimelineEntryCompare::by_timestamp(&a, &b), std::cmp::Ordering::Less);
    }

    #[test]
    fn entry_compare_new_items() {
        let old = vec![TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "1".into() }];
        let new = vec![
            TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "1".into() },
            TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "2".into() },
        ];
        assert_eq!(TimelineEntryCompare::new_items(&old, &new).len(), 1);
    }

    #[test]
    fn entry_compare_removed_items() {
        let old = vec![
            TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "1".into() },
            TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "2".into() },
        ];
        let new = vec![TimelineItem { timestamp: 0, message: "".into(), author: "".into(), sha: "1".into() }];
        assert_eq!(TimelineEntryCompare::removed_items(&old, &new).len(), 1);
    }

    #[test]
    fn debouncer_basic() {
        let mut d = TimelineRefreshDebouncer::new(100);
        assert!(d.request_refresh(100));
        assert!(!d.request_refresh(150));
        assert!(d.is_pending());
        assert!(d.request_refresh(200));
    }

    #[test]
    fn debouncer_check_pending() {
        let mut d = TimelineRefreshDebouncer::new(100);
        d.request_refresh(100);
        d.request_refresh(150);
        assert!(d.check_pending(200));
    }

    #[test]
    fn export_csv() {
        let items = vec![TimelineItem { timestamp: 100, message: "init".into(), author: "dev".into(), sha: "abc".into() }];
        let csv = TimelineExportFormatter::to_csv(&items);
        assert!(csv.contains("abc"));
        assert!(csv.contains("dev"));
    }

    #[test]
    fn export_json() {
        let items = vec![TimelineItem { timestamp: 100, message: "init".into(), author: "dev".into(), sha: "abc".into() }];
        let json = TimelineExportFormatter::to_json_text(&items);
        assert!(json.contains("abc"));
    }

    #[test]
    fn export_plain() {
        let items = vec![TimelineItem { timestamp: 100, message: "init".into(), author: "dev".into(), sha: "abc".into() }];
        let text = TimelineExportFormatter::to_plain_text(&items);
        assert!(text.contains("init"));
    }

    #[test]
    fn provider_registry_ids() {
        let mut reg = TimelineProviderRegistry::new();
        reg.register("git", Box::new(GitTimelineProvider::new("/tmp")));
        assert_eq!(reg.provider_ids(), vec!["git"]);
    }

    #[test]
    fn debouncer_default() {
        let d = TimelineRefreshDebouncer::default();
        assert_eq!(d.debounce_ms(), 500);
    }


    #[test]
    fn timelineEventFilter_new() {
        let s = TimelineEventFilter::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn timelineEventFilter_add_contains() {
        let mut s = TimelineEventFilter::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn timelineEventFilter_add_duplicate() {
        let mut s = TimelineEventFilter::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn timelineEventFilter_remove() {
        let mut s = TimelineEventFilter::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn timelineEventFilter_capacity() {
        let s = TimelineEventFilter::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn timelineEventFilter_search() {
        let mut s = TimelineEventFilter::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn timelineEventFilter_stats() {
        let mut s = TimelineEventFilter::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn timelineRangeSelector_new() {
        let m = TimelineRangeSelector::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn timelineRangeSelector_add_find() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn timelineRangeSelector_priority_filter() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("a", "A").with_priority(TimelineRangeSelectorPriority::High));
        m.add(TimelineRangeSelectorItem::new("b", "B").with_priority(TimelineRangeSelectorPriority::Low));
        m.add(TimelineRangeSelectorItem::new("c", "C").with_priority(TimelineRangeSelectorPriority::High));
        assert_eq!(m.by_priority(TimelineRangeSelectorPriority::High).len(), 2);
    }

    #[test]
    fn timelineRangeSelector_remove() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn timelineRangeSelector_search() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("id1", "Hello World"));
        m.add(TimelineRangeSelectorItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn timelineRangeSelector_total_weight() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("a", "A").with_priority(TimelineRangeSelectorPriority::Critical));
        m.add(TimelineRangeSelectorItem::new("b", "B").with_priority(TimelineRangeSelectorPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn timelineRangeSelector_capacity_limit() {
        let mut m = TimelineRangeSelector::new().with_max_items(2);
        m.add(TimelineRangeSelectorItem::new("1", "one"));
        m.add(TimelineRangeSelectorItem::new("2", "two"));
        assert!(!m.add(TimelineRangeSelectorItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn timelineRangeSelector_sorted_by_priority() {
        let mut m = TimelineRangeSelector::new();
        m.add(TimelineRangeSelectorItem::new("lo", "Low").with_priority(TimelineRangeSelectorPriority::Low));
        m.add(TimelineRangeSelectorItem::new("hi", "High").with_priority(TimelineRangeSelectorPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn timelineRangeSelector_item_metadata() {
        let mut item = TimelineRangeSelectorItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn timelineEventFilter_enabled_toggle() {
        let mut s = TimelineEventFilter::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn timelineRangeSelector_priority_display() {
        assert_eq!(format!("{}", TimelineRangeSelectorPriority::High), "high");
        assert_eq!(format!("{}", TimelineRangeSelectorPriority::Low), "low");
    }


    #[test]
    fn tlbuf_ringbuf_push_get() {
        let mut rb = TlBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn tlbuf_ringbuf_overflow() {
        let mut rb = TlBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn tlbuf_ringbuf_clear() {
        let mut rb = TlBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn tlbuf_ringbuf_newest_oldest() {
        let mut rb = TlBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn tlbuf_ringbuf_to_vec() {
        let mut rb = TlBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn tlbuf_ringbuf_is_full() {
        let mut rb = TlBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn tlfmt_fmt_list() {
        let f = TlFmtFmt::new(TlFmtFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn tlfmt_fmt_kv() {
        let f = TlFmtFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn tlfmt_fmt_section() {
        let f = TlFmtFmt::new(TlFmtFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn tlfmt_fmt_truncate() {
        let f = TlFmtFmt::new(TlFmtFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn tlfmt_fmt_opts_defaults() {
        let o = TlFmtFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    // -- timeline extended domain tests ----------------------------------------

    #[test]
    fn y_timeline_enum_index() {
        assert_eq!(YTimelineTimelineItemKind::Commit.index(), 0);
        assert_eq!(YTimelineTimelineItemKind::Save.index(), 1);
        assert_eq!(YTimelineTimelineItemKind::Rename.index(), 2);
        assert_eq!(YTimelineTimelineItemKind::Branch.index(), 3);
    }

    #[test]
    fn y_timeline_enum_label() {
        assert_eq!(YTimelineTimelineItemKind::Commit.label(), "Commit");
        assert_eq!(YTimelineTimelineItemKind::Save.label(), "Save");
        assert_eq!(YTimelineTimelineItemKind::Rename.label(), "Rename");
        assert_eq!(YTimelineTimelineItemKind::Branch.label(), "Branch");
    }

    #[test]
    fn y_timeline_enum_all() {
        let all = YTimelineTimelineItemKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_timeline_enum_is_default() {
        assert!(YTimelineTimelineItemKind::Commit.is_default());
        assert!(!YTimelineTimelineItemKind::Branch.is_default());
    }

    #[test]
    fn y_timeline_enum_display() {
        assert_eq!(format!("{}", YTimelineTimelineItemKind::Commit), "Commit");
    }

    #[test]
    fn y_timeline_struct_new() {
        let s = YTimelineTimelineRange::new();
        let _ = s.summary();
    }

    #[test]
    fn y_timeline_fingerprint_deterministic() {
        let h1 = y_timeline_fingerprint("hello");
        let h2 = y_timeline_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_timeline_fingerprint("a"), y_timeline_fingerprint("b"));
    }

    #[test]
    fn y_timeline_truncate_short() {
        assert_eq!(y_timeline_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_timeline_truncate_long() {
        let r = y_timeline_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_timeline_normalize_key_basic() {
        assert_eq!(y_timeline_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_timeline_split_path_basic() {
        let parts = y_timeline_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_timeline_count_occurrences_basic() {
        assert_eq!(y_timeline_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_timeline_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_timeline_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_timeline_in_range_basic() {
        assert!(y_timeline_in_range(5, 1, 10));
        assert!(y_timeline_in_range(1, 1, 10));
        assert!(y_timeline_in_range(10, 1, 10));
        assert!(!y_timeline_in_range(0, 1, 10));
        assert!(!y_timeline_in_range(11, 1, 10));
    }

    #[test]
    fn y_timeline_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_timeline_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_timeline_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_timeline_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- timeline Z-extended tests -----------------------------------------------

    #[test]
    fn z_timeline_priority_weight() {
        assert_eq!(ZTimelinePriority::Idle.weight(), 0);
        assert_eq!(ZTimelinePriority::Normal.weight(), 2);
        assert_eq!(ZTimelinePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_timeline_priority_label() {
        assert_eq!(ZTimelinePriority::Low.label(), "low");
        assert_eq!(ZTimelinePriority::High.label(), "high");
    }

    #[test]
    fn z_timeline_priority_is_elevated() {
        assert!(!ZTimelinePriority::Normal.is_elevated());
        assert!(ZTimelinePriority::High.is_elevated());
        assert!(ZTimelinePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_timeline_priority_display() {
        assert_eq!(format!("{}", ZTimelinePriority::Idle), "idle");
    }

    #[test]
    fn z_timeline_priority_all_asc() {
        let all = ZTimelinePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZTimelinePriority::Idle);
        assert_eq!(all[4], ZTimelinePriority::Realtime);
    }

    #[test]
    fn z_timeline_struct_new() {
        let s = ZTimelineTimelineCursor::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_timeline_struct_toggled_clone() {
        let s = ZTimelineTimelineCursor::new();
        let t = s.toggled_clone();
        assert_ne!(s.looping, t.looping);
    }

    #[test]
    fn z_timeline_rolling_hash_deterministic() {
        let h1 = z_timeline_rolling_hash(b"test");
        let h2 = z_timeline_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_timeline_rolling_hash(b"a"), z_timeline_rolling_hash(b"b"));
    }

    #[test]
    fn z_timeline_pad_to_basic() {
        assert_eq!(z_timeline_pad_to("hi", 5), "hi   ");
        assert_eq!(z_timeline_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_timeline_is_identifier_basic() {
        assert!(z_timeline_is_identifier("foo_bar"));
        assert!(z_timeline_is_identifier("abc123"));
        assert!(!z_timeline_is_identifier(""));
        assert!(!z_timeline_is_identifier("has space"));
    }

    #[test]
    fn z_timeline_levenshtein_basic() {
        assert_eq!(z_timeline_levenshtein("", ""), 0);
        assert_eq!(z_timeline_levenshtein("abc", "abc"), 0);
        assert_eq!(z_timeline_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_timeline_unique_words_basic() {
        let w = z_timeline_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_timeline_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_timeline_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_timeline_common_prefix_basic() {
        assert_eq!(z_timeline_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_timeline_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_timeline_struct_clear() {
        let mut s = ZTimelineTimelineCursor::new();
        s.positions_ms.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_timeline_rolling_hash_empty() {
        let h = z_timeline_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 183 ----

    #[test]
    fn xc_183_pool_new_empty() {
        let pool: super::Xc183Pool<i32> = super::Xc183Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_183_pool_release_acquire() {
        let mut pool = super::Xc183Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_183_pool_acquire_empty() {
        let mut pool: super::Xc183Pool<i32> = super::Xc183Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_183_pool_full() {
        let mut pool = super::Xc183Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_183_pool_drain() {
        let mut pool = super::Xc183Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_183_pool_stats() {
        let mut pool = super::Xc183Pool::new(8);
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
    fn xc_183_pool_clear() {
        let mut pool = super::Xc183Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_183_pool_shrink() {
        let mut pool = super::Xc183Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_183_pool_default() {
        let pool: super::Xc183Pool<String> = super::Xc183Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_183_pool_extend() {
        let mut pool = super::Xc183Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_183_pool_retain() {
        let mut pool = super::Xc183Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_183_scheduler_round_robin() {
        let mut sched = super::Xc183Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_183_scheduler_empty() {
        let mut sched = super::Xc183Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_183_scheduler_reset() {
        let mut sched = super::Xc183Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_183_scheduler_add_remove() {
        let mut sched = super::Xc183Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_183_scheduler_targets() {
        let sched = super::Xc183Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_183_hash_empty() {
        assert_eq!(super::xc_183_hash(b""), 5381);
    }

    #[test]
    fn xc_183_hash_data() {
        let h = super::xc_183_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_183_hash(b"hello"), h);
    }

    #[test]
    fn xc_183_reverse_str() {
        assert_eq!(super::xc_183_reverse("abc"), "cba");
        assert_eq!(super::xc_183_reverse(""), "");
    }


    // --- xd_3 deepening tests ---

    #[test]
    fn xd_3_sm_initial_state() {
        let sm = Xd3StateMachine::new();
        assert_eq!(sm.current_state(), Xd3State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_3_sm_valid_idle_to_running() {
        let mut sm = Xd3StateMachine::new();
        assert!(sm.transition(Xd3State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd3State::Running);
    }

    #[test]
    fn xd_3_sm_valid_running_to_paused() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        assert!(sm.transition(Xd3State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd3State::Paused);
    }

    #[test]
    fn xd_3_sm_valid_running_to_done() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        assert!(sm.transition(Xd3State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd3State::Done);
    }

    #[test]
    fn xd_3_sm_valid_paused_to_running() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        sm.transition(Xd3State::Paused).unwrap();
        assert!(sm.transition(Xd3State::Running).is_ok());
    }

    #[test]
    fn xd_3_sm_valid_done_to_idle() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        sm.transition(Xd3State::Done).unwrap();
        assert!(sm.transition(Xd3State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd3State::Idle);
    }

    #[test]
    fn xd_3_sm_invalid_idle_to_done() {
        let mut sm = Xd3StateMachine::new();
        assert!(sm.transition(Xd3State::Done).is_err());
    }

    #[test]
    fn xd_3_sm_invalid_idle_to_paused() {
        let mut sm = Xd3StateMachine::new();
        assert!(sm.transition(Xd3State::Paused).is_err());
    }

    #[test]
    fn xd_3_sm_history_tracking() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        sm.transition(Xd3State::Paused).unwrap();
        sm.transition(Xd3State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd3State::Idle);
        assert_eq!(sm.history()[0].to, Xd3State::Running);
        assert_eq!(sm.history()[1].from, Xd3State::Running);
        assert_eq!(sm.history()[2].to, Xd3State::Done);
    }

    #[test]
    fn xd_3_sm_serialize_deserialize() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd3StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd3State::Running));
    }

    #[test]
    fn xd_3_sm_deserialize_invalid() {
        assert_eq!(Xd3StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_3_sm_reset() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd3State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_3_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd3EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd3Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_3_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd3EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd3Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd3Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_3_bus_unsubscribe() {
        let mut bus = Xd3EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_3_event_kind_and_payload() {
        let e = Xd3Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd3Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_3_bus_clear_history() {
        let mut bus = Xd3EventBus::new();
        bus.publish(Xd3Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_3_sm_step_counter_increments() {
        let mut sm = Xd3StateMachine::new();
        sm.transition(Xd3State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd3State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #1 --

    #[test]
    fn xf1_trie_insert_search() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf1_trie_starts_with() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf1_trie_remove() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf1_trie_word_count() {
        let mut t = Xf1Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf1_trie_longest_prefix() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf1_trie_all_words() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf1_trie_autocomplete() {
        let mut t = Xf1Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf1_trie_empty_search() {
        let t = Xf1Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf1_bloom_add_contains() {
        let mut bf = Xf1BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf1_bloom_probably_absent() {
        let bf = Xf1BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf1_bloom_false_positive_rate() {
        let mut bf = Xf1BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf1_bloom_clear() {
        let mut bf = Xf1BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf1_bloom_union() {
        let mut a = Xf1BloomFilter::xf_new(512, 2);
        let mut b = Xf1BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf1_bloom_intersection_estimate() {
        let mut a = Xf1BloomFilter::xf_new(512, 2);
        let mut b = Xf1BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf1_bloom_union_size_mismatch() {
        let a = Xf1BloomFilter::xf_new(256, 2);
        let b = Xf1BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    // -- xg_120 graph tests ------------------------------------------------

    #[test]
    fn xg_120_graph_empty() {
        let g = super::Xg120Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_120_graph_add_node() {
        let mut g = super::Xg120Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_120_graph_add_edge() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_120_graph_neighbors() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_120_graph_has_path() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_120_graph_self_path() {
        let g = super::Xg120Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_120_graph_topo_sort() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_120_graph_cycle_detect_false() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_120_graph_cycle_detect_true() {
        let mut g = super::Xg120Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_120 heap tests -------------------------------------------------

    #[test]
    fn xg_120_heap_empty() {
        let h: super::Xg120Heap<i32> = super::Xg120Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_120_heap_push_pop() {
        let mut h = super::Xg120Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_120_heap_peek() {
        let mut h = super::Xg120Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_120_heap_drain_sorted() {
        let mut h = super::Xg120Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_120_heap_merge() {
        let mut a = super::Xg120Heap::new();
        let mut b = super::Xg120Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_120_heap_default() {
        let h: super::Xg120Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_120_graph_default() {
        let g: super::Xg120Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh182_skip_insert_contains() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh182_skip_remove() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh182_skip_len() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh182_skip_range_query() {
        let mut sl = super::Xh182SkipList::xh_new(4);
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
    fn xh182_skip_floor_ceiling() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh182_skip_rank() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh182_skip_empty() {
        let sl = super::Xh182SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh182_skip_duplicates() {
        let mut sl = super::Xh182SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh182_bitset_set_test() {
        let mut bs = super::Xh182BitSet::xh_new(256);
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
    fn xh182_bitset_clear_count() {
        let mut bs = super::Xh182BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh182_bitset_and_or_xor() {
        let mut a = super::Xh182BitSet::xh_new(128);
        let mut b = super::Xh182BitSet::xh_new(128);
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
    fn xh182_bitset_iter_ones() {
        let mut bs = super::Xh182BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh182_bitset_first_last() {
        let mut bs = super::Xh182BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh182_bitset_empty() {
        let bs = super::Xh182BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi182_deque_push_pop_back() {
        let mut dq = super::Xi182Deque::xi_new(4);
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
    fn xi182_deque_push_pop_front() {
        let mut dq = super::Xi182Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi182_deque_mixed_ops() {
        let mut dq = super::Xi182Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi182_deque_get_and_split() {
        let mut dq = super::Xi182Deque::xi_new(8);
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
    fn xi182_deque_rotate_left() {
        let mut dq = super::Xi182Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi182_deque_rotate_right() {
        let mut dq = super::Xi182Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi182_deque_grow() {
        let mut dq = super::Xi182Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi182_deque_empty() {
        let dq = super::Xi182Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi182_interval_tree_insert_query() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi182Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi182Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi182_interval_tree_overlap() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi182Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi182Interval::xi_new(12, 20));
        let q = super::Xi182Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi182_interval_tree_remove() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi182Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi182_interval_tree_gaps() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi182Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi182Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi182Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi182Interval::xi_new(8, 10));
    }

    #[test]
    fn xi182_interval_tree_merge() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi182Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi182Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi182Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi182Interval::xi_new(10, 15));
    }

    #[test]
    fn xi182_interval_tree_all() {
        let mut tree = super::Xi182IntervalTree::xi_new();
        tree.xi_insert(super::Xi182Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi182Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi182_interval_tree_empty() {
        let tree = super::Xi182IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi182_interval_tree_contains_point() {
        let iv = super::Xi182Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 182) ---

    #[test]
    fn xj_182_uf_make_and_find() {
        let mut uf = super::Xj182UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_182_uf_union_connected() {
        let mut uf = super::Xj182UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_182_uf_component_count() {
        let mut uf = super::Xj182UnionFind::xj_new();
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
    fn xj_182_uf_component_size() {
        let mut uf = super::Xj182UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_182_uf_largest_component() {
        let mut uf = super::Xj182UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_182_uf_many_elements() {
        let mut uf = super::Xj182UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_182_uf_separate_components() {
        let mut uf = super::Xj182UnionFind::xj_new();
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
    fn xj_182_uf_path_compression() {
        let mut uf = super::Xj182UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_182_bt_insert_get() {
        let mut bt = super::Xj182BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_182_bt_contains_len() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_182_bt_replace() {
        let mut bt = super::Xj182BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_182_bt_remove() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_182_bt_keys_values() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_182_bt_range() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_182_bt_min_max() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_182_bt_many_inserts() {
        let mut bt = super::Xj182BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_182 segment tree tests ---

    #[test]
    fn xk_182_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_182_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk182SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_182_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_182_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_182_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_182_st_single_element() {
        let data = vec![42];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_182_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk182SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_182_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk182SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_182 disjoint intervals tests ---

    #[test]
    fn xk_182_di_add_and_count() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_182_di_merge_overlap() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_182_di_contains() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_182_di_remove() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_182_di_covered_length() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_182_di_gaps() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_182_di_merge_adjacent() {
        let mut di = super::Xk182DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_182_di_empty() {
        let di = super::Xk182DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_182_rope_new_empty() {
        let rope = super::Xl182Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_182_rope_from_str() {
        let rope = super::Xl182Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_182_rope_insert_at() {
        let mut rope = super::Xl182Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_182_rope_delete_range() {
        let mut rope = super::Xl182Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_182_rope_char_at() {
        let rope = super::Xl182Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_182_rope_split_concat() {
        let rope = super::Xl182Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_182_rope_line_count() {
        let rope = super::Xl182Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_182_rope_line_at() {
        let rope = super::Xl182Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_182_sa_build_and_search() {
        let sa = super::Xl182SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_182_sa_count() {
        let sa = super::Xl182SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_182_sa_longest_repeated() {
        let sa = super::Xl182SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_182_sa_all_positions() {
        let sa = super::Xl182SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_182_sa_len() {
        let sa = super::Xl182SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_182_sa_empty() {
        let sa = super::Xl182SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_182_rope_slice() {
        let rope = super::Xl182Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_182_sa_search_start() {
        let sa = super::Xl182SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_182_sparse_set_get() {
        let mut m = super::Xm182MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_182_sparse_row_col() {
        let mut m = super::Xm182MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_182_sparse_transpose() {
        let mut m = super::Xm182MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_182_sparse_multiply_vec() {
        let mut m = super::Xm182MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_182_sparse_nnz_density() {
        let mut m = super::Xm182MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_182_sparse_clear() {
        let mut m = super::Xm182MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_182_sparse_overwrite_zero() {
        let mut m = super::Xm182MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_182_tokenizer_basic() {
        let t = super::Xm182Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_182_tokenizer_count() {
        let t = super::Xm182Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_182_tokenizer_unique() {
        let t = super::Xm182Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_182_tokenizer_frequency() {
        let t = super::Xm182Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_182_tokenizer_delimiter() {
        let t = super::Xm182Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_182_tokenizer_whitespace() {
        let t = super::Xm182Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_182_tokenizer_empty() {
        let t = super::Xm182Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 182 ----

    #[test]
    fn xn_182_fenwick_prefix_sum() {
        let mut ft = super::Xn182Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_182_fenwick_range_sum() {
        let mut ft = super::Xn182Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_182_fenwick_point_query() {
        let mut ft = super::Xn182Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_182_fenwick_len() {
        let ft = super::Xn182Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_182_fenwick_multiple_updates() {
        let mut ft = super::Xn182Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_182_fenwick_single_element() {
        let mut ft = super::Xn182Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_182_fenwick_find_kth() {
        let mut ft = super::Xn182Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_182_fenwick_negative_delta() {
        let mut ft = super::Xn182Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 182 ----

    #[test]
    fn xn_182_avl_insert_get() {
        let mut m = super::Xn182AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_182_avl_remove() {
        let mut m = super::Xn182AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_182_avl_in_order() {
        let mut m = super::Xn182AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_182_avl_min_max() {
        let mut m = super::Xn182AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_182_avl_floor_ceiling() {
        let mut m = super::Xn182AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_182_avl_height_balanced() {
        let mut m = super::Xn182AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_182_avl_overwrite() {
        let mut m = super::Xn182AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_182_avl_empty() {
        let m: super::Xn182AVL<i32, i32> = super::Xn182AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo182RedBlack tests ---

    #[test]
    fn xo_182_rb_insert_and_get() {
        let mut tree = super::Xo182RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_182_rb_len_and_empty() {
        let mut tree = super::Xo182RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_182_rb_min_max() {
        let mut tree = super::Xo182RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_182_rb_contains() {
        let mut tree = super::Xo182RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_182_rb_remove() {
        let mut tree = super::Xo182RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_182_rb_in_order() {
        let mut tree = super::Xo182RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_182_rb_black_height() {
        let mut tree = super::Xo182RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_182_rb_overwrite() {
        let mut tree = super::Xo182RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo182ConsistentHash tests ---

    #[test]
    fn xo_182_ch_add_and_count() {
        let mut ring = super::Xo182ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_182_ch_remove_node() {
        let mut ring = super::Xo182ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_182_ch_get_node() {
        let mut ring = super::Xo182ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_182_ch_empty_ring() {
        let ring = super::Xo182ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_182_ch_distribution() {
        let mut ring = super::Xo182ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_182_ch_rebalance() {
        let mut ring = super::Xo182ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_182_ch_virtual_nodes() {
        let mut ring = super::Xo182ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_182_ch_consistent_lookup() {
        let mut ring = super::Xo182ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_182_splay_insert_get() {
        let mut t = super::Xp182SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_182_splay_remove() {
        let mut t = super::Xp182SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_182_splay_count_increases() {
        let mut t = super::Xp182SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_182_splay_depth() {
        let mut t = super::Xp182SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_182_splay_len_empty() {
        let t = super::Xp182SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_182_splay_min_max() {
        let mut t = super::Xp182SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_182_splay_overwrite() {
        let mut t = super::Xp182SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_182_splay_remove_missing() {
        let mut t = super::Xp182SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}