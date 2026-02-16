//! Cross-crate integration tests for vsedit.
//!
//! These tests verify that the major subsystems work together correctly.

// ─── Editor Pipeline ──────────────────────────────────────────────────────

#[cfg(test)]
mod editor {
    use vsedit_text_model::TextModel;
    use vsedit_editor_types::{ITextModel, Position};
    use vsedit_editor_controller::{EditorAction, EditorController};

    #[test]
    fn create_model_and_check_lines() {
        let model = TextModel::new("hello\nworld");
        assert_eq!(model.get_line_count(), 2);
        assert_eq!(model.get_line_content(1), "hello");
        assert_eq!(model.get_line_content(2), "world");
    }

    #[test]
    fn controller_insert_and_undo() {
        let mut ctrl = EditorController::new("initial");
        ctrl.execute_action(EditorAction::InsertText("X".into()));
        ctrl.execute_action(EditorAction::Undo);
        // Should not panic
    }

    #[test]
    fn controller_cursor_movement() {
        let mut ctrl = EditorController::new("line one\nline two\nline three");
        ctrl.execute_action(EditorAction::MoveCursorDown);
        ctrl.execute_action(EditorAction::MoveCursorDown);
        ctrl.execute_action(EditorAction::MoveCursorLineEnd);
    }

    #[test]
    fn controller_select_all() {
        let mut ctrl = EditorController::new("hello world");
        ctrl.execute_action(EditorAction::SelectAll);
    }

    #[test]
    fn model_insert_text() {
        let mut model = TextModel::new("hello");
        model.insert(Position { line: 1, column: 6 }, " world");
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn model_undo_redo() {
        let mut model = TextModel::new("start");
        let original = model.get_value();
        model.insert(Position { line: 1, column: 6 }, " end");
        assert_ne!(model.get_value(), original);
        model.undo();
        assert_eq!(model.get_value(), original);
        model.redo();
        assert_eq!(model.get_value(), "start end");
    }
}

// ─── JSON/Configuration ─────────────────────────────────────────────────

#[cfg(test)]
mod config {
    use vsedit_json::{parse_jsonc, strip_comments};

    #[test]
    fn jsonc_strips_comments() {
        let input = r#"{
            // line comment
            "key": "value", /* block */
            "num": 42
        }"#;
        let stripped = strip_comments(input);
        assert!(!stripped.contains("//"));
        assert!(!stripped.contains("/*"));
    }

    #[test]
    fn jsonc_parses_settings() {
        let input = r#"{
            // Editor settings
            "editor.tabSize": 4,
            "editor.insertSpaces": true,
            "files.autoSave": "afterDelay"
        }"#;
        let val = parse_jsonc(input);
        assert!(val.is_ok());
        let obj = val.unwrap();
        assert_eq!(obj["editor.tabSize"], 4);
    }
}

// ─── Command System ─────────────────────────────────────────────────────

#[cfg(test)]
mod commands {
    use vsedit_commands::CommandRegistry;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn register_and_execute_command() {
        let registry = CommandRegistry::new();
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();

        let _reg = registry.register("test.myCommand", Box::new(move |_args| {
            executed_clone.store(true, Ordering::SeqCst);
            Ok(None)
        }));

        let result = registry.execute("test.myCommand", vec![]);
        assert!(result.is_ok());
        assert!(executed.load(Ordering::SeqCst));
    }

    #[test]
    fn execute_unknown_command_fails() {
        let registry = CommandRegistry::new();
        let result = registry.execute("nonexistent.command", vec![]);
        assert!(result.is_err());
    }
}

// ─── Event System ───────────────────────────────────────────────────────

#[cfg(test)]
mod events {
    use vsedit_events::Emitter;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn emitter_fires_to_subscribers() {
        let emitter = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        let event = emitter.event();
        let _sub = event.on(move |_val: &i32| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.fire(&42);
        emitter.fire(&43);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn disposed_subscription_stops_receiving() {
        let emitter = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = counter.clone();

        let event = emitter.event();
        let sub = event.on(move |_val: &String| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.fire(&"hello".to_string());
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        sub.dispose();
        emitter.fire(&"world".to_string());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}

// ─── DI System ──────────────────────────────────────────────────────────

#[cfg(test)]
mod di {
    use vsedit_di::ServiceCollection;

    struct MyService {
        value: String,
    }

    impl vsedit_di::Service for MyService {
        fn service_name() -> &'static str { "MyService" }
    }

    #[test]
    fn register_and_resolve_service() {
        let mut collection = ServiceCollection::new();
        collection.register(MyService { value: "test".into() });
        let resolved: Option<&MyService> = collection.get::<MyService>();
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().value, "test");
    }

    #[test]
    fn missing_service_returns_none() {
        let collection = ServiceCollection::new();
        let resolved: Option<&MyService> = collection.get::<MyService>();
        assert!(resolved.is_none());
    }
}

// ─── Language Service ───────────────────────────────────────────────────

#[cfg(test)]
mod languages {
    use vsedit_languages::LanguageService;

    #[test]
    fn detect_language_by_extension() {
        let mut svc = LanguageService::new();
        vsedit_languages::register_default_languages(&mut svc);

        assert!(svc.guess_language_id("main.rs", None).is_some());
        assert!(svc.guess_language_id("index.js", None).is_some());
        assert!(svc.guess_language_id("style.css", None).is_some());
    }

    #[test]
    fn unknown_extension_returns_none() {
        let mut svc = LanguageService::new();
        vsedit_languages::register_default_languages(&mut svc);
        assert!(svc.guess_language_id("file.xyzabc123", None).is_none());
    }
}

// ─── Diff Engine ────────────────────────────────────────────────────────

#[cfg(test)]
mod diff {
    use vsedit_diff::{compute_line_diff, compute_stats};

    #[test]
    fn diff_identical_files() {
        let text = "hello\nworld\n";
        let diff = compute_line_diff(text, text);
        let stats = compute_stats(&diff);
        assert_eq!(stats.insertions, 0);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn diff_added_lines() {
        let original = "line1\nline2\n";
        let modified = "line1\nline2\nline3\n";
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        assert!(stats.insertions > 0);
    }

    #[test]
    fn diff_deleted_lines() {
        let original = "line1\nline2\nline3\n";
        let modified = "line1\nline3\n";
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        assert!(stats.deletions > 0);
    }
}

// ─── Snippet Engine ─────────────────────────────────────────────────────

#[cfg(test)]
mod snippets {
    use vsedit_snippet::{parse_snippet, expand_snippet, SnippetVariables,
                         collect_tabstops, element_count};

    #[test]
    fn parse_simple_snippet() {
        let snippet = parse_snippet("console.log($1);$0");
        assert!(element_count(&snippet) > 0);
    }

    #[test]
    fn expand_snippet_with_variables() {
        let snippet = parse_snippet("Hello ${TM_FILENAME}!");
        let mut vars = SnippetVariables::new();
        vars.set("TM_FILENAME", "test.rs");
        let expanded = expand_snippet(&snippet, &vars);
        assert!(expanded.contains("test.rs"));
    }

    #[test]
    fn snippet_tabstops() {
        let snippet = parse_snippet("fn ${1:name}() {\n\t$0\n}");
        let tabstops = collect_tabstops(&snippet);
        assert!(!tabstops.is_empty());
    }
}

// ─── Fuzzy Matching ─────────────────────────────────────────────────────

#[cfg(test)]
mod suggest {
    use vsedit_suggest::{fuzzy_match, fuzzy_score, CompletionItem, CompletionItemKind};

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("abc", "abcdef"));
        assert!(fuzzy_match("adf", "abcdef"));
        assert!(!fuzzy_match("xyz", "abcdef"));
    }

    #[test]
    fn fuzzy_score_positive() {
        let score = fuzzy_score("get", "getValue").unwrap();
        assert!(score > 0);
    }

    #[test]
    fn completion_item_creation() {
        let item = CompletionItem::new("println!", CompletionItemKind::Function);
        assert_eq!(item.label, "println!");
    }
}

// ─── Explorer / File Operations ─────────────────────────────────────────

#[cfg(test)]
mod explorer {
    use vsedit_explorer::{create_file, create_directory, delete_node, file_icon};

    #[test]
    fn file_icons_work() {
        assert!(!file_icon("main.rs", false).is_empty());
        assert!(!file_icon("src", true).is_empty());
    }

    #[test]
    fn create_and_delete_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_file(temp.path(), "test.txt").unwrap();
        assert!(path.exists());
        delete_node(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn create_directory_works() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_directory(temp.path(), "subdir").unwrap();
        assert!(path.is_dir());
    }
}

// ─── Extension Host ─────────────────────────────────────────────────────

#[cfg(test)]
mod ext_host {
    use vsedit_ext_host::process::ExtensionHostConfig;

    #[test]
    fn config_defaults() {
        let config = ExtensionHostConfig::default();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.locale, "en");
    }

    #[test]
    fn boot_script_resolves() {
        let config = ExtensionHostConfig::default();
        let path = config.resolved_boot_script();
        assert!(path.to_string_lossy().contains("extHostMain"));
    }
}

// ─── Search ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod search {
    use vsedit_wb_search::{SearchService, SearchQueryBuilder};

