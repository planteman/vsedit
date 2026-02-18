//! Dirty file tracking.

use std::fmt;

/// Status of a resource in source control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScmStatus {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflict,
    Ignored,
}

/// A single source-control resource.
#[derive(Debug, Clone)]
pub struct ScmResource {
    pub uri: String,
    pub status: ScmStatus,
    pub original_uri: Option<String>,
}

/// A logical group of SCM resources (e.g. "Changes", "Staged").
#[derive(Debug, Clone)]
pub struct ScmGroup {
    pub id: String,
    pub label: String,
    pub resources: Vec<ScmResource>,
}

/// A source-control provider (e.g. Git).
#[derive(Debug, Clone)]
pub struct ScmProvider {
    pub id: String,
    pub label: String,
    pub root_uri: String,
    pub groups: Vec<ScmGroup>,
    pub count: usize,
}

impl ScmProvider {
    /// Number of groups in this provider.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Returns true when no group contains any resources.
    pub fn is_clean(&self) -> bool {
        self.groups.iter().all(|g| g.resources.is_empty())
    }
}

/// Aggregated statistics for a provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScmStats {
    pub total: usize,
    pub modified: usize,
    pub added: usize,
    pub deleted: usize,
    pub conflict: usize,
    pub untracked: usize,
}

/// Returns a human-readable label for a status variant.
pub fn status_label(status: ScmStatus) -> &'static str {
    match status {
        ScmStatus::Untracked => "Untracked",
        ScmStatus::Modified => "Modified",
        ScmStatus::Added => "Added",
        ScmStatus::Deleted => "Deleted",
        ScmStatus::Renamed => "Renamed",
        ScmStatus::Conflict => "Conflict",
        ScmStatus::Ignored => "Ignored",
    }
}

/// Service for working-copy workbench functionality.
pub struct WorkingCopyService {
    providers: Vec<ScmProvider>,
}

impl WorkingCopyService {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register_provider(&mut self, provider: ScmProvider) {
        self.providers.push(provider);
    }

    pub fn get_provider(&self, id: &str) -> Option<&ScmProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn get_provider_mut(&mut self, id: &str) -> Option<&mut ScmProvider> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    /// Adds a resource to a specific group within a provider.
    pub fn add_resource(&mut self, provider_id: &str, group_id: &str, resource: ScmResource) {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if let Some(group) = provider.groups.iter_mut().find(|g| g.id == group_id) {
                group.resources.push(resource);
                provider.count += 1;
            }
        }
    }

    /// Returns resources matching a given status across all groups for a provider.
    pub fn get_resources(&self, provider_id: &str, status: ScmStatus) -> Vec<&ScmResource> {
        self.providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .filter(|r| r.status == status)
            .collect()
    }

    /// Total number of changes across all providers.
    pub fn total_changes(&self) -> usize {
        self.providers.iter().map(|p| p.count).sum()
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Removes a resource by URI from a specific group. Returns true if found.
    pub fn remove_resource(&mut self, provider_id: &str, group_id: &str, uri: &str) -> bool {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if let Some(group) = provider.groups.iter_mut().find(|g| g.id == group_id) {
                if let Some(pos) = group.resources.iter().position(|r| r.uri == uri) {
                    group.resources.remove(pos);
                    provider.count = provider.count.saturating_sub(1);
                    return true;
                }
            }
        }
        false
    }

    /// Moves a resource from one group to another within the same provider.
    pub fn move_resource(
        &mut self,
        provider_id: &str,
        from_group: &str,
        to_group: &str,
        uri: &str,
    ) -> bool {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            let resource = {
                let src = provider.groups.iter_mut().find(|g| g.id == from_group);
                if let Some(group) = src {
                    if let Some(pos) = group.resources.iter().position(|r| r.uri == uri) {
                        Some(group.resources.remove(pos))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some(res) = resource {
                if let Some(dst) = provider.groups.iter_mut().find(|g| g.id == to_group) {
                    dst.resources.push(res);
                    return true;
                }
            }
        }
        false
    }

    /// Returns all resources across every group for a given provider.
    pub fn get_all_resources(&self, provider_id: &str) -> Vec<&ScmResource> {
        self.providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .collect()
    }

    /// Returns true if any resource in the provider has `Conflict` status.
    pub fn has_conflicts(&self, provider_id: &str) -> bool {
        self.providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .any(|r| r.status == ScmStatus::Conflict)
    }

    /// Computes aggregate statistics for a provider.
    pub fn get_stats(&self, provider_id: &str) -> ScmStats {
        let mut stats = ScmStats::default();
        for res in self.get_all_resources(provider_id) {
            stats.total += 1;
            match res.status {
                ScmStatus::Modified => stats.modified += 1,
                ScmStatus::Added => stats.added += 1,
                ScmStatus::Deleted => stats.deleted += 1,
                ScmStatus::Conflict => stats.conflict += 1,
                ScmStatus::Untracked => stats.untracked += 1,
                _ => {}
            }
        }
        stats
    }

    /// Adds a new group to a provider. Returns false if the provider is missing
    /// or a group with the same id already exists.
    pub fn add_group(&mut self, provider_id: &str, group: ScmGroup) -> bool {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if provider.groups.iter().any(|g| g.id == group.id) {
                return false;
            }
            provider.groups.push(group);
            return true;
        }
        false
    }

    /// Removes a group from a provider, adjusting the count. Returns false if
    /// the provider or group is missing.
    pub fn remove_group(&mut self, provider_id: &str, group_id: &str) -> bool {
        if let Some(provider) = self.providers.iter_mut().find(|p| p.id == provider_id) {
            if let Some(pos) = provider.groups.iter().position(|g| g.id == group_id) {
                let removed = provider.groups.remove(pos);
                provider.count = provider.count.saturating_sub(removed.resources.len());
                return true;
            }
        }
        false
    }

    /// Searches all groups for a resource by URI, returning the group id and resource.
    pub fn find_resource<'a>(
        &'a self,
        provider_id: &str,
        uri: &str,
    ) -> Option<(&'a str, &'a ScmResource)> {
        self.providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| &p.groups)
            .flat_map(|g| g.resources.iter().map(move |r| (g.id.as_str(), r)))
            .find(|(_, r)| r.uri == uri)
    }
}

impl Default for WorkingCopyService {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level statistics for a working copy snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingCopyStats {
    pub total_files: usize,
    pub dirty_count: usize,
    pub staged_count: usize,
    pub untracked_count: usize,
}

impl WorkingCopyStats {
    /// Build stats from a provider by inspecting well-known group ids.
    ///
    /// Resources in a group whose id is `"staged"` count as staged.
    /// Resources with `ScmStatus::Untracked` count as untracked regardless
    /// of group.  All remaining non-untracked, non-staged resources count as
    /// dirty.
    pub fn from_provider(provider: &ScmProvider) -> Self {
        let mut stats = WorkingCopyStats::default();
        for group in &provider.groups {
            for res in &group.resources {
                stats.total_files += 1;
                if group.id == "staged" {
                    stats.staged_count += 1;
                } else if res.status == ScmStatus::Untracked {
                    stats.untracked_count += 1;
                } else {
                    stats.dirty_count += 1;
                }
            }
        }
        stats
    }

    /// Returns true when nothing is dirty, staged, or untracked.
    pub fn is_clean(&self) -> bool {
        self.dirty_count == 0 && self.staged_count == 0 && self.untracked_count == 0
    }
}

/// Summary of insertions and deletions between two text snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiffSummary {
    pub insertions: usize,
    pub deletions: usize,
    pub file_path: String,
}

impl DiffSummary {
    /// Total number of changed lines (insertions + deletions).
    pub fn total_changes(&self) -> usize {
        self.insertions + self.deletions
    }

    /// Returns true when there are no differences.
    pub fn is_empty(&self) -> bool {
        self.insertions == 0 && self.deletions == 0
    }
}

/// Compute a line-based diff summary between an original and modified text.
///
/// Uses a simple longest-common-subsequence (LCS) approach on lines to count
/// how many lines were inserted and how many were deleted.  The `file_path`
/// field of the returned summary is left empty; callers can set it afterwards.
pub fn compute_diff_summary(original: &str, modified: &str) -> DiffSummary {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mod_lines: Vec<&str> = modified.lines().collect();

    let n = orig_lines.len();
    let m = mod_lines.len();

    // Build LCS length table.
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if orig_lines[i - 1] == mod_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    let lcs_len = dp[n][m];
    let deletions = n.saturating_sub(lcs_len);
    let insertions = m.saturating_sub(lcs_len);

    DiffSummary {
        insertions,
        deletions,
        file_path: String::new(),
    }
}

/// Filter criteria for selecting working-copy resources.
#[derive(Debug, Clone, Default)]
pub struct WorkingCopyFilter {
    /// If set, only resources with this status are included.
    pub status: Option<ScmStatus>,
    /// If set, only resources whose URI contains this substring are included.
    pub path_pattern: Option<String>,
}

impl WorkingCopyFilter {
    /// Create a filter that matches a single status.
    pub fn by_status(status: ScmStatus) -> Self {
        Self {
            status: Some(status),
            path_pattern: None,
        }
    }

    /// Create a filter that matches URIs containing `pattern`.
    pub fn by_path(pattern: impl Into<String>) -> Self {
        Self {
            status: None,
            path_pattern: Some(pattern.into()),
        }
    }

    /// Returns true when the resource satisfies all set criteria.
    pub fn matches(&self, resource: &ScmResource) -> bool {
        if let Some(st) = self.status {
            if resource.status != st {
                return false;
            }
        }
        if let Some(ref pat) = self.path_pattern {
            if !resource.uri.contains(pat.as_str()) {
                return false;
            }
        }
        true
    }

    /// Apply this filter to a slice of resources, returning matching ones.
    pub fn apply<'a>(&self, resources: &'a [ScmResource]) -> Vec<&'a ScmResource> {
        resources.iter().filter(|r| self.matches(r)).collect()
    }
}

impl WorkingCopyService {
    /// Returns resources from a provider that match the given filter.
    pub fn filter_resources(&self, provider_id: &str, filter: &WorkingCopyFilter) -> Vec<&ScmResource> {
        self.providers
            .iter()
            .filter(|p| p.id == provider_id)
            .flat_map(|p| &p.groups)
            .flat_map(|g| &g.resources)
            .filter(|r| filter.matches(r))
            .collect()
    }
}

/// Sort order for SCM resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScmSortOrder {
    ByStatus,
    ByUri,
    ByStatusThenUri,
}

/// Sorts SCM resources in place.
pub struct ScmResourceSorter;

impl ScmResourceSorter {
    fn status_rank(status: ScmStatus) -> u8 {
        match status {
            ScmStatus::Conflict => 0,
            ScmStatus::Modified => 1,
            ScmStatus::Added => 2,
            ScmStatus::Deleted => 3,
            ScmStatus::Renamed => 4,
            ScmStatus::Untracked => 5,
            ScmStatus::Ignored => 6,
        }
    }

    /// Sort resources by the given order.
    pub fn sort(resources: &mut [ScmResource], order: ScmSortOrder) {
        match order {
            ScmSortOrder::ByStatus => {
                resources.sort_by_key(|r| Self::status_rank(r.status));
            }
            ScmSortOrder::ByUri => {
                resources.sort_by(|a, b| a.uri.cmp(&b.uri));
            }
            ScmSortOrder::ByStatusThenUri => {
                resources.sort_by(|a, b| {
                    Self::status_rank(a.status)
                        .cmp(&Self::status_rank(b.status))
                        .then_with(|| a.uri.cmp(&b.uri))
                });
            }
        }
    }

    /// Return a sorted copy without modifying the original.
    pub fn sorted(resources: &[ScmResource], order: ScmSortOrder) -> Vec<ScmResource> {
        let mut copy: Vec<ScmResource> = resources.to_vec();
        Self::sort(&mut copy, order);
        copy
    }
}

/// A set of changes that can be staged or unstaged together.
#[derive(Debug, Clone)]
pub struct ScmChangeSet {
    pub label: String,
    pub resources: Vec<ScmResource>,
    pub staged: bool,
}

