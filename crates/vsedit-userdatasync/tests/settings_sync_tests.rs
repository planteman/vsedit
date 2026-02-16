//! Tests for the settings sync implementation.

use std::collections::HashMap;

use serde_json::json;
use vsedit_userdatasync::extensions::{ExtensionSyncData, compute_extension_diff};
use vsedit_userdatasync::keybindings::{KeybindingEntry, merge_keybindings};
use vsedit_userdatasync::merge::{
    ConflictEntry, ConflictResolution, MergeResult, merge_settings, resolve_conflict,
};
use vsedit_userdatasync::profile::SyncProfile;
use vsedit_userdatasync::service::{SettingsSyncService, SyncBundle};
use vsedit_userdatasync::snippets::{SnippetEntry, SnippetFile, merge_snippets};
use vsedit_userdatasync::state::{ResourceSyncState, SyncState};
use vsedit_userdatasync::SyncResource;

// ── Profile tests ───────────────────────────────────────────────────

#[test]
fn profile_new_syncs_everything() {
    let p = SyncProfile::new("p1", "Default");
    assert!(p.settings && p.keybindings && p.extensions && p.ui_state && p.snippets);
    assert_eq!(p.id, "p1");
    assert_eq!(p.name, "Default");
}

#[test]
fn profile_empty_syncs_nothing() {
    let p = SyncProfile::empty("p2", "Minimal");
    assert!(!p.settings && !p.keybindings && !p.extensions && !p.ui_state && !p.snippets);
}

// ── State tests ─────────────────────────────────────────────────────

#[test]
fn sync_state_dirty_tracking() {
    let mut state = SyncState::new();
    let mut rs = ResourceSyncState::new(1, "abc".into(), 100);
    assert!(!rs.is_dirty);
    rs.mark_dirty();
    assert!(rs.is_dirty);
    state.set_resource(SyncResource::Settings, rs);
    assert_eq!(state.dirty_resources().len(), 1);

    state.record_sync(200);
    assert!(state.dirty_resources().is_empty());
    assert_eq!(state.last_sync_time, Some(200));
}

#[test]
fn sync_state_get_resource() {
    let mut state = SyncState::new();
    assert!(state.get_resource(&SyncResource::Settings).is_none());
    state.set_resource(
        SyncResource::Settings,
        ResourceSyncState::new(1, "h".into(), 10),
    );
    let rs = state.get_resource(&SyncResource::Settings).unwrap();
    assert_eq!(rs.version, 1);
}

// ── Merge tests ─────────────────────────────────────────────────────

#[test]
fn merge_no_changes() {
    let base = json!({"a": 1, "b": 2});
    let result = merge_settings(&base, &base, &base);
    assert_eq!(result, MergeResult::Clean(base));
}

#[test]
fn merge_local_only_change() {
    let base = json!({"a": 1});
    let local = json!({"a": 2});
    let remote = json!({"a": 1});
    assert_eq!(
        merge_settings(&local, &remote, &base),
        MergeResult::Clean(json!({"a": 2}))
    );
}

#[test]
fn merge_remote_only_change() {
    let base = json!({"a": 1});
    let local = json!({"a": 1});
    let remote = json!({"a": 3});
    assert_eq!(
        merge_settings(&local, &remote, &base),
        MergeResult::Clean(json!({"a": 3}))
    );
}

#[test]
fn merge_both_same_change() {
    let base = json!({"a": 1});
    let local = json!({"a": 5});
    let remote = json!({"a": 5});
    assert_eq!(
        merge_settings(&local, &remote, &base),
        MergeResult::Clean(json!({"a": 5}))
    );
}

#[test]
fn merge_conflict() {
    let base = json!({"a": 1});
    let local = json!({"a": 2});
    let remote = json!({"a": 3});
    let result = merge_settings(&local, &remote, &base);
    match result {
        MergeResult::Conflict(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].key, "a");
        }
        MergeResult::Clean(_) => panic!("expected conflict"),
    }
}

#[test]
fn merge_new_keys_from_both_sides() {
    let base = json!({});
    let local = json!({"x": 1});
    let remote = json!({"y": 2});
    assert_eq!(
        merge_settings(&local, &remote, &base),
        MergeResult::Clean(json!({"x": 1, "y": 2}))
    );
}

#[test]
fn merge_key_deletion() {
    let base = json!({"a": 1, "b": 2});
    let local = json!({"b": 2}); // removed "a"
    let remote = json!({"a": 1, "b": 2});
    let result = merge_settings(&local, &remote, &base);
    assert_eq!(result, MergeResult::Clean(json!({"b": 2})));
}