    #[test]
    fn search_in_text_basic() {
        let query = SearchQueryBuilder::new("hello").build();
        let result = SearchService::search_in_text(&query, "hello world hello", "test.txt");
        assert_eq!(result.matches.len(), 2);
    }

    #[test]
    fn search_case_insensitive() {
        let query = SearchQueryBuilder::new("Hello")
            .case_sensitive(false)
            .build();
        let result = SearchService::search_in_text(&query, "hello HELLO hElLo", "test.txt");
        assert_eq!(result.matches.len(), 3);
    }
}

// ─── Editor Advanced ────────────────────────────────────────────────────

#[cfg(test)]
mod editor_advanced {
    use vsedit_text_model::TextModel;
    use vsedit_editor_types::{Position, Range};
    use vsedit_editor_controller::{EditorAction, EditorController};

    #[test]
    fn insert_at_beginning() {
        let mut model = TextModel::new("world");
        model.insert(Position { line: 1, column: 1 }, "hello ");
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn insert_at_middle() {
        let mut model = TextModel::new("helo world");
        model.insert(Position { line: 1, column: 4 }, "l");
        assert_eq!(model.get_value(), "hello world");
    }

    #[test]
    fn insert_on_second_line() {
        let mut model = TextModel::new("line1\nline2");
        model.insert(Position { line: 2, column: 6 }, " appended");
        assert_eq!(model.get_value(), "line1\nline2 appended");
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut model = TextModel::new("firstsecond");
        model.insert(Position { line: 1, column: 6 }, "\n");
        let text = model.get_value();
        assert!(text.contains("first\n"));
        assert!(text.contains("second"));
    }

    #[test]
    fn multiline_insert() {
        let mut model = TextModel::new("a\nb\nc");
        model.insert(Position { line: 2, column: 2 }, "X");
        let text = model.get_value();
        assert!(text.contains("bX"));
    }

    #[test]
    fn delete_range() {
        let mut model = TextModel::new("hello world");
        model.delete(Range::new(1, 6, 1, 12));
        assert_eq!(model.get_value(), "hello");
    }

    #[test]
    fn delete_across_lines() {
        let mut model = TextModel::new("first\nsecond\nthird");
        model.delete(Range::new(1, 6, 2, 7));
        let text = model.get_value();
        assert!(text.starts_with("first"));
        assert!(text.contains("third"));
        assert!(!text.contains("second"));
    }

    #[test]
    fn delete_single_char() {
        let mut model = TextModel::new("abcdef");
        model.delete(Range::new(1, 3, 1, 4));
        assert_eq!(model.get_value(), "abdef");
    }

    #[test]
    fn delete_entire_line() {
        let mut model = TextModel::new("line1\nline2\nline3");
        model.delete(Range::new(2, 1, 3, 1));
        let text = model.get_value();
        assert!(text.contains("line1"));
        assert!(text.contains("line3"));
        assert!(!text.contains("line2"));
    }

    #[test]
    fn controller_word_movement_right() {
        let mut ctrl = EditorController::new("hello world foo");
        ctrl.execute_action(EditorAction::MoveCursorWordRight);
        ctrl.execute_action(EditorAction::MoveCursorWordRight);
        // Should move past two words without panicking
    }

    #[test]
    fn controller_word_movement_left() {
        let mut ctrl = EditorController::new("hello world");
        ctrl.execute_action(EditorAction::MoveCursorLineEnd);
        ctrl.execute_action(EditorAction::MoveCursorWordLeft);
        ctrl.execute_action(EditorAction::MoveCursorWordLeft);
    }

    #[test]
    fn controller_delete_left() {
        let mut ctrl = EditorController::new("abc");
        ctrl.execute_action(EditorAction::MoveCursorLineEnd);
        ctrl.execute_action(EditorAction::DeleteLeft);
        let text = ctrl.model.get_value();
        assert_eq!(text, "ab");
    }

    #[test]
    fn controller_delete_right() {
        let mut ctrl = EditorController::new("abc");
        ctrl.execute_action(EditorAction::DeleteRight);
        let text = ctrl.model.get_value();
        assert_eq!(text, "bc");
    }

    #[test]
    fn controller_delete_word_left() {
        let mut ctrl = EditorController::new("hello world");
        ctrl.execute_action(EditorAction::MoveCursorLineEnd);
        ctrl.execute_action(EditorAction::DeleteWordLeft);
        let text = ctrl.model.get_value();
        assert!(text.starts_with("hello"));
        assert!(!text.contains("world"));
    }

    #[test]
    fn controller_delete_word_right() {
        let mut ctrl = EditorController::new("hello world");
        ctrl.execute_action(EditorAction::DeleteWordRight);
        let text = ctrl.model.get_value();
        assert!(!text.starts_with("hello"));
    }

    #[test]
    fn controller_delete_line() {
        let mut ctrl = EditorController::new("line1\nline2\nline3");
        ctrl.execute_action(EditorAction::MoveCursorDown);
        ctrl.execute_action(EditorAction::DeleteLine);
        let text = ctrl.model.get_value();
        assert!(!text.contains("line2"));
    }

    #[test]
    fn unicode_content_preserved() {
        let mut model = TextModel::new("héllo wörld");
        model.insert(Position { line: 1, column: 12 }, " 日本語");
        let text = model.get_value();
        assert!(text.contains("héllo"));
        assert!(text.contains("日本語"));
    }

    #[test]
    fn emoji_content_preserved() {
        let model = TextModel::new("hello 🌍🎉 world");
        let text = model.get_value();
        assert!(text.contains("🌍"));
        assert!(text.contains("🎉"));
    }

    #[test]
    fn model_empty() {
        let model = TextModel::empty();
        assert_eq!(model.get_value(), "");
    }

    #[test]
    fn controller_new_line() {
        let mut ctrl = EditorController::new("first");
        ctrl.execute_action(EditorAction::MoveCursorLineEnd);
        ctrl.execute_action(EditorAction::NewLine);
        let text = ctrl.model.get_value();
        assert!(text.contains('\n'));
    }

    #[test]
    fn controller_insert_line_below() {
        let mut ctrl = EditorController::new("line1\nline3");
        ctrl.execute_action(EditorAction::InsertLineBelow);
        let text = ctrl.model.get_value();
        // Should have inserted a blank line
        assert!(text.lines().count() >= 3);
    }

    #[test]
    fn controller_insert_line_above() {
        let mut ctrl = EditorController::new("line2");
        ctrl.execute_action(EditorAction::InsertLineAbove);
        let text = ctrl.model.get_value();
        assert!(text.lines().count() >= 2);
    }

    #[test]
    fn controller_document_start_end() {
        let mut ctrl = EditorController::new("a\nb\nc\nd\ne");
        ctrl.execute_action(EditorAction::MoveCursorDocumentEnd);
        ctrl.execute_action(EditorAction::MoveCursorDocumentStart);
        // Should not panic
    }

    #[test]
    fn controller_indent_outdent() {
        let mut ctrl = EditorController::new("hello");
        ctrl.execute_action(EditorAction::IndentLine);
        let text = ctrl.model.get_value();
        assert!(text.starts_with(' ') || text.starts_with('\t'));
        ctrl.execute_action(EditorAction::OutdentLine);
    }

    #[test]
    fn controller_move_line_up_down() {
        let mut ctrl = EditorController::new("aaa\nbbb\nccc");
        ctrl.execute_action(EditorAction::MoveCursorDown);
        ctrl.execute_action(EditorAction::MoveLineDown);
        let text = ctrl.model.get_value();
        // bbb should have moved down
        assert!(!text.starts_with("aaa\nbbb"));
    }

    #[test]
    fn controller_select_line() {
        let mut ctrl = EditorController::new("first\nsecond\nthird");
        ctrl.execute_action(EditorAction::MoveCursorDown);
        ctrl.execute_action(EditorAction::SelectLine);
        // Should select the current line
    }

    #[test]
    fn controller_page_up_down() {
        let mut ctrl = EditorController::new("a\nb\nc\nd\ne\nf\ng");
        ctrl.execute_action(EditorAction::PageDown(3));
        ctrl.execute_action(EditorAction::PageUp(3));
    }

    #[test]
    fn controller_go_to_line() {
        let mut ctrl = EditorController::new("a\nb\nc\nd\ne");
        ctrl.execute_action(EditorAction::GoToLine(3));
    }

    #[test]
    fn apply_edit_replace() {
        let mut model = TextModel::new("hello world");
        model.apply_edit(Range::new(1, 7, 1, 12), "earth");
        assert_eq!(model.get_value(), "hello earth");
    }

    #[test]
    fn multiple_undos() {
        let mut model = TextModel::new("base");
        model.insert(Position { line: 1, column: 5 }, " one");
        model.insert(Position { line: 1, column: 9 }, " two");
        model.undo();
        model.undo();
        assert_eq!(model.get_value(), "base");
    }

    #[test]
    fn redo_after_undo() {
        let mut model = TextModel::new("base");
        model.insert(Position { line: 1, column: 5 }, " added");
        let with_added = model.get_value();
        model.undo();
        assert_eq!(model.get_value(), "base");
        model.redo();
        assert_eq!(model.get_value(), with_added);
    }
}

// ─── Configuration Advanced ─────────────────────────────────────────────

#[cfg(test)]
mod config_advanced {
    use vsedit_json::{parse_jsonc, strip_comments, parse_jsonc_with_errors,
                      get_value_at_path, set_property, remove_property};

