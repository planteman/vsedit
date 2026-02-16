//! Dirty file tracking.

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
}