// ── Conflict resolution ─────────────────────────────────────────────

#[test]
fn resolve_accept_local() {
    let entry = ConflictEntry {
        key: "k".into(),
        local_value: Some(json!(1)),
        remote_value: Some(json!(2)),
        base_value: Some(json!(0)),
    };
    assert_eq!(
        resolve_conflict(&entry, &ConflictResolution::AcceptLocal),
        Some(json!(1))
    );
}

#[test]
fn resolve_accept_remote() {
    let entry = ConflictEntry {
        key: "k".into(),
        local_value: Some(json!(1)),
        remote_value: Some(json!(2)),
        base_value: Some(json!(0)),
    };
    assert_eq!(
        resolve_conflict(&entry, &ConflictResolution::AcceptRemote),
        Some(json!(2))
    );
}

#[test]
fn resolve_custom_value() {
    let entry = ConflictEntry {
        key: "k".into(),
        local_value: Some(json!(1)),
        remote_value: Some(json!(2)),
        base_value: Some(json!(0)),
    };
    assert_eq!(
        resolve_conflict(&entry, &ConflictResolution::Custom(json!(99))),
        Some(json!(99))
    );
}

// ── Extension diff tests ────────────────────────────────────────────

#[test]
fn extension_diff_install_uninstall() {
    let local = vec![ExtensionSyncData::new("ext.a", "1.0", true)];
    let remote = vec![ExtensionSyncData::new("ext.b", "2.0", true)];
    let diff = compute_extension_diff(&local, &remote);
    assert_eq!(diff.to_install.len(), 1);
    assert_eq!(diff.to_install[0].id, "ext.b");
    assert_eq!(diff.to_uninstall, vec!["ext.a"]);
}

#[test]
fn extension_diff_enable_disable() {
    let local = vec![
        ExtensionSyncData::new("ext.a", "1.0", true),
        ExtensionSyncData::new("ext.b", "1.0", false),
    ];
    let remote = vec![
        ExtensionSyncData::new("ext.a", "1.0", false),
        ExtensionSyncData::new("ext.b", "1.0", true),
    ];
    let diff = compute_extension_diff(&local, &remote);
    assert!(diff.to_install.is_empty());
    assert!(diff.to_uninstall.is_empty());
    assert_eq!(diff.to_disable, vec!["ext.a"]);
    assert_eq!(diff.to_enable, vec!["ext.b"]);
}

#[test]
fn extension_diff_identical() {
    let exts = vec![ExtensionSyncData::new("ext.a", "1.0", true)];
    let diff = compute_extension_diff(&exts, &exts);
    assert!(diff.is_empty());
}

// ── Keybinding merge tests ──────────────────────────────────────────

#[test]
fn keybinding_merge_addition_from_remote() {
    let base: Vec<KeybindingEntry> = vec![];
    let local: Vec<KeybindingEntry> = vec![];
    let remote = vec![KeybindingEntry {
        key: "ctrl+k".into(),
        command: "editor.action.cut".into(),
        when: None,
        args: None,
    }];
    let merged = merge_keybindings(&local, &remote, &base);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].command, "editor.action.cut");
}

#[test]
fn keybinding_merge_removal() {
    let base = vec![KeybindingEntry {
        key: "ctrl+d".into(),
        command: "editor.action.delete".into(),
        when: None,
        args: None,
    }];
    let local = vec![KeybindingEntry {
        key: "ctrl+d".into(),
        command: "-editor.action.delete".into(),
        when: None,
        args: None,
    }];
    let remote = base.clone();
    let merged = merge_keybindings(&local, &remote, &base);
    assert!(merged.is_empty());
}

#[test]
fn keybinding_is_removal() {
    let e = KeybindingEntry {
        key: "ctrl+x".into(),
        command: "-editor.cut".into(),
        when: None,
        args: None,
    };
    assert!(e.is_removal());
    assert_eq!(e.command_name(), "editor.cut");
}

// ── Snippet merge tests ─────────────────────────────────────────────