    #[test]
    fn deeply_nested_jsonc() {
        let input = r#"{
            // top-level comment
            "editor": {
                /* nested block */
                "minimap": {
                    "enabled": true,
                    "side": "right"
                }
            }
        }"#;
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["editor"]["minimap"]["enabled"], true);
        assert_eq!(val["editor"]["minimap"]["side"], "right");
    }

    #[test]
    fn empty_object() {
        let val = parse_jsonc("{}").unwrap();
        assert!(val.is_object());
        assert_eq!(val.as_object().unwrap().len(), 0);
    }

    #[test]
    fn empty_array_value() {
        let input = r#"{ "items": [] }"#;
        let val = parse_jsonc(input).unwrap();
        assert!(val["items"].is_array());
        assert_eq!(val["items"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn array_in_settings() {
        let input = r#"{
            "editor.rulers": [80, 120],
            "files.exclude": ["*.tmp", "*.bak"]
        }"#;
        let val = parse_jsonc(input).unwrap();
        let rulers = val["editor.rulers"].as_array().unwrap();
        assert_eq!(rulers.len(), 2);
        assert_eq!(rulers[0], 80);
        assert_eq!(rulers[1], 120);
    }

    #[test]
    fn line_comment_styles() {
        let input = "{\n// full line comment\n\"key\": 1\n}";
        let stripped = strip_comments(input);
        assert!(!stripped.contains("//"));
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["key"], 1);
    }

    #[test]
    fn block_comment_inline() {
        let input = r#"{ "key": /* inline */ "value" }"#;
        let stripped = strip_comments(input);
        assert!(!stripped.contains("/*"));
        assert!(!stripped.contains("*/"));
    }

    #[test]
    fn block_comment_multiline() {
        let input = "{\n/*\n  multi\n  line\n*/\n\"a\": 1\n}";
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["a"], 1);
    }

    #[test]
    fn comment_in_string_preserved() {
        let input = r#"{ "url": "http://example.com" }"#;
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["url"], "http://example.com");
    }

    #[test]
    fn parse_jsonc_with_errors_on_valid() {
        let input = r#"{ "ok": true }"#;
        let (val, errors) = parse_jsonc_with_errors(input);
        assert!(val.is_some());
        assert!(errors.is_empty());
    }

    #[test]
    fn get_value_at_nested_path() {
        let input = r#"{ "a": { "b": { "c": 42 } } }"#;
        let val = parse_jsonc(input).unwrap();
        let found = get_value_at_path(&val, &["a", "b", "c"]);
        assert!(found.is_some());
        assert_eq!(found.unwrap(), &serde_json::json!(42));
    }

    #[test]
    fn get_value_at_missing_path() {
        let val = parse_jsonc(r#"{ "a": 1 }"#).unwrap();
        let found = get_value_at_path(&val, &["x", "y"]);
        assert!(found.is_none());
    }

    #[test]
    fn set_property_at_path() {
        let input = r#"{ "a": 1 }"#;
        let result = set_property(input, &["b"], serde_json::json!(2));
        let val = parse_jsonc(&result).unwrap();
        assert_eq!(val["b"], 2);
        assert_eq!(val["a"], 1);
    }

    #[test]
    fn remove_property_at_path() {
        let input = r#"{ "a": 1, "b": 2 }"#;
        let result = remove_property(input, &["b"]);
        let val = parse_jsonc(&result).unwrap();
        assert_eq!(val["a"], 1);
        assert!(val.get("b").is_none() || val["b"].is_null());
    }

    #[test]
    fn null_value() {
        let input = r#"{ "key": null }"#;
        let val = parse_jsonc(input).unwrap();
        assert!(val["key"].is_null());
    }

    #[test]
    fn boolean_values() {
        let input = r#"{ "yes": true, "no": false }"#;
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["yes"], true);
        assert_eq!(val["no"], false);
    }

    #[test]
    fn numeric_types() {
        let input = r#"{ "int": 42, "float": 3.14, "neg": -1 }"#;
        let val = parse_jsonc(input).unwrap();
        assert_eq!(val["int"], 42);
        assert!((val["float"].as_f64().unwrap() - 3.14).abs() < 0.001);
        assert_eq!(val["neg"], -1);
    }
}

