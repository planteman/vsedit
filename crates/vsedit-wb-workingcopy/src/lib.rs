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
}