impl ScmChangeSet {
    /// Create a new unstaged change set.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            resources: Vec::new(),
            staged: false,
        }
    }

    /// Add a resource to the change set.
    pub fn add(&mut self, resource: ScmResource) {
        self.resources.push(resource);
    }

    /// Remove a resource by URI. Returns true if found and removed.
    pub fn remove(&mut self, uri: &str) -> bool {
        let before = self.resources.len();
        self.resources.retain(|r| r.uri != uri);
        self.resources.len() < before
    }

    /// Stage all resources in this set.
    pub fn stage(&mut self) {
        self.staged = true;
    }

    /// Unstage all resources in this set.
    pub fn unstage(&mut self) {
        self.staged = false;
    }

    /// Return the number of resources.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether this change set is empty.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Return URIs of all resources in the set.
    pub fn uris(&self) -> Vec<&str> {
        self.resources.iter().map(|r| r.uri.as_str()).collect()
    }

    /// Count resources by status.
    pub fn count_by_status(&self) -> Vec<(ScmStatus, usize)> {
        let mut counts: Vec<(ScmStatus, usize)> = Vec::new();
        for r in &self.resources {
            if let Some(entry) = counts.iter_mut().find(|(s, _)| *s == r.status) {
                entry.1 += 1;
            } else {
                counts.push((r.status, 1));
            }
        }
        counts
    }
}

/// Generates a preview summary of what would be committed.
pub struct ScmCommitPreview;

impl ScmCommitPreview {
    /// Build a human-readable summary of the changes in a group.
    pub fn preview(group: &ScmGroup) -> String {
        if group.resources.is_empty() {
            return format!("{}: (no changes)", group.label);
        }
        let mut added = 0usize;
        let mut modified = 0usize;
        let mut deleted = 0usize;
        let mut renamed = 0usize;
        let mut other = 0usize;
        for r in &group.resources {
            match r.status {
                ScmStatus::Added => added += 1,
                ScmStatus::Modified => modified += 1,
                ScmStatus::Deleted => deleted += 1,
                ScmStatus::Renamed => renamed += 1,
                _ => other += 1,
            }
        }
        let mut parts = Vec::new();
        if added > 0 {
            parts.push(format!("{added} added"));
        }
        if modified > 0 {
            parts.push(format!("{modified} modified"));
        }
        if deleted > 0 {
            parts.push(format!("{deleted} deleted"));
        }
        if renamed > 0 {
            parts.push(format!("{renamed} renamed"));
        }
        if other > 0 {
            parts.push(format!("{other} other"));
        }
        format!("{}: {}", group.label, parts.join(", "))
    }

    /// Build a detailed file listing for the commit preview.
    pub fn file_listing(group: &ScmGroup) -> Vec<String> {
        group
            .resources
            .iter()
            .map(|r| {
                let status_char = match r.status {
                    ScmStatus::Added => 'A',
                    ScmStatus::Modified => 'M',
                    ScmStatus::Deleted => 'D',
                    ScmStatus::Renamed => 'R',
                    ScmStatus::Untracked => '?',
                    ScmStatus::Conflict => 'C',
                    ScmStatus::Ignored => '!',
                };
                if let Some(ref orig) = r.original_uri {
                    format!("{status_char} {orig} -> {}", r.uri)
                } else {
                    format!("{status_char} {}", r.uri)
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// working_copy_stage_all / working_copy_discard — SCM operations
// ---------------------------------------------------------------------------

/// Result of a stage-all operation.
#[derive(Debug, Clone)]
pub struct StageAllResult {
    pub provider_id: String,
    pub staged_count: usize,
    pub from_group: String,
    pub to_group: String,
}

impl fmt::Display for StageAllResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "staged {} files from '{}' to '{}'",
            self.staged_count, self.from_group, self.to_group,
        )
    }
}

/// Result of a discard operation.
#[derive(Debug, Clone)]
pub struct DiscardResult {
    pub discarded: Vec<String>,
    pub failed: Vec<String>,
}

impl DiscardResult {
    pub fn success_count(&self) -> usize {
        self.discarded.len()
    }

    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }

    pub fn all_succeeded(&self) -> bool {
        self.failed.is_empty()
    }
}

impl fmt::Display for DiscardResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "discarded {} files, {} failed",
            self.discarded.len(),
            self.failed.len(),
        )
    }
}

/// Move all resources from a source group to a target group within a provider.
/// This simulates `git add .` by moving from "changes" to "staged".
pub fn working_copy_stage_all(
    service: &mut WorkingCopyService,
    provider_id: &str,
    from_group_id: &str,
    to_group_id: &str,
) -> Option<StageAllResult> {
    let provider = service.get_provider_mut(provider_id)?;
    let from_idx = provider.groups.iter().position(|g| g.id == from_group_id)?;
    let resources: Vec<ScmResource> = provider.groups[from_idx].resources.drain(..).collect();
    let count = resources.len();
    let to_idx = provider.groups.iter().position(|g| g.id == to_group_id)?;
    provider.groups[to_idx].resources.extend(resources);
    Some(StageAllResult {
        provider_id: provider_id.to_string(),
        staged_count: count,
        from_group: from_group_id.to_string(),
        to_group: to_group_id.to_string(),
    })
}

/// Discard (remove) resources matching the given URIs from a group.
/// Returns which URIs were successfully discarded and which were not found.
pub fn working_copy_discard(
    service: &mut WorkingCopyService,
    provider_id: &str,
    group_id: &str,
    uris: &[&str],
) -> Option<DiscardResult> {
    let provider = service.get_provider_mut(provider_id)?;
    let group = provider.groups.iter_mut().find(|g| g.id == group_id)?;
    let mut discarded = Vec::new();
    let mut failed = Vec::new();
    for uri in uris {
        if let Some(pos) = group.resources.iter().position(|r| r.uri == *uri) {
            group.resources.remove(pos);
            discarded.push(uri.to_string());
        } else {
            failed.push(uri.to_string());
        }
    }
    Some(DiscardResult { discarded, failed })
}

// ---------------------------------------------------------------------------
// ChangesetSummary — aggregate description of a set of changes
// ---------------------------------------------------------------------------

/// A high-level summary describing a changeset for display or logging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesetSummary {
    pub title: String,
    pub file_count: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub has_conflicts: bool,
}

impl ChangesetSummary {
    /// Build a summary from a slice of diff summaries and an optional conflict flag.
    pub fn from_diffs(title: impl Into<String>, diffs: &[DiffSummary], has_conflicts: bool) -> Self {
        let mut insertions = 0;
        let mut deletions = 0;
        for d in diffs {
            insertions += d.insertions;
            deletions += d.deletions;
        }
        Self {
            title: title.into(),
            file_count: diffs.len(),
            insertions,
            deletions,
            has_conflicts,
        }
    }

    /// Total number of changed lines across all files.
    pub fn total_changes(&self) -> usize {
        self.insertions + self.deletions
    }

    /// Returns true when there are no file changes at all.
    pub fn is_empty(&self) -> bool {
        self.file_count == 0
    }
}

impl fmt::Display for ChangesetSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} file(s), +{} -{}{}",
            self.title,
            self.file_count,
            self.insertions,
            self.deletions,
            if self.has_conflicts { " (conflicts)" } else { "" },
        )
    }
}

// ---------------------------------------------------------------------------
// WorkingCopyDiff — collection of per-file diffs for a working copy
// ---------------------------------------------------------------------------

/// Holds per-file diff summaries for the entire working copy.
#[derive(Debug, Clone, Default)]
pub struct WorkingCopyDiff {
    pub diffs: Vec<DiffSummary>,
}

impl WorkingCopyDiff {
    pub fn new() -> Self {
        Self { diffs: Vec::new() }
    }

    /// Add a diff for a single file.
    pub fn add(&mut self, diff: DiffSummary) {
        self.diffs.push(diff);
    }

    /// Total insertions across all files.
    pub fn total_insertions(&self) -> usize {
        self.diffs.iter().map(|d| d.insertions).sum()
    }

    /// Total deletions across all files.
    pub fn total_deletions(&self) -> usize {
        self.diffs.iter().map(|d| d.deletions).sum()
    }

    /// Number of files with changes.
    pub fn file_count(&self) -> usize {
        self.diffs.iter().filter(|d| !d.is_empty()).count()
    }

    /// Get the diff for a specific file path, if present.
    pub fn get(&self, file_path: &str) -> Option<&DiffSummary> {
        self.diffs.iter().find(|d| d.file_path == file_path)
    }

    /// Build a `ChangesetSummary` from this diff collection.
    pub fn summarize(&self, title: impl Into<String>) -> ChangesetSummary {
        ChangesetSummary::from_diffs(title, &self.diffs, false)
    }
}

// ---------------------------------------------------------------------------
// StagingArea — stage and unstage individual files
// ---------------------------------------------------------------------------

/// Manages staged and unstaged resources for a single provider, providing
/// fine-grained control over which files are included in the next commit.
#[derive(Debug, Clone)]
pub struct StagingArea {
    staged: Vec<ScmResource>,
    unstaged: Vec<ScmResource>,
}

impl StagingArea {
    pub fn new() -> Self {
        Self {
            staged: Vec::new(),
            unstaged: Vec::new(),
        }
    }

    /// Populate the unstaged list from a provider's non-staged groups.
    pub fn from_provider(provider: &ScmProvider) -> Self {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        for group in &provider.groups {
            if group.id == "staged" {
                staged.extend(group.resources.iter().cloned());
            } else {
                unstaged.extend(group.resources.iter().cloned());
            }
        }
        Self { staged, unstaged }
    }

    /// Stage a file by URI. Returns true if the file was found in unstaged.
    pub fn stage(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.unstaged.iter().position(|r| r.uri == uri) {
            let resource = self.unstaged.remove(pos);
            self.staged.push(resource);
            true
        } else {
            false
        }
    }

    /// Unstage a file by URI. Returns true if the file was found in staged.
    pub fn unstage(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.staged.iter().position(|r| r.uri == uri) {
            let resource = self.staged.remove(pos);
            self.unstaged.push(resource);
            true
        } else {
            false
        }
    }

    /// Stage all currently unstaged files.
    pub fn stage_all(&mut self) {
        self.staged.append(&mut self.unstaged);
    }

    /// Unstage all currently staged files.
    pub fn unstage_all(&mut self) {
        self.unstaged.append(&mut self.staged);
    }

    pub fn staged_files(&self) -> &[ScmResource] {
        &self.staged
    }

    pub fn unstaged_files(&self) -> &[ScmResource] {
        &self.unstaged
    }

    pub fn staged_count(&self) -> usize {
        self.staged.len()
    }

    pub fn unstaged_count(&self) -> usize {
        self.unstaged.len()
    }

    /// Returns true when nothing is staged.
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty()
    }
}

impl Default for StagingArea {
    fn default() -> Self {
        Self::new()
    }
}

impl StagingArea {
    /// Returns true if a file is currently staged.
    pub fn is_staged(&self, uri: &str) -> bool {
        self.staged.iter().any(|r| r.uri == uri)
    }

    /// Returns true if a file is currently unstaged.
    pub fn is_unstaged(&self, uri: &str) -> bool {
        self.unstaged.iter().any(|r| r.uri == uri)
    }

    /// Total number of files (staged + unstaged).
    pub fn total_count(&self) -> usize {
        self.staged.len() + self.unstaged.len()
    }

    /// Returns all URIs currently staged.
    pub fn staged_uris(&self) -> Vec<&str> {
        self.staged.iter().map(|r| r.uri.as_str()).collect()
    }