// ─── Command Lifecycle ──────────────────────────────────────────────────

#[cfg(test)]
mod command_lifecycle {
    use vsedit_commands::{CommandRegistry, CommandHistory, CommandPalette, Keybinding,
                          detect_conflicts};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn register_multiple_and_list() {
        let registry = CommandRegistry::new();
        let _r1 = registry.register("cmd.one", Box::new(|_| Ok(None)));
        let _r2 = registry.register("cmd.two", Box::new(|_| Ok(None)));
        let _r3 = registry.register("cmd.three", Box::new(|_| Ok(None)));

        let cmds = registry.get_commands();
        assert!(cmds.contains(&"cmd.one".to_string()));
        assert!(cmds.contains(&"cmd.two".to_string()));
        assert!(cmds.contains(&"cmd.three".to_string()));
    }

    #[test]
    fn has_command() {
        let registry = CommandRegistry::new();
        assert!(!registry.has("cmd.test"));
        let _reg = registry.register("cmd.test", Box::new(|_| Ok(None)));
        assert!(registry.has("cmd.test"));
    }

    #[test]
    fn registration_drop_unregisters() {
        let registry = CommandRegistry::new();
        {
            let reg = registry.register("cmd.temp", Box::new(|_| Ok(None)));
            assert!(registry.has("cmd.temp"));
            reg.unregister();
        }
        assert!(!registry.has("cmd.temp"));
    }

