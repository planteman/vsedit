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
}