    /// Returns all URIs currently unstaged.
    pub fn unstaged_uris(&self) -> Vec<&str> {
        self.unstaged.iter().map(|r| r.uri.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// ConflictResolver — helpers for resolving merge conflicts
// ---------------------------------------------------------------------------

/// Strategy for resolving a merge conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    AcceptCurrent,
    AcceptIncoming,
    AcceptBoth,
    Manual,
}

/// Tracks conflict resolution decisions for resources.
#[derive(Debug, Clone)]
pub struct ConflictResolver {
    resolutions: Vec<(String, ConflictResolution)>,
}

impl ConflictResolver {
    pub fn new() -> Self {
        Self {
            resolutions: Vec::new(),
        }
    }

    /// Record a resolution for a file URI.
    pub fn resolve(&mut self, uri: impl Into<String>, resolution: ConflictResolution) {
        let uri = uri.into();
        if let Some(entry) = self.resolutions.iter_mut().find(|(u, _)| *u == uri) {
            entry.1 = resolution;
        } else {
            self.resolutions.push((uri, resolution));
        }
    }

    /// Get the resolution for a file, if one has been recorded.
    pub fn get_resolution(&self, uri: &str) -> Option<ConflictResolution> {
        self.resolutions.iter().find(|(u, _)| u == uri).map(|(_, r)| *r)
    }

    /// Number of resolved conflicts.
    pub fn resolved_count(&self) -> usize {
        self.resolutions.len()
    }

    /// Returns the URIs that still need manual resolution.
    pub fn pending_manual(&self) -> Vec<&str> {
        self.resolutions
            .iter()
            .filter(|(_, r)| *r == ConflictResolution::Manual)
            .map(|(u, _)| u.as_str())
            .collect()
    }

    /// Returns true when all recorded resolutions are non-manual.
    pub fn all_auto_resolved(&self) -> bool {
        self.resolutions.iter().all(|(_, r)| *r != ConflictResolution::Manual)
    }

    /// Extract conflicting resources from a provider.
    pub fn find_conflicts(provider: &ScmProvider) -> Vec<&ScmResource> {
        provider
            .groups
            .iter()
            .flat_map(|g| &g.resources)
            .filter(|r| r.status == ScmStatus::Conflict)
            .collect()
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictResolver {
    /// Remove a previously recorded resolution. Returns true if it existed.
    pub fn remove_resolution(&mut self, uri: &str) -> bool {
        let before = self.resolutions.len();
        self.resolutions.retain(|(u, _)| u != uri);
        self.resolutions.len() < before
    }

    /// Clear all recorded resolutions.
    pub fn clear(&mut self) {
        self.resolutions.clear();
    }

    /// Return URIs of all resolved conflicts (regardless of strategy).
    pub fn resolved_uris(&self) -> Vec<&str> {
        self.resolutions.iter().map(|(u, _)| u.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// WorkingCopyExporter — serialize working copy state for external consumption
// ---------------------------------------------------------------------------

/// Exports a working copy provider state into a portable text format.
pub struct WorkingCopyExporter;

impl ScmStatus {
    /// Returns true for statuses that represent an active change (not ignored).
    pub fn is_active(&self) -> bool {
        !matches!(self, ScmStatus::Ignored)
    }

    /// Returns true for statuses that modify existing file content.
    pub fn is_content_change(&self) -> bool {
        matches!(self, ScmStatus::Modified | ScmStatus::Renamed)
    }

    /// Returns a single-character short code for display.
    pub fn short_code(&self) -> char {
        match self {
            ScmStatus::Untracked => '?',
            ScmStatus::Modified => 'M',
            ScmStatus::Added => 'A',
            ScmStatus::Deleted => 'D',
            ScmStatus::Renamed => 'R',
            ScmStatus::Conflict => 'C',
            ScmStatus::Ignored => '!',
        }
    }
}

impl fmt::Display for ScmStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(status_label(*self))
    }
}

impl ScmResource {
    /// Create a new resource with no original URI.
    pub fn new(uri: impl Into<String>, status: ScmStatus) -> Self {
        Self {
            uri: uri.into(),
            status,
            original_uri: None,
        }
    }

    /// Create a renamed resource tracking the original URI.
    pub fn renamed(uri: impl Into<String>, original: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            status: ScmStatus::Renamed,
            original_uri: Some(original.into()),
        }
    }

    /// Returns the file extension from the URI, if any.
    pub fn extension(&self) -> Option<&str> {
        self.uri.rsplit_once('.').map(|(_, ext)| ext)
    }

    /// Returns the file name portion of the URI (after the last `/`).
    pub fn file_name(&self) -> &str {
        self.uri.rsplit('/').next().unwrap_or(&self.uri)
    }

    /// Returns the directory portion of the URI (before the last `/`).
    pub fn directory(&self) -> Option<&str> {
        self.uri.rsplit_once('/').map(|(dir, _)| dir)
    }
}

impl fmt::Display for ScmResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status.short_code(), self.uri)?;
        if let Some(ref orig) = self.original_uri {
            write!(f, " (was {})", orig)?;
        }
        Ok(())
    }
}

impl ScmGroup {
    /// Create a new empty group.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            resources: Vec::new(),
        }
    }

    /// Number of resources in this group.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Whether this group has no resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Return all URIs in this group.
    pub fn uris(&self) -> Vec<&str> {
        self.resources.iter().map(|r| r.uri.as_str()).collect()
    }

    /// Return resources filtered by status.
    pub fn by_status(&self, status: ScmStatus) -> Vec<&ScmResource> {
        self.resources.iter().filter(|r| r.status == status).collect()
    }

    /// Returns true if any resource has the given status.
    pub fn has_status(&self, status: ScmStatus) -> bool {
        self.resources.iter().any(|r| r.status == status)
    }

    /// Retain only resources matching a predicate.
    pub fn retain<F: Fn(&ScmResource) -> bool>(&mut self, predicate: F) {
        self.resources.retain(|r| predicate(r));
    }
}

impl DiffSummary {
    /// Create a new diff summary for a file.
    pub fn new(file_path: impl Into<String>, insertions: usize, deletions: usize) -> Self {
        Self {
            file_path: file_path.into(),
            insertions,
            deletions,
        }
    }

    /// Returns true if the diff only has insertions.
    pub fn is_pure_addition(&self) -> bool {
        self.insertions > 0 && self.deletions == 0
    }

    /// Returns true if the diff only has deletions.
    pub fn is_pure_deletion(&self) -> bool {
        self.deletions > 0 && self.insertions == 0
    }

    /// Returns the ratio of insertions to total changes (0.0..=1.0).
    /// Returns 0.0 if there are no changes.
    pub fn insertion_ratio(&self) -> f64 {
        let total = self.total_changes();
        if total == 0 {
            0.0
        } else {
            self.insertions as f64 / total as f64
        }
    }
}

impl fmt::Display for DiffSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.file_path.is_empty() {
            write!(f, "+{} -{}", self.insertions, self.deletions)
        } else {
            write!(f, "{}: +{} -{}", self.file_path, self.insertions, self.deletions)
        }
    }
}

impl WorkingCopyDiff {
    /// Remove a diff by file path. Returns true if found.
    pub fn remove(&mut self, file_path: &str) -> bool {
        let before = self.diffs.len();
        self.diffs.retain(|d| d.file_path != file_path);
        self.diffs.len() < before
    }

    /// Returns true when there are no diffs at all.
    pub fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }

    /// Returns file paths that have the most changes, sorted descending.
    pub fn top_changed(&self, limit: usize) -> Vec<&DiffSummary> {
        let mut sorted: Vec<&DiffSummary> = self.diffs.iter().filter(|d| !d.is_empty()).collect();
        sorted.sort_by(|a, b| b.total_changes().cmp(&a.total_changes()));
        sorted.truncate(limit);
        sorted
    }

    /// Merge another WorkingCopyDiff into this one.
    pub fn merge(&mut self, other: &WorkingCopyDiff) {
        for diff in &other.diffs {
            if let Some(existing) = self.diffs.iter_mut().find(|d| d.file_path == diff.file_path) {
                existing.insertions += diff.insertions;
                existing.deletions += diff.deletions;
            } else {
                self.diffs.push(diff.clone());
            }
        }
    }
}

impl ScmChangeSet {
    /// Return resources matching a status.
    pub fn by_status(&self, status: ScmStatus) -> Vec<&ScmResource> {
        self.resources.iter().filter(|r| r.status == status).collect()
    }

    /// Returns true if any resource has the given status.
    pub fn has_status(&self, status: ScmStatus) -> bool {
        self.resources.iter().any(|r| r.status == status)
    }

    /// Split this change set into two: resources matching the predicate and
    /// those that don't. Returns (matching, rest).
    pub fn partition<F: Fn(&ScmResource) -> bool>(self, predicate: F) -> (Self, Self) {
        let (matching, rest): (Vec<_>, Vec<_>) = self.resources.into_iter().partition(|r| predicate(r));
        (
            ScmChangeSet {
                label: format!("{} (selected)", self.label),
                resources: matching,
                staged: self.staged,
            },
            ScmChangeSet {
                label: format!("{} (remaining)", self.label),
                resources: rest,
                staged: self.staged,
            },
        )
    }
}

impl WorkingCopyExporter {
    /// Export a provider's state as a list of status lines, one per resource.
    /// Format: `GROUP_ID\tSTATUS\tURI[\tORIGINAL_URI]`
    pub fn export_lines(provider: &ScmProvider) -> Vec<String> {
        let mut lines = Vec::new();
        for group in &provider.groups {
            for res in &group.resources {
                let status_str = status_label(res.status);
                let line = if let Some(ref orig) = res.original_uri {
                    format!("{}\t{}\t{}\t{}", group.id, status_str, res.uri, orig)
                } else {
                    format!("{}\t{}\t{}", group.id, status_str, res.uri)
                };
                lines.push(line);
            }
        }
        lines
    }

    /// Export a provider's state as a single newline-separated string.
    pub fn export_string(provider: &ScmProvider) -> String {
        Self::export_lines(provider).join("\n")
    }

    /// Count total resources across all groups.
    pub fn total_resource_count(provider: &ScmProvider) -> usize {
        provider.groups.iter().map(|g| g.resources.len()).sum()
    }
}


// ── Working Copy Conflict Detector ──

/// Detects and categorizes conflicts across multiple providers.
#[derive(Debug)]
pub struct WorkingCopyConflictDetector {
    conflict_uris: Vec<String>,
}

impl WorkingCopyConflictDetector {
    pub fn new() -> Self {
        Self {
            conflict_uris: Vec::new(),
        }
    }

    /// Scan a provider for conflicted resources.
    pub fn scan_provider(&mut self, provider: &ScmProvider) {
        for group in &provider.groups {
            for resource in &group.resources {
                if resource.status == ScmStatus::Conflict
                    && !self.conflict_uris.contains(&resource.uri)
                {
                    self.conflict_uris.push(resource.uri.clone());
                }
            }
        }
    }

    /// Scan all providers in a service.
    pub fn scan_service(&mut self, service: &WorkingCopyService) {
        for provider in &service.providers {
            self.scan_provider(provider);
        }
    }

    /// Number of detected conflicts.
    pub fn conflict_count(&self) -> usize {
        self.conflict_uris.len()
    }

    /// Whether there are any conflicts.
    pub fn has_conflicts(&self) -> bool {
        !self.conflict_uris.is_empty()
    }

    /// Return the list of conflicted URIs.
    pub fn conflicted_uris(&self) -> &[String] {
        &self.conflict_uris
    }

    /// Check if a specific URI is in conflict.
    pub fn is_conflicted(&self, uri: &str) -> bool {
        self.conflict_uris.iter().any(|u| u == uri)
    }

    /// Clear all recorded conflicts.
    pub fn clear(&mut self) {
        self.conflict_uris.clear();
    }

    /// Remove a specific URI from the conflict list (e.g. after resolution).
    pub fn mark_resolved(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.conflict_uris.iter().position(|u| u == uri) {
            self.conflict_uris.remove(pos);
            true
        } else {
            false
        }
    }
}

// ── Dirty File Tracker ──

/// Tracks dirty (modified, unsaved) files with timestamps.
#[derive(Debug, Clone)]
pub struct DirtyFileEntry {
    pub uri: String,
    pub dirty_since: u64,
    pub last_modified: u64,
}

#[derive(Debug)]
pub struct DirtyFileTracker {
    entries: Vec<DirtyFileEntry>,
}