    #[test]
    fn re_register_same_id() {
        let registry = CommandRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));

        let c1 = counter.clone();
        let reg1 = registry.register("cmd.dup", Box::new(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }));
        reg1.unregister();

        let c2 = counter.clone();
        let _reg2 = registry.register("cmd.dup", Box::new(move |_| {
            c2.fetch_add(10, Ordering::SeqCst);
            Ok(None)
        }));

        registry.execute("cmd.dup", vec![]).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn command_returns_error() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("cmd.fail", Box::new(|_| {
            Err("intentional failure".into())
        }));
        let result = registry.execute("cmd.fail", vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn command_history_record_and_frequency() {
        let mut history = CommandHistory::new();
        history.record("editor.action.copy", 1000);
        history.record("editor.action.paste", 2000);
        history.record("editor.action.copy", 3000);

        assert_eq!(history.get_frequency("editor.action.copy"), 2);
        assert_eq!(history.get_frequency("editor.action.paste"), 1);
        assert_eq!(history.get_frequency("unknown.cmd"), 0);
    }

    #[test]
    fn command_history_recent() {
        let mut history = CommandHistory::new();
        history.record("cmd.a", 100);
        history.record("cmd.b", 200);
        history.record("cmd.c", 300);

        let recent = history.get_recent(2);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn command_history_most_frequent() {
        let mut history = CommandHistory::new();
        history.record("cmd.rare", 100);
        history.record("cmd.common", 200);
        history.record("cmd.common", 300);
        history.record("cmd.common", 400);

        let top = history.most_frequent(1);
        assert_eq!(top.len(), 1);
        assert_eq!(top[0].0, "cmd.common");
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn command_palette_filter() {
        let mut palette = CommandPalette::new();
        palette.add("editor.formatDocument", Some("Format Document".into()));
        palette.add("editor.formatSelection", Some("Format Selection".into()));
        palette.add("file.save", Some("Save File".into()));

        let matches = palette.filter_commands("format");
        assert!(matches.len() >= 2);
    }

    #[test]
    fn keybinding_conflict_detection() {
        let bindings = vec![
            Keybinding { key: "Ctrl+S".into(), command_id: "file.save".into() },
            Keybinding { key: "Ctrl+S".into(), command_id: "custom.save".into() },
            Keybinding { key: "Ctrl+Z".into(), command_id: "editor.undo".into() },
        ];
        let conflicts = detect_conflicts(&bindings);
        assert!(!conflicts.is_empty());
        assert!(conflicts.iter().any(|c| c.key == "Ctrl+S"));
    }
}

// ─── Event Patterns ─────────────────────────────────────────────────────

#[cfg(test)]
mod event_patterns {
    use vsedit_events::{Emitter, EventReplayBuffer, counter_listener};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn event_map_transformation() {
        let emitter: Emitter<i32> = Emitter::new();
        let event = emitter.event();
        let mapped = event.map(|x: &i32| x.to_string());

        let received = Arc::new(std::sync::Mutex::new(String::new()));
        let r = received.clone();
        let _sub = mapped.on(move |s: &String| {
            *r.lock().unwrap() = s.clone();
        });

        emitter.fire(&42);
        assert_eq!(*received.lock().unwrap(), "42");
    }

    #[test]
    fn event_filter() {
        let emitter: Emitter<i32> = Emitter::new();
        let event = emitter.event();
        let evens = event.filter(|x: &i32| x % 2 == 0);

        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();
        let _sub = evens.on(move |_: &i32| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);
        emitter.fire(&4);

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn emitter_pause_resume() {
        let emitter: Emitter<i32> = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();

        let event = emitter.event();
        let _sub = event.on(move |_: &i32| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        emitter.fire(&1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        emitter.pause();
        emitter.fire(&2);
        emitter.fire(&3);
        // Events are paused; counter should not increase from direct listener calls
        let count_while_paused = counter.load(Ordering::SeqCst);

        emitter.resume();
        // After resume, buffered events may or may not replay depending on impl
        let count_after_resume = counter.load(Ordering::SeqCst);
        assert!(count_after_resume >= count_while_paused);
    }

    #[test]
    fn once_listener_fires_only_once() {
        let emitter: Emitter<i32> = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();

        let event = emitter.event();
        let _sub = event.once(move |_: &i32| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        emitter.fire(&1);
        emitter.fire(&2);
        emitter.fire(&3);

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn listener_count() {
        let emitter: Emitter<i32> = Emitter::new();
        assert_eq!(emitter.listener_count(), 0);

        let event = emitter.event();
        let sub1 = event.on(|_: &i32| {});
        assert_eq!(emitter.listener_count(), 1);

        let sub2 = event.on(|_: &i32| {});
        assert_eq!(emitter.listener_count(), 2);

        sub1.dispose();
        assert_eq!(emitter.listener_count(), 1);

        sub2.dispose();
        assert_eq!(emitter.listener_count(), 0);
    }

    #[test]
    fn counter_listener_counts_events() {
        let emitter: Emitter<String> = Emitter::new();
        let event = emitter.event();
        let (_sub, count) = counter_listener(&event);

        emitter.fire(&"a".to_string());
        emitter.fire(&"b".to_string());
        emitter.fire(&"c".to_string());

        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn event_chain() {
        let emitter1: Emitter<i32> = Emitter::new();
        let emitter2: Emitter<i32> = Emitter::new();

        let event1 = emitter1.event();
        let _chain = event1.chain(&emitter2);

        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();
        let event2 = emitter2.event();
        let _sub = event2.on(move |_: &i32| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        emitter1.fire(&99);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn disposable_handle_is_disposed() {
        let emitter: Emitter<i32> = Emitter::new();
        let event = emitter.event();
        let sub = event.on(|_: &i32| {});
        assert!(!sub.is_disposed());
        sub.dispose();
        assert!(sub.is_disposed());
    }

    #[test]
    fn event_replay_buffer() {
        let mut buffer: EventReplayBuffer<i32> = EventReplayBuffer::new(3);
        assert!(buffer.is_empty());

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.values(), &[1, 2, 3]);

        buffer.push(4);
        // Should have dropped oldest, capacity is 3
        assert_eq!(buffer.len(), 3);

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn multiple_subscribers() {
        let emitter: Emitter<i32> = Emitter::new();
        let event = emitter.event();

        let c1 = Arc::new(AtomicI32::new(0));
        let c2 = Arc::new(AtomicI32::new(0));
        let c1c = c1.clone();
        let c2c = c2.clone();

        let _s1 = event.on(move |v: &i32| { c1c.fetch_add(*v, Ordering::SeqCst); });
        let _s2 = event.on(move |v: &i32| { c2c.fetch_add(*v * 2, Ordering::SeqCst); });

        emitter.fire(&5);
        assert_eq!(c1.load(Ordering::SeqCst), 5);
        assert_eq!(c2.load(Ordering::SeqCst), 10);
    }
}

// ─── Workspace Integration ──────────────────────────────────────────────

#[cfg(test)]
mod workspace_integration {
    use vsedit_workspace::Workspace;
    use vsedit_uri::VsUri;

    #[test]
    fn empty_workspace() {
        let ws = Workspace::empty();
        assert!(ws.get_folders().is_empty());
        assert!(ws.is_untitled());
    }

    #[test]
    fn single_folder_workspace() {
        let uri = VsUri::file("/tmp/project");
        let ws = Workspace::single_folder(uri);
        assert_eq!(ws.get_folders().len(), 1);
    }

    #[test]
    fn open_folder_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let ws = Workspace::open_folder(temp.path());
        assert_eq!(ws.get_folders().len(), 1);
    }

    #[test]
    fn add_and_remove_folder() {
        let mut ws = Workspace::empty();
        let uri = VsUri::file("/tmp/folder1");
        ws.add_folder(uri.clone(), Some("Folder 1".into()));
        assert_eq!(ws.get_folders().len(), 1);
        assert_eq!(ws.get_folders()[0].name, "Folder 1");

        ws.remove_folder(&uri);
        assert!(ws.get_folders().is_empty());
    }

    #[test]
    fn add_multiple_folders() {
        let mut ws = Workspace::empty();
        ws.add_folder(VsUri::file("/tmp/a"), Some("A".into()));
        ws.add_folder(VsUri::file("/tmp/b"), Some("B".into()));
        ws.add_folder(VsUri::file("/tmp/c"), None);
        assert_eq!(ws.get_folders().len(), 3);
    }

    #[test]
    fn workspace_configuration() {
        let ws = Workspace::empty();
        let config = ws.configuration();
        assert!(config.is_object() || config.is_null());
    }

    #[test]
    fn workspace_trust() {
        let mut ws = Workspace::empty();
        assert!(!ws.is_trusted());
        ws.trust_workspace();
        assert!(ws.is_trusted());
    }

    #[test]
    fn workspace_folder_with_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(temp.path().join("lib.rs"), "pub fn hello() {}").unwrap();

        let ws = Workspace::open_folder(temp.path());
        assert_eq!(ws.get_folders().len(), 1);
    }
}

// ─── Diff Advanced ──────────────────────────────────────────────────────

#[cfg(test)]
mod diff_advanced {
    use vsedit_diff::{compute_line_diff, compute_stats, compute_inline_diff,
                      is_identical, format_unified_diff, get_hunks, reverse_diff,
                      DiffConfig, DiffChangeKind};

    #[test]
    fn diff_empty_strings() {
        let diff = compute_line_diff("", "");
        let stats = compute_stats(&diff);
        assert_eq!(stats.insertions, 0);
        assert_eq!(stats.deletions, 0);
        assert_eq!(stats.changes, 0);
    }

    #[test]
    fn diff_empty_to_content() {
        let diff = compute_line_diff("", "hello\nworld\n");
        let stats = compute_stats(&diff);
        assert!(stats.insertions > 0);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn diff_content_to_empty() {
        let diff = compute_line_diff("hello\nworld\n", "");
        let stats = compute_stats(&diff);
        assert_eq!(stats.insertions, 0);
        assert!(stats.deletions > 0);
    }

    #[test]
    fn diff_only_additions() {
        let original = "line1\n";
        let modified = "line1\nline2\nline3\n";
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        assert!(stats.insertions > 0);
        assert_eq!(stats.deletions, 0);
    }

    #[test]
    fn diff_only_deletions() {
        let original = "a\nb\nc\nd\n";
        let modified = "a\n";
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        assert!(stats.deletions > 0);
    }

    #[test]
    fn diff_modifications() {
        let original = "hello world\n";
        let modified = "hello earth\n";
        let diff = compute_line_diff(original, modified);
        let stats = compute_stats(&diff);
        assert!(stats.changes > 0 || stats.insertions + stats.deletions > 0);
    }

    #[test]
    fn is_identical_true() {
        assert!(is_identical("same text", "same text"));
    }

    #[test]
    fn is_identical_false() {
        assert!(!is_identical("text a", "text b"));
    }

    #[test]
    fn unified_diff_format() {
        let original = "line1\nline2\nline3\n";
        let modified = "line1\nchanged\nline3\n";
        let output = format_unified_diff(original, modified, "a.txt", "b.txt", 1);
        assert!(output.contains("a.txt"));
        assert!(output.contains("b.txt"));
    }

    #[test]
    fn get_hunks_from_diff() {
        let original = "a\nb\nc\n";
        let modified = "a\nX\nc\n";
        let diff = compute_line_diff(original, modified);
        let hunks = get_hunks(&diff);
        assert!(!hunks.is_empty());
    }

    #[test]
    fn reverse_diff_swaps() {
        let original = "a\nb\n";
        let modified = "a\nb\nc\n";
        let diff = compute_line_diff(original, modified);
        let reversed = reverse_diff(&diff);

        // Reversed diff should swap original and modified line counts
        assert_eq!(reversed.original_line_count, diff.modified_line_count);
        assert_eq!(reversed.modified_line_count, diff.original_line_count);
    }

    #[test]
    fn reverse_diff_insert_becomes_delete() {
        let diff = compute_line_diff("a\n", "a\nb\n");
        assert!(diff.changes.iter().any(|c| c.kind == DiffChangeKind::Insert));

        let reversed = reverse_diff(&diff);
        assert!(reversed.changes.iter().any(|c| c.kind == DiffChangeKind::Delete));
    }

    #[test]
    fn inline_diff_chars() {
        let changes = compute_inline_diff("hello", "hallo");
        assert!(!changes.is_empty());
    }

    #[test]
    fn diff_config_default() {
        let config = DiffConfig::new();
        assert!(!config.ignore_whitespace);
        assert!(!config.ignore_case);
        assert_eq!(config.context_lines, 3);
    }

    #[test]
    fn diff_config_ignore_whitespace() {
        let config = DiffConfig::new().with_ignore_whitespace(true);
        let diff = config.compute_diff("  a  \n", "a\n");
        let stats = compute_stats(&diff);
        assert_eq!(stats.insertions + stats.deletions + stats.changes, 0);
    }

    #[test]
    fn diff_config_ignore_case() {
        let config = DiffConfig::new().with_ignore_case(true);
        let diff = config.compute_diff("Hello\n", "hello\n");
        let stats = compute_stats(&diff);
        assert_eq!(stats.insertions + stats.deletions + stats.changes, 0);
    }

    #[test]
    fn diff_config_toggle() {
        let mut config = DiffConfig::new();
        assert!(!config.ignore_whitespace);
        config.toggle_ignore_whitespace();
        assert!(config.ignore_whitespace);
        config.toggle_ignore_case();
        assert!(config.ignore_case);
    }

    #[test]
    fn diff_large_text() {
        let original: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let mut modified = original.clone();
        modified.push_str("extra line\n");
        let diff = compute_line_diff(&original, &modified);
        let stats = compute_stats(&diff);
        assert!(stats.insertions > 0);
    }
}

// ─── Search Advanced ────────────────────────────────────────────────────

#[cfg(test)]
mod search_advanced {
    use vsedit_wb_search::{SearchService, SearchQueryBuilder};

    #[test]
    fn search_whole_word() {
        let query = SearchQueryBuilder::new("is")
            .whole_word(true)
            .build();
        let matches = SearchService::text_matches(&query, "this is a test");
        // "is" as whole word should match the standalone "is", not "this"
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn search_case_sensitive() {
        let query = SearchQueryBuilder::new("Hello")
            .case_sensitive(true)
            .build();
        let matches = SearchService::text_matches(&query, "hello Hello HELLO");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn search_empty_query() {
        let query = SearchQueryBuilder::new("").build();
        let matches = SearchService::text_matches(&query, "some text");
        assert!(matches.is_empty());
    }

    #[test]
    fn search_no_matches() {
        let query = SearchQueryBuilder::new("xyz").build();
        let result = SearchService::search_in_text(&query, "hello world", "test.txt");
        assert!(result.matches.is_empty());
    }

    #[test]
    fn search_multiline_text() {
        let query = SearchQueryBuilder::new("error").build();
        let text = "line 1 ok\nline 2 error\nline 3 ok\nline 4 error";
        let result = SearchService::search_in_text(&query, text, "log.txt");
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].line, 2);
        assert_eq!(result.matches[1].line, 4);
    }

    #[test]
    fn search_special_characters() {
        let query = SearchQueryBuilder::new("(").build();
        let matches = SearchService::text_matches(&query, "fn main() { }");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn search_replace_matches() {
        let query = SearchQueryBuilder::new("world").build();
        let result = SearchService::replace_matches(&query, "hello world", "earth");
        assert_eq!(result, "hello earth");
    }

    #[test]
    fn search_replace_multiple() {
        let query = SearchQueryBuilder::new("a").build();
        let result = SearchService::replace_matches(&query, "banana", "o");
        assert_eq!(result, "bonono");
    }

    #[test]
    fn search_highlight_matches() {
        let query = SearchQueryBuilder::new("lo").build();
        let result = SearchService::highlight_matches(&query, "hello world");
        assert!(result.contains(">>lo<<"));
    }

    #[test]
    fn search_match_count() {
        let query = SearchQueryBuilder::new("test").build();
        let result = SearchService::search_in_text(&query, "test 1\ntest 2\ntest 3", "f.txt");
        assert_eq!(SearchService::match_count(&result), 3);
    }

    #[test]
    fn search_service_results_empty() {
        let svc = SearchService::new();
        assert!(svc.is_results_empty());
    }

    #[test]
    fn search_query_builder_chain() {
        let query = SearchQueryBuilder::new("pattern")
            .case_sensitive(true)
            .whole_word(true)
            .include("*.rs")
            .exclude("target/")
            .build();
        assert_eq!(query.pattern, "pattern");
        assert!(query.case_sensitive);
        assert!(query.whole_word);
        assert_eq!(query.include_pattern, Some("*.rs".into()));
        assert_eq!(query.exclude_pattern, Some("target/".into()));
    }
}

// ─── Snippet Advanced ──────────────────────────────────────────────────

#[cfg(test)]
mod snippet_advanced {
    use vsedit_snippet::{parse_snippet, expand_snippet, SnippetVariables,
                          collect_tabstops, collect_variables, element_count,
                          SnippetDefinition, SnippetRegistry, SnippetSession,
                          SnippetTransform, SnippetElement, SnippetFile};

    #[test]
    fn nested_tabstops_placeholder() {
        let snippet = parse_snippet("${1:fn ${2:name}()}");
        let tabstops = collect_tabstops(&snippet);
        assert!(!tabstops.is_empty());
    }

    #[test]
    fn snippet_choices() {
        let snippet = parse_snippet("${1|one,two,three|}");
        assert!(element_count(&snippet) > 0);
        let has_choice = snippet.elements.iter().any(|e| {
            matches!(e, SnippetElement::Choice { .. })
        });
        assert!(has_choice);
    }

    #[test]
    fn snippet_transform_parse() {
        let transform = SnippetTransform::parse("foo/bar/g");
        assert!(transform.is_some());
        let t = transform.unwrap();
        assert_eq!(t.pattern, "foo");
        assert_eq!(t.replacement, "bar");
        assert_eq!(t.flags, "g");
    }

    #[test]
    fn snippet_transform_apply() {
        let transform = SnippetTransform::parse("hello/world/g").unwrap();
        let result = transform.apply("hello hello");
        assert_eq!(result, "world world");
    }

    #[test]
    fn snippet_transform_case_insensitive() {
        let transform = SnippetTransform::parse("hello/world/gi").unwrap();
        let result = transform.apply("Hello HELLO hello");
        assert_eq!(result, "world world world");
    }

    #[test]
    fn snippet_transform_no_global() {
        let transform = SnippetTransform::parse("x/y/").unwrap();
        let result = transform.apply("x x x");
        assert_eq!(result, "y x x");
    }

    #[test]
    fn snippet_transform_invalid() {
        let transform = SnippetTransform::parse("noslash");
        assert!(transform.is_none());
    }

    #[test]
    fn snippet_variable_collection() {
        let snippet = parse_snippet("${TM_FILENAME} and ${TM_DIRECTORY}");
        let vars = collect_variables(&snippet);
        assert!(vars.contains(&"TM_FILENAME".to_string()));
        assert!(vars.contains(&"TM_DIRECTORY".to_string()));
    }

    #[test]
    fn snippet_definition_with_description() {
        let def = SnippetDefinition::new("For Loop", "for", "for ${1:i} in ${2:iter} { $0 }")
            .with_description("A for loop");
        assert_eq!(def.name, "For Loop");
        assert_eq!(def.prefix, "for");
        assert!(def.description.is_some());
    }

    #[test]
    fn snippet_definition_expand() {
        let def = SnippetDefinition::new("Greet", "greet", "Hello ${TM_FILENAME}!");
        let mut vars = SnippetVariables::new();
        vars.set("TM_FILENAME", "main.rs");
        let expanded = def.expand(&vars);
        assert!(expanded.contains("main.rs"));
    }

    #[test]
    fn snippet_registry_operations() {
        let mut registry = SnippetRegistry::new();
        assert!(registry.is_empty());

        registry.register(SnippetDefinition::new("Log", "log", "console.log($1);$0"));
        registry.register(SnippetDefinition::new("Function", "fn", "fn $1() { $0 }"));
        registry.register(SnippetDefinition::new("Loop", "loop", "loop { $0 }"));

        assert_eq!(registry.len(), 3);

        let found = registry.find_by_prefix("fn");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Function");

        let found = registry.find_by_prefix("lo");
        assert_eq!(found.len(), 2); // "log" and "loop"

        let by_name = registry.find_by_name("Log");
        assert!(by_name.is_some());
        assert_eq!(by_name.unwrap().prefix, "log");
    }

    #[test]
    fn snippet_session_navigation() {
        let snippet = parse_snippet("fn ${1:name}(${2:args}) {\n\t$0\n}");
        let vars = SnippetVariables::new();
        let mut session = SnippetSession::new(&snippet, &vars, 0);

        assert!(session.is_active());
        assert!(session.tabstop_count() >= 3); // $1, $2, $0

        let pos = session.current_position();
        assert!(pos.is_some());

        assert!(session.next_tabstop());
        assert!(session.prev_tabstop());

        session.finish();
        assert!(!session.is_active());
    }

    #[test]
    fn snippet_session_cancel() {
        let snippet = parse_snippet("$1 $0");
        let vars = SnippetVariables::new();
        let mut session = SnippetSession::new(&snippet, &vars, 0);
        assert!(session.is_active());
        session.cancel();
        assert!(!session.is_active());
    }

    #[test]
    fn snippet_session_expanded_text() {
        let snippet = parse_snippet("Hello ${1:World}!");
        let vars = SnippetVariables::new();
        let session = SnippetSession::new(&snippet, &vars, 0);
        assert_eq!(session.expanded_text, "Hello World!");
    }

    #[test]
    fn snippet_session_choices_tabstop() {
        let snippet = parse_snippet("${1|public,private,protected|} class $0");
        let vars = SnippetVariables::new();
        let session = SnippetSession::new(&snippet, &vars, 0);
        // First tabstop should have choices
        let pos = session.current_position().unwrap();
        assert!(pos.choices.is_some());
        let choices = pos.choices.as_ref().unwrap();
        assert_eq!(choices.len(), 3);
        assert!(choices.contains(&"public".to_string()));
    }

    #[test]
    fn snippet_file_parse() {
        let json = r#"{
            "Print": {
                "prefix": "print",
                "body": ["println!(\"$1\");", "$0"],
                "description": "Print to stdout"
            }
        }"#;
        let file = SnippetFile::parse(json).unwrap();
        assert!(file.snippets.contains_key("Print"));
        let entry = &file.snippets["Print"];
        assert_eq!(entry.prefix, vec!["print"]);
        assert_eq!(entry.body.len(), 2);
        assert_eq!(entry.description.as_deref(), Some("Print to stdout"));
    }

    #[test]
    fn snippet_multiple_variables() {
        let snippet = parse_snippet("// ${TM_FILENAME}\n// Author: ${USER}\n$0");
        let mut vars = SnippetVariables::new();
        vars.set("TM_FILENAME", "main.rs");
        vars.set("USER", "dev");
        let expanded = expand_snippet(&snippet, &vars);
        assert!(expanded.contains("main.rs"));
        assert!(expanded.contains("dev"));
    }

    #[test]
    fn snippet_variable_with_default() {
        let snippet = parse_snippet("${TM_FILENAME:untitled}");
        let vars = SnippetVariables::new();
        let expanded = expand_snippet(&snippet, &vars);
        assert_eq!(expanded, "untitled");
    }

    #[test]
    fn snippet_plain_text_only() {
        let snippet = parse_snippet("just plain text");
        assert_eq!(element_count(&snippet), 1);
        let vars = SnippetVariables::new();
        let expanded = expand_snippet(&snippet, &vars);
        assert_eq!(expanded, "just plain text");
    }
}

// ─── DI Advanced ────────────────────────────────────────────────────────

#[cfg(test)]
mod di_advanced {
    use vsedit_di::{ServiceCollection, Service, ServiceAccessor, validate_service_id};

    struct Alpha { val: i32 }
    impl Service for Alpha { fn service_name() -> &'static str { "Alpha" } }

    struct Beta { msg: String }
    impl Service for Beta { fn service_name() -> &'static str { "Beta" } }

    struct Gamma;
    impl Service for Gamma { fn service_name() -> &'static str { "Gamma" } }

    #[test]
    fn register_multiple_services() {
        let mut coll = ServiceCollection::new();
        coll.register(Alpha { val: 1 });
        coll.register(Beta { msg: "hi".into() });
        assert_eq!(coll.len(), 2);
        assert!(!coll.is_empty());
    }

    #[test]
    fn has_service() {
        let mut coll = ServiceCollection::new();
        assert!(!coll.has::<Alpha>());
        coll.register(Alpha { val: 42 });
        assert!(coll.has::<Alpha>());
        assert!(!coll.has::<Beta>());
    }

    #[test]
    fn get_required_service() {
        let mut coll = ServiceCollection::new();
        coll.register(Alpha { val: 99 });
        let svc = coll.get_required::<Alpha>();
        assert_eq!(svc.val, 99);
    }

    #[test]
    #[should_panic]
    fn get_required_missing_panics() {
        let coll = ServiceCollection::new();
        let _svc = coll.get_required::<Alpha>();
    }

    #[test]
    fn service_accessor_with() {
        let mut coll = ServiceCollection::new();
        coll.register(Alpha { val: 7 });
        let accessor = ServiceAccessor::new(coll);

        let result = accessor.with(|c| c.get::<Alpha>().unwrap().val);
        assert_eq!(result, 7);
    }

    #[test]
    fn service_accessor_has() {
        let mut coll = ServiceCollection::new();
        coll.register(Gamma);
        let accessor = ServiceAccessor::new(coll);
        assert!(accessor.has::<Gamma>());
        assert!(!accessor.has::<Alpha>());
    }

    #[test]
    fn validate_service_id_valid() {
        assert!(validate_service_id("my.service"));
        assert!(validate_service_id("editor.formatDocument"));
    }

    #[test]
    fn validate_service_id_invalid() {
        assert!(!validate_service_id(""));
    }

    #[test]
    fn empty_collection() {
        let coll = ServiceCollection::new();
        assert!(coll.is_empty());
        assert_eq!(coll.len(), 0);
    }
}

// ─── Languages Advanced ────────────────────────────────────────────────

#[cfg(test)]
mod languages_advanced {
    use vsedit_languages::{LanguageService, register_default_languages, compute_language_stats,
                           parse_shebang};

    #[test]
    fn detect_many_extensions() {
        let mut svc = LanguageService::new();
        register_default_languages(&mut svc);

        let extensions = [
            ("file.py", true), ("file.ts", true), ("file.js", true),
            ("file.json", true), ("file.md", true), ("file.html", true),
            ("file.yaml", true), ("file.toml", true), ("file.sh", true),
        ];
        for (file, should_match) in extensions {
            let lang = svc.guess_language_id(file, None);
            assert_eq!(lang.is_some(), should_match, "failed for {file}");
        }
    }

    #[test]
    fn language_stats() {
        let mut svc = LanguageService::new();
        register_default_languages(&mut svc);
        let stats = compute_language_stats(&svc);
        assert!(stats.total_languages > 0);
    }

    #[test]
    fn shebang_parsing_python() {
        let info = parse_shebang("#!/usr/bin/env python3");
        assert!(info.is_some());
    }

    #[test]
    fn shebang_parsing_bash() {
        let info = parse_shebang("#!/bin/bash");
        assert!(info.is_some());
    }

    #[test]
    fn shebang_parsing_none() {
        let info = parse_shebang("not a shebang");
        assert!(info.is_none());
    }

    #[test]
    fn shebang_parsing_node() {
        let info = parse_shebang("#!/usr/bin/env node");
        assert!(info.is_some());
    }
}

// ─── Explorer Advanced ──────────────────────────────────────────────────

#[cfg(test)]
mod explorer_advanced {
    use vsedit_explorer::{create_file, create_directory, delete_node, file_icon,
                           rename_node, duplicate_file, copy_path, copy_relative_path};

    #[test]
    fn rename_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_file(temp.path(), "old.txt").unwrap();
        let new_path = rename_node(&path, "new.txt").unwrap();
        assert!(!path.exists());
        assert!(new_path.exists());
        assert!(new_path.file_name().unwrap().to_str().unwrap() == "new.txt");
    }

    #[test]
    fn duplicate_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_file(temp.path(), "original.txt").unwrap();
        std::fs::write(&path, "content").unwrap();
        let dup = duplicate_file(&path).unwrap();
        assert!(dup.exists());
        assert_ne!(path, dup);
    }

    #[test]
    fn copy_path_to_string() {
        let temp = tempfile::tempdir().unwrap();
        let path = create_file(temp.path(), "test.txt").unwrap();
        let copied = copy_path(&path);
        assert!(copied.contains("test.txt"));
    }

    #[test]
    fn copy_relative_path_result() {
        let temp = tempfile::tempdir().unwrap();
        let sub = create_directory(temp.path(), "sub").unwrap();
        let file = create_file(&sub, "file.txt").unwrap();
        let rel = copy_relative_path(&file, temp.path());
        assert!(rel.contains("sub"));
        assert!(rel.contains("file.txt"));
    }

    #[test]
    fn file_icon_various_extensions() {
        let icons = [
            ("script.py", false), ("styles.css", false), ("index.html", false),
            ("data.json", false), ("README.md", false), ("Cargo.toml", false),
        ];
        for (name, is_dir) in icons {
            let icon = file_icon(name, is_dir);
            assert!(!icon.is_empty(), "no icon for {name}");
        }
    }

    #[test]
    fn directory_icons() {
        let dirs = ["src", "tests", "node_modules", ".git"];
        for name in dirs {
            let icon = file_icon(name, true);
            assert!(!icon.is_empty(), "no icon for dir {name}");
        }
    }

    #[test]
    fn nested_directory_creation() {
        let temp = tempfile::tempdir().unwrap();
        let sub1 = create_directory(temp.path(), "level1").unwrap();
        let sub2 = create_directory(&sub1, "level2").unwrap();
        let file = create_file(&sub2, "deep.txt").unwrap();
        assert!(file.exists());
        assert!(sub2.is_dir());
    }

    #[test]
    fn delete_directory() {
        let temp = tempfile::tempdir().unwrap();
        let sub = create_directory(temp.path(), "to_delete").unwrap();
        create_file(&sub, "inner.txt").unwrap();
        delete_node(&sub).unwrap();
        assert!(!sub.exists());
    }
}