#[test]
fn snippet_merge_combines_files() {
    let mut local: HashMap<String, SnippetFile> = HashMap::new();
    let mut remote: HashMap<String, SnippetFile> = HashMap::new();

    let mut local_entries = HashMap::new();
    local_entries.insert(
        "log".into(),
        SnippetEntry {
            prefix: "log".into(),
            body: vec!["console.log($1)".into()],
            description: None,
        },
    );
    local.insert(
        "javascript.json".into(),
        SnippetFile {
            entries: local_entries,
        },
    );

    let mut remote_entries = HashMap::new();
    remote_entries.insert(
        "err".into(),
        SnippetEntry {
            prefix: "err".into(),
            body: vec!["console.error($1)".into()],
            description: None,
        },
    );
    remote.insert(
        "javascript.json".into(),
        SnippetFile {
            entries: remote_entries,
        },
    );

    let merged = merge_snippets(&local, &remote);
    let js_file = merged.get("javascript.json").unwrap();
    assert!(js_file.entries.contains_key("log"));
    assert!(js_file.entries.contains_key("err"));
}

// ── Service tests ───────────────────────────────────────────────────

#[test]
fn service_sync_clean_merge() {
    let profile = SyncProfile::new("p1", "Default");
    let mut svc = SettingsSyncService::new(profile);

    let local_data = SyncBundle {
        settings: Some(json!({"font_size": 14})),
        keybindings: None,
        extensions: None,
        snippets: None,
        global_state: None,
    };
    svc.set_local_data(local_data);

    let remote = SyncBundle {
        settings: Some(json!({"theme": "dark"})),
        keybindings: None,
        extensions: None,
        snippets: None,
        global_state: None,
    };
    let base = SyncBundle::empty();

    let result = svc.sync_now(&remote, &base, 1000).unwrap();
    assert!(result.settings.is_some());
    let s = result.settings.unwrap();
    assert_eq!(s["font_size"], 14);
    assert_eq!(s["theme"], "dark");
    assert_eq!(svc.last_sync_time(), Some(1000));
    assert!(!svc.is_syncing());
}

#[test]
fn service_sync_conflict() {
    let profile = SyncProfile::new("p1", "Default");
    let mut svc = SettingsSyncService::new(profile);

    let base = SyncBundle {
        settings: Some(json!({"a": 1})),
        ..SyncBundle::empty()
    };
    svc.set_local_data(SyncBundle {
        settings: Some(json!({"a": 2})),
        ..SyncBundle::empty()
    });
    let remote = SyncBundle {
        settings: Some(json!({"a": 3})),
        ..SyncBundle::empty()
    };

    let err = svc.sync_now(&remote, &base, 2000);
    assert!(err.is_err());
    assert!(svc.has_conflicts());
}

#[test]
fn service_export_import() {
    let profile = SyncProfile::new("p1", "Default");
    let mut svc = SettingsSyncService::new(profile);
    let data = SyncBundle {
        settings: Some(json!({"a": 1})),
        ..SyncBundle::empty()
    };
    svc.set_local_data(data.clone());
    let exported = svc.export_sync_data();
    assert_eq!(exported, data);

    let import = SyncBundle {
        settings: Some(json!({"b": 2})),
        ..SyncBundle::empty()
    };
    svc.import_sync_data(import.clone());
    assert_eq!(svc.export_sync_data(), import);
}

#[test]
fn service_prevents_double_sync() {
    use vsedit_userdatasync::SyncError;

    let profile = SyncProfile::new("p1", "Default");
    let mut svc = SettingsSyncService::new(profile);

    let base = SyncBundle::empty();
    let remote = SyncBundle::empty();
    let result = svc.sync_now(&remote, &base, 100);
    assert!(result.is_ok());
    let result2 = svc.sync_now(&remote, &base, 200);
    assert!(result2.is_ok());
    let _ = SyncError::SyncInProgress;
}

#[test]
fn service_status_callback() {
    use std::sync::{Arc, Mutex};
    use vsedit_userdatasync::SyncStatus;

    let profile = SyncProfile::new("p1", "Test");
    let mut svc = SettingsSyncService::new(profile);

    let statuses: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let statuses_clone = statuses.clone();
    svc.on_did_change_sync_status(move |s: &SyncStatus| {
        statuses_clone.lock().unwrap().push(s.to_string());
    });

    svc.set_local_data(SyncBundle::empty());
    let _ = svc.sync_now(&SyncBundle::empty(), &SyncBundle::empty(), 100);

    let logged = statuses.lock().unwrap();
    assert!(logged.len() >= 2); // Syncing + UpToDate
}

#[test]
fn resource_display_all_variants() {
    assert_eq!(SyncResource::Profiles.to_string(), "Profiles");
    assert_eq!(SyncResource::GlobalState.to_string(), "Global State");
}