impl DirtyFileTracker {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Mark a file as dirty with the given timestamp.
    pub fn mark_dirty(&mut self, uri: &str, timestamp: u64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.uri == uri) {
            entry.last_modified = timestamp;
        } else {
            self.entries.push(DirtyFileEntry {
                uri: uri.to_string(),
                dirty_since: timestamp,
                last_modified: timestamp,
            });
        }
    }

    /// Mark a file as clean (saved).
    pub fn mark_clean(&mut self, uri: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.uri == uri) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Check if a file is dirty.
    pub fn is_dirty(&self, uri: &str) -> bool {
        self.entries.iter().any(|e| e.uri == uri)
    }

    /// Number of dirty files.
    pub fn dirty_count(&self) -> usize {
        self.entries.len()
    }

    /// Get all dirty file URIs, sorted.
    pub fn dirty_uris(&self) -> Vec<&str> {
        let mut uris: Vec<&str> = self.entries.iter().map(|e| e.uri.as_str()).collect();
        uris.sort_unstable();
        uris
    }

    /// Get the entry for a specific URI.
    pub fn get_entry(&self, uri: &str) -> Option<&DirtyFileEntry> {
        self.entries.iter().find(|e| e.uri == uri)
    }

    /// Get files dirty for longer than `duration` time units.
    pub fn dirty_longer_than(&self, current_time: u64, duration: u64) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| current_time.saturating_sub(e.dirty_since) > duration)
            .map(|e| e.uri.as_str())
            .collect()
    }

    /// Mark all files as clean.
    pub fn mark_all_clean(&mut self) {
        self.entries.clear();
    }

    /// Return the oldest dirty file (by dirty_since timestamp).
    pub fn oldest_dirty(&self) -> Option<&DirtyFileEntry> {
        self.entries.iter().min_by_key(|e| e.dirty_since)
    }
}

// ── Working Copy Diff Summary ──

/// A provider-level summary of changes in a working copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderDiffSummary {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

impl fmt::Display for ProviderDiffSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} file(s) changed, {} insertion(s), {} deletion(s)",
            self.files_changed, self.insertions, self.deletions
        )
    }
}

/// Builds diff summaries from provider data.
pub struct WorkingCopyDiffSummary;

impl WorkingCopyDiffSummary {
    /// Compute a summary from an SCM provider by counting resources per status.
    pub fn from_provider(provider: &ScmProvider) -> ProviderDiffSummary {
        let mut summary = ProviderDiffSummary::default();
        for group in &provider.groups {
            for resource in &group.resources {
                summary.files_changed += 1;
                match resource.status {
                    ScmStatus::Added | ScmStatus::Untracked => summary.insertions += 1,
                    ScmStatus::Deleted => summary.deletions += 1,
                    ScmStatus::Modified | ScmStatus::Renamed => {
                        summary.insertions += 1;
                        summary.deletions += 1;
                    }
                    _ => {}
                }
            }
        }
        summary
    }

    /// Compute a summary from stats.
    pub fn from_stats(stats: &ScmStats) -> ProviderDiffSummary {
        ProviderDiffSummary {
            files_changed: stats.total,
            insertions: stats.added + stats.untracked,
            deletions: stats.deleted,
        }
    }

    /// Merge two summaries.
    pub fn merge(a: &ProviderDiffSummary, b: &ProviderDiffSummary) -> ProviderDiffSummary {
        ProviderDiffSummary {
            files_changed: a.files_changed + b.files_changed,
            insertions: a.insertions + b.insertions,
            deletions: a.deletions + b.deletions,
        }
    }

    /// Whether a summary represents a clean state.
    pub fn is_clean(summary: &ProviderDiffSummary) -> bool {
        summary.files_changed == 0
    }

    /// Format a one-line summary string.
    pub fn one_line(summary: &ProviderDiffSummary) -> String {
        if Self::is_clean(summary) {
            "Working tree clean".to_string()
        } else {
            format!(
                "{} changed, +{} -{}", 
                summary.files_changed, summary.insertions, summary.deletions
            )
        }
    }
}

// ── Working Copy Revert Handler ──

/// Tracks which files should be reverted and their status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevertStatus {
    Pending,
    Reverted,
    Failed(String),
}

/// Handler for reverting working copy changes.
#[derive(Debug)]
pub struct WorkingCopyRevertHandler {
    items: Vec<(String, RevertStatus)>,
}

impl WorkingCopyRevertHandler {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Queue a URI for revert.
    pub fn queue_revert(&mut self, uri: &str) {
        if !self.items.iter().any(|(u, _)| u == uri) {
            self.items.push((uri.to_string(), RevertStatus::Pending));
        }
    }

    /// Queue all resources in a provider for revert.
    pub fn queue_provider(&mut self, provider: &ScmProvider) {
        for group in &provider.groups {
            for resource in &group.resources {
                self.queue_revert(&resource.uri);
            }
        }
    }

    /// Mark a URI as successfully reverted.
    pub fn mark_reverted(&mut self, uri: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|(u, _)| u == uri) {
            item.1 = RevertStatus::Reverted;
            true
        } else {
            false
        }
    }

    /// Mark a URI as failed.
    pub fn mark_failed(&mut self, uri: &str, reason: &str) -> bool {
        if let Some(item) = self.items.iter_mut().find(|(u, _)| u == uri) {
            item.1 = RevertStatus::Failed(reason.to_string());
            true
        } else {
            false
        }
    }

    /// Number of items queued.
    pub fn queued_count(&self) -> usize {
        self.items.len()
    }

    /// Number of pending items.
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|(_, s)| *s == RevertStatus::Pending).count()
    }

    /// Number of successfully reverted items.
    pub fn reverted_count(&self) -> usize {
        self.items.iter().filter(|(_, s)| *s == RevertStatus::Reverted).count()
    }

    /// Number of failed items.
    pub fn failed_count(&self) -> usize {
        self.items.iter().filter(|(_, s)| matches!(s, RevertStatus::Failed(_))).count()
    }

    /// Get all failed URIs with their error messages.
    pub fn failed_items(&self) -> Vec<(&str, &str)> {
        self.items.iter()
            .filter_map(|(u, s)| match s {
                RevertStatus::Failed(msg) => Some((u.as_str(), msg.as_str())),
                _ => None,
            })
            .collect()
    }

    /// Whether all items have been processed (none pending).
    pub fn is_complete(&self) -> bool {
        self.items.iter().all(|(_, s)| *s != RevertStatus::Pending)
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}


// ---------------------------------------------------------------------------
// wb_workingcopy – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbWorkingcopyLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbWorkingcopyPanelState {
    pub region: XWbWorkingcopyLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbWorkingcopyPanelState {
    pub fn new(region: XWbWorkingcopyLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_workingcopy_total_visible_area(panels: &[XWbWorkingcopyPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_workingcopy_count_in_region(
    panels: &[XWbWorkingcopyPanelState],
    region: XWbWorkingcopyLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_workingcopy_widest_panel(panels: &[XWbWorkingcopyPanelState]) -> Option<&XWbWorkingcopyPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_workingcopy_collapse_region(
    panels: &mut [XWbWorkingcopyPanelState],
    region: XWbWorkingcopyLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbWorkingcopyLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbWorkingcopyLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_workingcopy – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for working copy management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbWorkingcopyWorkingCopyStatus {
    Clean,
    Modified,
    Staged,
    Conflicted,
}

impl YWbWorkingcopyWorkingCopyStatus {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Clean => 0,
            Self::Modified => 1,
            Self::Staged => 2,
            Self::Conflicted => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Clean => "Clean",
            Self::Modified => "Modified",
            Self::Staged => "Staged",
            Self::Conflicted => "Conflicted",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbWorkingcopyWorkingCopyStatus] {
        &[
            YWbWorkingcopyWorkingCopyStatus::Clean,
            YWbWorkingcopyWorkingCopyStatus::Modified,
            YWbWorkingcopyWorkingCopyStatus::Staged,
            YWbWorkingcopyWorkingCopyStatus::Conflicted,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbWorkingcopyWorkingCopyStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks working copy diff data.
#[derive(Debug, Clone)]
pub struct YWbWorkingcopyWorkingCopyDiff {
    pub added_lines: usize,
    pub removed_lines: usize,
    pub file_path: String,
}

impl YWbWorkingcopyWorkingCopyDiff {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            added_lines: 0,
            removed_lines: 0,
            file_path: String::new(),
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbWorkingcopyWorkingCopyDiff({}: {:?})", "added_lines", self.added_lines)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_workingcopy_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_workingcopy_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_workingcopy_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_workingcopy_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_workingcopy_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_workingcopy_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_workingcopy_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_workingcopy_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_workingcopy – Extended working copy checkpoint helpers
// ---------------------------------------------------------------------------

/// Priority levels for working copy checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbWorkingcopyPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbWorkingcopyPriority {
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
    pub fn all_asc() -> [ZWbWorkingcopyPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbWorkingcopyPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks working copy checkpoint data.
#[derive(Debug, Clone)]
pub struct ZWbWorkingcopyWorkingCopyCheckpoint {
    pub file_hashes: Vec<(String, u64)>,
    pub timestamp_ms: u64,
    pub label: String,
}

impl ZWbWorkingcopyWorkingCopyCheckpoint {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            file_hashes: Vec::new(),
            timestamp_ms: 0,
            label: String::new(),
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.file_hashes.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.file_hashes.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.file_hashes.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbWorkingcopyWorkingCopyCheckpoint[timestamp_ms={:?}, label={:?}]", self.timestamp_ms, self.label)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for working copy checkpoint.
pub fn z_wb_workingcopy_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_workingcopy_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_workingcopy_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_workingcopy_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_wb_workingcopy_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_workingcopy_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_workingcopy_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 43
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer43 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer43 {
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
pub fn xb_fnv1a_43(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_43<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_43<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_43(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_43(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 234
// ---------------------------------------------------------------------------

/// Generic object pool `Xc234Pool<T>`.
pub struct Xc234Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc234Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc234PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc234Pool<T> {
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
    pub fn stats(&self) -> Xc234PoolStats {
        Xc234PoolStats {
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

impl<T> Default for Xc234Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc234Scheduler`.
pub struct Xc234Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc234Scheduler {
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

impl Default for Xc234Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_234 hash for the given byte slice.
pub fn xc_234_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_234 convention.
pub fn xc_234_reverse(s: &str) -> String {
    s.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_provider() -> ScmProvider {
        ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/workspace".into(),
            groups: vec![ScmGroup {
                id: "changes".into(),
                label: "Changes".into(),
                resources: Vec::new(),
            }],
            count: 0,
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        assert_eq!(svc.provider_count(), 1);
        let p = svc.get_provider("git").unwrap();
        assert_eq!(p.label, "Git");
    }

    #[test]
    fn add_resource_and_filter() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.add_resource(
            "git",
            "changes",
            ScmResource {
                uri: "src/main.rs".into(),
                status: ScmStatus::Modified,
                original_uri: None,
            },
        );
        svc.add_resource(
            "git",
            "changes",
            ScmResource {
                uri: "README.md".into(),
                status: ScmStatus::Added,
                original_uri: None,
            },
        );
        assert_eq!(svc.total_changes(), 2);
        let modified = svc.get_resources("git", ScmStatus::Modified);
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].uri, "src/main.rs");
    }

    #[test]
    fn get_resources_empty_for_missing_provider() {
        let svc = WorkingCopyService::new();
        assert!(svc.get_resources("nope", ScmStatus::Modified).is_empty());
    }

    #[test]
    fn total_changes_across_providers() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.register_provider(ScmProvider {
            id: "svn".into(),
            label: "SVN".into(),
            root_uri: "/other".into(),
            groups: vec![ScmGroup {
                id: "changes".into(),
                label: "Changes".into(),
                resources: Vec::new(),
            }],
            count: 0,
        });
        svc.add_resource(
            "git",
            "changes",
            ScmResource {
                uri: "a.rs".into(),
                status: ScmStatus::Deleted,
                original_uri: None,
            },
        );
        svc.add_resource(
            "svn",
            "changes",
            ScmResource {
                uri: "b.rs".into(),
                status: ScmStatus::Conflict,
                original_uri: None,
            },
        );
        assert_eq!(svc.total_changes(), 2);
    }

    fn provider_with_two_groups() -> ScmProvider {
        ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/workspace".into(),
            groups: vec![
                ScmGroup {
                    id: "changes".into(),
                    label: "Changes".into(),
                    resources: Vec::new(),
                },
                ScmGroup {
                    id: "staged".into(),
                    label: "Staged Changes".into(),
                    resources: Vec::new(),
                },
            ],
            count: 0,
        }
    }

    #[test]
    fn remove_resource_returns_true_when_found() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.add_resource("git", "changes", ScmResource {
            uri: "a.rs".into(),
            status: ScmStatus::Modified,
            original_uri: None,
        });
        assert!(svc.remove_resource("git", "changes", "a.rs"));
        assert_eq!(svc.total_changes(), 0);
        assert!(svc.get_all_resources("git").is_empty());
    }

    #[test]
    fn remove_resource_returns_false_when_missing() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        assert!(!svc.remove_resource("git", "changes", "nope.rs"));
        assert!(!svc.remove_resource("missing", "changes", "a.rs"));
    }

    #[test]
    fn move_resource_between_groups() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        svc.add_resource("git", "changes", ScmResource {
            uri: "main.rs".into(),
            status: ScmStatus::Modified,
            original_uri: None,
        });
        assert!(svc.move_resource("git", "changes", "staged", "main.rs"));
        assert!(svc.get_resources("git", ScmStatus::Modified).len() == 1);
        let (group_id, _) = svc.find_resource("git", "main.rs").unwrap();
        assert_eq!(group_id, "staged");
    }

    #[test]
    fn move_resource_returns_false_for_missing_resource() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        assert!(!svc.move_resource("git", "changes", "staged", "nope.rs"));
    }

    #[test]
    fn get_all_resources_across_groups() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        svc.add_resource("git", "changes", ScmResource {
            uri: "a.rs".into(),
            status: ScmStatus::Modified,
            original_uri: None,
        });
        svc.add_resource("git", "staged", ScmResource {
            uri: "b.rs".into(),
            status: ScmStatus::Added,
            original_uri: None,
        });
        assert_eq!(svc.get_all_resources("git").len(), 2);
        assert!(svc.get_all_resources("missing").is_empty());
    }

    #[test]
    fn has_conflicts_detection() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        assert!(!svc.has_conflicts("git"));
        svc.add_resource("git", "changes", ScmResource {
            uri: "c.rs".into(),
            status: ScmStatus::Conflict,
            original_uri: None,
        });
        assert!(svc.has_conflicts("git"));
    }

    #[test]
    fn get_stats_aggregates_correctly() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        svc.add_resource("git", "changes", ScmResource {
            uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None,
        });
        svc.add_resource("git", "changes", ScmResource {
            uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None,
        });
        svc.add_resource("git", "changes", ScmResource {
            uri: "c.rs".into(), status: ScmStatus::Deleted, original_uri: None,
        });
        svc.add_resource("git", "staged", ScmResource {
            uri: "d.rs".into(), status: ScmStatus::Conflict, original_uri: None,
        });
        svc.add_resource("git", "staged", ScmResource {
            uri: "e.rs".into(), status: ScmStatus::Untracked, original_uri: None,
        });
        let stats = svc.get_stats("git");
        assert_eq!(stats, ScmStats {
            total: 5, modified: 1, added: 1, deleted: 1, conflict: 1, untracked: 1,
        });
    }

    #[test]
    fn add_and_remove_group() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        assert!(svc.add_group("git", ScmGroup {
            id: "staged".into(),
            label: "Staged".into(),
            resources: Vec::new(),
        }));
        assert_eq!(svc.get_provider("git").unwrap().group_count(), 2);
        // duplicate group id is rejected
        assert!(!svc.add_group("git", ScmGroup {
            id: "staged".into(),
            label: "Staged".into(),
            resources: Vec::new(),
        }));
        assert!(svc.remove_group("git", "staged"));
        assert_eq!(svc.get_provider("git").unwrap().group_count(), 1);
        assert!(!svc.remove_group("git", "staged"));
    }

    #[test]
    fn remove_group_adjusts_count() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.add_resource("git", "changes", ScmResource {
            uri: "x.rs".into(), status: ScmStatus::Modified, original_uri: None,
        });
        assert_eq!(svc.total_changes(), 1);
        svc.remove_group("git", "changes");
        assert_eq!(svc.total_changes(), 0);
    }

    #[test]
    fn find_resource_locates_in_correct_group() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        svc.add_resource("git", "staged", ScmResource {
            uri: "found.rs".into(),
            status: ScmStatus::Added,
            original_uri: None,
        });
        let (gid, res) = svc.find_resource("git", "found.rs").unwrap();
        assert_eq!(gid, "staged");
        assert_eq!(res.status, ScmStatus::Added);
        assert!(svc.find_resource("git", "missing.rs").is_none());
    }

    #[test]
    fn status_label_returns_expected_strings() {
        assert_eq!(status_label(ScmStatus::Modified), "Modified");
        assert_eq!(status_label(ScmStatus::Added), "Added");
        assert_eq!(status_label(ScmStatus::Deleted), "Deleted");
        assert_eq!(status_label(ScmStatus::Conflict), "Conflict");
        assert_eq!(status_label(ScmStatus::Untracked), "Untracked");
        assert_eq!(status_label(ScmStatus::Renamed), "Renamed");
        assert_eq!(status_label(ScmStatus::Ignored), "Ignored");
    }

    #[test]
    fn provider_is_clean_and_group_count() {
        let p = sample_provider();
        assert!(p.is_clean());
        assert_eq!(p.group_count(), 1);

        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.add_resource("git", "changes", ScmResource {
            uri: "dirty.rs".into(),
            status: ScmStatus::Modified,
            original_uri: None,
        });
        assert!(!svc.get_provider("git").unwrap().is_clean());
    }

    // ---- new tests ----

    #[test]
    fn working_copy_stats_from_provider() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(provider_with_two_groups());
        svc.add_resource("git", "changes", ScmResource {
            uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None,
        });
        svc.add_resource("git", "changes", ScmResource {
            uri: "b.rs".into(), status: ScmStatus::Untracked, original_uri: None,
        });
        svc.add_resource("git", "staged", ScmResource {
            uri: "c.rs".into(), status: ScmStatus::Added, original_uri: None,
        });
        let stats = WorkingCopyStats::from_provider(svc.get_provider("git").unwrap());
        assert_eq!(stats, WorkingCopyStats {
            total_files: 3, dirty_count: 1, staged_count: 1, untracked_count: 1,
        });
        assert!(!stats.is_clean());
    }

    #[test]
    fn working_copy_stats_clean() {
        let stats = WorkingCopyStats::from_provider(&sample_provider());
        assert!(stats.is_clean());
        assert_eq!(stats.total_files, 0);
    }

    #[test]
    fn diff_summary_identical_texts() {
        let text = "line one\nline two\nline three\n";
        let summary = compute_diff_summary(text, text);
        assert!(summary.is_empty());
        assert_eq!(summary.total_changes(), 0);
    }

    #[test]
    fn diff_summary_insertions_and_deletions() {
        let original = "aaa\nbbb\nccc\n";
        let modified = "aaa\nxxx\nccc\nyyy\n";
        let summary = compute_diff_summary(original, modified);
        // bbb removed (1 deletion), xxx and yyy added (2 insertions)
        assert_eq!(summary.deletions, 1);
        assert_eq!(summary.insertions, 2);
        assert_eq!(summary.total_changes(), 3);
    }

    #[test]
    fn diff_summary_completely_different() {
        let summary = compute_diff_summary("a\nb\n", "x\ny\nz\n");
        assert_eq!(summary.deletions, 2);
        assert_eq!(summary.insertions, 3);
    }

    #[test]
    fn filter_by_status() {
        let resources = vec![
            ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None },
            ScmResource { uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None },
            ScmResource { uri: "c.rs".into(), status: ScmStatus::Modified, original_uri: None },
        ];
        let filter = WorkingCopyFilter::by_status(ScmStatus::Modified);
        let matched = filter.apply(&resources);
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().all(|r| r.status == ScmStatus::Modified));
    }

    #[test]
    fn filter_by_path_pattern() {
        let resources = vec![
            ScmResource { uri: "src/main.rs".into(), status: ScmStatus::Modified, original_uri: None },
            ScmResource { uri: "tests/test.rs".into(), status: ScmStatus::Added, original_uri: None },
            ScmResource { uri: "src/lib.rs".into(), status: ScmStatus::Deleted, original_uri: None },
        ];
        let filter = WorkingCopyFilter::by_path("src/");
        let matched = filter.apply(&resources);
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().all(|r| r.uri.contains("src/")));
    }

    #[test]
    fn filter_resources_on_service() {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(sample_provider());
        svc.add_resource("git", "changes", ScmResource {
            uri: "src/a.rs".into(), status: ScmStatus::Modified, original_uri: None,
        });
        svc.add_resource("git", "changes", ScmResource {
            uri: "docs/readme.md".into(), status: ScmStatus::Modified, original_uri: None,
        });
        svc.add_resource("git", "changes", ScmResource {
            uri: "src/b.rs".into(), status: ScmStatus::Untracked, original_uri: None,
        });
        let filter = WorkingCopyFilter {
            status: Some(ScmStatus::Modified),
            path_pattern: Some("src/".into()),
        };
        let matched = svc.filter_resources("git", &filter);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].uri, "src/a.rs");
    }

    #[test]
    fn sort_resources_by_status() {
        let mut resources = vec![
            ScmResource { uri: "z.rs".into(), status: ScmStatus::Untracked, original_uri: None },
            ScmResource { uri: "a.rs".into(), status: ScmStatus::Conflict, original_uri: None },
            ScmResource { uri: "m.rs".into(), status: ScmStatus::Modified, original_uri: None },
        ];
        ScmResourceSorter::sort(&mut resources, ScmSortOrder::ByStatus);
        assert_eq!(resources[0].status, ScmStatus::Conflict);
        assert_eq!(resources[1].status, ScmStatus::Modified);
        assert_eq!(resources[2].status, ScmStatus::Untracked);
    }

    #[test]
    fn sort_resources_by_uri() {
        let mut resources = vec![
            ScmResource { uri: "z.rs".into(), status: ScmStatus::Added, original_uri: None },
            ScmResource { uri: "a.rs".into(), status: ScmStatus::Added, original_uri: None },
        ];
        ScmResourceSorter::sort(&mut resources, ScmSortOrder::ByUri);
        assert_eq!(resources[0].uri, "a.rs");
        assert_eq!(resources[1].uri, "z.rs");
    }

    #[test]
    fn changeset_stage_unstage() {
        let mut cs = ScmChangeSet::new("my changes");
        cs.add(ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None });
        cs.add(ScmResource { uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None });
        assert_eq!(cs.len(), 2);
        assert!(!cs.staged);
        cs.stage();
        assert!(cs.staged);
        cs.unstage();
        assert!(!cs.staged);
        assert!(cs.remove("a.rs"));
        assert_eq!(cs.len(), 1);
        assert!(!cs.remove("nonexistent"));
    }

    #[test]
    fn changeset_count_by_status() {
        let mut cs = ScmChangeSet::new("test");
        cs.add(ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None });
        cs.add(ScmResource { uri: "b.rs".into(), status: ScmStatus::Modified, original_uri: None });
        cs.add(ScmResource { uri: "c.rs".into(), status: ScmStatus::Added, original_uri: None });
        let counts = cs.count_by_status();
        let modified_count = counts.iter().find(|(s, _)| *s == ScmStatus::Modified).unwrap().1;
        assert_eq!(modified_count, 2);
        let added_count = counts.iter().find(|(s, _)| *s == ScmStatus::Added).unwrap().1;
        assert_eq!(added_count, 1);
    }

    #[test]
    fn commit_preview_summary() {
        let group = ScmGroup {
            id: "staged".into(),
            label: "Staged Changes".into(),
            resources: vec![
                ScmResource { uri: "a.rs".into(), status: ScmStatus::Added, original_uri: None },
                ScmResource { uri: "b.rs".into(), status: ScmStatus::Modified, original_uri: None },
                ScmResource { uri: "c.rs".into(), status: ScmStatus::Deleted, original_uri: None },
            ],
        };
        let summary = ScmCommitPreview::preview(&group);
        assert!(summary.contains("1 added"));
        assert!(summary.contains("1 modified"));
        assert!(summary.contains("1 deleted"));
    }

    #[test]
    fn commit_preview_file_listing() {
        let group = ScmGroup {
            id: "staged".into(),
            label: "Staged".into(),
            resources: vec![
                ScmResource { uri: "new.rs".into(), status: ScmStatus::Added, original_uri: None },
                ScmResource { uri: "new_name.rs".into(), status: ScmStatus::Renamed, original_uri: Some("old_name.rs".into()) },
            ],
        };
        let listing = ScmCommitPreview::file_listing(&group);
        assert_eq!(listing[0], "A new.rs");
        assert_eq!(listing[1], "R old_name.rs -> new_name.rs");
    }

    // -- working_copy_stage_all / discard tests ------------------------------

    fn make_working_copy_service() -> WorkingCopyService {
        let mut svc = WorkingCopyService::new();
        svc.register_provider(ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/workspace".into(),
            groups: vec![
                ScmGroup {
                    id: "changes".into(),
                    label: "Changes".into(),
                    resources: vec![
                        ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None },
                        ScmResource { uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None },
                    ],
                },
                ScmGroup {
                    id: "staged".into(),
                    label: "Staged Changes".into(),
                    resources: vec![],
                },
            ],
            count: 2,
        });
        svc
    }

    #[test]
    fn stage_all_moves_resources() {
        let mut svc = make_working_copy_service();
        let result = working_copy_stage_all(&mut svc, "git", "changes", "staged").unwrap();
        assert_eq!(result.staged_count, 2);
        let provider = svc.get_provider_mut("git").unwrap();
        assert!(provider.groups.iter().find(|g| g.id == "changes").unwrap().resources.is_empty());
        assert_eq!(provider.groups.iter().find(|g| g.id == "staged").unwrap().resources.len(), 2);
    }

    #[test]
    fn stage_all_no_provider() {
        let mut svc = make_working_copy_service();
        assert!(working_copy_stage_all(&mut svc, "svn", "changes", "staged").is_none());
    }

    #[test]
    fn discard_removes_resources() {
        let mut svc = make_working_copy_service();
        let result = working_copy_discard(&mut svc, "git", "changes", &["a.rs"]).unwrap();
        assert_eq!(result.success_count(), 1);
        assert!(result.all_succeeded());
    }

    #[test]
    fn discard_reports_not_found() {
        let mut svc = make_working_copy_service();
        let result = working_copy_discard(&mut svc, "git", "changes", &["x.rs"]).unwrap();
        assert_eq!(result.failure_count(), 1);
        assert!(!result.all_succeeded());
    }

    #[test]
    fn stage_all_result_display() {
        let r = StageAllResult {
            provider_id: "git".into(),
            staged_count: 3,
            from_group: "changes".into(),
            to_group: "staged".into(),
        };
        let s = format!("{r}");
        assert!(s.contains("3 files"));
    }

    #[test]
    fn discard_result_display() {
        let r = DiscardResult {
            discarded: vec!["a.rs".into()],
            failed: vec![],
        };
        assert!(format!("{r}").contains("discarded 1"));
    }

    // -- ChangesetSummary tests ---------------------------------------------

    #[test]
    fn changeset_summary_from_diffs() {
        let diffs = vec![
            DiffSummary { insertions: 10, deletions: 3, file_path: "a.rs".into() },
            DiffSummary { insertions: 0, deletions: 5, file_path: "b.rs".into() },
        ];
        let summary = ChangesetSummary::from_diffs("my commit", &diffs, false);
        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.insertions, 10);
        assert_eq!(summary.deletions, 8);
        assert_eq!(summary.total_changes(), 18);
        assert!(!summary.has_conflicts);
        assert!(!summary.is_empty());
        let display = format!("{summary}");
        assert!(display.contains("+10"));
        assert!(display.contains("-8"));
        assert!(!display.contains("conflicts"));
    }

    #[test]
    fn changeset_summary_with_conflicts_display() {
        let summary = ChangesetSummary::from_diffs("merge", &[], true);
        assert!(summary.is_empty());
        let display = format!("{summary}");
        assert!(display.contains("(conflicts)"));
    }

    // -- WorkingCopyDiff tests ----------------------------------------------

    #[test]
    fn working_copy_diff_aggregation() {
        let mut wcd = WorkingCopyDiff::new();
        wcd.add(DiffSummary { insertions: 5, deletions: 2, file_path: "x.rs".into() });
        wcd.add(DiffSummary { insertions: 0, deletions: 0, file_path: "y.rs".into() });
        wcd.add(DiffSummary { insertions: 3, deletions: 1, file_path: "z.rs".into() });
        assert_eq!(wcd.total_insertions(), 8);
        assert_eq!(wcd.total_deletions(), 3);
        assert_eq!(wcd.file_count(), 2); // y.rs has no changes
        assert!(wcd.get("x.rs").is_some());
        assert!(wcd.get("missing.rs").is_none());
        let summary = wcd.summarize("snapshot");
        assert_eq!(summary.file_count, 3);
        assert_eq!(summary.total_changes(), 11);
    }

    // -- StagingArea tests --------------------------------------------------

    #[test]
    fn staging_area_stage_and_unstage() {
        let mut sa = StagingArea::new();
        sa.unstaged.push(ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None });
        sa.unstaged.push(ScmResource { uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None });
        assert_eq!(sa.unstaged_count(), 2);
        assert_eq!(sa.staged_count(), 0);
        assert!(sa.is_empty());

        assert!(sa.stage("a.rs"));
        assert_eq!(sa.staged_count(), 1);
        assert_eq!(sa.unstaged_count(), 1);
        assert!(!sa.stage("nonexistent.rs"));

        assert!(sa.unstage("a.rs"));
        assert_eq!(sa.staged_count(), 0);
        assert_eq!(sa.unstaged_count(), 2);

        sa.stage_all();
        assert_eq!(sa.staged_count(), 2);
        assert_eq!(sa.unstaged_count(), 0);

        sa.unstage_all();
        assert_eq!(sa.staged_count(), 0);
        assert_eq!(sa.unstaged_count(), 2);
    }

    #[test]
    fn staging_area_from_provider() {
        let provider = ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/ws".into(),
            groups: vec![
                ScmGroup {
                    id: "changes".into(), label: "Changes".into(),
                    resources: vec![
                        ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None },
                    ],
                },
                ScmGroup {
                    id: "staged".into(), label: "Staged".into(),
                    resources: vec![
                        ScmResource { uri: "b.rs".into(), status: ScmStatus::Added, original_uri: None },
                    ],
                },
            ],
            count: 2,
        };
        let sa = StagingArea::from_provider(&provider);
        assert_eq!(sa.staged_count(), 1);
        assert_eq!(sa.unstaged_count(), 1);
        assert_eq!(sa.staged_files()[0].uri, "b.rs");
        assert_eq!(sa.unstaged_files()[0].uri, "a.rs");
    }

    // -- ConflictResolver tests ---------------------------------------------

    #[test]
    fn conflict_resolver_resolve_and_query() {
        let mut cr = ConflictResolver::new();
        cr.resolve("a.rs", ConflictResolution::AcceptCurrent);
        cr.resolve("b.rs", ConflictResolution::Manual);
        cr.resolve("c.rs", ConflictResolution::AcceptIncoming);
        assert_eq!(cr.resolved_count(), 3);
        assert_eq!(cr.get_resolution("a.rs"), Some(ConflictResolution::AcceptCurrent));
        assert_eq!(cr.get_resolution("missing"), None);
        assert!(!cr.all_auto_resolved());
        let pending = cr.pending_manual();
        assert_eq!(pending, vec!["b.rs"]);

        // overwrite resolution
        cr.resolve("b.rs", ConflictResolution::AcceptBoth);
        assert!(cr.all_auto_resolved());
        assert_eq!(cr.resolved_count(), 3);
    }

    // -- WorkingCopyExporter tests ------------------------------------------

    #[test]
    fn exporter_produces_correct_lines() {
        let provider = ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/ws".into(),
            groups: vec![
                ScmGroup {
                    id: "changes".into(), label: "Changes".into(),
                    resources: vec![
                        ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None },
                    ],
                },
                ScmGroup {
                    id: "staged".into(), label: "Staged".into(),
                    resources: vec![
                        ScmResource { uri: "new.rs".into(), status: ScmStatus::Renamed, original_uri: Some("old.rs".into()) },
                    ],
                },
            ],
            count: 2,
        };
        let lines = WorkingCopyExporter::export_lines(&provider);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "changes\tModified\ta.rs");
        assert_eq!(lines[1], "staged\tRenamed\tnew.rs\told.rs");
        assert_eq!(WorkingCopyExporter::total_resource_count(&provider), 2);

        let export_str = WorkingCopyExporter::export_string(&provider);
        assert!(export_str.contains("changes\tModified\ta.rs"));
        assert!(export_str.contains('\n'));
    }

    // -- ScmStatus impl tests -----------------------------------------------

    #[test]
    fn scm_status_is_active() {
        assert!(ScmStatus::Modified.is_active());
        assert!(ScmStatus::Added.is_active());
        assert!(ScmStatus::Deleted.is_active());
        assert!(ScmStatus::Conflict.is_active());
        assert!(ScmStatus::Untracked.is_active());
        assert!(ScmStatus::Renamed.is_active());
        assert!(!ScmStatus::Ignored.is_active());
    }

    #[test]
    fn scm_status_is_content_change() {
        assert!(ScmStatus::Modified.is_content_change());
        assert!(ScmStatus::Renamed.is_content_change());
        assert!(!ScmStatus::Added.is_content_change());
        assert!(!ScmStatus::Deleted.is_content_change());
    }

    #[test]
    fn scm_status_short_code_and_display() {
        assert_eq!(ScmStatus::Modified.short_code(), 'M');
        assert_eq!(ScmStatus::Added.short_code(), 'A');
        assert_eq!(ScmStatus::Deleted.short_code(), 'D');
        assert_eq!(ScmStatus::Renamed.short_code(), 'R');
        assert_eq!(ScmStatus::Conflict.short_code(), 'C');
        assert_eq!(ScmStatus::Untracked.short_code(), '?');
        assert_eq!(ScmStatus::Ignored.short_code(), '!');
        assert_eq!(format!("{}", ScmStatus::Modified), "Modified");
        assert_eq!(format!("{}", ScmStatus::Conflict), "Conflict");
    }

    // -- ScmResource impl tests ---------------------------------------------

    #[test]
    fn scm_resource_constructors() {
        let r = ScmResource::new("src/main.rs", ScmStatus::Modified);
        assert_eq!(r.uri, "src/main.rs");
        assert_eq!(r.status, ScmStatus::Modified);
        assert!(r.original_uri.is_none());

        let r2 = ScmResource::renamed("new.rs", "old.rs");
        assert_eq!(r2.status, ScmStatus::Renamed);
        assert_eq!(r2.original_uri.as_deref(), Some("old.rs"));
    }

    #[test]
    fn scm_resource_path_helpers() {
        let r = ScmResource::new("src/utils/helpers.rs", ScmStatus::Added);
        assert_eq!(r.extension(), Some("rs"));
        assert_eq!(r.file_name(), "helpers.rs");
        assert_eq!(r.directory(), Some("src/utils"));

        let r2 = ScmResource::new("Makefile", ScmStatus::Modified);
        assert_eq!(r2.extension(), None);
        assert_eq!(r2.file_name(), "Makefile");
        assert_eq!(r2.directory(), None);
    }

    #[test]
    fn scm_resource_display() {
        let r = ScmResource::new("src/main.rs", ScmStatus::Modified);
        assert_eq!(format!("{r}"), "M src/main.rs");

        let r2 = ScmResource::renamed("new.rs", "old.rs");
        assert_eq!(format!("{r2}"), "R new.rs (was old.rs)");
    }

    // -- ScmGroup impl tests ------------------------------------------------

    #[test]
    fn scm_group_methods() {
        let mut g = ScmGroup::new("changes", "Changes");
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        g.resources.push(ScmResource::new("a.rs", ScmStatus::Modified));
        g.resources.push(ScmResource::new("b.rs", ScmStatus::Added));
        g.resources.push(ScmResource::new("c.rs", ScmStatus::Modified));
        assert_eq!(g.len(), 3);
        assert!(!g.is_empty());
        assert_eq!(g.uris(), vec!["a.rs", "b.rs", "c.rs"]);
        assert_eq!(g.by_status(ScmStatus::Modified).len(), 2);
        assert!(g.has_status(ScmStatus::Added));
        assert!(!g.has_status(ScmStatus::Deleted));
        g.retain(|r| r.status == ScmStatus::Modified);
        assert_eq!(g.len(), 2);
        assert!(!g.has_status(ScmStatus::Added));
    }

    // -- DiffSummary impl tests ---------------------------------------------

    #[test]
    fn diff_summary_new_and_helpers() {
        let d = DiffSummary::new("a.rs", 10, 0);
        assert!(d.is_pure_addition());
        assert!(!d.is_pure_deletion());
        assert!((d.insertion_ratio() - 1.0).abs() < f64::EPSILON);

        let d2 = DiffSummary::new("b.rs", 0, 5);
        assert!(!d2.is_pure_addition());
        assert!(d2.is_pure_deletion());
        assert!((d2.insertion_ratio() - 0.0).abs() < f64::EPSILON);

        let d3 = DiffSummary::new("c.rs", 3, 7);
        assert!(!d3.is_pure_addition());
        assert!(!d3.is_pure_deletion());
        assert!((d3.insertion_ratio() - 0.3).abs() < f64::EPSILON);

        let d4 = DiffSummary::new("d.rs", 0, 0);
        assert!((d4.insertion_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn diff_summary_display() {
        let d = DiffSummary::new("main.rs", 5, 3);
        assert_eq!(format!("{d}"), "main.rs: +5 -3");

        let d2 = DiffSummary { file_path: String::new(), insertions: 2, deletions: 1 };
        assert_eq!(format!("{d2}"), "+2 -1");
    }

    // -- WorkingCopyDiff extended tests --------------------------------------

    #[test]
    fn working_copy_diff_remove_and_empty() {
        let mut wcd = WorkingCopyDiff::new();
        assert!(wcd.is_empty());
        wcd.add(DiffSummary::new("a.rs", 5, 2));
        wcd.add(DiffSummary::new("b.rs", 3, 1));
        assert!(!wcd.is_empty());
        assert!(wcd.remove("a.rs"));
        assert!(!wcd.remove("nonexistent.rs"));
        assert_eq!(wcd.diffs.len(), 1);
        assert_eq!(wcd.diffs[0].file_path, "b.rs");
    }

    #[test]
    fn working_copy_diff_top_changed() {
        let mut wcd = WorkingCopyDiff::new();
        wcd.add(DiffSummary::new("small.rs", 1, 0));
        wcd.add(DiffSummary::new("big.rs", 100, 50));
        wcd.add(DiffSummary::new("medium.rs", 10, 5));
        wcd.add(DiffSummary::new("empty.rs", 0, 0));
        let top = wcd.top_changed(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].file_path, "big.rs");
        assert_eq!(top[1].file_path, "medium.rs");
    }

    #[test]
    fn working_copy_diff_merge() {
        let mut wcd1 = WorkingCopyDiff::new();
        wcd1.add(DiffSummary::new("a.rs", 5, 2));
        wcd1.add(DiffSummary::new("b.rs", 3, 1));

        let mut wcd2 = WorkingCopyDiff::new();
        wcd2.add(DiffSummary::new("a.rs", 2, 1)); // same file, should merge
        wcd2.add(DiffSummary::new("c.rs", 4, 0)); // new file

        wcd1.merge(&wcd2);
        assert_eq!(wcd1.diffs.len(), 3);
        let a = wcd1.get("a.rs").unwrap();
        assert_eq!(a.insertions, 7);
        assert_eq!(a.deletions, 3);
        assert!(wcd1.get("c.rs").is_some());
    }

    // -- ScmChangeSet extended tests ----------------------------------------

    #[test]
    fn changeset_by_status_and_has_status() {
        let mut cs = ScmChangeSet::new("test");
        cs.add(ScmResource::new("a.rs", ScmStatus::Modified));
        cs.add(ScmResource::new("b.rs", ScmStatus::Added));
        cs.add(ScmResource::new("c.rs", ScmStatus::Modified));
        assert_eq!(cs.by_status(ScmStatus::Modified).len(), 2);
        assert!(cs.has_status(ScmStatus::Added));
        assert!(!cs.has_status(ScmStatus::Deleted));
    }

    #[test]
    fn changeset_partition() {
        let mut cs = ScmChangeSet::new("all");
        cs.add(ScmResource::new("a.rs", ScmStatus::Modified));
        cs.add(ScmResource::new("b.rs", ScmStatus::Added));
        cs.add(ScmResource::new("c.rs", ScmStatus::Modified));
        let (selected, rest) = cs.partition(|r| r.status == ScmStatus::Modified);
        assert_eq!(selected.len(), 2);
        assert_eq!(rest.len(), 1);
        assert!(selected.label.contains("selected"));
        assert!(rest.label.contains("remaining"));
    }

    // -- StagingArea extended tests -----------------------------------------

    #[test]
    fn staging_area_query_helpers() {
        let mut sa = StagingArea::new();
        sa.unstaged.push(ScmResource::new("a.rs", ScmStatus::Modified));
        sa.unstaged.push(ScmResource::new("b.rs", ScmStatus::Added));
        assert_eq!(sa.total_count(), 2);
        assert!(sa.is_unstaged("a.rs"));
        assert!(!sa.is_staged("a.rs"));
        assert_eq!(sa.unstaged_uris(), vec!["a.rs", "b.rs"]);

        sa.stage("a.rs");
        assert!(sa.is_staged("a.rs"));
        assert!(!sa.is_unstaged("a.rs"));
        assert_eq!(sa.staged_uris(), vec!["a.rs"]);
        assert_eq!(sa.total_count(), 2);
    }

    // -- ConflictResolver extended tests ------------------------------------

    #[test]
    fn conflict_resolver_remove_and_clear() {
        let mut cr = ConflictResolver::new();
        cr.resolve("a.rs", ConflictResolution::AcceptCurrent);
        cr.resolve("b.rs", ConflictResolution::Manual);
        cr.resolve("c.rs", ConflictResolution::AcceptIncoming);
        assert_eq!(cr.resolved_uris(), vec!["a.rs", "b.rs", "c.rs"]);

        assert!(cr.remove_resolution("b.rs"));
        assert!(!cr.remove_resolution("nonexistent.rs"));
        assert_eq!(cr.resolved_count(), 2);

        cr.clear();
        assert_eq!(cr.resolved_count(), 0);
        assert!(cr.resolved_uris().is_empty());
    }

    // ── Conflict detector tests ──

    fn provider_with_conflict() -> ScmProvider {
        ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/workspace".into(),
            groups: vec![ScmGroup {
                id: "changes".into(),
                label: "Changes".into(),
                resources: vec![
                    ScmResource { uri: "a.rs".into(), status: ScmStatus::Modified, original_uri: None },
                    ScmResource { uri: "b.rs".into(), status: ScmStatus::Conflict, original_uri: None },
                    ScmResource { uri: "c.rs".into(), status: ScmStatus::Conflict, original_uri: None },
                ],
            }],
            count: 3,
        }
    }

    #[test]
    fn conflict_detector_scan() {
        let mut detector = WorkingCopyConflictDetector::new();
        let provider = provider_with_conflict();
        detector.scan_provider(&provider);
        assert_eq!(detector.conflict_count(), 2);
        assert!(detector.is_conflicted("b.rs"));
        assert!(!detector.is_conflicted("a.rs"));
    }

    #[test]
    fn conflict_detector_mark_resolved() {
        let mut detector = WorkingCopyConflictDetector::new();
        let provider = provider_with_conflict();
        detector.scan_provider(&provider);
        assert!(detector.mark_resolved("b.rs"));
        assert!(!detector.is_conflicted("b.rs"));
        assert_eq!(detector.conflict_count(), 1);
    }

    #[test]
    fn conflict_detector_clear() {
        let mut detector = WorkingCopyConflictDetector::new();
        let provider = provider_with_conflict();
        detector.scan_provider(&provider);
        detector.clear();
        assert!(!detector.has_conflicts());
    }

    // ── Dirty file tracker tests ──

    #[test]
    fn dirty_tracker_basic() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("a.rs", 100);
        tracker.mark_dirty("b.rs", 200);
        assert_eq!(tracker.dirty_count(), 2);
        assert!(tracker.is_dirty("a.rs"));
        assert!(!tracker.is_dirty("c.rs"));
    }

    #[test]
    fn dirty_tracker_mark_clean() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("a.rs", 100);
        assert!(tracker.mark_clean("a.rs"));
        assert!(!tracker.is_dirty("a.rs"));
        assert!(!tracker.mark_clean("nonexistent.rs"));
    }

    #[test]
    fn dirty_tracker_update_timestamp() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("a.rs", 100);
        tracker.mark_dirty("a.rs", 200);
        assert_eq!(tracker.dirty_count(), 1);
        let entry = tracker.get_entry("a.rs").unwrap();
        assert_eq!(entry.dirty_since, 100);
        assert_eq!(entry.last_modified, 200);
    }

    #[test]
    fn dirty_tracker_dirty_longer_than() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("old.rs", 10);
        tracker.mark_dirty("new.rs", 90);
        let old_files = tracker.dirty_longer_than(100, 50);
        assert_eq!(old_files.len(), 1);
        assert_eq!(old_files[0], "old.rs");
    }

    #[test]
    fn dirty_tracker_oldest() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("b.rs", 200);
        tracker.mark_dirty("a.rs", 100);
        assert_eq!(tracker.oldest_dirty().unwrap().uri, "a.rs");
    }

    #[test]
    fn dirty_tracker_mark_all_clean() {
        let mut tracker = DirtyFileTracker::new();
        tracker.mark_dirty("a.rs", 100);
        tracker.mark_dirty("b.rs", 200);
        tracker.mark_all_clean();
        assert_eq!(tracker.dirty_count(), 0);
    }

    // ── Diff summary tests ──

    #[test]
    fn diff_summary_from_provider() {
        let provider = ScmProvider {
            id: "git".into(),
            label: "Git".into(),
            root_uri: "/ws".into(),
            groups: vec![ScmGroup {
                id: "changes".into(),
                label: "Changes".into(),
                resources: vec![
                    ScmResource { uri: "a.rs".into(), status: ScmStatus::Added, original_uri: None },
                    ScmResource { uri: "b.rs".into(), status: ScmStatus::Deleted, original_uri: None },
                    ScmResource { uri: "c.rs".into(), status: ScmStatus::Modified, original_uri: None },
                ],
            }],
            count: 3,
        };
        let summary = WorkingCopyDiffSummary::from_provider(&provider);
        assert_eq!(summary.files_changed, 3);
        assert_eq!(summary.insertions, 2); // added + modified
        assert_eq!(summary.deletions, 2);  // deleted + modified
    }

    #[test]
    fn diff_summary_merge() {
        let a = ProviderDiffSummary { files_changed: 2, insertions: 3, deletions: 1 };
        let b = ProviderDiffSummary { files_changed: 1, insertions: 1, deletions: 0 };
        let merged = WorkingCopyDiffSummary::merge(&a, &b);
        assert_eq!(merged.files_changed, 3);
        assert_eq!(merged.insertions, 4);
    }

    #[test]
    fn diff_summary_one_line_clean() {
        let s = ProviderDiffSummary::default();
        assert_eq!(WorkingCopyDiffSummary::one_line(&s), "Working tree clean");
    }

    #[test]
    fn provider_diff_summary_display() {
        let s = ProviderDiffSummary { files_changed: 2, insertions: 3, deletions: 1 };
        let display = format!("{s}");
        assert!(display.contains("2 file(s)"));
    }

    // ── Revert handler tests ──

    #[test]
    fn revert_handler_queue_and_process() {
        let mut handler = WorkingCopyRevertHandler::new();
        handler.queue_revert("a.rs");
        handler.queue_revert("b.rs");
        handler.queue_revert("a.rs"); // duplicate ignored
        assert_eq!(handler.queued_count(), 2);
        assert_eq!(handler.pending_count(), 2);

        handler.mark_reverted("a.rs");
        assert_eq!(handler.pending_count(), 1);
        assert_eq!(handler.reverted_count(), 1);

        handler.mark_failed("b.rs", "permission denied");
        assert_eq!(handler.failed_count(), 1);
        assert!(handler.is_complete());
    }

    #[test]
    fn revert_handler_failed_items() {
        let mut handler = WorkingCopyRevertHandler::new();
        handler.queue_revert("x.rs");
        handler.mark_failed("x.rs", "read only");
        let failed = handler.failed_items();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0], ("x.rs", "read only"));
    }

    #[test]
    fn revert_handler_queue_provider() {
        let mut handler = WorkingCopyRevertHandler::new();
        let provider = provider_with_conflict();
        handler.queue_provider(&provider);
        assert_eq!(handler.queued_count(), 3);
    }

    #[test]
    fn revert_handler_clear() {
        let mut handler = WorkingCopyRevertHandler::new();
        handler.queue_revert("a.rs");
        handler.clear();
        assert_eq!(handler.queued_count(), 0);
    }


    // -- wb_workingcopy additional tests -------------------------------------------

    #[test]
    fn x_wb_workingcopy_panel_state_new() {
        let p = XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbWorkingcopyLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_workingcopy_panel_area() {
        let p = XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_workingcopy_panel_toggle() {
        let mut p = XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_workingcopy_panel_resize() {
        let mut p = XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_workingcopy_panel_is_narrow() {
        let mut p = XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_workingcopy_total_visible_area_basic() {
        let panels = vec![
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "a"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_workingcopy_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_workingcopy_total_visible_area_hidden() {
        let mut panels = vec![
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "a"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_workingcopy_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_workingcopy_count_in_region_basic() {
        let panels = vec![
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "a"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "b"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_workingcopy_count_in_region(&panels, XWbWorkingcopyLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_workingcopy_count_in_region(&panels, XWbWorkingcopyLayoutRegion::Editor), 1);
        assert_eq!(x_wb_workingcopy_count_in_region(&panels, XWbWorkingcopyLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_workingcopy_widest_panel_basic() {
        let mut panels = vec![
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "narrow"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_workingcopy_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_workingcopy_collapse_region_basic() {
        let mut panels = vec![
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "a"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Sidebar, "b"),
            XWbWorkingcopyPanelState::new(XWbWorkingcopyLayoutRegion::Editor, "c"),
        ];
        x_wb_workingcopy_collapse_region(&mut panels, XWbWorkingcopyLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_workingcopy_layout_constraint_clamp() {
        let lc = XWbWorkingcopyLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_workingcopy_layout_constraint_satisfied() {
        let lc = XWbWorkingcopyLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_workingcopy_widest_panel_empty() {
        let panels: Vec<XWbWorkingcopyPanelState> = vec![];
        assert!(x_wb_workingcopy_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_workingcopy_layout_region_eq() {
        assert_eq!(XWbWorkingcopyLayoutRegion::Sidebar, XWbWorkingcopyLayoutRegion::Sidebar);
        assert_ne!(XWbWorkingcopyLayoutRegion::Sidebar, XWbWorkingcopyLayoutRegion::Panel);
    }


    // -- wb_workingcopy extended domain tests ----------------------------------------

    #[test]
    fn y_wb_workingcopy_enum_index() {
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Clean.index(), 0);
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Modified.index(), 1);
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Staged.index(), 2);
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Conflicted.index(), 3);
    }

    #[test]
    fn y_wb_workingcopy_enum_label() {
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Clean.label(), "Clean");
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Modified.label(), "Modified");
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Staged.label(), "Staged");
        assert_eq!(YWbWorkingcopyWorkingCopyStatus::Conflicted.label(), "Conflicted");
    }

    #[test]
    fn y_wb_workingcopy_enum_all() {
        let all = YWbWorkingcopyWorkingCopyStatus::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_workingcopy_enum_is_default() {
        assert!(YWbWorkingcopyWorkingCopyStatus::Clean.is_default());
        assert!(!YWbWorkingcopyWorkingCopyStatus::Conflicted.is_default());
    }

    #[test]
    fn y_wb_workingcopy_enum_display() {
        assert_eq!(format!("{}", YWbWorkingcopyWorkingCopyStatus::Clean), "Clean");
    }

    #[test]
    fn y_wb_workingcopy_struct_new() {
        let s = YWbWorkingcopyWorkingCopyDiff::new();
        let _ = s.summary();
    }

    #[test]
    fn y_wb_workingcopy_fingerprint_deterministic() {
        let h1 = y_wb_workingcopy_fingerprint("hello");
        let h2 = y_wb_workingcopy_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_workingcopy_fingerprint("a"), y_wb_workingcopy_fingerprint("b"));
    }

    #[test]
    fn y_wb_workingcopy_truncate_short() {
        assert_eq!(y_wb_workingcopy_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_workingcopy_truncate_long() {
        let r = y_wb_workingcopy_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_workingcopy_normalize_key_basic() {
        assert_eq!(y_wb_workingcopy_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_workingcopy_split_path_basic() {
        let parts = y_wb_workingcopy_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_workingcopy_count_occurrences_basic() {
        assert_eq!(y_wb_workingcopy_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_workingcopy_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_workingcopy_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_workingcopy_in_range_basic() {
        assert!(y_wb_workingcopy_in_range(5, 1, 10));
        assert!(y_wb_workingcopy_in_range(1, 1, 10));
        assert!(y_wb_workingcopy_in_range(10, 1, 10));
        assert!(!y_wb_workingcopy_in_range(0, 1, 10));
        assert!(!y_wb_workingcopy_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_workingcopy_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_workingcopy_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_workingcopy_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_workingcopy_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_workingcopy Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_workingcopy_priority_weight() {
        assert_eq!(ZWbWorkingcopyPriority::Idle.weight(), 0);
        assert_eq!(ZWbWorkingcopyPriority::Normal.weight(), 2);
        assert_eq!(ZWbWorkingcopyPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_workingcopy_priority_label() {
        assert_eq!(ZWbWorkingcopyPriority::Low.label(), "low");
        assert_eq!(ZWbWorkingcopyPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_workingcopy_priority_is_elevated() {
        assert!(!ZWbWorkingcopyPriority::Normal.is_elevated());
        assert!(ZWbWorkingcopyPriority::High.is_elevated());
        assert!(ZWbWorkingcopyPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_workingcopy_priority_display() {
        assert_eq!(format!("{}", ZWbWorkingcopyPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_workingcopy_priority_all_asc() {
        let all = ZWbWorkingcopyPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbWorkingcopyPriority::Idle);
        assert_eq!(all[4], ZWbWorkingcopyPriority::Realtime);
    }

    #[test]
    fn z_wb_workingcopy_struct_new() {
        let s = ZWbWorkingcopyWorkingCopyCheckpoint::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_workingcopy_struct_toggled_clone() {
        let s = ZWbWorkingcopyWorkingCopyCheckpoint::new();
        let t = s.toggled_clone();
        let _ = t.label;
    }

    #[test]
    fn z_wb_workingcopy_rolling_hash_deterministic() {
        let h1 = z_wb_workingcopy_rolling_hash(b"test");
        let h2 = z_wb_workingcopy_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_workingcopy_rolling_hash(b"a"), z_wb_workingcopy_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_workingcopy_pad_to_basic() {
        assert_eq!(z_wb_workingcopy_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_workingcopy_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_workingcopy_is_identifier_basic() {
        assert!(z_wb_workingcopy_is_identifier("foo_bar"));
        assert!(z_wb_workingcopy_is_identifier("abc123"));
        assert!(!z_wb_workingcopy_is_identifier(""));
        assert!(!z_wb_workingcopy_is_identifier("has space"));
    }

    #[test]
    fn z_wb_workingcopy_levenshtein_basic() {
        assert_eq!(z_wb_workingcopy_levenshtein("", ""), 0);
        assert_eq!(z_wb_workingcopy_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_workingcopy_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_workingcopy_unique_words_basic() {
        let w = z_wb_workingcopy_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_workingcopy_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_workingcopy_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_workingcopy_common_prefix_basic() {
        assert_eq!(z_wb_workingcopy_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_workingcopy_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_workingcopy_struct_clear() {
        let mut s = ZWbWorkingcopyWorkingCopyCheckpoint::new();
        s.file_hashes.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_workingcopy_rolling_hash_empty() {
        let h = z_wb_workingcopy_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_43_push_and_len() {
        let mut rb = super::XbRingBuffer43::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_43_overwrite() {
        let mut rb = super::XbRingBuffer43::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_43_get_out_of_bounds() {
        let rb = super::XbRingBuffer43::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_43_drain_all() {
        let mut rb = super::XbRingBuffer43::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_43_peek_front_back() {
        let mut rb = super::XbRingBuffer43::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_43_clear() {
        let mut rb = super::XbRingBuffer43::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_43_capacity() {
        let rb = super::XbRingBuffer43::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_43_basic() {
        let h = super::xb_fnv1a_43(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_43(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_43_different_inputs() {
        let h1 = super::xb_fnv1a_43(b"abc");
        let h2 = super::xb_fnv1a_43(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_43_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_43(&data);
        let dec = super::xb_rle_decode_43(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_43_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_43(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_43(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_43_values() {
        assert!((super::xb_clamp_43(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_43(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_43(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_43_values() {
        assert!((super::xb_lerp_43(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_43(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_43(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_43_wrap_around_twice() {
        let mut rb = super::XbRingBuffer43::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 234 ----

    #[test]
    fn xc_234_pool_new_empty() {
        let pool: super::Xc234Pool<i32> = super::Xc234Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_234_pool_release_acquire() {
        let mut pool = super::Xc234Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_234_pool_acquire_empty() {
        let mut pool: super::Xc234Pool<i32> = super::Xc234Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_234_pool_full() {
        let mut pool = super::Xc234Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_234_pool_drain() {
        let mut pool = super::Xc234Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_234_pool_stats() {
        let mut pool = super::Xc234Pool::new(8);
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
    fn xc_234_pool_clear() {
        let mut pool = super::Xc234Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_234_pool_shrink() {
        let mut pool = super::Xc234Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_234_pool_default() {
        let pool: super::Xc234Pool<String> = super::Xc234Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_234_pool_extend() {
        let mut pool = super::Xc234Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_234_pool_retain() {
        let mut pool = super::Xc234Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_234_scheduler_round_robin() {
        let mut sched = super::Xc234Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_234_scheduler_empty() {
        let mut sched = super::Xc234Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_234_scheduler_reset() {
        let mut sched = super::Xc234Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_234_scheduler_add_remove() {
        let mut sched = super::Xc234Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_234_scheduler_targets() {
        let sched = super::Xc234Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_234_hash_empty() {
        assert_eq!(super::xc_234_hash(b""), 5381);
    }

    #[test]
    fn xc_234_hash_data() {
        let h = super::xc_234_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_234_hash(b"hello"), h);
    }

    #[test]
    fn xc_234_reverse_str() {
        assert_eq!(super::xc_234_reverse("abc"), "cba");
        assert_eq!(super::xc_234_reverse(""), "");
    }

}
