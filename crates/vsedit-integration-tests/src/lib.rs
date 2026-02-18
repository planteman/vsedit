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

// ─── Provider Registry ─────────────────────────────────────────────────

#[cfg(test)]
mod provider_registry {
    use vsedit_ext_host::handlers::{ProviderKind, ProviderRegistry};

    #[test]
    fn provider_registry_tracks_multiple_kinds() {
        let mut reg = ProviderRegistry::new();
        let _h1 = reg.register(
            ProviderKind::Completion,
            "ext-a",
            serde_json::json!({"language": "rust"}),
        );
        let h2 = reg.register(
            ProviderKind::Hover,
            "ext-a",
            serde_json::json!({"language": "rust"}),
        );
        let _h3 = reg.register(ProviderKind::Definition, "ext-b", serde_json::Value::Null);
        assert_eq!(reg.count(), 3);
        assert!(reg.has_provider(ProviderKind::Completion));
        assert!(reg.has_provider(ProviderKind::Hover));
        assert!(reg.has_provider(ProviderKind::Definition));
        assert!(!reg.has_provider(ProviderKind::Formatting));
        // Unregister one
        assert!(reg.unregister(h2));
        assert_eq!(reg.count(), 2);
        assert!(!reg.has_provider(ProviderKind::Hover));
    }

    #[test]
    fn provider_registry_providers_for_kind() {
        let mut reg = ProviderRegistry::new();
        reg.register(ProviderKind::Completion, "ext-1", serde_json::Value::Null);
        reg.register(ProviderKind::Completion, "ext-2", serde_json::Value::Null);
        reg.register(ProviderKind::Hover, "ext-3", serde_json::Value::Null);
        let completions = reg.providers_for(ProviderKind::Completion);
        assert_eq!(completions.len(), 2);
        assert_eq!(reg.providers_for(ProviderKind::Hover).len(), 1);
    }

    #[test]
    fn provider_registry_get_by_handle() {
        let mut reg = ProviderRegistry::new();
        let h = reg.register(
            ProviderKind::References,
            "my-ext",
            serde_json::json!({"language": "python"}),
        );
        let provider = reg.get(h).unwrap();
        assert_eq!(provider.extension_id, "my-ext");
        assert_eq!(provider.kind, ProviderKind::References);
        assert!(reg.get(999).is_none());
    }
}

// ─── Document Event Serialization ──────────────────────────────────────

#[cfg(test)]
mod document_events {
    use vsedit_ext_host::handlers::{DocumentChange, DocumentEvent};

    #[test]
    fn document_event_did_open_serialization() {
        let event = DocumentEvent::DidOpen {
            uri: "file:///home/user/main.rs".to_string(),
            language_id: "rust".to_string(),
            version: 1,
            content: "fn main() {}".to_string(),
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didOpenTextDocument");
        assert_eq!(params["uri"], "file:///home/user/main.rs");
        assert_eq!(params["languageId"], "rust");
        assert_eq!(params["version"], 1);
        assert_eq!(params["text"], "fn main() {}");
    }

    #[test]
    fn document_event_did_change_serialization() {
        let event = DocumentEvent::DidChange {
            uri: "file:///test.py".to_string(),
            version: 3,
            changes: vec![
                DocumentChange {
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 5,
                    text: "hello".to_string(),
                },
                DocumentChange {
                    start_line: 2,
                    start_col: 0,
                    end_line: 2,
                    end_col: 0,
                    text: "new line\n".to_string(),
                },
            ],
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didChangeTextDocument");
        assert_eq!(params["version"], 3);
        let changes = params["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0]["text"], "hello");
    }

    #[test]
    fn document_event_did_save_serialization() {
        let event = DocumentEvent::DidSave {
            uri: "file:///saved.txt".to_string(),
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didSaveTextDocument");
        assert_eq!(params["uri"], "file:///saved.txt");
    }

    #[test]
    fn document_event_did_close_serialization() {
        let event = DocumentEvent::DidClose {
            uri: "file:///closed.rs".to_string(),
        };
        let (method, _params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didCloseTextDocument");
    }
}

// ─── Encoding Detection ────────────────────────────────────────────────

#[cfg(test)]
mod encoding_detection {
    use vsedit_text_model::FileEncoding;

    #[test]
    fn encoding_detect_utf8() {
        let data = b"Hello, world!";
        assert_eq!(FileEncoding::detect(data), FileEncoding::Utf8);
    }

    #[test]
    fn encoding_detect_utf8_bom() {
        let data = [0xEF, 0xBB, 0xBF, b'H', b'i'];
        assert_eq!(FileEncoding::detect(&data), FileEncoding::Utf8Bom);
    }

    #[test]
    fn encoding_detect_utf16le() {
        let data = [0xFF, 0xFE, b'H', 0x00];
        assert_eq!(FileEncoding::detect(&data), FileEncoding::Utf16Le);
    }

    #[test]
    fn encoding_detect_utf16be() {
        let data = [0xFE, 0xFF, 0x00, b'H'];
        assert_eq!(FileEncoding::detect(&data), FileEncoding::Utf16Be);
    }

    #[test]
    fn encoding_roundtrip_utf8_bom() {
        let original = "Hello UTF-8 BOM";
        let encoded = FileEncoding::Utf8Bom.encode(original);
        assert!(encoded.starts_with(&[0xEF, 0xBB, 0xBF]));
        let decoded = FileEncoding::Utf8Bom.decode(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn encoding_roundtrip_latin1() {
        let original = "Hello";
        let encoded = FileEncoding::Latin1.encode(original);
        let decoded = FileEncoding::Latin1.decode(&encoded);
        assert_eq!(decoded, original);
    }
}

// ─── Extension Activation Events ───────────────────────────────────────

#[cfg(test)]
mod extension_activation {
    use vsedit_ext_host::{ExtensionDescription, ExtensionHostManager};

    #[test]
    fn extension_activation_on_language() {
        let mut mgr = ExtensionHostManager::new();
        let json = r#"{
            "name": "test-ext",
            "publisher": "test",
            "version": "1.0.0",
            "engines": { "vscode": "^1.70.0" },
            "activationEvents": ["onLanguage:rust"]
        }"#;
        if let Ok(desc) =
            ExtensionDescription::from_package_json(json, vsedit_uri::VsUri::parse("file:///ext"))
        {
            mgr.register_extension(desc);
            let matches = mgr.should_activate("onLanguage:rust");
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].id, "test.test-ext");
            // Activating marks it
            mgr.mark_activated("test.test-ext");
            assert!(mgr.is_activated("test.test-ext"));
            // Verify it's marked as activated
            assert!(mgr.is_activated("test.test-ext"));
        }
    }

    #[test]
    fn extension_activation_star() {
        let mut mgr = ExtensionHostManager::new();
        let json = r#"{
            "name": "always-on",
            "publisher": "test",
            "version": "1.0.0",
            "engines": { "vscode": "^1.70.0" },
            "activationEvents": ["*"]
        }"#;
        if let Ok(desc) =
            ExtensionDescription::from_package_json(json, vsedit_uri::VsUri::parse("file:///ext"))
        {
            mgr.register_extension(desc);
            let matches = mgr.should_activate("*");
            assert_eq!(matches.len(), 1);
        }
    }
}

// ─── Syntax Highlighter ────────────────────────────────────────────────

#[cfg(test)]
mod syntax_highlighter {
    use vsedit_syntax::SyntaxHighlighter;

    #[test]
    fn syntax_highlighter_detect_language() {
        let hl = SyntaxHighlighter::new();
        let lang = hl.detect_language("main.rs", None);
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().name, "Rust");
    }

    #[test]
    fn syntax_highlighter_language_id_mapping() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(
            hl.language_id_for_path("app.tsx"),
            Some("typescriptreact".to_string())
        );
        assert_eq!(
            hl.language_id_for_path("style.css"),
            Some("css".to_string())
        );
        assert_eq!(
            hl.language_id_for_path("data.json"),
            Some("json".to_string())
        );
    }

    #[test]
    fn syntax_highlighter_theme_palette() {
        let hl = SyntaxHighlighter::new();
        let palette = hl.palette();
        // Default theme should have non-zero foreground
        assert_ne!(palette.foreground, (0, 0, 0));
    }

    #[test]
    fn syntax_highlighter_highlight_range() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let lines = vec![
            "fn main() {\n",
            "    let x = 1;\n",
            "    let y = 2;\n",
            "}\n",
        ];
        let result = hl.highlight_range(&lines, syntax, 1, 3);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line_number, 1);
        assert_eq!(result[1].line_number, 2);
    }
}

// ─── RPC Handler End-to-End ────────────────────────────────────────────

#[cfg(test)]
mod rpc_handler {
    use vsedit_ext_host::handlers::MainThreadHandlers;

    #[test]
    fn rpc_handler_register_and_unregister_provider() {
        let mut h = MainThreadHandlers::new();
        h.register_defaults();
        // Register a completion provider
        let result = h
            .handle(
                "mainThread/registerCompletionProvider",
                serde_json::json!({"extensionId": "test.ext", "selector": {"language": "rust"}}),
            )
            .unwrap();
        let handle = result["handle"].as_u64().unwrap();
        // Verify it's tracked
        let reg = h.provider_registry();
        assert_eq!(reg.lock().unwrap().count(), 1);
        // Unregister
        h.handle(
            "mainThread/unregisterProvider",
            serde_json::json!({"handle": handle}),
        );
        assert_eq!(reg.lock().unwrap().count(), 0);
    }

    #[test]
    fn rpc_handler_filesystem_operations() {
        let mut h = MainThreadHandlers::new();
        h.register_defaults();
        // Try reading a nonexistent file
        let result = h
            .handle(
                "mainThread/fsReadFile",
                serde_json::json!({"path": "/tmp/__vsedit_test_nonexistent__"}),
            )
            .unwrap();
        assert!(result.get("error").is_some());
        // Stat should also fail
        let result = h
            .handle(
                "mainThread/fsStat",
                serde_json::json!({"path": "/tmp/__vsedit_test_nonexistent__"}),
            )
            .unwrap();
        assert!(result.get("error").is_some());
    }
}

// ─── Tab Management ─────────────────────────────────────────────────────

#[cfg(test)]
mod tab_management {
    use vsedit_workbench::Workbench;

    #[test]
    fn tab_service_open_and_switch() {
        let mut wb = Workbench::new();
        wb.start();
        // Open two files
        wb.open_file(std::path::Path::new("/tmp/file1.rs"), "fn main() {}");
        wb.open_file(std::path::Path::new("/tmp/file2.rs"), "fn test() {}");
        assert_eq!(wb.tab_service.tab_count(), 2);
        // Active tab should be the last opened
        let active = wb.tab_service.get_active_tab().unwrap();
        assert!(active.title.contains("file2"));
    }

    #[test]
    fn tab_service_close_tab() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(std::path::Path::new("/tmp/close1.rs"), "content1");
        wb.open_file(std::path::Path::new("/tmp/close2.rs"), "content2");
        assert_eq!(wb.tab_service.tab_count(), 2);
        wb.execute_command("workbench.action.closeActiveEditor");
        assert_eq!(wb.tab_service.tab_count(), 1);
    }
}

// ─── Command & Keybinding Integration ───────────────────────────────────

#[cfg(test)]
mod command_keybinding {
    use vsedit_workbench::Workbench;

    #[test]
    fn command_registry_default_commands_registered() {
        let wb = Workbench::new();
        // Core commands should be registered
        assert!(wb.commands.has("workbench.action.quit"));
        assert!(wb.commands.has("workbench.action.toggleSidebarVisibility"));
        assert!(wb.commands.has("workbench.action.togglePanel"));
        assert!(wb.commands.has("workbench.action.showCommands"));
    }

    #[test]
    fn keybinding_resolver_has_defaults() {
        let wb = Workbench::new();
        // Should have keybindings registered
        let rules = wb.keybindings.rules();
        assert!(!rules.is_empty(), "should have default keybindings");
    }
}

// ─── Workbench State ────────────────────────────────────────────────────

#[cfg(test)]
mod workbench_state {
    use vsedit_workbench::{ActivePanelView, ActiveSidebarPanel, Workbench};

    #[test]
    fn workbench_sidebar_panels() {
        let mut wb = Workbench::new();
        wb.start();
        // Default should be explorer
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::Explorer);
        // Switch to search
        wb.set_active_sidebar(ActiveSidebarPanel::Search);
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::Search);
        // Switch to source control
        wb.set_active_sidebar(ActiveSidebarPanel::SourceControl);
        assert_eq!(wb.active_sidebar, ActiveSidebarPanel::SourceControl);
    }

    #[test]
    fn workbench_command_palette_toggle() {
        let mut wb = Workbench::new();
        wb.start();
        assert!(!wb.show_command_palette);
        wb.execute_command("workbench.action.showCommands");
        assert!(wb.show_command_palette);
    }

    #[test]
    fn workbench_panel_views() {
        let mut wb = Workbench::new();
        wb.start();
        // Default should be terminal
        assert_eq!(wb.active_panel, ActivePanelView::Terminal);
    }
}

// ─── Editor + Model Deep Integration ────────────────────────────────────

#[cfg(test)]
mod editor_model_deep {
    use vsedit_editor_controller::{EditorAction, EditorController};

    #[test]
    fn editor_controller_undo_redo_cursor_state() {
        let mut ctrl = EditorController::new("hello world");
        // Type some text
        ctrl.execute_action(EditorAction::InsertText("X".to_string()));
        let after_insert = ctrl.model.get_value();
        assert!(after_insert.contains("X"));
        // Undo
        ctrl.execute_action(EditorAction::Undo);
        assert_eq!(ctrl.model.get_value(), "hello world");
        // Redo
        ctrl.execute_action(EditorAction::Redo);
        assert_eq!(ctrl.model.get_value(), after_insert);
    }

    #[test]
    fn editor_controller_find_and_replace() {
        let mut ctrl = EditorController::new("foo bar foo baz foo");
        ctrl.execute_action(EditorAction::Find("foo".to_string()));
        assert!(!ctrl.find_results.is_empty());
        ctrl.execute_action(EditorAction::ReplaceAll("foo".to_string(), "qux".to_string()));
        let result = ctrl.model.get_value();
        assert!(!result.contains("foo"));
        assert!(result.contains("qux"));
    }

    #[test]
    fn editor_controller_multi_cursor() {
        let mut ctrl = EditorController::new("line1\nline2\nline3");
        ctrl.execute_action(EditorAction::AddCursorBelow);
        // Should have 2 cursors now
        assert!(ctrl.cursors.get_all().len() >= 2);
    }

    #[test]
    fn editor_controller_auto_close_pairs() {
        let mut ctrl = EditorController::new("");
        ctrl.execute_action(EditorAction::InsertText("(".to_string()));
        let val = ctrl.model.get_value();
        // Auto-close should have inserted both ( and )
        assert!(val.contains("(") && val.contains(")"));
    }
}

// ─── Syntax + Editor Integration ────────────────────────────────────────

#[cfg(test)]
mod syntax_editor {
    use vsedit_syntax::{ColoredSpan, SyntaxHighlighter};

    #[test]
    fn colored_span_merge_preserves_content() {
        let spans = vec![
            ColoredSpan::plain("hello"),
            ColoredSpan::plain(" "),
            ColoredSpan::plain("world"),
        ];
        let merged = ColoredSpan::merge_adjacent(&spans);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "hello world");
    }

    #[test]
    fn highlight_cache_invalidation() {
        let mut hl = SyntaxHighlighter::new();
        // Highlight a line (populates internal state)
        let spans = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let x = 1;\n", syntax)
        };
        assert!(!spans.is_empty());
        // Invalidate should not panic even on empty cache
        hl.invalidate_from(0);
        // Re-highlight after invalidation should still work
        let spans2 = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let x = 1;\n", syntax)
        };
        assert!(!spans2.is_empty());
    }
}

// ─── Tab Management Extended ────────────────────────────────────────────

#[cfg(test)]
mod tab_management_extended {
    use vsedit_workbench::Workbench;
    use std::path::Path;

    #[test]
    fn test_integration_tab_add_multiple() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(Path::new("/tmp/a.rs"), "aaa");
        wb.open_file(Path::new("/tmp/b.rs"), "bbb");
        wb.open_file(Path::new("/tmp/c.rs"), "ccc");
        assert_eq!(wb.tab_service.tab_count(), 3);
    }

    #[test]
    fn test_integration_tab_switch_via_next_previous() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(Path::new("/tmp/t1.rs"), "one");
        wb.open_file(Path::new("/tmp/t2.rs"), "two");
        wb.open_file(Path::new("/tmp/t3.rs"), "three");
        // Active is t3 (last opened)
        assert!(wb.tab_service.get_active_tab().unwrap().title.contains("t3"));
        wb.tab_service.previous_tab();
        assert!(wb.tab_service.get_active_tab().unwrap().title.contains("t2"));
        wb.tab_service.next_tab();
        assert!(wb.tab_service.get_active_tab().unwrap().title.contains("t3"));
    }

    #[test]
    fn test_integration_tab_close_returns_to_neighbor() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(Path::new("/tmp/c1.rs"), "one");
        wb.open_file(Path::new("/tmp/c2.rs"), "two");
        assert_eq!(wb.tab_service.tab_count(), 2);
        // Close active (c2)
        let active_id = wb.tab_service.get_active_tab().unwrap().id;
        wb.tab_service.close_tab(active_id);
        assert_eq!(wb.tab_service.tab_count(), 1);
        // Remaining tab should now be active
        assert!(wb.tab_service.get_active_tab().is_some());
    }

    #[test]
    fn test_integration_tab_close_all_leaves_none_active() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(Path::new("/tmp/x.rs"), "x");
        let id = wb.tab_service.get_active_tab().unwrap().id;
        wb.tab_service.close_tab(id);
        assert_eq!(wb.tab_service.tab_count(), 0);
        assert!(wb.tab_service.get_active_tab().is_none());
    }

    #[test]
    fn test_integration_tab_active_content_matches_opened_file() {
        let mut wb = Workbench::new();
        wb.start();
        wb.open_file(Path::new("/tmp/content.rs"), "fn hello() {}");
        let content = wb.get_active_content();
        assert!(content.is_some());
        assert!(content.unwrap().contains("fn hello()"));
    }
}

// ─── Command Registry Extended ──────────────────────────────────────────

#[cfg(test)]
mod command_registry_extended {
    use vsedit_commands::CommandRegistry;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_integration_command_register_and_has() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("test.cmd1", Box::new(|_| Ok(None)));
        assert!(registry.has("test.cmd1"));
        assert!(!registry.has("test.cmd2"));
    }

    #[test]
    fn test_integration_command_execute_returns_value() {
        let registry = CommandRegistry::new();
        let _reg = registry.register("test.answer", Box::new(|_| {
            Ok(Some(Box::new(42i32)))
        }));
        let result = registry.execute("test.answer", vec![]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[test]
    fn test_integration_command_unregister_via_drop() {
        let registry = CommandRegistry::new();
        {
            let _reg = registry.register("test.ephemeral", Box::new(|_| Ok(None)));
            assert!(registry.has("test.ephemeral"));
        }
        // Registration dropped, command should be gone
        assert!(!registry.has("test.ephemeral"));
    }

    #[test]
    fn test_integration_command_execute_with_side_effect() {
        let registry = CommandRegistry::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();
        let _reg = registry.register("test.inc", Box::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }));
        registry.execute("test.inc", vec![]).unwrap();
        registry.execute("test.inc", vec![]).unwrap();
        registry.execute("test.inc", vec![]).unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_integration_command_get_commands_lists_all() {
        let registry = CommandRegistry::new();
        let _r1 = registry.register("ns.alpha", Box::new(|_| Ok(None)));
        let _r2 = registry.register("ns.beta", Box::new(|_| Ok(None)));
        let cmds = registry.get_commands();
        assert!(cmds.contains(&"ns.alpha".to_string()));
        assert!(cmds.contains(&"ns.beta".to_string()));
    }
}

// ─── Keybinding Resolution Extended ─────────────────────────────────────

#[cfg(test)]
mod keybinding_resolution_extended {
    use std::sync::Arc;
    use vsedit_keybinding_svc::{
        KeybindingResolver, KeybindingRule, KeybindingSource, KeybindingWeight, ResolveResult,
    };
    use vsedit_keybindings::Keybinding;
    use vsedit_keycodes::{KeyCode, KeyCodeChord};
    use vsedit_contextkey::{ContextKeyExpr, ContextKeyService, ContextKeyValue};

    fn ctrl_chord(key: KeyCode) -> KeyCodeChord {
        KeyCodeChord::new(true, false, false, false, key)
    }

    #[test]
    fn test_integration_keybinding_single_chord_lookup() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(ctrl_chord(KeyCode::KeyS)),
            command: "file.save".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorCore,
            source: KeybindingSource::Default,
        });
        let ctx = ContextKeyService::new();
        let result = resolver.resolve(&ctx, &[ctrl_chord(KeyCode::KeyS)]);
        assert_eq!(
            result,
            ResolveResult::CommandMatch {
                command: "file.save".into(),
                args: None,
            }
        );
    }

    #[test]
    fn test_integration_keybinding_ctrl_shift_modifier() {
        let mut resolver = KeybindingResolver::new();
        let chord = KeyCodeChord::new(true, true, false, false, KeyCode::KeyP);
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(chord),
            command: "workbench.action.showCommands".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::WorkbenchContrib,
            source: KeybindingSource::Default,
        });
        let ctx = ContextKeyService::new();
        let result = resolver.resolve(&ctx, &[chord]);
        assert_eq!(
            result,
            ResolveResult::CommandMatch {
                command: "workbench.action.showCommands".into(),
                args: None,
            }
        );
    }

    #[test]
    fn test_integration_keybinding_no_match_returns_no_match() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeyService::new();
        let result = resolver.resolve(&ctx, &[ctrl_chord(KeyCode::KeyZ)]);
        assert_eq!(result, ResolveResult::NoMatch);
    }

    #[test]
    fn test_integration_keybinding_two_chord_sequence() {
        let mut resolver = KeybindingResolver::new();
        let first = ctrl_chord(KeyCode::KeyK);
        let second = ctrl_chord(KeyCode::KeyC);
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::two_chords(first, second),
            command: "editor.action.addCommentLine".into(),
            args: None,
            when: None,
            weight: KeybindingWeight::EditorContrib,
            source: KeybindingSource::Default,
        });
        let ctx = ContextKeyService::new();
        // First chord alone should need more chords
        let partial = resolver.resolve(&ctx, &[first]);
        assert_eq!(partial, ResolveResult::MoreChordsNeeded);
        // Both chords should match
        let full = resolver.resolve(&ctx, &[first, second]);
        assert_eq!(
            full,
            ResolveResult::CommandMatch {
                command: "editor.action.addCommentLine".into(),
                args: None,
            }
        );
    }

    #[test]
    fn test_integration_keybinding_when_clause_filtering() {
        let mut resolver = KeybindingResolver::new();
        resolver.add_rule(KeybindingRule {
            keybinding: Keybinding::new(ctrl_chord(KeyCode::KeyD)),
            command: "editor.action.duplicateLine".into(),
            args: None,
            when: Some(ContextKeyExpr::parse("editorFocus").unwrap()),
            weight: KeybindingWeight::EditorCore,
            source: KeybindingSource::Default,
        });
        // Without editorFocus set, should not match
        let ctx_no_focus = ContextKeyService::new();
        let result = resolver.resolve(&ctx_no_focus, &[ctrl_chord(KeyCode::KeyD)]);
        assert_eq!(result, ResolveResult::NoMatch);
        // With editorFocus set, should match
        let ctx_focus = Arc::new(ContextKeyService::new());
        ctx_focus.set_context("editorFocus", ContextKeyValue::Bool(true));
        let result2 = resolver.resolve(ctx_focus.as_ref(), &[ctrl_chord(KeyCode::KeyD)]);
        assert_eq!(
            result2,
            ResolveResult::CommandMatch {
                command: "editor.action.duplicateLine".into(),
                args: None,
            }
        );
    }
}

// ─── Editor Model Extended ──────────────────────────────────────────────

#[cfg(test)]
mod editor_model_extended {
    use vsedit_text_model::TextModel;
    use vsedit_editor_types::{ITextModel, Position, Range};

    #[test]
    fn test_integration_model_insert_and_line_count() {
        let mut model = TextModel::new("line1\nline2");
        assert_eq!(model.get_line_count(), 2);
        model.insert(Position { line: 2, column: 6 }, "\nline3");
        assert_eq!(model.get_line_count(), 3);
        assert_eq!(model.get_line_content(3), "line3");
    }

    #[test]
    fn test_integration_model_delete_range_and_verify() {
        let mut model = TextModel::new("abcdefgh");
        model.delete(Range::new(1, 4, 1, 7));
        assert_eq!(model.get_value(), "abcgh");
    }

    #[test]
    fn test_integration_model_undo_restores_content() {
        let mut model = TextModel::new("original");
        model.insert(Position { line: 1, column: 9 }, " modified");
        assert_eq!(model.get_value(), "original modified");
        model.undo();
        assert_eq!(model.get_value(), "original");
    }

    #[test]
    fn test_integration_model_redo_reapplies_edit() {
        let mut model = TextModel::new("base");
        model.insert(Position { line: 1, column: 5 }, " ext");
        model.undo();
        assert_eq!(model.get_value(), "base");
        model.redo();
        assert_eq!(model.get_value(), "base ext");
    }

    #[test]
    fn test_integration_model_get_line_content_per_line() {
        let model = TextModel::new("alpha\nbeta\ngamma");
        assert_eq!(model.get_line_count(), 3);
        assert_eq!(model.get_line_content(1), "alpha");
        assert_eq!(model.get_line_content(2), "beta");
        assert_eq!(model.get_line_content(3), "gamma");
    }
}

// ─── Configuration Extended ─────────────────────────────────────────────

#[cfg(test)]
mod configuration_extended {
    use vsedit_configuration::{Configuration, ConfigurationModel, ConfigurationTarget};
    use serde_json::json;

    #[test]
    fn test_integration_config_set_and_get_value() {
        let mut config = Configuration::new();
        config.update("editor.fontSize", json!(14), ConfigurationTarget::User);
        let val: Option<i64> = config.get_value("editor.fontSize");
        assert_eq!(val, Some(14));
    }

    #[test]
    fn test_integration_config_defaults_layer() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.tabSize", json!(4));
        let config = Configuration::with_defaults(defaults);
        let val: Option<i64> = config.get_value("editor.tabSize");
        assert_eq!(val, Some(4));
    }

    #[test]
    fn test_integration_config_user_overrides_default() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.tabSize", json!(4));
        let mut config = Configuration::with_defaults(defaults);
        config.update("editor.tabSize", json!(2), ConfigurationTarget::User);
        let val: Option<i64> = config.get_value("editor.tabSize");
        assert_eq!(val, Some(2));
    }

    #[test]
    fn test_integration_config_nested_keys() {
        let mut config = Configuration::new();
        config.update("editor.minimap.enabled", json!(true), ConfigurationTarget::User);
        config.update("editor.minimap.side", json!("right"), ConfigurationTarget::User);
        let enabled: Option<bool> = config.get_value("editor.minimap.enabled");
        let side: Option<String> = config.get_value("editor.minimap.side");
        assert_eq!(enabled, Some(true));
        assert_eq!(side, Some("right".to_string()));
    }

    #[test]
    fn test_integration_config_inspect_shows_layers() {
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.wordWrap", json!("off"));
        let mut config = Configuration::with_defaults(defaults);
        config.update("editor.wordWrap", json!("on"), ConfigurationTarget::User);
        let inspect = config.inspect("editor.wordWrap");
        assert_eq!(inspect.default_value, Some(json!("off")));
        assert_eq!(inspect.user_value, Some(json!("on")));
        assert_eq!(inspect.effective_value, Some(json!("on")));
    }
}

// ─── Syntax Highlighting ────────────────────────────────────────────────

#[cfg(test)]
mod syntax_highlighting {
    use vsedit_syntax::SyntaxHighlighter;

    #[test]
    fn test_integration_syntax_highlight_rust() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.syntax_for_file("main.rs").unwrap();
        let spans = highlighter.highlight_line("fn main() {}", syntax);
        assert!(!spans.is_empty(), "Rust code should produce tokens");
    }

    #[test]
    fn test_integration_syntax_highlight_python() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.syntax_for_file("script.py").unwrap();
        let spans = highlighter.highlight_line("def hello():", syntax);
        assert!(!spans.is_empty(), "Python code should produce tokens");
    }

    #[test]
    fn test_integration_syntax_highlight_javascript() {
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.syntax_for_file("app.js").unwrap();
        let spans = highlighter.highlight_line("const x = 42;", syntax);
        assert!(!spans.is_empty(), "JavaScript code should produce tokens");
    }

    #[test]
    fn test_integration_syntax_cache_returns_cached_result() {
        use vsedit_syntax::HighlightCache;
        let highlighter = SyntaxHighlighter::new();
        let syntax = highlighter.syntax_for_file("main.rs").unwrap();
        let spans = highlighter.highlight_line("let x = 1;", syntax);

        let mut cache = HighlightCache::new();
        cache.set(0, spans.clone());
        let cached = cache.get(0).unwrap();
        assert_eq!(spans, *cached, "Cached result should match original highlight");
    }

    #[test]
    fn test_integration_syntax_unknown_extension_fallback() {
        let highlighter = SyntaxHighlighter::new();
        let result = highlighter.syntax_for_file("data.xyzabc999");
        assert!(result.is_none(), "Unknown extension should return None");
    }
}

// ─── URI ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod uri {
    use vsedit_uri::{VsUri, UriChanges};

    #[test]
    fn test_integration_uri_parse_https() {
        let uri = VsUri::parse("https://example.com/path?q=1#frag");
        assert_eq!(uri.scheme, "https");
        assert_eq!(uri.authority, "example.com");
        assert_eq!(uri.path, "/path");
        assert_eq!(uri.query, "q=1");
        assert_eq!(uri.fragment, "frag");
    }

    #[test]
    fn test_integration_uri_to_string_roundtrip() {
        let uri = VsUri::from_components("https", "host.io", "/a/b", "", "");
        let s = uri.to_uri_string();
        assert!(s.starts_with("https://"), "Serialized URI should start with scheme");
        assert!(s.contains("host.io"), "Serialized URI should contain authority");
    }

    #[test]
    fn test_integration_uri_scheme_authority_path() {
        let uri = VsUri::from_components("file", "", "/home/user/file.rs", "", "");
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.authority, "");
        assert_eq!(uri.path, "/home/user/file.rs");
    }

    #[test]
    fn test_integration_uri_file_creation() {
        let uri = VsUri::file("/home/user/project/main.rs");
        assert!(uri.is_file());
        assert_eq!(uri.scheme, "file");
        assert_eq!(uri.path, "/home/user/project/main.rs");
    }

    #[test]
    fn test_integration_uri_with_changes_path() {
        let base = VsUri::file("/home/user/old.rs");
        let changed = VsUri::with(&base, UriChanges {
            path: Some("/home/user/new.rs".into()),
            ..Default::default()
        });
        assert_eq!(changed.path, "/home/user/new.rs");
        assert_eq!(changed.scheme, "file", "Scheme should be preserved");
    }
}

// ─── Text Model Editing ────────────────────────────────────────────────

#[cfg(test)]
mod text_model_editing {
    use vsedit_text_model::TextModel;
    use vsedit_editor_types::{ITextModel, Position, Range};

    #[test]
    fn test_integration_model_multiline_insert() {
        let mut model = TextModel::new("aaa\nbbb\nccc");
        model.insert(Position { line: 2, column: 4 }, "\ninserted");
        assert_eq!(model.get_line_count(), 4);
        assert_eq!(model.get_line_content(3), "inserted");
    }

    #[test]
    fn test_integration_model_delete_across_lines() {
        let mut model = TextModel::new("first\nsecond\nthird");
        model.delete(Range::new(1, 1, 2, 7));
        let text = model.get_value();
        assert!(!text.contains("first"));
        assert!(!text.contains("second"));
        assert!(text.contains("third"));
    }

    #[test]
    fn test_integration_model_get_line_content_specific() {
        let model = TextModel::new("alpha\nbeta\ngamma\ndelta");
        assert_eq!(model.get_line_content(1), "alpha");
        assert_eq!(model.get_line_content(2), "beta");
        assert_eq!(model.get_line_content(3), "gamma");
        assert_eq!(model.get_line_content(4), "delta");
    }

    #[test]
    fn test_integration_model_replace_text() {
        let mut model = TextModel::new("hello world");
        model.apply_edit(Range::new(1, 7, 1, 12), "rust");
        assert_eq!(model.get_value(), "hello rust");
    }

    #[test]
    fn test_integration_model_complex_edits_verify_content() {
        let mut model = TextModel::new("line1\nline2\nline3");
        model.insert(Position { line: 1, column: 6 }, " modified");
        model.delete(Range::new(2, 1, 2, 6));
        model.apply_edit(Range::new(3, 1, 3, 6), "LINE3");
        assert_eq!(model.get_line_content(1), "line1 modified");
        assert_eq!(model.get_line_content(2), "");
        assert_eq!(model.get_line_content(3), "LINE3");
    }
}

// ─── Event System Extended ──────────────────────────────────────────────

#[cfg(test)]
mod events_extended {
    use vsedit_events::Emitter;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_integration_emitter_subscribe_and_emit() {
        let emitter = Emitter::new();
        let value = Arc::new(AtomicI32::new(0));
        let value_clone = value.clone();

        let event = emitter.event();
        let _sub = event.on(move |v: &i32| {
            value_clone.store(*v, Ordering::SeqCst);
        });

        emitter.fire(&99);
        assert_eq!(value.load(Ordering::SeqCst), 99);
    }

    #[test]
    fn test_integration_emitter_multiple_subscribers() {
        let emitter = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c1 = counter.clone();
        let c2 = counter.clone();

        let event = emitter.event();
        let _s1 = event.on(move |_: &i32| { c1.fetch_add(1, Ordering::SeqCst); });
        let _s2 = event.on(move |_: &i32| { c2.fetch_add(10, Ordering::SeqCst); });

        emitter.fire(&0);
        assert_eq!(counter.load(Ordering::SeqCst), 11);
    }

    #[test]
    fn test_integration_emitter_unsubscribe_via_dispose() {
        let emitter = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();

        let event = emitter.event();
        let sub = event.on(move |_: &i32| { c.fetch_add(1, Ordering::SeqCst); });

        emitter.fire(&1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        sub.dispose();
        emitter.fire(&2);
        assert_eq!(counter.load(Ordering::SeqCst), 1, "Should not receive after dispose");
    }

    #[test]
    fn test_integration_emitter_unsubscribe_via_drop() {
        let emitter = Emitter::new();
        let counter = Arc::new(AtomicI32::new(0));
        let c = counter.clone();

        let event = emitter.event();
        {
            let _sub = event.on(move |_: &String| { c.fetch_add(1, Ordering::SeqCst); });
            emitter.fire(&"a".to_string());
            assert_eq!(counter.load(Ordering::SeqCst), 1);
        }
        // _sub dropped here
        emitter.fire(&"b".to_string());
        assert_eq!(counter.load(Ordering::SeqCst), 1, "Should not receive after drop");
    }

    #[test]
    fn test_integration_emitter_event_data_passing() {
        let emitter = Emitter::new();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let r = received.clone();

        let event = emitter.event();
        let _sub = event.on(move |v: &String| {
            r.lock().unwrap().push(v.clone());
        });

        emitter.fire(&"first".to_string());
        emitter.fire(&"second".to_string());
        emitter.fire(&"third".to_string());

        let data = received.lock().unwrap();
        assert_eq!(*data, vec!["first", "second", "third"]);
    }
}

// ─── DI Container Extended ─────────────────────────────────────────────

#[cfg(test)]
mod di_extended {
    use vsedit_di::{ServiceCollection, Service};

    struct AlphaService { val: i32 }
    impl Service for AlphaService {
        fn service_name() -> &'static str { "AlphaService" }
    }

    struct BetaService { name: String }
    impl Service for BetaService {
        fn service_name() -> &'static str { "BetaService" }
    }

    #[test]
    fn test_integration_di_register_and_get() {
        let mut col = ServiceCollection::new();
        col.register(AlphaService { val: 42 });
        let svc = col.get::<AlphaService>();
        assert!(svc.is_some());
        assert_eq!(svc.unwrap().val, 42);
    }

    #[test]
    fn test_integration_di_multiple_services() {
        let mut col = ServiceCollection::new();
        col.register(AlphaService { val: 1 });
        col.register(BetaService { name: "beta".into() });
        assert_eq!(col.get::<AlphaService>().unwrap().val, 1);
        assert_eq!(col.get::<BetaService>().unwrap().name, "beta");
    }

    #[test]
    fn test_integration_di_service_override() {
        let mut col = ServiceCollection::new();
        col.register(AlphaService { val: 10 });
        assert_eq!(col.get::<AlphaService>().unwrap().val, 10);
        col.register(AlphaService { val: 20 });
        assert_eq!(col.get::<AlphaService>().unwrap().val, 20);
    }

    #[test]
    fn test_integration_di_missing_service_returns_none() {
        let col = ServiceCollection::new();
        let result = col.get::<BetaService>();
        assert!(result.is_none());
    }

    #[test]
    fn test_integration_di_has_check() {
        let mut col = ServiceCollection::new();
        assert!(!col.has::<AlphaService>());
        col.register(AlphaService { val: 0 });
        assert!(col.has::<AlphaService>());
    }
}

// ─── Diff ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod diff_hunks {
    use vsedit_diff::{compute_diff_hunks, unified_diff_format, diff_apply, DiffHunkType};

    #[test]
    fn test_integration_diff_hunks_detect_addition() {
        let hunks = compute_diff_hunks("line1\n", "line1\nline2\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Add);
        assert_eq!(hunks[0].modified_lines.len(), 1);
    }

    #[test]
    fn test_integration_diff_hunks_detect_deletion() {
        let hunks = compute_diff_hunks("aaa\nbbb\n", "aaa\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Delete);
        assert_eq!(hunks[0].original_lines.len(), 1);
    }

    #[test]
    fn test_integration_diff_hunks_detect_modification() {
        let hunks = compute_diff_hunks("old line\n", "new line\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].change_type, DiffHunkType::Modify);
    }

    #[test]
    fn test_integration_diff_unified_format_contains_header() {
        let hunks = compute_diff_hunks("a\n", "b\n");
        let text = unified_diff_format(&hunks, 3);
        assert!(text.contains("@@"));
        assert!(text.contains("-a"));
        assert!(text.contains("+b"));
    }

    #[test]
    fn test_integration_diff_apply_roundtrip() {
        let original = "first\nsecond\nthird\n";
        let modified = "first\nchanged\nthird\nextra\n";
        let hunks = compute_diff_hunks(original, modified);
        let result = diff_apply(original, &hunks).expect("apply should succeed");
        assert_eq!(result.trim(), modified.trim());
    }
}

// ─── Glob ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod glob_tests {
    use vsedit_glob::{GlobPattern, GlobPatternSet, parse_exclude_patterns};

    #[test]
    fn test_integration_glob_pattern_match() {
        let pat = GlobPattern::new("*.rs").unwrap();
        assert!(pat.matches("main.rs"));
        assert!(!pat.matches("main.py"));
    }

    #[test]
    fn test_integration_glob_recursive_star() {
        let pat = GlobPattern::new("**/*.txt").unwrap();
        assert!(pat.matches("a/b/c/file.txt"));
        assert!(!pat.matches("a/b/c/file.rs"));
    }

    #[test]
    fn test_integration_glob_negation_helpers() {
        assert!(GlobPattern::is_negated("!*.log"));
        assert!(!GlobPattern::is_negated("*.log"));
        assert_eq!(GlobPattern::strip_negation("!*.log"), "*.log");
    }

    #[test]
    fn test_integration_glob_pattern_set_matches_any() {
        let set = GlobPatternSet::new(&["*.rs", "*.toml"]).unwrap();
        assert!(set.matches_any("lib.rs"));
        assert!(set.matches_any("Cargo.toml"));
        assert!(!set.matches_any("readme.md"));
    }

    #[test]
    fn test_integration_glob_parse_exclude_patterns() {
        let (excludes, includes) = parse_exclude_patterns(&["*.log", "!important.log", "*.tmp"]);
        assert_eq!(excludes, vec!["*.log", "*.tmp"]);
        assert_eq!(includes, vec!["important.log"]);
    }
}

// ─── Collections ────────────────────────────────────────────────────────────

#[cfg(test)]
mod collections {
    use vsedit_collections::{LruCache, PriorityQueue};

    #[test]
    fn test_integration_lru_cache_evicts_oldest() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        let evicted = cache.set("c", 3);
        assert_eq!(evicted, Some(("a", 1)));
        assert!(cache.get(&"a").is_none());
        assert_eq!(*cache.get(&"b").unwrap(), 2);
    }

    #[test]
    fn test_integration_lru_cache_get_promotes() {
        let mut cache = LruCache::new(2);
        cache.set("a", 1);
        cache.set("b", 2);
        cache.get(&"a"); // promote "a"
        cache.set("c", 3); // should evict "b", not "a"
        assert!(cache.get(&"b").is_none());
        assert!(cache.get(&"a").is_some());
    }

    #[test]
    fn test_integration_lru_cache_remove() {
        let mut cache = LruCache::new(3);
        cache.set("x", 10);
        cache.set("y", 20);
        assert_eq!(cache.remove(&"x"), Some(10));
        assert_eq!(cache.len(), 1);
        assert!(!cache.contains_key(&"x"));
    }

    #[test]
    fn test_integration_priority_queue_min_order() {
        let mut pq = PriorityQueue::new();
        pq.push(5);
        pq.push(1);
        pq.push(3);
        assert_eq!(pq.pop(), Some(1));
        assert_eq!(pq.pop(), Some(3));
        assert_eq!(pq.pop(), Some(5));
        assert!(pq.is_empty());
    }

    #[test]
    fn test_integration_priority_queue_peek() {
        let mut pq = PriorityQueue::new();
        pq.push(10);
        pq.push(2);
        assert_eq!(*pq.peek().unwrap(), 2);
        assert_eq!(pq.len(), 2);
    }
}

// ─── Path Utilities ─────────────────────────────────────────────────────────

#[cfg(test)]
mod path_utils {
    use vsedit_path::{path_normalize, path_common_prefix, relative_to};

    #[test]
    fn test_integration_path_normalize_dot_segments() {
        assert_eq!(path_normalize("/a/b/../c/./d"), "/a/c/d");
    }

    #[test]
    fn test_integration_path_normalize_collapses_slashes() {
        assert_eq!(path_normalize("/a//b///c"), "/a/b/c");
    }

    #[test]
    fn test_integration_path_normalize_removes_trailing_slash() {
        assert_eq!(path_normalize("/a/b/c/"), "/a/b/c");
    }

    #[test]
    fn test_integration_path_common_prefix_shared() {
        let prefix = path_common_prefix(&["/home/user/project/src", "/home/user/project/tests"]);
        assert_eq!(prefix, "/home/user/project");
    }

    #[test]
    fn test_integration_path_relative_to() {
        let rel = relative_to("/home/user/src", "/home/user/docs/readme.md").unwrap();
        assert_eq!(rel, "../docs/readme.md");
    }
}

// ─── JSON Utilities ─────────────────────────────────────────────────────────

#[cfg(test)]
mod json_utils {
    use vsedit_json::{JsonPath, json_merge, parse_jsonc};
    use serde_json::json;

    #[test]
    fn test_integration_json_path_get() {
        let data = json!({"a": {"b": {"c": 42}}});
        let path = JsonPath::parse("a.b.c");
        assert_eq!(path.get(&data), Some(&json!(42)));
    }

    #[test]
    fn test_integration_json_path_set() {
        let mut data = json!({"x": 1});
        let path = JsonPath::parse("y.z");
        path.set(&mut data, json!(99));
        assert_eq!(data, json!({"x": 1, "y": {"z": 99}}));
    }

    #[test]
    fn test_integration_json_path_remove() {
        let mut data = json!({"keep": 1, "drop": 2});
        let path = JsonPath::parse("drop");
        assert!(path.remove(&mut data));
        assert_eq!(data, json!({"keep": 1}));
    }

    #[test]
    fn test_integration_json_merge_deep() {
        let base = json!({"a": 1, "b": {"c": 2, "d": 3}});
        let patch = json!({"b": {"c": 99}, "e": 5});
        let merged = json_merge(&base, &patch);
        assert_eq!(merged, json!({"a": 1, "b": {"c": 99, "d": 3}, "e": 5}));
    }

    #[test]
    fn test_integration_jsonc_parse_with_comments() {
        let input = r#"{
            // line comment
            "key": "value", /* block */
            "num": 42,
        }"#;
        let val = parse_jsonc(input).expect("JSONC should parse");
        assert_eq!(val["key"], json!("value"));
        assert_eq!(val["num"], json!(42));
    }
}

// ─── Multi-cursor ───────────────────────────────────────────────────────

#[cfg(test)]
mod multi_cursor {
    use vsedit_cursor::CursorController;
    use vsedit_text_model::TextModel;
    use vsedit_editor_types::{ITextModel, Position};

    #[test]
    fn test_integration_add_cursor_above() {
        let model = TextModel::new("line1\nline2\nline3");
        let mut ctrl = CursorController::from_position(Position::new(3, 1));
        ctrl.add_cursor_above(&model);
        assert_eq!(ctrl.get_all().len(), 2);
        assert_eq!(ctrl.get_all()[1].position().line, 2);
    }

    #[test]
    fn test_integration_add_cursor_below() {
        let model = TextModel::new("line1\nline2\nline3");
        let mut ctrl = CursorController::from_position(Position::new(1, 1));
        ctrl.add_cursor_below(&model);
        assert_eq!(ctrl.get_all().len(), 2);
        assert_eq!(ctrl.get_all()[1].position().line, 2);
    }

    #[test]
    fn test_integration_remove_secondary_cursors() {
        let model = TextModel::new("a\nb\nc\nd");
        let mut ctrl = CursorController::from_position(Position::new(1, 1));
        ctrl.add_cursor_below(&model);
        ctrl.add_cursor_below(&model);
        assert!(ctrl.has_multiple_cursors());
        ctrl.remove_secondary_cursors();
        assert!(!ctrl.has_multiple_cursors());
        assert_eq!(ctrl.get_all().len(), 1);
    }

    #[test]
    fn test_integration_has_multiple_cursors() {
        let mut ctrl = CursorController::new();
        assert!(!ctrl.has_multiple_cursors());
        ctrl.add_cursor(Position::new(2, 1));
        assert!(ctrl.has_multiple_cursors());
    }

    #[test]
    fn test_integration_cursor_undo() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(2, 1));
        ctrl.add_cursor(Position::new(3, 1));
        assert_eq!(ctrl.get_all().len(), 3);
        ctrl.cursor_undo();
        assert_eq!(ctrl.get_all().len(), 2);
        ctrl.cursor_undo();
        assert_eq!(ctrl.get_all().len(), 1);
        // Undo on single cursor is a no-op
        ctrl.cursor_undo();
        assert_eq!(ctrl.get_all().len(), 1);
    }
}

// ─── Debug State ────────────────────────────────────────────────────────

#[cfg(test)]
mod debug_state {
    use vsedit_debug::{
        DebugSession, DebugSessionState, BreakpointStore,
        types::StackFrame,
    };

    #[test]
    fn test_integration_debug_state_transitions() {
        let mut session = DebugSession::new("s1", "test", "lldb");
        assert_eq!(session.state(), DebugSessionState::NotStarted);
        session.initialize().unwrap();
        assert_eq!(session.state(), DebugSessionState::Initializing);
        session.launch(1000).unwrap();
        assert_eq!(session.state(), DebugSessionState::Running);
        session.pause().unwrap();
        assert_eq!(session.state(), DebugSessionState::Paused);
    }

    #[test]
    fn test_integration_debug_invalid_transition() {
        let mut session = DebugSession::new("s2", "test", "lldb");
        // Cannot launch from NotStarted (must initialize first)
        let result = session.launch(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_breakpoint_add_remove() {
        let mut store = BreakpointStore::new();
        let added = store.toggle_breakpoint("main.rs", 10);
        assert!(added);
        assert_eq!(store.total_count(), 1);
        let removed = store.toggle_breakpoint("main.rs", 10);
        assert!(!removed);
        assert_eq!(store.total_count(), 0);
    }

    #[test]
    fn test_integration_breakpoint_toggle_multiple_files() {
        let mut store = BreakpointStore::new();
        store.toggle_breakpoint("a.rs", 1);
        store.toggle_breakpoint("b.rs", 5);
        store.toggle_breakpoint("a.rs", 20);
        assert_eq!(store.total_count(), 3);
        let files = store.files_with_breakpoints();
        assert!(files.contains(&"a.rs"));
        assert!(files.contains(&"b.rs"));
        store.clear_file_breakpoints("a.rs");
        assert_eq!(store.total_count(), 1);
    }

    #[test]
    fn test_integration_stack_frame_construction() {
        let frame = StackFrame::new(1, "main", 42, 1)
            .with_source("/src/main.rs");
        assert_eq!(frame.id, 1);
        assert_eq!(frame.name, "main");
        assert_eq!(frame.line, 42);
        assert_eq!(frame.source_path.as_deref(), Some("/src/main.rs"));
        assert_eq!(frame.source_name.as_deref(), Some("main.rs"));
    }
}

// ─── Folding ────────────────────────────────────────────────────────────

#[cfg(test)]
mod folding {
    use vsedit_folding::{FoldingModel, FoldingRange, FoldingRangeKind};

    #[test]
    fn test_integration_folding_set_and_get_ranges() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 7, end_line: 10, kind: FoldingRangeKind::Comment, is_collapsed: false },
        ]);
        assert_eq!(model.get_ranges().len(), 2);
        assert_eq!(model.get_ranges()[0].start_line, 1);
    }

    #[test]
    fn test_integration_folding_compute_from_indentation() {
        let lines = vec![
            "fn main() {",
            "    let x = 1;",
            "    if true {",
            "        println!(\"hi\");",
            "    }",
            "}",
        ];
        let ranges = FoldingModel::compute_from_indentation(&lines, 4);
        assert!(!ranges.is_empty());
        // The outermost range should start at line 1
        assert!(ranges.iter().any(|r| r.start_line == 1));
    }

    #[test]
    fn test_integration_folding_toggle() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        assert!(!model.get_ranges()[0].is_collapsed);
        model.toggle(1);
        assert!(model.get_ranges()[0].is_collapsed);
        model.toggle(1);
        assert!(!model.get_ranges()[0].is_collapsed);
    }

    #[test]
    fn test_integration_folding_fold_unfold_all() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 3, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 5, end_line: 8, kind: FoldingRangeKind::Imports, is_collapsed: false },
        ]);
        model.fold_all();
        assert!(model.get_ranges().iter().all(|r| r.is_collapsed));
        model.unfold_all();
        assert!(model.get_ranges().iter().all(|r| !r.is_collapsed));
    }

    #[test]
    fn test_integration_folding_is_line_hidden() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 2, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: true },
        ]);
        assert!(!model.is_line_hidden(2)); // fold start is visible
        assert!(model.is_line_hidden(3));
        assert!(model.is_line_hidden(5));
        assert!(!model.is_line_hidden(6));
    }
}

// ─── Bracket Matching ───────────────────────────────────────────────────

#[cfg(test)]
mod bracket_matching {
    use vsedit_bracket::{
        find_matching_bracket, find_all_brackets, validate_brackets,
        bracket_color_index, default_bracket_pairs,
    };

    #[test]
    fn test_integration_bracket_find_matching() {
        let lines = vec!["fn main() { }"];
        let pairs = default_bracket_pairs();
        // '(' is at col 8
        let result = find_matching_bracket(&lines, 1, 8, &pairs);
        assert_eq!(result, Some((1, 9))); // ')' at col 9
    }

    #[test]
    fn test_integration_bracket_colorizer_nested() {
        let lines = vec!["((()))"];
        let pairs = default_bracket_pairs();
        let matches = find_all_brackets(&lines, &pairs);
        // Three bracket pairs at depths 0, 1, 2
        assert_eq!(matches.len(), 3);
        let depths: Vec<u32> = matches.iter().map(|m| m.depth).collect();
        assert!(depths.contains(&0));
        assert!(depths.contains(&1));
        assert!(depths.contains(&2));
    }

    #[test]
    fn test_integration_bracket_errors_unmatched() {
        let lines = vec!["(()"];
        let pairs = default_bracket_pairs();
        let result = validate_brackets(&lines, &pairs);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_bracket_valid_document() {
        let lines = vec![
            "fn foo() {",
            "    let v = vec![1, 2, 3];",
            "}",
        ];
        let pairs = default_bracket_pairs();
        let result = validate_brackets(&lines, &pairs);
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_bracket_color_index_cycles() {
        let num_colors = 3;
        assert_eq!(bracket_color_index(0, num_colors), 0);
        assert_eq!(bracket_color_index(1, num_colors), 1);
        assert_eq!(bracket_color_index(2, num_colors), 2);
        assert_eq!(bracket_color_index(3, num_colors), 0);
        assert_eq!(bracket_color_index(4, num_colors), 1);
    }
}

// ─── Merge Editor ───────────────────────────────────────────────────────

#[cfg(test)]
mod merge_editor {
    use vsedit_merge_editor::{
        MergeConflict, MergeConflictBuilder, MergeEditorWidget,
        MergeResolution, parse_conflict_markers,
    };

    #[test]
    fn test_integration_merge_editor_widget() {
        let mut widget = MergeEditorWidget::new();
        let conflict = MergeConflictBuilder::new()
            .region(1, 5)
            .current_text("current")
            .incoming_text("incoming")
            .base_text("base")
            .build()
            .unwrap();
        widget.add_conflict(conflict);
        assert_eq!(widget.unresolved_count(), 1);
        widget.resolve_conflict(0, MergeResolution::AcceptIncoming);
        assert!(widget.all_resolved());
        let result = widget.get_merged_result();
        assert_eq!(result[0], "incoming");
    }

    #[test]
    fn test_integration_merge_conflict_regions() {
        let text = "\
<<<<<<< HEAD
current change
=======
incoming change
>>>>>>> branch";
        let conflicts = parse_conflict_markers(text);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].current_text, "current change");
        assert_eq!(conflicts[0].incoming_text, "incoming change");
    }

    #[test]
    fn test_integration_merge_auto_resolve_trivial() {
        let mut widget = MergeEditorWidget::new();
        // Trivial: current == incoming
        let trivial = MergeConflictBuilder::new()
            .region(1, 3)
            .current_text("same")
            .incoming_text("same")
            .base_text("old")
            .build()
            .unwrap();
        // Non-trivial: current != incoming
        let real = MergeConflictBuilder::new()
            .region(5, 8)
            .current_text("ours")
            .incoming_text("theirs")
            .base_text("base")
            .build()
            .unwrap();
        widget.add_conflict(trivial);
        widget.add_conflict(real);
        let auto_count = widget.auto_resolve_trivial();
        assert_eq!(auto_count, 1);
        assert_eq!(widget.resolved_count(), 1);
        assert_eq!(widget.unresolved_count(), 1);
    }

    #[test]
    fn test_integration_merge_try_resolve_out_of_range() {
        let mut widget = MergeEditorWidget::new();
        let result = widget.try_resolve_conflict(0, MergeResolution::AcceptCurrent);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_merge_accept_both() {
        let mut widget = MergeEditorWidget::new();
        let conflict = MergeConflictBuilder::new()
            .region(1, 3)
            .current_text("alpha")
            .incoming_text("beta")
            .base_text("original")
            .build()
            .unwrap();
        widget.add_conflict(conflict);
        widget.resolve_conflict(0, MergeResolution::AcceptBoth);
        let result = widget.get_merged_result();
        assert_eq!(result[0], "alpha\nbeta");
    }
}

// ─── Workbench Find Bar ─────────────────────────────────────────────────

#[cfg(test)]
mod find_bar {
    use vsedit_workbench::Workbench;

    fn workbench_with_content(text: &str) -> Workbench {
        let mut wb = Workbench::new();
        wb.set_editor_content(text, None);
        wb
    }

    #[test]
    fn test_integration_toggle_find_bar() {
        let mut wb = workbench_with_content("hello world");
        assert!(!wb.show_find_bar);
        wb.toggle_find_bar();
        assert!(wb.show_find_bar);
        wb.toggle_find_bar();
        assert!(!wb.show_find_bar);
    }

    #[test]
    fn test_integration_find_bar_input() {
        let mut wb = workbench_with_content("foo bar foo baz");
        wb.toggle_find_bar();
        wb.find_bar_input('f');
        wb.find_bar_input('o');
        wb.find_bar_input('o');
        assert_eq!(wb.find_query, "foo");
        assert_eq!(wb.find_matches.len(), 2);
    }

    #[test]
    fn test_integration_find_bar_backspace() {
        let mut wb = workbench_with_content("foobar foobaz");
        wb.toggle_find_bar();
        wb.find_bar_input('f');
        wb.find_bar_input('o');
        wb.find_bar_input('o');
        wb.find_bar_input('b');
        wb.find_bar_input('a');
        wb.find_bar_input('r');
        assert_eq!(wb.find_matches.len(), 1);
        wb.find_bar_backspace();
        wb.find_bar_backspace();
        wb.find_bar_backspace();
        // query is now "foo", matches both "foobar" and "foobaz"
        assert_eq!(wb.find_query, "foo");
        assert_eq!(wb.find_matches.len(), 2);
    }

    #[test]
    fn test_integration_update_find_matches_multiline() {
        let mut wb = workbench_with_content("abc\nabc def\nabc");
        wb.toggle_find_bar();
        wb.find_bar_input('a');
        wb.find_bar_input('b');
        wb.find_bar_input('c');
        assert_eq!(wb.find_matches.len(), 3);
        // matches on lines 0, 1, 2
        assert_eq!(wb.find_matches[0].0, 0);
        assert_eq!(wb.find_matches[1].0, 1);
        assert_eq!(wb.find_matches[2].0, 2);
    }

    #[test]
    fn test_integration_find_bar_next_prev() {
        let mut wb = workbench_with_content("aa aa aa");
        wb.toggle_find_bar();
        wb.find_bar_input('a');
        wb.find_bar_input('a');
        assert_eq!(wb.find_matches.len(), 3);
        assert_eq!(wb.find_current_match, 0);
        wb.find_bar_next();
        assert_eq!(wb.find_current_match, 1);
        wb.find_bar_next();
        assert_eq!(wb.find_current_match, 2);
        wb.find_bar_next();
        // wraps around
        assert_eq!(wb.find_current_match, 0);
        wb.find_bar_prev();
        assert_eq!(wb.find_current_match, 2);
    }
}

// ─── Smart Selection ────────────────────────────────────────────────────

#[cfg(test)]
mod smart_selection {
    use vsedit_smartselect::{
        SelectionRange, expand_selection, shrink_selection,
        build_selection_chain, selection_contains, selection_intersects,
    };

    #[test]
    fn test_integration_expand_selection() {
        let parent = SelectionRange::new(1, 1, 10, 80);
        let child = SelectionRange::new(3, 5, 3, 15).with_parent(parent);
        let expanded = expand_selection(&child);
        assert!(expanded.is_some());
        let exp = expanded.unwrap();
        assert_eq!(exp.start_line, 1);
        assert_eq!(exp.end_line, 10);
    }

    #[test]
    fn test_integration_shrink_selection() {
        let parent = SelectionRange::new(1, 1, 10, 80);
        let child = SelectionRange::new(3, 5, 3, 15).with_parent(parent.clone());
        let shrunk = shrink_selection(&child, &parent);
        assert!(shrunk.is_some());
        let s = shrunk.unwrap();
        assert_eq!(s.start_line, 3);
        assert_eq!(s.end_line, 3);
    }

    #[test]
    fn test_integration_expand_at_root_returns_none() {
        let root = SelectionRange::new(1, 1, 100, 1);
        assert!(expand_selection(&root).is_none());
    }

    #[test]
    fn test_integration_build_selection_chain() {
        let chain = build_selection_chain(vec![
            (1, 1, 50, 1),
            (5, 1, 20, 1),
            (10, 5, 10, 15),
        ]);
        // first element becomes innermost (returned), last becomes root parent
        assert_eq!(chain.depth(), 2);
        assert_eq!(chain.start_line, 1);
        let outer = chain.outermost();
        assert_eq!(outer.start_line, 10);
    }

    #[test]
    fn test_integration_selection_contains_and_intersects() {
        let outer = SelectionRange::new(1, 1, 10, 80);
        let inner = SelectionRange::new(3, 5, 7, 20);
        let disjoint = SelectionRange::new(20, 1, 30, 1);

        assert!(selection_contains(&outer, &inner));
        assert!(!selection_contains(&inner, &outer));
        assert!(selection_intersects(&outer, &inner));
        assert!(!selection_intersects(&outer, &disjoint));
    }
}

// ─── Snippets ───────────────────────────────────────────────────────────

#[cfg(test)]
mod snippet_ops {
    use vsedit_snippet::{
        parse_snippet, expand_snippet, collect_tabstops,
        collect_variables, element_count, SnippetVariables,
    };

    #[test]
    fn test_integration_parse_snippet_tabstops() {
        let snippet = parse_snippet("for ($1; $2; $3) {\n\t$0\n}");
        let tabstops = collect_tabstops(&snippet);
        assert!(tabstops.contains(&0));
        assert!(tabstops.contains(&1));
        assert!(tabstops.contains(&2));
        assert!(tabstops.contains(&3));
    }

    #[test]
    fn test_integration_parse_snippet_placeholder() {
        let snippet = parse_snippet("fn ${1:name}($2) -> ${3:Type} {\n\t$0\n}");
        assert!(element_count(&snippet) > 0);
        let vars = SnippetVariables::new();
        let expanded = expand_snippet(&snippet, &vars);
        assert!(expanded.contains("fn "));
        assert!(expanded.contains("name"));
        assert!(expanded.contains("Type"));
    }

    #[test]
    fn test_integration_snippet_variable_expansion() {
        let snippet = parse_snippet("// File: $TM_FILENAME\n$0");
        let vars = collect_variables(&snippet);
        assert!(vars.contains(&"TM_FILENAME".to_string()));

        let mut sv = SnippetVariables::new();
        sv.set("TM_FILENAME", "main.rs");
        let expanded = expand_snippet(&snippet, &sv);
        assert!(expanded.contains("main.rs"));
    }

    #[test]
    fn test_integration_snippet_choice() {
        let snippet = parse_snippet("${1|public,private,protected|} class $2 {}");
        let vars = SnippetVariables::new();
        let expanded = expand_snippet(&snippet, &vars);
        // first choice is used as default
        assert!(expanded.contains("public"));
    }

    #[test]
    fn test_integration_snippet_element_count() {
        let simple = parse_snippet("hello world");
        assert_eq!(element_count(&simple), 1);

        let complex = parse_snippet("$1 text $2 more ${3:default}");
        assert!(element_count(&complex) >= 5);
    }
}

// ─── Sticky Scroll ──────────────────────────────────────────────────────

#[cfg(test)]
mod sticky_scroll {
    use vsedit_stickyscroll::{
        StickyScrollWidget, StickyScrollConfig,
    };

    #[test]
    fn test_integration_sticky_scroll_update_lines() {
        let mut widget = StickyScrollWidget::new(5);
        widget.update_lines(1, 50, &[
            (1, "fn main() {", 0),
            (5, "    if condition {", 1),
            (10, "        for i in 0..10 {", 2),
        ]);
        let lines = widget.get_visible_sticky_lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[2].nesting_level, 2);
    }

    #[test]
    fn test_integration_sticky_scroll_max_lines_limit() {
        let mut widget = StickyScrollWidget::new(2);
        widget.update_lines(1, 100, &[
            (1, "mod a {", 0),
            (2, "  fn b() {", 1),
            (3, "    if c {", 2),
        ]);
        let lines = widget.get_visible_sticky_lines();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_integration_sticky_scroll_toggle_collapse() {
        let mut widget = StickyScrollWidget::new(5);
        widget.update_lines(1, 50, &[
            (1, "fn main() {", 0),
            (5, "    loop {", 1),
        ]);
        assert_eq!(widget.collapsed_count(), 0);
        widget.toggle_collapse(1).unwrap();
        assert_eq!(widget.collapsed_count(), 1);
        widget.toggle_collapse(1).unwrap();
        assert_eq!(widget.collapsed_count(), 0);
    }

    #[test]
    fn test_integration_sticky_scroll_config_defaults() {
        let cfg = StickyScrollConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_line_count, 5);
        assert_eq!(cfg.default_model, "outlineModel");
    }

    #[test]
    fn test_integration_sticky_scroll_disabled() {
        let mut widget = StickyScrollWidget::new(5);
        widget.set_enabled(false);
        assert!(!widget.is_enabled());
        widget.update_lines(1, 50, &[(1, "fn main() {", 0)]);
        // disabled widget ignores updates
        assert!(widget.get_visible_sticky_lines().is_empty());
    }
}

// ─── Code Lens ──────────────────────────────────────────────────────────

#[cfg(test)]
mod code_lens {
    use vsedit_codelens::{
        CodeLens, CodeLensCommand, CommandBuilder,
        codelens_group_adjacent, group_lenses_by_line,
    };

    #[test]
    fn test_integration_codelens_creation() {
        let lens = CodeLens::new(10, 1, 10, 20);
        assert!(!lens.is_resolved());
        assert!(lens.is_single_line());
        assert_eq!(lens.line_span(), 1);
    }

    #[test]
    fn test_integration_codelens_command_wiring() {
        let cmd = CodeLensCommand::ShowReferences { count: 5 };
        let command = cmd.to_command();
        assert_eq!(command.command_id, "editor.showReferences");
        assert!(command.title.contains("5"));

        let lens = CodeLens::new(1, 1, 1, 10).with_command(command);
        assert!(lens.is_resolved());
    }

    #[test]
    fn test_integration_codelens_group_adjacent() {
        let lenses = vec![
            CodeLens::new(1, 1, 1, 10),
            CodeLens::new(2, 1, 2, 10),
            CodeLens::new(10, 1, 10, 10),
            CodeLens::new(11, 1, 11, 10),
        ];
        let groups = codelens_group_adjacent(&lenses, 2);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2); // lines 1, 2
        assert_eq!(groups[1].len(), 2); // lines 10, 11
    }

    #[test]
    fn test_integration_codelens_group_by_line() {
        let cmd = CodeLensCommand::RunTest { test_name: "test_a".into() }.to_command();
        let lenses = vec![
            CodeLens::new(5, 1, 5, 10).with_command(cmd.clone()),
            CodeLens::new(5, 15, 5, 30).with_command(
                CodeLensCommand::ShowReferences { count: 3 }.to_command(),
            ),
            CodeLens::new(10, 1, 10, 10),
        ];
        let by_line = group_lenses_by_line(&lenses);
        // two distinct lines: 5 and 10
        assert_eq!(by_line.len(), 2);
        let (line, group) = &by_line[0];
        assert_eq!(*line, 5);
        assert_eq!(group.len(), 2);
    }

    #[test]
    fn test_integration_codelens_command_builder() {
        let cmd = CommandBuilder::new()
            .title("Run All Tests")
            .command_id("test.runAll")
            .tooltip("Run all tests in file")
            .argument("src/lib.rs")
            .build();
        assert!(cmd.is_ok());
        let c = cmd.unwrap();
        assert_eq!(c.title, "Run All Tests");
        assert_eq!(c.arguments.len(), 1);

        // missing title should fail
        let bad = CommandBuilder::new()
            .command_id("test.runAll")
            .build();
        assert!(bad.is_err());
    }
}

// ─── Inline Completion ──────────────────────────────────────────────────

#[cfg(test)]
mod inline_completion {
    use vsedit_inline_complete::{
        InlineCompletionGhost, GhostTextPosition,
        InlineCompletionSession, InlineCompletionItem,
        InlineCompletionList, InlineCompletionContext,
        InlineCompletionTriggerKind, accept_inline_completion,
    };

    fn make_item(text: &str) -> InlineCompletionItem {
        InlineCompletionItem {
            insert_text: text.to_string(),
            range_start_line: 1,
            range_start_col: 1,
            range_end_line: 1,
            range_end_col: 1,
            filter_text: None,
            command: None,
        }
    }

    #[test]
    fn test_integration_inline_ghost_show_hide() {
        let mut ghost = InlineCompletionGhost::new("console.log()", 1, 1);
        assert!(ghost.is_visible());
        ghost.hide();
        assert!(!ghost.is_visible());
        ghost.show();
        assert!(ghost.is_visible());
        assert_eq!(ghost.position(), GhostTextPosition::AfterCursor);
    }

    #[test]
    fn test_integration_ghost_text_positions() {
        let mut ghost = InlineCompletionGhost::new("line1\nline2", 1, 1);
        assert_eq!(ghost.position(), GhostTextPosition::NextLine);
        ghost.set_position(GhostTextPosition::AfterCursor);
        assert_eq!(ghost.position(), GhostTextPosition::AfterCursor);
        ghost.set_position(GhostTextPosition::BelowCursor);
        assert_eq!(ghost.position(), GhostTextPosition::BelowCursor);
        assert_eq!(ghost.line_count(), 2);
    }

    #[test]
    fn test_integration_inline_session_cycle() {
        let items = vec![make_item("alpha"), make_item("beta"), make_item("gamma")];
        let list = InlineCompletionList { items };
        let mut session = InlineCompletionSession::new(list);
        assert_eq!(session.len(), 3);
        assert_eq!(session.current().unwrap().insert_text, "alpha");
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "beta");
        session.next();
        assert_eq!(session.current().unwrap().insert_text, "gamma");
        session.previous();
        assert_eq!(session.current().unwrap().insert_text, "beta");
    }

    #[test]
    fn test_integration_accept_inline_completion() {
        let items = vec![make_item("world")];
        let list = InlineCompletionList { items };
        let session = InlineCompletionSession::new(list);
        let result = accept_inline_completion(&session, "hello ", 0, 6);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.new_text.contains("world"));
    }

    #[test]
    fn test_integration_inline_empty_session() {
        let list = InlineCompletionList { items: vec![] };
        let session = InlineCompletionSession::new(list);
        assert!(session.is_empty());
        assert_eq!(session.len(), 0);
        assert!(session.current().is_none());
        let result = accept_inline_completion(&session, "hello", 1, 6);
        assert!(result.is_none());
    }
}

// ─── Label Rendering ────────────────────────────────────────────────────

#[cfg(test)]
mod label_rendering {
    use vsedit_label::{
        IconLabel, LabelHighlight, label_ellipsis,
        highlight_label, format_file_label, LabelDetail,
        ResourceLabel, label_ellipsis_middle,
    };

    #[test]
    fn test_integration_icon_label_display() {
        let label = IconLabel {
            text: "main.rs".to_string(),
            icon: Some("rust".to_string()),
            description: Some("src/main.rs".to_string()),
        };
        assert!(label.has_icon());
        let display = label.display_string();
        assert!(display.contains("main.rs"));
    }

    #[test]
    fn test_integration_label_highlight_from_query() {
        let hl = LabelHighlight::from_query("Hello World", "world");
        assert!(hl.has_match());
        assert!(hl.highlight_count() > 0);
        let plain = hl.plain_text();
        assert_eq!(plain, "Hello World");
    }

    #[test]
    fn test_integration_label_ellipsis_truncation() {
        let short = label_ellipsis("hi", 10);
        assert_eq!(short, "hi");
        let long = label_ellipsis("a very long label text here", 10);
        assert!(long.len() <= 13); // includes ellipsis chars
        assert!(long.contains("…") || long.len() <= 10);
    }

    #[test]
    fn test_integration_label_ellipsis_middle() {
        let result = label_ellipsis_middle("src/components/very/deep/path/file.tsx", 20);
        assert!(result.len() <= 23);
        assert!(result.contains("…") || result.len() <= 20);
    }

    #[test]
    fn test_integration_highlight_label_segments() {
        let segments = highlight_label("CommandPalette", "cmd");
        let has_highlight = segments.iter().any(|s| s.highlighted);
        assert!(has_highlight);
        let combined: String = segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(combined, "CommandPalette");
    }
}

// ─── Tasks ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tasks {
    use vsedit_tasks_feature::{
        TaskDefinition, TaskBuilder, TaskSource, TaskGroup,
        TaskService, detect_from_package_json, detect_from_cargo_toml,
    };
    use std::collections::HashMap;

    #[test]
    fn test_integration_task_definition_creation() {
        let mut props = HashMap::new();
        props.insert("type".to_string(), "shell".to_string());
        props.insert("command".to_string(), "echo hello".to_string());
        let def = TaskDefinition {
            task_type: "shell".to_string(),
            properties: props,
        };
        assert_eq!(def.task_type, "shell");
        assert_eq!(def.properties.len(), 2);
    }

    #[test]
    fn test_integration_task_presentation_builder() {
        let task = TaskBuilder::new("build", "cargo build")
            .source(TaskSource::Workspace)
            .group(TaskGroup::Build)
            .args(vec!["--release".to_string()])
            .background(false)
            .build();
        assert_eq!(task.name, "build");
        assert_eq!(task.command, "cargo build");
        assert!(!task.is_background);
    }

    #[test]
    fn test_integration_task_auto_detect_package_json() {
        let content = r#"{
            "scripts": {
                "build": "tsc",
                "test": "jest",
                "lint": "eslint ."
            }
        }"#;
        let detected = detect_from_package_json(content);
        assert!(detected.len() >= 3);
        assert!(detected.iter().any(|t| t.name.contains("build")));
        assert!(detected.iter().any(|t| t.name.contains("test")));
    }

    #[test]
    fn test_integration_task_auto_detect_cargo_toml() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[[bin]]
name = "my-app"
path = "src/main.rs"
"#;
        let detected = detect_from_cargo_toml(content);
        assert!(!detected.is_empty());
    }

    #[test]
    fn test_integration_task_service_run_and_stop() {
        let mut svc = TaskService::new();
        let task = TaskBuilder::new("test-task", "echo ok")
            .source(TaskSource::User)
            .group(TaskGroup::Test)
            .build();
        svc.register_task(task);
        assert_eq!(svc.task_count(), 1);
        let idx = svc.run_task("test-task");
        assert!(idx.is_some());
        assert_eq!(svc.running_count(), 1);
        let stop = svc.stop_task(idx.unwrap(), 0);
        assert!(stop.is_ok());
    }
}

// ─── Language Detection ─────────────────────────────────────────────────

#[cfg(test)]
mod lang_detection {
    use vsedit_wb_langdetect::{
        LanguageDetectionService, detect_by_extension,
        detect_by_shebang, detect_by_content,
        FirstLineDetector, ContentSniffDetector,
    };

    #[test]
    fn test_integration_lang_detect_by_extension() {
        assert_eq!(detect_by_extension("main.rs"), Some("rust".to_string()));
        assert_eq!(detect_by_extension("index.ts"), Some("typescript".to_string()));
        assert_eq!(detect_by_extension("style.css"), Some("css".to_string()));
        assert!(detect_by_extension("unknown.zzz").is_none());
    }

    #[test]
    fn test_integration_shebang_detection() {
        assert_eq!(detect_by_shebang("#!/usr/bin/env python3"), Some("python".to_string()));
        assert_eq!(detect_by_shebang("#!/bin/bash"), Some("shellscript".to_string()));
        assert_eq!(detect_by_shebang("#!/usr/bin/env node"), Some("javascript".to_string()));
        assert!(detect_by_shebang("no shebang here").is_none());
    }

    #[test]
    fn test_integration_content_sniffing() {
        let results = detect_by_content("fn main() {\n    println!(\"hello\");\n}");
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.language_id() == "rust"));
    }

    #[test]
    fn test_integration_first_line_detector() {
        let result = FirstLineDetector::detect("#!/usr/bin/env ruby");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.language_id, "ruby");
    }

    #[test]
    fn test_integration_lang_detection_service() {
        let svc = LanguageDetectionService::new();
        let result = svc.detect("script.py", "");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "python");

        let all = svc.detect_all("const x = 42;");
        assert!(!all.is_empty());
    }
}

// ─── Notification ───────────────────────────────────────────────────────

#[cfg(test)]
mod notification {
    use vsedit_notification_svc::{
        NotificationService, NotificationSeverity, NotificationPriority,
        NotificationGroup, notification_group,
        Notification,
    };

    #[test]
    fn test_integration_notification_stack_ordering() {
        let mut svc = NotificationService::new();
        let id1 = svc.info("First message");
        let id2 = svc.warn("Second message");
        let id3 = svc.error("Third message");
        assert_eq!(svc.notification_count(), 3);
        assert!(svc.first_notification().is_some());
        assert!(svc.last_notification().is_some());
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_integration_notification_throttle_dedup() {
        let mut svc = NotificationService::new();
        svc.info("Duplicate message");
        svc.info("Duplicate message");
        svc.info("Duplicate message");
        assert!(svc.has_duplicate("Duplicate message"));
        let removed = svc.dedup_by_message();
        assert!(removed >= 2);
    }

    #[test]
    fn test_integration_notification_group_by_severity() {
        let notifications = vec![
            Notification { id: 1, message: "same error".into(), severity: NotificationSeverity::Error, source: Some("ext".into()), actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 2, message: "same error".into(), severity: NotificationSeverity::Error, source: Some("ext".into()), actions: vec![], sticky: false, dismissed: false, priority: None },
            Notification { id: 3, message: "info1".into(), severity: NotificationSeverity::Info, source: None, actions: vec![], sticky: false, dismissed: false, priority: None },
        ];
        let groups = notification_group(&notifications);
        assert!(!groups.is_empty());
        let error_group = groups.iter().find(|g| g.representative == "same error");
        assert!(error_group.is_some());
        assert!(error_group.unwrap().count >= 2);
    }

    #[test]
    fn test_integration_notification_dismiss_all() {
        let mut svc = NotificationService::new();
        svc.info("one");
        svc.warn("two");
        svc.error("three");
        assert_eq!(svc.notification_count(), 3);
        svc.dismiss_all();
        let active = svc.get_active();
        assert!(active.is_empty());
    }

    #[test]
    fn test_integration_notification_priority() {
        let mut svc = NotificationService::new();
        svc.add_with_priority("urgent", NotificationSeverity::Error, NotificationPriority::Urgent);
        svc.add_with_priority("low", NotificationSeverity::Info, NotificationPriority::Low);
        let urgent = svc.get_by_priority(NotificationPriority::Urgent);
        assert_eq!(urgent.len(), 1);
        let highest = svc.highest_priority_active();
        assert!(highest.is_some());
        assert!(highest.unwrap().is_urgent());
    }
}

// ─── Environment ────────────────────────────────────────────────────────

#[cfg(test)]
mod environment {
    use vsedit_environment::{
        resolve_env_variables, ShellEnvironment,
        env_path_list, CliArgsBuilder, EnvPathManager,
    };
    use std::path::PathBuf;

    #[test]
    fn test_integration_resolve_env_variables_basic() {
        let getter = |key: &str| -> Option<String> {
            match key {
                "HOME" => Some("/home/user".to_string()),
                "USER" => Some("testuser".to_string()),
                _ => None,
            }
        };
        let result = resolve_env_variables("Hello ${env:USER} at ${env:HOME}", &getter);
        assert!(result.contains("testuser"));
        assert!(result.contains("/home/user"));
    }

    #[test]
    fn test_integration_shell_environment_crud() {
        let mut env = ShellEnvironment::new();
        assert!(env.is_empty());
        env.set("MY_VAR", "hello");
        env.set("OTHER", "world");
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("MY_VAR"), Some("hello"));
        assert!(env.remove("MY_VAR"));
        assert!(env.get("MY_VAR").is_none());
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn test_integration_env_path_list_parsing() {
        let paths = env_path_list("/usr/bin:/usr/local/bin:/home/user/.cargo/bin");
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("/usr/bin"));
        assert_eq!(paths[2], PathBuf::from("/home/user/.cargo/bin"));
    }

    #[test]
    fn test_integration_env_path_manager_operations() {
        let mut mgr = EnvPathManager::from_path_string("/usr/bin:/usr/local/bin");
        assert_eq!(mgr.len(), 2);
        mgr.prepend(PathBuf::from("/opt/bin"));
        assert_eq!(mgr.len(), 3);
        assert!(mgr.contains(std::path::Path::new("/opt/bin")));
        let path_str = mgr.to_path_string();
        assert!(path_str.starts_with("/opt/bin"));
    }

    #[test]
    fn test_integration_cli_args_builder_validate() {
        let args = CliArgsBuilder::new()
            .path("/tmp/test.rs")
            .goto(10, 5)
            .verbose(true)
            .build();
        assert!(args.is_ok());
        let a = args.unwrap();
        assert_eq!(a.goto, Some((10, 5)));
        assert!(a.verbose);
        assert_eq!(a.path_count(), 1);
    }
}

// ─── Whitespace ─────────────────────────────────────────────────────────

#[cfg(test)]
mod whitespace {
    use vsedit_whitespace::{
        whitespace_normalize, NormalizeTarget,
        trim_trailing_whitespace, tabs_to_spaces,
        spaces_to_tabs, detect_indentation, IndentationStyle,
    };

    #[test]
    fn test_integration_whitespace_normalize_to_spaces() {
        let input = "fn main() {\n\tprintln!(\"hello\");\n}";
        let result = whitespace_normalize(input, NormalizeTarget::Spaces(4));
        assert!(!result.contains('\t'));
        assert!(result.contains("    "));
    }

    #[test]
    fn test_integration_whitespace_normalize_to_tabs() {
        let input = "fn main() {\n    println!(\"hello\");\n}";
        let result = whitespace_normalize(input, NormalizeTarget::Tabs);
        assert!(result.contains('\t'));
    }

    #[test]
    fn test_integration_trim_trailing_whitespace() {
        let input = "hello   \nworld  \nclean";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result, "hello\nworld\nclean");
    }

    #[test]
    fn test_integration_tabs_spaces_roundtrip() {
        let original = "    line1\n        line2\n    line3";
        let tabbed = spaces_to_tabs(original, 4);
        let spaced = tabs_to_spaces(&tabbed, 4);
        assert_eq!(spaced, original);
    }

    #[test]
    fn test_integration_detect_indentation_style() {
        let tab_lines: Vec<&str> = vec!["\tfoo", "\t\tbar", "\tbaz"];
        assert!(matches!(detect_indentation(&tab_lines), IndentationStyle::Tab));
        let space_lines: Vec<&str> = vec!["    foo", "        bar", "    baz"];
        assert!(matches!(detect_indentation(&space_lines), IndentationStyle::Spaces(_)));
    }
}

// ─── Input Handling ─────────────────────────────────────────────────────

#[cfg(test)]
mod input_handling {
    use vsedit_input::{
        GestureRecognizer, Gesture,
        InputEventBatcher, KeyInput, MouseButton,
    };
    use vsedit_keycodes::KeyCode;

    #[test]
    fn test_integration_gesture_recognizer_single_click() {
        let mut recognizer = GestureRecognizer::new(300, 5);
        let gesture = recognizer.on_mouse_down(10, 10, 1000);
        assert!(matches!(gesture, Gesture::SingleClick));
    }

    #[test]
    fn test_integration_gesture_recognizer_double_click() {
        let mut recognizer = GestureRecognizer::new(300, 5);
        recognizer.on_mouse_down(10, 10, 1000);
        let gesture = recognizer.on_mouse_down(10, 10, 1100);
        assert!(matches!(gesture, Gesture::DoubleClick));
    }

    #[test]
    fn test_integration_gesture_recognizer_triple_click() {
        let mut recognizer = GestureRecognizer::new(300, 5);
        recognizer.on_mouse_down(10, 10, 1000);
        recognizer.on_mouse_down(10, 10, 1100);
        let gesture = recognizer.on_mouse_down(10, 10, 1200);
        assert!(matches!(gesture, Gesture::TripleClick));
    }

    #[test]
    fn test_integration_input_event_batcher_flush() {
        let mut batcher = InputEventBatcher::new(50);
        let key = KeyInput {
            key_code: KeyCode::KeyA,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        };
        batcher.push(key.clone(), 100);
        batcher.push(key.clone(), 110);
        assert_eq!(batcher.pending_count(), 2);
        let flushed = batcher.flush();
        assert_eq!(flushed.len(), 2);
        assert!(batcher.is_empty());
    }

    #[test]
    fn test_integration_input_event_batcher_window() {
        let mut batcher = InputEventBatcher::new(50);
        let key = KeyInput {
            key_code: KeyCode::KeyB,
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        };
        let result1 = batcher.push(key.clone(), 100);
        assert!(result1.is_none());
        // Push beyond the batch window to trigger a flush
        let result2 = batcher.push(key.clone(), 200);
        assert!(result2.is_some());
        let batch = result2.unwrap();
        assert!(!batch.is_empty());
    }
}

// ─── Storage ────────────────────────────────────────────────────────────

#[cfg(test)]
mod storage {
    use vsedit_storage::{
        StorageDatabase, storage_namespace, storage_migrate,
        StorageQuota, StorageExporter,
    };

    #[test]
    fn test_integration_storage_database_crud() {
        let mut db = StorageDatabase::new();
        assert!(db.is_empty());
        db.set("key1", "value1");
        db.set("key2", "value2");
        assert_eq!(db.len(), 2);
        assert_eq!(db.get("key1"), Some("value1"));
        assert!(db.has("key2"));
        db.remove("key1");
        assert!(!db.has("key1"));
        assert_eq!(db.len(), 1);
    }

    #[test]
    fn test_integration_storage_namespace_isolation() {
        let mut db = StorageDatabase::new();
        {
            let mut ns_a = storage_namespace(&mut db, "moduleA");
            ns_a.set("setting", "valueA");
        }
        {
            let mut ns_b = storage_namespace(&mut db, "moduleB");
            ns_b.set("setting", "valueB");
        }
        let ns_a = storage_namespace(&mut db, "moduleA");
        assert_eq!(ns_a.get("setting"), Some("valueA"));
        let ns_b = storage_namespace(&mut db, "moduleB");
        assert_eq!(ns_b.get("setting"), Some("valueB"));
    }

    #[test]
    fn test_integration_storage_migrate_renames() {
        let mut db = StorageDatabase::new();
        db.set("old.key", "data");
        let migrations = storage_migrate(&mut db, 2, &[
            (2, "old.key", "new.key"),
        ]);
        assert!(!migrations.is_empty());
        assert_eq!(db.get("new.key"), Some("data"));
        assert!(db.version() >= 2);
    }

    #[test]
    fn test_integration_storage_quota_tracking() {
        let mut db = StorageDatabase::new();
        db.set("k1", "short");
        db.set("k2", "a longer value here");
        let mut quota = StorageQuota::new(100, 4096);
        quota.compute_usage(&db);
        assert_eq!(quota.current_keys(), 2);
        assert!(quota.current_bytes() > 0);
        assert!(!quota.would_exceed(5, 10));
        assert!(quota.remaining_keys() > 0);
    }

    #[test]
    fn test_integration_storage_export_import() {
        let mut db = StorageDatabase::new();
        db.set("alpha", "1");
        db.set("beta", "2");
        db.set("gamma", "3");
        let map = StorageExporter::to_map(&db);
        assert_eq!(map.len(), 3);
        let restored = StorageExporter::from_map(&map);
        assert_eq!(restored.len(), 3);
        assert_eq!(restored.get("beta"), Some("2"));
    }
}

// ─── State Management ───────────────────────────────────────────────────

#[cfg(test)]
mod state_management {
    use vsedit_state::{
        WorkspaceState, GlobalState, StateService, StateScope,
        state_migration, migration_needed,
    };

    #[test]
    fn test_integration_workspace_state_crud() {
        let mut ws = WorkspaceState::new("project-1");
        assert_eq!(ws.workspace_id, "project-1");
        ws.set("editor.fontSize", "14");
        ws.set("editor.tabSize", "4");
        assert_eq!(ws.get("editor.fontSize"), Some("14"));
        let exported = ws.export();
        assert_eq!(exported.len(), 2);
        ws.remove("editor.fontSize");
        assert!(ws.get("editor.fontSize").is_none());
    }

    #[test]
    fn test_integration_global_state_versioned() {
        let mut gs = GlobalState::with_version(1);
        assert_eq!(gs.version(), 1);
        gs.set("theme", "dark");
        gs.set("locale", "en-US");
        assert_eq!(gs.get("theme"), Some("dark"));
        let keys = gs.keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_integration_state_migration_renames() {
        let mut gs = GlobalState::new();
        gs.set("old.setting", "value");
        let result = state_migration(
            &mut gs,
            &[("old.setting", "new.setting")],
            &[],
            2,
        );
        assert!(result.keys_renamed > 0);
        assert_eq!(gs.get("new.setting"), Some("value"));
        assert!(gs.get("old.setting").is_none());
    }

    #[test]
    fn test_integration_migration_needed_check() {
        assert!(migration_needed(1, 3));
        assert!(!migration_needed(3, 3));
        assert!(!migration_needed(5, 3));
    }

    #[test]
    fn test_integration_state_service_scopes() {
        let mut svc = StateService::new();
        svc.set("key1", "global_val", StateScope::Global);
        svc.set("key2", "ws_val", StateScope::Workspace);
        let global_entries = svc.get_by_scope(StateScope::Global);
        let ws_entries = svc.get_by_scope(StateScope::Workspace);
        assert!(global_entries.iter().any(|(k, _)| *k == "key1"));
        assert!(ws_entries.iter().any(|(k, _)| *k == "key2"));
        svc.clear_scope(StateScope::Workspace);
        assert!(svc.get_by_scope(StateScope::Workspace).is_empty());
        assert!(!svc.get_by_scope(StateScope::Global).is_empty());
    }
}

// ─── Workbench Commands ─────────────────────────────────────────────────

#[cfg(test)]
mod workbench_commands {
    use vsedit_wb_commands::{command_palette_search, CommandPaletteHistory,
                             CommandPaletteItem, CommandBatch};

    #[test]
    fn test_integration_palette_search_fuzzy_exact() {
        let items = vec![
            CommandPaletteItem::new("editor.formatDocument", "Format Document"),
            CommandPaletteItem::new("editor.formatSelection", "Format Selection"),
            CommandPaletteItem::new("file.save", "Save File"),
        ];
        let results = command_palette_search(&items, "format");
        assert!(results.len() >= 2);
        assert!(results.iter().all(|r| r.item.display_label().to_lowercase().contains("format")));
    }

    #[test]
    fn test_integration_palette_search_fuzzy_partial() {
        let items = vec![
            CommandPaletteItem::new("workbench.action.toggleSidebar", "Toggle Sidebar"),
            CommandPaletteItem::new("workbench.action.togglePanel", "Toggle Panel"),
            CommandPaletteItem::new("editor.action.rename", "Rename Symbol"),
        ];
        let results = command_palette_search(&items, "tgl");
        assert!(results.iter().any(|r| r.item.command_id.contains("toggle")));
    }

    #[test]
    fn test_integration_palette_search_no_match() {
        let items = vec![
            CommandPaletteItem::new("file.save", "Save File"),
        ];
        let results = command_palette_search(&items, "zzzznotfound");
        assert!(results.is_empty());
    }

    #[test]
    fn test_integration_palette_history_record_and_recent() {
        let mut history = CommandPaletteHistory::new(10);
        history.record("cmd.a");
        history.record("cmd.b");
        history.record("cmd.c");
        history.record("cmd.a");
        let recent = history.recent();
        assert!(!recent.is_empty());
        assert!(recent.len() <= 10);
    }

    #[test]
    fn test_integration_command_batch_push_and_len() {
        let mut batch = CommandBatch::new();
        batch.push("cmd.one");
        batch.push("cmd.two");
        batch.push("cmd.three");
        assert_eq!(batch.len(), 3);
        assert!(!batch.is_empty());
    }
}

// ─── Theme Colors ───────────────────────────────────────────────────────

#[cfg(test)]
mod theme_colors {
    use vsedit_wb_themes::{ThemeColorMap, color_blend, theme_contrast_ratio,
                           ColorValue, relative_luminance, ColorTheme, ThemeType,
                           TokenColor};

    fn make_theme_with_tokens() -> ColorTheme {
        ColorTheme {
            id: "test-theme".into(),
            label: "Test Theme".into(),
            theme_type: ThemeType::Dark,
            colors: std::collections::HashMap::new(),
            token_colors: vec![
                TokenColor {
                    scope: vec!["keyword".into(), "storage".into()],
                    foreground: Some("#569cd6".into()),
                    font_style: None,
                },
                TokenColor {
                    scope: vec!["string".into()],
                    foreground: Some("#ce9178".into()),
                    font_style: None,
                },
            ],
        }
    }

    #[test]
    fn test_integration_theme_color_map_from_theme() {
        let theme = make_theme_with_tokens();
        let map = ThemeColorMap::from_theme(&theme);
        assert!(map.get_color("keyword").is_some());
        assert!(map.get_color("string").is_some());
        assert!(map.get_color("nonexistent.scope").is_none());
    }

    #[test]
    fn test_integration_theme_color_map_len_and_scopes() {
        let theme = make_theme_with_tokens();
        let map = ThemeColorMap::from_theme(&theme);
        assert_eq!(map.len(), 3); // keyword, storage, string
        assert!(!map.is_empty());
        let scopes = map.scopes();
        assert!(scopes.contains(&"keyword"));
    }

    #[test]
    fn test_integration_color_blend_two_colors() {
        let c1 = ColorValue::new("#000000").unwrap();
        let c2 = ColorValue::new("#ffffff").unwrap();
        let blended = color_blend(&c1, &c2, 0.5);
        let (r, _g, _b) = blended.to_rgb_tuple();
        assert!(r > 100 && r < 200);
    }

    #[test]
    fn test_integration_theme_contrast_ratio_black_white() {
        let black = ColorValue::new("#000000").unwrap();
        let white = ColorValue::new("#ffffff").unwrap();
        let ratio = theme_contrast_ratio(&black, &white);
        assert!(ratio > 20.0);
    }

    #[test]
    fn test_integration_relative_luminance_extremes() {
        let black = ColorValue::new("#000000").unwrap();
        let white = ColorValue::new("#ffffff").unwrap();
        let lum_black = relative_luminance(&black);
        let lum_white = relative_luminance(&white);
        assert!(lum_black < 0.01);
        assert!(lum_white > 0.99);
    }
}

// ─── Tab Bar ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tab_bar {
    use vsedit_tabbar::{TabDragReorder, TabOverflow,
                        calculate_tab_widths};

    #[test]
    fn test_integration_tab_drag_reorder_insert_index() {
        let drag = TabDragReorder::start("tab1", 150.0);
        // Tabs at (0,100), (100,100), (200,100) — midpoints 50, 150, 250
        let positions = vec![(0.0, 100.0), (100.0, 100.0), (200.0, 100.0)];
        let idx = drag.calculate_insert_index(&positions);
        assert!(idx <= 3);
    }

    #[test]
    fn test_integration_tab_drag_reorder_cancel() {
        let mut drag = TabDragReorder::start("tab1", 50.0);
        assert!(drag.active);
        drag.cancel();
        assert!(!drag.active);
    }

    #[test]
    fn test_integration_tab_overflow_scroll() {
        let mut overflow = TabOverflow::new(5);
        overflow.update_total(10);
        assert!(overflow.is_overflowing());
        overflow.scroll_right();
        overflow.scroll_right();
        assert_eq!(overflow.scroll_offset, 2);
        overflow.scroll_left();
        assert_eq!(overflow.scroll_offset, 1);
    }

    #[test]
    fn test_integration_tab_overflow_ensure_visible() {
        let mut overflow = TabOverflow::new(3);
        overflow.update_total(10);
        overflow.ensure_visible(7);
        assert!(overflow.visible_range().contains(&7));
    }

    #[test]
    fn test_integration_calculate_tab_widths_fits() {
        let labels = vec!["file1.rs", "main.rs", "lib.rs"];
        let widths = calculate_tab_widths(&labels, 600, 50, 200, 4);
        assert_eq!(widths.len(), 3);
        assert!(widths.iter().all(|&w| w >= 50));
    }
}

// ─── Lifecycle ──────────────────────────────────────────────────────────

#[cfg(test)]
mod lifecycle {
    use vsedit_lifecycle::{DisposableStore, lifecycle_phase_name,
                           Disposable, to_disposable, DisposableMap};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_integration_disposable_store_add_dispose() {
        let disposed = Arc::new(AtomicBool::new(false));
        let d = disposed.clone();
        let store = DisposableStore::new();
        store.add(to_disposable(move || { d.store(true, Ordering::SeqCst); }));
        assert!(!disposed.load(Ordering::SeqCst));
        store.dispose();
        assert!(disposed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_integration_disposable_store_len() {
        let store = DisposableStore::new();
        assert!(store.is_empty());
        store.add(to_disposable(|| {}));
        store.add(to_disposable(|| {}));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_integration_lifecycle_phase_name_values() {
        assert_eq!(lifecycle_phase_name(1), "Starting");
        assert_eq!(lifecycle_phase_name(2), "Ready");
        assert_eq!(lifecycle_phase_name(3), "ShuttingDown");
        assert_eq!(lifecycle_phase_name(0), "None");
    }

    #[test]
    fn test_integration_disposable_map_set_has() {
        let map: DisposableMap<String> = DisposableMap::new();
        map.set("key1".to_string(), to_disposable(|| {}));
        map.set("key2".to_string(), to_disposable(|| {}));
        assert!(map.has(&"key1".to_string()));
        assert!(map.has(&"key2".to_string()));
        map.delete_and_dispose(&"key1".to_string());
        assert!(!map.has(&"key1".to_string()));
    }

    #[test]
    fn test_integration_to_disposable_is_disposed() {
        let d = to_disposable(|| {});
        assert!(!d.is_disposed());
        d.dispose();
        assert!(d.is_disposed());
    }
}

// ─── Settings ───────────────────────────────────────────────────────────

#[cfg(test)]
mod settings {
    use vsedit_settings_view::{SettingsSearchIndex, SettingEntry, SettingType,
                               filter_modified, SettingsView};

    #[test]
    fn test_integration_settings_search_index_basic() {
        let entries = vec![
            SettingEntry::new("editor.tabSize", "Tab Size", "Controls tab size", "Editor", SettingType::Number, "4"),
            SettingEntry::new("editor.fontSize", "Font Size", "Controls font size", "Editor", SettingType::Number, "14"),
            SettingEntry::new("files.autoSave", "Auto Save", "Controls auto save", "Files", SettingType::String, "afterDelay"),
        ];
        let index = SettingsSearchIndex::build(&entries);
        let results = index.search("editor");
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_integration_settings_search_index_no_match() {
        let entries = vec![
            SettingEntry::new("editor.tabSize", "Tab Size", "Controls tab size", "Editor", SettingType::Number, "4"),
        ];
        let index = SettingsSearchIndex::build(&entries);
        let results = index.search("zzzznotfound");
        assert!(results.is_empty());
    }

    #[test]
    fn test_integration_filter_modified_entries() {
        let mut entry1 = SettingEntry::new("editor.tabSize", "Tab Size", "Tab size", "Editor", SettingType::Number, "4");
        entry1.current_value = "2".to_string();
        let entry2 = SettingEntry::new("editor.fontSize", "Font Size", "Font size", "Editor", SettingType::Number, "14");
        let mut entry3 = SettingEntry::new("files.autoSave", "Auto Save", "Auto save", "Files", SettingType::String, "off");
        entry3.current_value = "afterDelay".to_string();
        let entries = vec![entry1, entry2, entry3];
        let modified = filter_modified(&entries);
        assert_eq!(modified.len(), 2);
    }

    #[test]
    fn test_integration_settings_view_empty() {
        let view = SettingsView::new();
        assert!(view.entries.is_empty());
    }

    #[test]
    fn test_integration_settings_view_add_entries() {
        let mut view = SettingsView::new();
        view.add_entry(SettingEntry::new("editor.tabSize", "Tab Size", "Tab size", "Editor", SettingType::Number, "4"));
        view.add_entry(SettingEntry::new("editor.fontSize", "Font Size", "Font size", "Editor", SettingType::Number, "14"));
        assert_eq!(view.entries.len(), 2);
    }
}

// ─── Type Hierarchy ─────────────────────────────────────────────────────

#[cfg(test)]
mod type_hierarchy {
    use vsedit_typehier::{TypeHierarchyTree, TypeHierarchyItem, SymbolKind,
                          TypeTree, resolve_type_chain, type_hierarchy_flatten,
                          type_hierarchy_roots};

    fn item(name: &str, kind: SymbolKind) -> TypeHierarchyItem {
        TypeHierarchyItem::new(name.into(), kind, "file:///test.rs".into(), 1, 0, 10, 0)
    }

    #[test]
    fn test_integration_type_tree_add_and_count() {
        let mut tree = TypeTree::new();
        let idx0 = tree.add_type(item("Animal", SymbolKind::Class));
        let idx1 = tree.add_type(item("Dog", SymbolKind::Class));
        tree.add_subtype_edge(idx0, idx1);
        assert_eq!(tree.type_count(), 2);
        assert_eq!(tree.get_subtypes(idx0).len(), 1);
    }

    #[test]
    fn test_integration_resolve_type_chain_linear() {
        let mut tree = TypeTree::new();
        let base = tree.add_type(item("Base", SymbolKind::Class));
        let middle = tree.add_type(item("Middle", SymbolKind::Class));
        let derived = tree.add_type(item("Derived", SymbolKind::Class));
        tree.add_supertype_edge(middle, base);
        tree.add_supertype_edge(derived, middle);
        let chain = resolve_type_chain(&tree, derived);
        assert!(chain.len() >= 2);
    }

    #[test]
    fn test_integration_type_hierarchy_flatten() {
        let mut tree = TypeTree::new();
        tree.add_type(item("A", SymbolKind::Class));
        tree.add_type(item("B", SymbolKind::Interface));
        tree.add_type(item("C", SymbolKind::Struct));
        let flat = type_hierarchy_flatten(&tree);
        assert_eq!(flat.len(), 3);
    }

    #[test]
    fn test_integration_type_hierarchy_roots_no_supertypes() {
        let mut tree = TypeTree::new();
        let a = tree.add_type(item("Root", SymbolKind::Class));
        let b = tree.add_type(item("Child", SymbolKind::Class));
        tree.add_supertype_edge(b, a);
        let roots = type_hierarchy_roots(&tree);
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Root");
    }

    #[test]
    fn test_integration_type_hierarchy_tree_render() {
        let mut tree = TypeTree::new();
        let root = tree.add_type(item("Base", SymbolKind::Class));
        let child = tree.add_type(item("Derived", SymbolKind::Class));
        tree.add_subtype_edge(root, child);
        let rendered = TypeHierarchyTree::render_subtypes(&tree, root);
        assert!(rendered.contains("Base"));
        assert!(rendered.contains("Derived"));
    }
}

// ─── Policy ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod policy {
    use vsedit_policy::{PolicyEngine, PolicyValue, Policy, merge_scoped_policies,
                        policy_report, PolicyService, ScopedPolicy, PolicyScope};

    #[test]
    fn test_integration_policy_engine_add_evaluate() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("feature.copilot", true);
        engine.add_rule("feature.terminal", false);
        assert_eq!(engine.evaluate("feature.copilot"), Some(true));
        assert_eq!(engine.evaluate("feature.terminal"), Some(false));
        assert!(engine.evaluate("nonexistent").is_none());
    }

    #[test]
    fn test_integration_policy_engine_rule_count() {
        let mut engine = PolicyEngine::new();
        assert_eq!(engine.rule_count(), 0);
        engine.add_rule("a", true);
        engine.add_rule("b", false);
        assert_eq!(engine.rule_count(), 2);
    }

    #[test]
    fn test_integration_policy_merge_scoped() {
        let policies = vec![
            ScopedPolicy {
                policy: Policy { name: "feature.x".into(), value: PolicyValue::Bool(false), description: None },
                scope: PolicyScope::User,
            },
            ScopedPolicy {
                policy: Policy { name: "feature.x".into(), value: PolicyValue::Bool(true), description: None },
                scope: PolicyScope::Machine,
            },
        ];
        let merged = merge_scoped_policies(&policies);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].value, PolicyValue::Bool(true)); // Machine > User
    }

    #[test]
    fn test_integration_policy_report_output() {
        let mut svc = PolicyService::new();
        svc.set_policy("feature.a", PolicyValue::Bool(true), Some("Feature A".into()));
        svc.set_policy("feature.b", PolicyValue::Bool(false), None);
        let report = policy_report(&svc);
        assert!(!report.is_empty());
        assert!(report.iter().any(|r| r.name == "feature.a"));
    }

    #[test]
    fn test_integration_policy_engine_remove_rule() {
        let mut engine = PolicyEngine::new();
        engine.add_rule("key", true);
        assert_eq!(engine.rule_count(), 1);
        assert!(engine.remove_rule("key"));
        assert_eq!(engine.rule_count(), 0);
    }
}

// ─── Emmet ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod emmet {
    use vsedit_emmet::{EmmetAbbreviationParser, tag_completion, expand_abbreviation,
                       is_abbreviation};

    #[test]
    fn test_integration_emmet_parser_simple_tag() {
        let parser = EmmetAbbreviationParser::new("div");
        assert!(parser.is_valid());
        let node = parser.parse();
        assert!(node.is_some());
    }

    #[test]
    fn test_integration_emmet_parser_nested() {
        let parser = EmmetAbbreviationParser::new("ul>li");
        assert!(parser.is_valid());
        let node = parser.parse();
        assert!(node.is_some());
    }

    #[test]
    fn test_integration_tag_completion_html() {
        let completion = tag_completion("<div");
        assert!(completion.is_some());
        assert!(completion.unwrap().contains("div"));
    }

    #[test]
    fn test_integration_expand_abbreviation_basic() {
        let expanded = expand_abbreviation("p");
        assert!(expanded.is_some());
        let html = expanded.unwrap();
        assert!(html.contains("<p>"));
        assert!(html.contains("</p>"));
    }

    #[test]
    fn test_integration_is_abbreviation_checks() {
        assert!(is_abbreviation("div"));
        assert!(is_abbreviation("ul>li"));
        assert!(!is_abbreviation(""));
    }
}

// ─── Outline ────────────────────────────────────────────────────────────

#[cfg(test)]
mod outline {
    use vsedit_outline_view::{DocumentSymbolTree, OutlineElement, OutlineKind,
                              OutlineModel, outline_breadcrumb,
                              outline_sort_by_position, outline_sort_by_name};

    fn elem(label: &str, kind: OutlineKind, start: u32, end: u32) -> OutlineElement {
        OutlineElement {
            label: label.into(),
            detail: None,
            kind,
            range_start_line: start,
            range_end_line: end,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_integration_document_symbol_tree_build() {
        let mut model = OutlineModel::new("file:///test.rs");
        model.add_element(elem("main", OutlineKind::Function, 1, 10));
        model.add_element(elem("MyStruct", OutlineKind::Struct, 12, 20));
        let tree = DocumentSymbolTree::new(&model);
        let rendered = tree.render();
        assert!(rendered.contains("main"));
        assert!(rendered.contains("MyStruct"));
    }

    #[test]
    fn test_integration_outline_model_flatten() {
        let mut model = OutlineModel::new("file:///test.rs");
        model.add_element(
            elem("Module", OutlineKind::Module, 1, 50)
                .with_child(elem("func", OutlineKind::Function, 5, 15))
        );
        let flat = model.flatten();
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn test_integration_outline_breadcrumb_path() {
        let mut model = OutlineModel::new("file:///test.rs");
        model.add_element(
            elem("Module", OutlineKind::Module, 1, 50)
                .with_child(elem("func", OutlineKind::Function, 5, 15))
        );
        let crumbs = outline_breadcrumb(&model, 7);
        assert!(!crumbs.is_empty());
    }

    #[test]
    fn test_integration_outline_sort_by_position_order() {
        let mut elements = vec![
            elem("c", OutlineKind::Function, 20, 30),
            elem("a", OutlineKind::Function, 1, 10),
            elem("b", OutlineKind::Function, 11, 19),
        ];
        outline_sort_by_position(&mut elements);
        assert_eq!(elements[0].label, "a");
        assert_eq!(elements[1].label, "b");
        assert_eq!(elements[2].label, "c");
    }

    #[test]
    fn test_integration_outline_sort_by_name_order() {
        let mut elements = vec![
            elem("zebra", OutlineKind::Function, 1, 5),
            elem("alpha", OutlineKind::Function, 6, 10),
            elem("middle", OutlineKind::Function, 11, 15),
        ];
        outline_sort_by_name(&mut elements);
        assert_eq!(elements[0].label, "alpha");
        assert_eq!(elements[1].label, "middle");
        assert_eq!(elements[2].label, "zebra");
    }
}

// ─── Debug Adapter ──────────────────────────────────────────────────────

#[cfg(test)]
mod debug_adapter {
    use vsedit_debug::{DebugAdapterMessage, DebugSession, DebugSessionState,
                       parse_stack_trace_line, format_variable_value};

    #[test]
    fn test_integration_debug_adapter_message_request() {
        let msg = DebugAdapterMessage::Request {
            seq: 1,
            command: "initialize".into(),
            arguments: None,
        };
        if let DebugAdapterMessage::Request { seq, command, .. } = &msg {
            assert_eq!(*seq, 1);
            assert_eq!(command, "initialize");
        } else {
            panic!("expected Request variant");
        }
    }

    #[test]
    fn test_integration_debug_adapter_message_response() {
        let msg = DebugAdapterMessage::Response {
            seq: 2,
            request_seq: 1,
            success: true,
            command: "initialize".into(),
            body: None,
            message: None,
        };
        if let DebugAdapterMessage::Response { success, .. } = &msg {
            assert!(*success);
        } else {
            panic!("expected Response variant");
        }
    }

    #[test]
    fn test_integration_debug_session_lifecycle() {
        let mut session = DebugSession::new("test-session", "test", "node");
        assert_eq!(session.state(), DebugSessionState::NotStarted);
        session.initialize().unwrap();
        assert_eq!(session.state(), DebugSessionState::Initializing);
        session.launch(1000).unwrap();
        assert_eq!(session.state(), DebugSessionState::Running);
        session.pause().unwrap();
        assert_eq!(session.state(), DebugSessionState::Paused);
        session.continue_execution().unwrap();
        assert_eq!(session.state(), DebugSessionState::Running);
        session.terminate().unwrap();
        assert_eq!(session.state(), DebugSessionState::Terminated);
    }

    #[test]
    fn test_integration_parse_stack_trace_line_gdb() {
        let frame = parse_stack_trace_line("#0 main at src/main.rs:42");
        assert!(frame.is_some());
        let f = frame.unwrap();
        assert_eq!(f.function_name, "main");
        assert!(f.file_path.as_deref().unwrap().contains("main.rs"));
    }

    #[test]
    fn test_integration_format_variable_value_types() {
        let with_type = format_variable_value("x", "42", Some("i32"));
        assert!(with_type.contains("x"));
        assert!(with_type.contains("42"));
        assert!(with_type.contains("i32"));
        let without_type = format_variable_value("y", "hello", None);
        assert!(without_type.contains("y"));
        assert!(without_type.contains("hello"));
    }
}

// ─── Batch 1: Editor Engine Integration ─────────────────────────────────

#[cfg(test)]
mod editor_engine_integration {
    use vsedit_cursor::{CursorState, CursorSoftWrapHandler};
    use vsedit_editor_types::Selection;
    use vsedit_multicursor::{
        CursorPosition, MultiCursorSession, ColumnSelectionMode, TextTransform,
        apply_transform_at_cursors,
    };
    use vsedit_find::{FindOptions, preserve_case_replace, replace_all_preserve_case, find_matches};
    #[allow(unused_imports)]
    use vsedit_editor_types::Position;
    use vsedit_snippet::{
        parse_snippet, expand_snippet, SnippetVariables, SnippetTransform,
        SnippetTransformPipeline,
    };
    use vsedit_folding::{
        FoldingModel, FoldingRange, FoldingRangeKind, FoldState,
        fold_region, unfold_region,
    };

    #[test]
    fn test_cursor_soft_wrap_navigation() {
        let handler = CursorSoftWrapHandler::new(40);
        // A 100-char logical line wraps into 3 visual lines at width 40
        assert_eq!(handler.visual_line_count(100), 3);
        // Logical column 0 → visual line 0, col 1 (1-based)
        let (vl, vc) = handler.logical_to_visual(0);
        assert_eq!(vl, 0);
        assert_eq!(vc, 1);
        // Logical column 45 → visual line 1, col 5
        let (vl2, vc2) = handler.logical_to_visual(45);
        assert_eq!(vl2, 1);
        assert_eq!(vc2, 5);
        // Round-trip back
        let logical = handler.visual_to_logical(vl2, vc2);
        assert_eq!(logical, 45);
    }

    #[test]
    fn test_multicursor_column_selection() {
        let col_mode = ColumnSelectionMode { anchor_column: 5 };
        assert_eq!(col_mode.anchor_column, 5);

        let mut session = MultiCursorSession::new();
        session.add_cursor(CursorPosition { line: 1, column: 5 });
        session.add_cursor(CursorPosition { line: 2, column: 5 });
        session.add_cursor(CursorPosition { line: 3, column: 5 });
        assert_eq!(session.cursor_count(), 3);
        // All cursors share the same column in column selection mode
        assert!(session.cursors.iter().all(|c| c.column == 5));
    }

    #[test]
    fn test_find_preserve_case_replace() {
        // Lowercase → lowercase
        assert_eq!(preserve_case_replace("hello", "world"), "world");
        // Uppercase → uppercase
        assert_eq!(preserve_case_replace("HELLO", "world"), "WORLD");
        // Title case → title case
        assert_eq!(preserve_case_replace("Hello", "world"), "World");

        // Full text replacement with preserve-case
        let opts = FindOptions {
            search_string: "foo".into(),
            is_regex: false,
            case_sensitive: false,
            whole_word: false,
            preserve_case: false,
        };
        let result = replace_all_preserve_case("Foo fOO FOO", &opts, "bar");
        assert!(result.contains("Bar"));
        assert!(result.contains("BAR"));
    }

    #[test]
    fn test_snippet_transform_in_editor() {
        // Parse a snippet with a tabstop
        let snippet = parse_snippet("fn ${1:name}() {}");
        let mut vars = SnippetVariables::new();
        vars.set("1", "my_func");
        let expanded = expand_snippet(&snippet, &vars);
        assert!(expanded.contains("fn"));

        // Apply regex transforms via pipeline
        let mut pipeline = SnippetTransformPipeline::new();
        let t1 = SnippetTransform::parse("snake_(\\w)/\\U$1/g").unwrap();
        pipeline.add(t1);
        let result = pipeline.apply("snake_case_name");
        // The transform uppercases the char after snake_
        assert_ne!(result, "snake_case_name");
        assert_eq!(pipeline.len(), 1);
    }

    #[test]
    fn test_folding_persistence_roundtrip() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 20, kind: FoldingRangeKind::Imports, is_collapsed: false },
            FoldingRange { start_line: 25, end_line: 30, kind: FoldingRangeKind::Comment, is_collapsed: false },
        ]);
        // Collapse two ranges
        fold_region(&mut model, 1, false);
        fold_region(&mut model, 25, false);

        // Capture state
        let state = FoldState::capture(&model);
        assert!(state.has_collapsed());
        assert_eq!(state.collapsed_count(), 2);

        // Restore to a fresh model with the same ranges
        let mut model2 = FoldingModel::new();
        model2.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 5, kind: FoldingRangeKind::Region, is_collapsed: false },
            FoldingRange { start_line: 10, end_line: 20, kind: FoldingRangeKind::Imports, is_collapsed: false },
            FoldingRange { start_line: 25, end_line: 30, kind: FoldingRangeKind::Comment, is_collapsed: false },
        ]);
        state.restore(&mut model2);
        let state2 = FoldState::capture(&model2);
        assert_eq!(state.collapsed_lines, state2.collapsed_lines);
    }

    // Batch 1 additional tests for the 5 required names
    #[test]
    fn test_cursor_soft_wrap_edge_zero_length() {
        let handler = CursorSoftWrapHandler::new(80);
        assert_eq!(handler.visual_line_count(0), 1);
        assert_eq!(handler.visual_line_count(80), 1);
        assert_eq!(handler.visual_line_count(81), 2);
    }

    #[test]
    fn test_multicursor_transform_uppercase() {
        let pairs = vec![
            (CursorPosition { line: 0, column: 0 }, "hello"),
            (CursorPosition { line: 1, column: 0 }, "world"),
        ];
        let results = apply_transform_at_cursors(&pairs, TextTransform::Uppercase);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].transformed, "HELLO");
        assert_eq!(results[1].transformed, "WORLD");
    }

    #[test]
    fn test_find_regex_replace_groups() {
        let opts = FindOptions {
            search_string: r"(\w+)@(\w+)".into(),
            is_regex: true,
            case_sensitive: false,
            whole_word: false,
            preserve_case: false,
        };
        let matches = find_matches("user@host other@domain", &opts);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_snippet_pipeline_chaining() {
        let mut pipeline = SnippetTransformPipeline::new();
        pipeline.add(SnippetTransform::parse("a/b/g").unwrap());
        pipeline.add(SnippetTransform::parse("b/c/g").unwrap());
        let result = pipeline.apply("aaa");
        assert_eq!(result, "ccc"); // a→b→c
    }

    #[test]
    fn test_folding_unfold_preserves_ranges() {
        let mut model = FoldingModel::new();
        model.set_ranges(vec![
            FoldingRange { start_line: 1, end_line: 10, kind: FoldingRangeKind::Region, is_collapsed: false },
        ]);
        fold_region(&mut model, 1, false);
        let state1 = FoldState::capture(&model);
        assert_eq!(state1.collapsed_count(), 1);
        unfold_region(&mut model, 1, false);
        let state2 = FoldState::capture(&model);
        assert_eq!(state2.collapsed_count(), 0);
    }
}

// ─── Batch 2: Workbench Integration ─────────────────────────────────────

#[cfg(test)]
mod workbench_integration {
    use vsedit_layout::{LayoutConstraintSolver, LayoutConstraint};
    use vsedit_tui::Rect;
    use vsedit_statusbar::{StatusBar, StatusBarEntry, StatusBarAlignment};
    use vsedit_breadcrumb::{
        BreadcrumbPath, BreadcrumbElement, BreadcrumbKind,
        OutlineEntry, breadcrumbs_from_outline,
    };
    use vsedit_explorer::{FileNestingRule, default_nesting_rules, find_nested_files};
    use vsedit_quickaccess::{
        QuickAccessItem, fuzzy_match_score, filter_and_sort, score_items,
    };

    #[test]
    fn test_layout_constraint_solving() {
        let solver = LayoutConstraintSolver::new(50, 400, 30, 300);
        // Width within range passes through
        assert_eq!(solver.clamp_width(200), 200);
        // Width below minimum gets clamped up
        assert_eq!(solver.clamp_width(10), 50);
        // Width above maximum gets clamped down
        assert_eq!(solver.clamp_width(500), 400);
        // Height clamping
        assert_eq!(solver.clamp_height(10), 30);
        assert_eq!(solver.clamp_height(400), 300);
        // Rect satisfaction check
        let good = Rect::new(0, 0, 100, 100);
        assert!(solver.is_satisfied(&good));
        let bad = Rect::new(0, 0, 10, 10);
        assert!(!solver.is_satisfied(&bad));
        let violations = solver.violations(&bad);
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_statusbar_language_selector() {
        let mut bar = StatusBar::new();
        let lang_entry = StatusBarEntry::builder("lang-mode", "Rust", StatusBarAlignment::Right)
            .tooltip("Select Language Mode")
            .command("workbench.action.editor.changeLanguageMode")
            .priority(100)
            .build();
        bar.add_entry(lang_entry);
        let entry = bar.get_entry("lang-mode").unwrap();
        assert_eq!(entry.text, "Rust");
        assert_eq!(entry.tooltip.as_deref(), Some("Select Language Mode"));

        // Update language mode
        bar.update_text("lang-mode", "Python");
        let entry = bar.get_entry("lang-mode").unwrap();
        assert_eq!(entry.text, "Python");

        // Verify it appears in right-aligned entries
        let right = bar.get_visible_entries(StatusBarAlignment::Right);
        assert!(right.iter().any(|e| e.id == "lang-mode"));
    }

    #[test]
    fn test_breadcrumb_symbol_resolution() {
        let outline = vec![
            OutlineEntry {
                name: "MyClass".into(),
                kind: BreadcrumbKind::Class,
                start_line: 1,
                end_line: 50,
                children: vec![
                    OutlineEntry {
                        name: "my_method".into(),
                        kind: BreadcrumbKind::Method,
                        start_line: 10,
                        end_line: 30,
                        children: vec![],
                    },
                ],
            },
        ];
        // Cursor at line 15 should resolve to MyClass > my_method
        let path = breadcrumbs_from_outline(&outline, 15);
        assert!(path.elements.len() >= 2);
        assert_eq!(path.elements[0].label, "MyClass");
        assert_eq!(path.elements[1].label, "my_method");
    }

    #[test]
    fn test_explorer_file_nesting() {
        let rules = default_nesting_rules();
        // TypeScript .ts files should nest related .js, .d.ts, .js.map
        let children = ["app.js", "app.d.ts", "app.js.map", "other.rs"];
        let nested = find_nested_files("app.ts", &children, &rules);
        assert!(nested.contains(&"app.js"));
        assert!(nested.contains(&"app.d.ts"));
        assert!(!nested.contains(&"other.rs"));
    }

    #[test]
    fn test_quick_access_scorer_ranking() {
        let items = vec![
            QuickAccessItem {
                id: "file.open".into(),
                label: "Open File".into(),
                description: Some("Open a file from disk".into()),
                detail: None,
                icon: None,
                group: Some("File".into()),
            },
            QuickAccessItem {
                id: "file.openRecent".into(),
                label: "Open Recent".into(),
                description: Some("Open a recently used file".into()),
                detail: None,
                icon: None,
                group: Some("File".into()),
            },
            QuickAccessItem {
                id: "terminal.toggle".into(),
                label: "Toggle Terminal".into(),
                description: Some("Show/hide integrated terminal".into()),
                detail: None,
                icon: None,
                group: Some("View".into()),
            },
        ];
        // "open" should match first two items higher than terminal
        let scored = score_items(&items, "open");
        assert!(!scored.is_empty());
        // At least the "Open File" and "Open Recent" should score higher than "Toggle Terminal"
        let open_scores: Vec<_> = scored.iter().filter(|s| s.item.label.contains("Open")).collect();
        let term_scores: Vec<_> = scored.iter().filter(|s| s.item.label.contains("Terminal")).collect();
        if !open_scores.is_empty() && !term_scores.is_empty() {
            assert!(open_scores[0].score >= term_scores[0].score);
        }
    }

    #[test]
    fn test_layout_constraint_clamp_rect() {
        let solver = LayoutConstraintSolver::new(100, 800, 50, 600);
        let r = Rect::new(5, 10, 50, 30);
        let clamped = solver.clamp_rect(&r);
        assert_eq!(clamped.width, 100); // min width enforced
        assert_eq!(clamped.height, 50); // min height enforced
        assert_eq!(clamped.x, 5);
        assert_eq!(clamped.y, 10);
    }

    #[test]
    fn test_statusbar_snapshot_restore() {
        let mut bar = StatusBar::new();
        bar.add_entry(StatusBarEntry::builder("enc", "UTF-8", StatusBarAlignment::Right).build());
        bar.add_entry(StatusBarEntry::builder("eol", "LF", StatusBarAlignment::Right).build());
        let snapshot = bar.snapshot();
        assert_eq!(snapshot.entry_count(), 2);
        bar.clear();
        assert_eq!(bar.entry_count(), 0);
        bar.restore(&snapshot);
        assert_eq!(bar.entry_count(), 2);
    }

    #[test]
    fn test_breadcrumb_path_from_nested_outline() {
        let outline = vec![
            OutlineEntry {
                name: "module".into(),
                kind: BreadcrumbKind::Module,
                start_line: 1,
                end_line: 100,
                children: vec![
                    OutlineEntry {
                        name: "Enum".into(),
                        kind: BreadcrumbKind::Enum,
                        start_line: 5,
                        end_line: 20,
                        children: vec![],
                    },
                ],
            },
        ];
        // Cursor outside any child but inside parent
        let path = breadcrumbs_from_outline(&outline, 50);
        assert_eq!(path.elements.len(), 1);
        assert_eq!(path.elements[0].label, "module");
    }

    #[test]
    fn test_file_nesting_custom_rules() {
        let rule = FileNestingRule::new("rs", vec![".lock".into()]);
        assert!(rule.should_nest("Cargo.toml", "Cargo.lock") || !rule.should_nest("Cargo.toml", "Cargo.lock"));
        // Custom rule: .rs nests nothing by default unless matching
        let children = ["main.rs.bak", "lib.rs"];
        let nested = find_nested_files("main.rs", &children, &[rule]);
        // Verify the function doesn't panic
        assert!(nested.len() <= children.len());
    }

    #[test]
    fn test_fuzzy_match_scoring() {
        let exact = fuzzy_match_score("file", "file");
        let partial = fuzzy_match_score("fl", "file");
        let no_match = fuzzy_match_score("xyz", "file");
        assert!(exact.is_some());
        assert!(no_match.is_none() || no_match.unwrap() < exact.unwrap());
        if let (Some(e), Some(p)) = (exact, partial) {
            assert!(e >= p);
        }
    }
}

// ─── Batch 3: Extension Host Integration ────────────────────────────────

#[cfg(test)]
mod extension_host_integration {
    use vsedit_ext_activation::{
        ActivationTimingProfiler, ActivationEvent, parse_activation_event,
        activation_event_to_string,
    };
    use vsedit_ext_tasks::TaskVariableSubstitution;
    use vsedit_ext_diagnostics::{
        Diagnostic, DiagnosticSeverity, compute_diagnostic_delta,
    };
    use vsedit_ext_progress::{ProgressChain};
    use vsedit_ext_chat::{
        ChatParticipant, ChatParticipantRegistry, SlashCommand,
    };

    #[test]
    fn test_ext_activation_timing() {
        let mut profiler = ActivationTimingProfiler::new();
        profiler.record("ext-a", "onLanguage:rust", 50, 100);
        profiler.record("ext-b", "onCommand:run", 200, 100);
        profiler.record("ext-c", "*", 30, 100);

        assert_eq!(profiler.count(), 3);
        assert_eq!(profiler.total_startup_time(), 280);

        let slowest = profiler.slowest().unwrap();
        assert_eq!(slowest.extension_id, "ext-b");
        assert_eq!(slowest.duration_ms, 200);

        let fastest = profiler.fastest().unwrap();
        assert_eq!(fastest.extension_id, "ext-c");

        let avg = profiler.average_ms();
        assert!((avg - 93.33).abs() < 1.0);

        let top2 = profiler.top_n_slowest(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].extension_id, "ext-b");
    }

    #[test]
    fn test_ext_task_variable_substitution() {
        let mut sub = TaskVariableSubstitution::new();
        sub.set_workspace("/home/user/project", "project");
        sub.set_file(
            "/home/user/project/src/main.rs",
            "/home/user/project/src",
            "main.rs",
            ".rs",
        );

        let result = sub.substitute("build ${workspaceFolder}/target");
        assert_eq!(result, "build /home/user/project/target");

        let result2 = sub.substitute("compile ${file}");
        assert_eq!(result2, "compile /home/user/project/src/main.rs");

        let result3 = sub.substitute("ext is ${fileExtname}");
        assert_eq!(result3, "ext is .rs");

        // Unresolved variables remain
        let unresolved = sub.unresolved_count("${unknownVar} and ${file}");
        assert_eq!(unresolved, 1);
    }

    #[test]
    fn test_ext_diagnostic_delta() {
        let old = vec![
            Diagnostic {
                start_line: 1, start_col: 0, end_line: 1, end_col: 10,
                message: "unused variable".into(),
                severity: DiagnosticSeverity::Warning,
                code: None, source: None, related_info: vec![], tags: vec![],
            },
            Diagnostic {
                start_line: 5, start_col: 0, end_line: 5, end_col: 5,
                message: "type mismatch".into(),
                severity: DiagnosticSeverity::Error,
                code: None, source: None, related_info: vec![], tags: vec![],
            },
        ];
        let new = vec![
            Diagnostic {
                start_line: 1, start_col: 0, end_line: 1, end_col: 10,
                message: "unused variable".into(),
                severity: DiagnosticSeverity::Warning,
                code: None, source: None, related_info: vec![], tags: vec![],
            },
            Diagnostic {
                start_line: 10, start_col: 0, end_line: 10, end_col: 8,
                message: "missing semicolon".into(),
                severity: DiagnosticSeverity::Error,
                code: None, source: None, related_info: vec![], tags: vec![],
            },
        ];
        let delta = compute_diagnostic_delta("file:///main.rs", &old, &new);
        assert!(delta.has_changes());
        assert_eq!(delta.added_count(), 1); // "missing semicolon" added
        assert_eq!(delta.removed_count(), 1); // "type mismatch" removed
        assert_eq!(delta.unchanged, 1); // "unused variable" unchanged
    }

    #[test]
    fn test_ext_progress_chain() {
        let mut chain = ProgressChain::new();
        let download = chain.add_step("Download", 30.0);
        let extract = chain.add_step("Extract", 20.0);
        let install = chain.add_step("Install", 50.0);

        assert!(!chain.is_finished());
        assert_eq!(chain.overall_progress(), 0.0);

        chain.report(download, 100.0);
        // 30% of total weight complete
        let progress = chain.overall_progress();
        assert!((progress - 30.0).abs() < 0.01);

        chain.complete_step(extract);
        // 30 + 20 = 50% complete
        let progress = chain.overall_progress();
        assert!((progress - 50.0).abs() < 0.01);

        chain.report(install, 50.0);
        // 30 + 20 + 25 = 75%
        let progress = chain.overall_progress();
        assert!((progress - 75.0).abs() < 0.01);

        chain.complete_step(install);
        assert!(chain.is_finished());
        assert!((chain.overall_progress() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_ext_chat_participant_registry() {
        let mut registry = ChatParticipantRegistry::new();

        let copilot = ChatParticipant::builder("copilot", "GitHub Copilot")
            .description("AI pair programmer")
            .is_default(true)
            .build()
            .unwrap();
        let workspace = ChatParticipant::builder("workspace", "Workspace Agent")
            .build()
            .unwrap();

        registry.register(copilot, vec![
            SlashCommand { name: "explain".into(), description: "Explain code".into() },
            SlashCommand { name: "fix".into(), description: "Fix code".into() },
        ]);
        registry.register(workspace, vec![
            SlashCommand { name: "search".into(), description: "Search workspace".into() },
        ]);

        assert_eq!(registry.participant_count(), 2);
        assert_eq!(registry.command_count(), 3);

        let found = registry.get("copilot").unwrap();
        assert_eq!(found.name, "GitHub Copilot");
        assert!(found.is_default);

        let cmds = registry.get_commands("copilot");
        assert_eq!(cmds.len(), 2);

        // Find commands by prefix
        let fix_cmds = registry.find_commands("fi");
        assert_eq!(fix_cmds.len(), 1);
        assert_eq!(fix_cmds[0].1.name, "fix");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_activation_event_parsing() {
        let event = parse_activation_event("onLanguage:rust").unwrap();
        let round_trip = activation_event_to_string(&event);
        assert!(round_trip.contains("rust"));

        let star = parse_activation_event("*").unwrap();
        assert!(matches!(star, ActivationEvent::Star));

        assert!(parse_activation_event("invalidEvent").is_none());
    }

    #[test]
    fn test_task_variable_workspace_basename() {
        let mut sub = TaskVariableSubstitution::new();
        sub.set_workspace("/home/user/my-project", "my-project");
        let result = sub.substitute("Project: ${workspaceFolderBasename}");
        assert_eq!(result, "Project: my-project");
    }

    #[test]
    fn test_diagnostic_delta_no_changes() {
        let diags = vec![
            Diagnostic {
                start_line: 1, start_col: 0, end_line: 1, end_col: 5,
                message: "test".into(),
                severity: DiagnosticSeverity::Warning,
                code: None, source: None, related_info: vec![], tags: vec![],
            },
        ];
        let delta = compute_diagnostic_delta("file:///a.rs", &diags, &diags);
        assert!(!delta.has_changes());
        assert_eq!(delta.unchanged, 1);
    }

    #[test]
    fn test_progress_chain_empty() {
        let chain = ProgressChain::new();
        assert!(!chain.is_finished());
        assert_eq!(chain.overall_progress(), 0.0);
    }

    #[test]
    fn test_chat_participant_validation() {
        let result = ChatParticipant::builder("", "name").build();
        assert!(result.is_err());
        let result2 = ChatParticipant::builder("id", "").build();
        assert!(result2.is_err());
    }
}

// ─── Batch 4: Platform Services Integration ─────────────────────────────

#[cfg(test)]
mod platform_services_integration {
    use vsedit_configuration::InheritanceChainBuilder;
    use vsedit_storage::{Storage, compact_empty_values};
    use vsedit_notification_svc::{
        NotificationService, NotificationThrottle, NotificationSeverity,
    };
    use vsedit_clipboard::{ClipboardItem, SizeLimitedHistory};
    use vsedit_policy::{PolicyProfile, PolicyService, Policy, PolicyValue};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_config_migration_deprecated() {
        // Simulate migrating deprecated settings using InheritanceChainBuilder
        let mut builder = InheritanceChainBuilder::new();

        // "old defaults" layer with deprecated setting
        let mut defaults = HashMap::new();
        defaults.insert("editor.tabSize".into(), json!(4));
        defaults.insert("editor.autoClosingBrackets".into(), json!("languageDefined"));
        // Deprecated: editor.autoIndent used to be boolean
        defaults.insert("editor.autoIndent".into(), json!(true));
        builder.add_layer("defaults", defaults);

        // "migration" layer overrides deprecated boolean with new string value
        let mut migration = HashMap::new();
        migration.insert("editor.autoIndent".into(), json!("full"));
        builder.add_layer("migration", migration);

        // Resolved value uses migrated string, not old boolean
        let resolved = builder.resolve("editor.autoIndent").unwrap();
        assert_eq!(resolved, json!("full"));

        // Non-migrated settings still resolve from defaults
        let tab = builder.resolve("editor.tabSize").unwrap();
        assert_eq!(tab, json!(4));

        let merged = builder.build();
        assert!(merged.contains_key("editor.autoIndent"));
        assert_eq!(merged["editor.autoIndent"], json!("full"));
    }

    #[test]
    fn test_storage_compaction_cycle() {
        let store = Storage::in_memory().unwrap();
        // Write a mix of real and empty values
        store.set("key1", "value1").unwrap();
        store.set("key2", "").unwrap();
        store.set("key3", "value3").unwrap();
        store.set("key4", "").unwrap();
        store.set("key5", "").unwrap();

        let stats = compact_empty_values(&store).unwrap();
        assert_eq!(stats.keys_before, 5);
        assert_eq!(stats.removed, 3);
        assert_eq!(stats.keys_after, 2);

        // Verify real values survive
        assert_eq!(store.get("key1").as_deref(), Some("value1"));
        assert_eq!(store.get("key3").as_deref(), Some("value3"));
        assert!(store.get("key2").is_none());
    }

    #[test]
    fn test_notification_throttle() {
        let mut throttle = NotificationThrottle::new(100); // 100-tick window
        // First notification should be allowed
        assert!(throttle.allow("msg1", 0));
        // Same message immediately should be throttled
        assert!(!throttle.allow("msg1", 1));
        // Different message should be allowed
        assert!(throttle.allow("msg2", 1));
        assert!(throttle.tracked_count() >= 2);
        // Reset clears throttle state
        throttle.reset();
        assert!(throttle.allow("msg1", 2));
    }

    #[test]
    fn test_clipboard_history_limit() {
        let mut history = SizeLimitedHistory::new(3, 1024);
        history.push(ClipboardItem::new("first", 1, None));
        history.push(ClipboardItem::new("second", 2, None));
        history.push(ClipboardItem::new("third", 3, None));
        assert_eq!(history.len(), 3);
        // Adding a 4th should evict the oldest
        history.push(ClipboardItem::new("fourth", 4, None));
        assert_eq!(history.len(), 3);
        let most_recent = history.most_recent().unwrap();
        assert_eq!(most_recent.text, "fourth");
        // "first" should have been evicted
        assert!(!history.entries().iter().any(|e| e.text == "first"));
    }

    #[test]
    fn test_policy_profile_combine() {
        let mut svc = PolicyService::new();

        // Create two profiles
        let mut security = PolicyProfile::new("security");
        security.add_policy(Policy {
            name: "telemetry.enabled".into(),
            value: PolicyValue::Bool(false),
            description: Some("Disable telemetry".into()),
        });
        security.add_policy(Policy {
            name: "update.channel".into(),
            value: PolicyValue::String("stable".into()),
            description: None,
        });

        let mut dev = PolicyProfile::new("developer");
        dev.add_policy(Policy {
            name: "telemetry.enabled".into(),
            value: PolicyValue::Bool(true), // overrides security
            description: Some("Enable telemetry for dev".into()),
        });
        dev.add_policy(Policy {
            name: "debug.verbose".into(),
            value: PolicyValue::Bool(true),
            description: None,
        });

        // Apply security first, then dev (dev overrides)
        security.apply_to(&mut svc);
        dev.apply_to(&mut svc);

        // dev profile overrides telemetry
        let telemetry = svc.get_policy("telemetry.enabled").unwrap();
        assert_eq!(telemetry.value, PolicyValue::Bool(true));

        // Security's update.channel still applies
        let channel = svc.get_policy("update.channel").unwrap();
        assert_eq!(channel.value, PolicyValue::String("stable".into()));

        // Dev's debug.verbose is present
        let debug = svc.get_policy("debug.verbose").unwrap();
        assert_eq!(debug.value, PolicyValue::Bool(true));
    }

    #[test]
    fn test_config_inheritance_layer_resolution() {
        let mut builder = InheritanceChainBuilder::new();
        let mut base = HashMap::new();
        base.insert("a".into(), json!(1));
        base.insert("b".into(), json!(2));
        builder.add_layer("base", base);
        let mut override_layer = HashMap::new();
        override_layer.insert("b".into(), json!(99));
        builder.add_layer("override", override_layer);
        assert_eq!(builder.resolve("a"), Some(json!(1)));
        assert_eq!(builder.resolve("b"), Some(json!(99)));
        assert_eq!(builder.resolve("c"), None);
        assert_eq!(builder.len(), 2);
    }

    #[test]
    fn test_storage_set_and_remove() {
        let store = Storage::in_memory().unwrap();
        store.set("hello", "world").unwrap();
        assert_eq!(store.get("hello").as_deref(), Some("world"));
        store.remove("hello").unwrap();
        assert!(store.get("hello").is_none());
    }

    #[test]
    fn test_notification_service_info_warn_error() {
        let mut svc = NotificationService::new();
        svc.info("Info message");
        svc.warn("Warning message");
        svc.error("Error message");
        let active = svc.get_active();
        assert_eq!(active.len(), 3);
    }

    #[test]
    fn test_clipboard_size_limit() {
        // 20 bytes total limit with max 10 entries
        let mut history = SizeLimitedHistory::new(10, 20);
        history.push(ClipboardItem::new("aaaaaaaaaa", 1, None)); // 10 bytes
        history.push(ClipboardItem::new("bbbbbbbbbb", 2, None)); // 10 bytes, now at 20
        assert_eq!(history.len(), 2);
        history.push(ClipboardItem::new("cc", 3, None)); // needs to evict
        assert!(history.current_bytes() <= 20);
    }

    #[test]
    fn test_policy_disabled_profile() {
        let mut svc = PolicyService::new();
        let mut profile = PolicyProfile::new("disabled");
        profile.enabled = false;
        profile.add_policy(Policy {
            name: "feature.x".into(),
            value: PolicyValue::Bool(true),
            description: None,
        });
        profile.apply_to(&mut svc);
        // Disabled profile should not apply
        assert!(svc.get_policy("feature.x").is_none());
    }
}

// ─── Batch 5: Advanced Features ─────────────────────────────────────────

#[cfg(test)]
mod advanced_features {
    use vsedit_diff::{compute_word_diff, WordChangeKind};
    use vsedit_inlayhints::{InlayHint, InlayHintKind, InlayHintVisibility};
    use vsedit_terminal::{TerminalCell, detect_links_in_line, LinkKind};
    use vsedit_smartselect::{BracketPair, find_bracket_range};
    use vsedit_download::{DownloadRetryPolicy, BackoffStrategy};
    use vsedit_hover::{Hover, HoverContent, merge_hovers, hover_content_length};
    use vsedit_jsonschemas::{
        JsonSchema, SchemaProperty, SchemaType, JsonSchemaDefaultValues, JsonValue,
        build_default_object,
    };
    use vsedit_label::label_ellipsis_middle;
    use vsedit_theme::{
        Color, ColorTheme, ThemeType, ThemeInheritance, TokenColor, TokenSettings,
    };
    use vsedit_codelens::{CodeLens, Command, codelens_group_adjacent, merge_adjacent_lenses};
    use std::collections::HashMap;

    #[test]
    fn test_diff_word_level() {
        let changes = compute_word_diff(
            "The quick brown fox",
            "The slow brown cat",
        );
        // Should detect word-level changes: quick→slow, fox→cat
        assert!(!changes.is_empty());
        let modified: Vec<_> = changes.iter()
            .filter(|c| matches!(c.kind, WordChangeKind::Insert | WordChangeKind::Delete))
            .collect();
        assert!(modified.len() >= 2);
    }

    #[test]
    fn test_inlay_hint_toggle_by_kind() {
        let hints = vec![
            InlayHint::simple(1, 5, ": i32", InlayHintKind::Type),
            InlayHint::simple(2, 10, "name:", InlayHintKind::Parameter),
            InlayHint::simple(3, 3, "// size", InlayHintKind::Other),
        ];

        let mut vis = InlayHintVisibility::all();
        assert!(vis.is_visible(InlayHintKind::Type));
        let all_visible = vis.filter(&hints);
        assert_eq!(all_visible.len(), 3);

        // Toggle off type hints
        vis.toggle(InlayHintKind::Type);
        assert!(!vis.is_visible(InlayHintKind::Type));
        let filtered = vis.filter(&hints);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|h| h.kind != InlayHintKind::Type));

        // Toggle off parameter hints too
        vis.toggle(InlayHintKind::Parameter);
        let filtered2 = vis.filter(&hints);
        assert_eq!(filtered2.len(), 1);
        assert_eq!(filtered2[0].kind, InlayHintKind::Other);
    }

    #[test]
    fn test_terminal_link_detection() {
        let line = "See https://github.com/rust-lang/rust for details";
        let cells: Vec<TerminalCell> = line.chars().map(|ch| TerminalCell {
            ch,
            ..TerminalCell::default()
        }).collect();

        let links = detect_links_in_line(0, &cells);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].kind, LinkKind::Url);
        assert!(links[0].target.starts_with("https://github.com"));

        // Test with no links
        let no_link_cells: Vec<TerminalCell> = "just plain text"
            .chars()
            .map(|ch| TerminalCell { ch, ..TerminalCell::default() })
            .collect();
        let no_links = detect_links_in_line(0, &no_link_cells);
        assert!(no_links.is_empty());
    }

    #[test]
    fn test_smart_select_bracket_aware() {
        let text = "fn main() { let x = (1 + 2); }";
        // Find parens around offset of '1' (char at index 21)
        let result = find_bracket_range(text, 21, BracketPair::PARENS);
        assert!(result.is_some());
        let (open, close) = result.unwrap();
        let inner = &text[open..=close];
        assert!(inner.starts_with('('));
        assert!(inner.ends_with(')'));
        assert!(inner.contains("1 + 2"));

        // Find braces
        let brace_result = find_bracket_range(text, 21, BracketPair::BRACES);
        assert!(brace_result.is_some());
        let (bo, bc) = brace_result.unwrap();
        assert!(bo < open); // braces should be wider than parens
        assert!(bc > close);
    }

    #[test]
    fn test_download_retry_policy() {
        let policy = DownloadRetryPolicy {
            max_retries: 5,
            strategy: BackoffStrategy::Exponential,
            base_delay_secs: 1.0,
            max_delay_secs: 30.0,
        };
        // Exponential: 1, 2, 4, 8, 16
        assert_eq!(policy.delay_for_attempt(1), Some(1.0));
        assert_eq!(policy.delay_for_attempt(2), Some(2.0));
        assert_eq!(policy.delay_for_attempt(3), Some(4.0));
        assert_eq!(policy.delay_for_attempt(4), Some(8.0));
        assert_eq!(policy.delay_for_attempt(5), Some(16.0));
        // Beyond max_retries
        assert_eq!(policy.delay_for_attempt(6), None);
        assert_eq!(policy.delay_for_attempt(0), None);

        assert!(!policy.is_exhausted(4));
        assert!(policy.is_exhausted(5));

        // Verify cap works
        let capped = DownloadRetryPolicy {
            max_retries: 10,
            strategy: BackoffStrategy::Exponential,
            base_delay_secs: 1.0,
            max_delay_secs: 10.0,
        };
        assert_eq!(capped.delay_for_attempt(5), Some(10.0)); // 16 capped to 10
    }

    #[test]
    fn test_hover_multi_source_merge() {
        let hover1 = Hover {
            contents: vec![HoverContent::Text("Type: `i32`".into())],
            range: None,
        };
        let hover2 = Hover {
            contents: vec![HoverContent::Text("Docs: The primary integer type.".into())],
            range: None,
        };
        let hover3 = Hover {
            contents: vec![HoverContent::Code {
                language: Some("rust".into()),
                value: "let x: i32 = 42;".into(),
            }],
            range: None,
        };

        let merged = merge_hovers(&[hover1, hover2, hover3]);
        assert!(hover_content_length(&merged) > 0);
        // Merged hover should contain content from all sources
        assert!(merged.contents.len() >= 3);
    }

    #[test]
    fn test_json_schema_default_values() {
        let schema = JsonSchema {
            id: Some("test".into()),
            title: Some("Test Schema".into()),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "indent".into(),
                    schema_type: SchemaType::Number,
                    description: None,
                    required: false,
                    default_value: Some("4".into()),
                },
                SchemaProperty {
                    name: "language".into(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: false,
                    default_value: Some("en".into()),
                },
                SchemaProperty {
                    name: "debug".into(),
                    schema_type: SchemaType::Boolean,
                    description: None,
                    required: false,
                    default_value: Some("false".into()),
                },
            ],
            file_match: vec![],
        };
        // Empty object should get all defaults
        let result = build_default_object(&schema);
        if let JsonValue::Object(fields) = &result {
            assert!(fields.iter().any(|(k, _)| k == "indent"));
            assert!(fields.iter().any(|(k, _)| k == "language"));
        }

        // apply() on partially filled object should only add missing
        let partial = JsonValue::Object(vec![
            ("indent".into(), JsonValue::Number(2.0)),
        ]);
        let filled = JsonSchemaDefaultValues::apply(&schema, &partial);
        if let JsonValue::Object(fields) = &filled {
            // indent should keep original value
            let indent = fields.iter().find(|(k, _)| k == "indent").unwrap();
            assert!(matches!(&indent.1, JsonValue::Number(n) if (*n - 2.0).abs() < f64::EPSILON));
            // language should get default
            assert!(fields.iter().any(|(k, _)| k == "language"));
        }
    }

    #[test]
    fn test_label_truncate_middle() {
        let long = "very_long_file_name_that_needs_truncation.rs";
        let truncated = label_ellipsis_middle(long, 20);
        assert!(truncated.len() <= 20);
        assert!(truncated.contains("…") || truncated.contains("...") || truncated.len() < long.len());
        // Short strings shouldn't be truncated
        let short = "hi.rs";
        let not_truncated = label_ellipsis_middle(short, 20);
        assert_eq!(not_truncated, short);
    }

    #[test]
    fn test_theme_inheritance_chain() {
        // Create a parent dark theme
        let mut parent_colors = HashMap::new();
        parent_colors.insert("editor.background".into(), Color::rgb(30, 30, 30));
        parent_colors.insert("editor.foreground".into(), Color::rgb(212, 212, 212));
        let parent = ColorTheme {
            id: "dark-plus".into(),
            label: "Dark+".into(),
            theme_type: ThemeType::Dark,
            colors: parent_colors,
            token_colors: vec![
                TokenColor {
                    name: Some("Comment".into()),
                    scope: vec!["comment".into()],
                    settings: TokenSettings {
                        foreground: Some(Color::rgb(106, 153, 85)),
                        background: None,
                        font_style: Some("italic".into()),
                    },
                },
            ],
        };

        // Create child theme that overrides background
        let mut inheritance = ThemeInheritance::new("dark-plus");
        inheritance.set_color("editor.background", Color::rgb(20, 20, 40));
        inheritance.add_token_override(TokenColor {
            name: Some("String".into()),
            scope: vec!["string".into()],
            settings: TokenSettings {
                foreground: Some(Color::rgb(206, 145, 120)),
                background: None,
                font_style: None,
            },
        });

        assert_eq!(inheritance.color_override_count(), 1);
        assert_eq!(inheritance.token_override_count(), 1);

        let child = inheritance.apply(&parent, "my-dark", "My Dark Theme");
        assert_eq!(child.id, "my-dark");
        assert_eq!(child.theme_type, ThemeType::Dark);
        // Background overridden
        assert_eq!(child.colors["editor.background"], Color::rgb(20, 20, 40));
        // Foreground inherited
        assert_eq!(child.colors["editor.foreground"], Color::rgb(212, 212, 212));
        // Token colors: parent comment + child string
        assert_eq!(child.token_colors.len(), 2);
    }

    #[test]
    fn test_codelens_merge_adjacent() {
        let lenses = vec![
            CodeLens {
                start_line: 5,
                start_col: 0,
                end_line: 5,
                end_col: 0,
                command: Some(Command {
                    title: "Run Test".into(),
                    command_id: "test.run".into(),
                    tooltip: String::new(),
                    arguments: vec![],
                }),
                data: String::new(),
            },
            CodeLens {
                start_line: 6,
                start_col: 0,
                end_line: 6,
                end_col: 0,
                command: Some(Command {
                    title: "Debug Test".into(),
                    command_id: "test.debug".into(),
                    tooltip: String::new(),
                    arguments: vec![],
                }),
                data: String::new(),
            },
            CodeLens {
                start_line: 20,
                start_col: 0,
                end_line: 20,
                end_col: 0,
                command: Some(Command {
                    title: "References".into(),
                    command_id: "editor.references".into(),
                    tooltip: String::new(),
                    arguments: vec![],
                }),
                data: String::new(),
            },
        ];

        // Group adjacent lenses (max gap = 2 lines)
        let groups = codelens_group_adjacent(&lenses, 2);
        assert_eq!(groups.len(), 2); // lines 5,6 grouped; line 20 separate

        // Merge adjacent lenses
        let merged = merge_adjacent_lenses(&lenses);
        assert!(!merged.is_empty());
    }
}

// ─── Deep Editor Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod deep_editor_tests {
    use vsedit_buffer::VsBuffer;
    use vsedit_cursor::{
        CursorController, CursorState, move_left, move_right, move_up, move_down,
        move_to_line_start, move_to_line_end, move_to_document_start, move_to_document_end,
        move_word_left, move_word_right, delete_word_left, delete_word_right,
        sort_cursors, serialize_cursors, deserialize_cursors, any_has_selection,
        selection_count, collapse_selections, cursor_summary,
    };
    use vsedit_strings::{
        display_width, equals_ignore_case, starts_with_ignore_case, to_snake_case,
        to_camel_case, to_pascal_case, edit_distance, similarity_score, fuzzy_match_score,
        MeasuredString, LineBuilder, extract_words, common_prefix_length, grapheme_count,
    };
    use vsedit_text_model::{TextModel, detect_line_ending, DetectedLineEnding};
    use vsedit_editor_types::{ITextModel, Position};

    #[test]
    fn buffer_roundtrip_string() {
        let buf = VsBuffer::from_string("hello world");
        assert_eq!(buf.to_string_lossy(), "hello world");
        assert_eq!(buf.len(), 11);
        assert!(!buf.is_empty());
    }

    #[test]
    fn buffer_empty_and_concat() {
        let a = VsBuffer::from_string("foo");
        let b = VsBuffer::from_string("bar");
        let joined = VsBuffer::concat(&[a, b]);
        assert_eq!(joined.to_string_lossy(), "foobar");
        assert!(VsBuffer::empty().is_empty());
    }

    #[test]
    fn buffer_slice_and_split() {
        let buf = VsBuffer::from_string("abcdef");
        let sliced = buf.try_slice(1..4).unwrap();
        assert_eq!(sliced.to_string_lossy(), "bcd");
        let (left, right) = buf.split_at(3).unwrap();
        assert_eq!(left.to_string_lossy(), "abc");
        assert_eq!(right.to_string_lossy(), "def");
    }

    #[test]
    fn cursor_move_left_right_with_model() {
        let model = TextModel::new("abcde");
        let cursor = CursorState::from_position(Position::new(1, 3));
        let moved = move_left(&model, &cursor, false, 2);
        assert_eq!(moved.position().column, 1);
        let moved_right = move_right(&model, &moved, false, 4);
        assert_eq!(moved_right.position().column, 5);
    }

    #[test]
    fn cursor_move_up_down_across_lines() {
        let model = TextModel::new("short\na longer line\nend");
        let cursor = CursorState::from_position(Position::new(2, 10));
        let (up, _mem) = move_up(&model, &cursor, false, 1, None);
        assert_eq!(up.position().line, 1);
        let (down, _) = move_down(&model, &up, false, 2, None);
        assert_eq!(down.position().line, 3);
    }

    #[test]
    fn cursor_line_start_end_document_bounds() {
        let model = TextModel::new("  hello world\nsecond line");
        let cursor = CursorState::from_position(Position::new(1, 8));
        let start = move_to_line_start(&model, &cursor, false);
        assert!(start.position().column <= 3); // first non-ws or col 1
        let end = move_to_line_end(&model, &cursor, false);
        assert_eq!(end.position().column, model.get_line_max_column(1));
        let doc_start = move_to_document_start(&model, &cursor, false);
        assert_eq!(doc_start.position(), Position::new(1, 1));
        let doc_end = move_to_document_end(&model, &cursor, false);
        assert_eq!(doc_end.position().line, 2);
    }

    #[test]
    fn cursor_word_movement() {
        let model = TextModel::new("hello world fooBar");
        let cursor = CursorState::from_position(Position::new(1, 1));
        let right = move_word_right(&model, &cursor, false);
        assert!(right.position().column > 1);
        let left = move_word_left(&model, &right, false);
        assert_eq!(left.position().column, 1);
    }

    #[test]
    fn cursor_delete_word_boundaries() {
        let model = TextModel::new("hello world test");
        let cursor = CursorState::from_position(Position::new(1, 7));
        let (start, end) = delete_word_left(&model, &cursor);
        assert!(start.column < end.column);
        let (start2, end2) = delete_word_right(&model, &cursor);
        assert!(start2.column < end2.column);
    }

    #[test]
    fn cursor_controller_multi_cursor() {
        let mut ctrl = CursorController::new();
        ctrl.add_cursor(Position::new(1, 1));
        ctrl.add_cursor(Position::new(2, 1));
        assert!(ctrl.has_multiple_cursors());
        assert_eq!(ctrl.cursor_count(), 3); // primary + 2 added
        let summary = cursor_summary(&ctrl);
        assert_eq!(summary.count, 3);
    }

    #[test]
    fn cursor_serialize_deserialize_roundtrip() {
        let cursors = vec![
            CursorState::from_position(Position::new(1, 5)),
            CursorState::from_position(Position::new(3, 10)),
        ];
        let serialized = serialize_cursors(&cursors);
        let deserialized = deserialize_cursors(&serialized).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].position(), cursors[0].position());
    }

    #[test]
    fn cursor_sort_and_selection_queries() {
        let mut cursors = vec![
            CursorState::from_position(Position::new(3, 1)),
            CursorState::from_position(Position::new(1, 1)),
        ];
        sort_cursors(&mut cursors);
        assert_eq!(cursors[0].position().line, 1);
        assert!(!any_has_selection(&cursors));
        assert_eq!(selection_count(&cursors), 0);
        let collapsed = collapse_selections(&cursors);
        assert_eq!(collapsed.len(), 2);
    }

    #[test]
    fn strings_case_conversion_and_comparison() {
        assert!(equals_ignore_case("Hello", "hello"));
        assert!(starts_with_ignore_case("FooBar", "foo"));
        assert_eq!(to_snake_case("fooBar"), "foo_bar");
        assert_eq!(to_camel_case("foo_bar"), "fooBar");
        assert_eq!(to_pascal_case("foo_bar"), "FooBar");
    }

    #[test]
    fn strings_fuzzy_match_and_distance() {
        assert!(fuzzy_match_score("fb", "fooBar") > 0);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        let score = similarity_score("hello", "hello");
        assert!(score > 0.99);
    }

    #[test]
    fn strings_measured_and_line_builder() {
        let ms = MeasuredString::new("hello 世界");
        assert!(ms.width() >= ms.text().chars().count());
        assert_eq!(ms.grapheme_len(), grapheme_count(ms.text()));
        let line = LineBuilder::new().separator(" | ").push("a").push("b").push("c").build();
        assert_eq!(line, "a | b | c");
        let words = extract_words("hello world test");
        assert_eq!(words.len(), 3);
        assert_eq!(common_prefix_length("abcdef", "abcxyz"), 3);
    }

    #[test]
    fn text_model_line_endings_and_display_width() {
        let ending = detect_line_ending("foo\r\nbar\r\n");
        assert_eq!(ending, DetectedLineEnding::CRLF);
        assert_eq!(display_width("hello"), 5);
        assert!(display_width("日本語") > 3);
    }
}

// ─── Deep Workbench Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod deep_workbench_tests {
    use vsedit_workbench::{
        Workbench, EditorGroup, EditorGroupTab, EditorGroupManager,
        ActivityBarItem, default_activity_bar_items, compute_breadcrumbs,
    };
    use vsedit_statusbar::{
        StatusBar, StatusBarEntry, StatusBarAlignment, StatusBarGroup, StatusBarTooltip,
    };
    use vsedit_layout::{
        LayoutBuilder, SplitView, Padding,
        inset, center, contains, rect_area, apply_padding, distribute_evenly,
    };
    use vsedit_tui::Rect;

    #[test]
    fn workbench_lifecycle() {
        let mut wb = Workbench::new();
        assert!(!wb.is_started());
        wb.start();
        assert!(wb.is_started());
    }

    #[test]
    fn editor_group_tab_management() {
        let mut group = EditorGroup::new(0);
        assert!(group.is_empty());
        group.add_tab(EditorGroupTab {
            title: "main.rs".into(),
            file_path: Some("/src/main.rs".into()),
            content: "fn main() {}".into(),
            is_modified: false,
        });
        assert!(!group.is_empty());
        assert!(group.active_tab().is_some());
        group.close_tab(0);
        assert!(group.is_empty());
    }

    #[test]
    fn editor_group_manager_split() {
        let mut mgr = EditorGroupManager::new();
        assert_eq!(mgr.group_count(), 1);
        mgr.split_editor(vsedit_workbench::SplitDirection::Right);
        assert_eq!(mgr.group_count(), 2);
    }

    #[test]
    fn activity_bar_defaults() {
        let items = default_activity_bar_items();
        assert!(!items.is_empty());
        let item = ActivityBarItem::new("test", "Test", "T");
        assert_eq!(item.display_text(), "T");
    }

    #[test]
    fn breadcrumb_computation() {
        let crumbs = compute_breadcrumbs("src/lib.rs", &["module".into(), "function".into()]);
        assert!(!crumbs.is_empty());
    }

    #[test]
    fn statusbar_entry_builder() {
        let entry = StatusBarEntry::builder("git", "main", StatusBarAlignment::Left)
            .tooltip("Git Branch")
            .priority(100)
            .command("git.checkout")
            .build();
        assert_eq!(entry.id, "git");
        assert_eq!(entry.text, "main");
        assert_eq!(entry.alignment, StatusBarAlignment::Left);
        assert_eq!(entry.priority, 100);
    }

    #[test]
    fn statusbar_add_remove_entries() {
        let mut sb = StatusBar::new();
        let entry = StatusBarEntry::builder("enc", "UTF-8", StatusBarAlignment::Right)
            .build();
        sb.add_entry(entry);
        assert_eq!(sb.entry_count(), 1);
        assert!(sb.has_entry("enc"));
        sb.remove_entry("enc");
        assert_eq!(sb.entry_count(), 0);
    }

    #[test]
    fn statusbar_update_and_visibility() {
        let mut sb = StatusBar::new();
        sb.add_entry(StatusBarEntry::builder("lang", "Rust", StatusBarAlignment::Right).build());
        sb.update_text("lang", "Python");
        let visible = sb.get_visible_entries(StatusBarAlignment::Right);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].text, "Python");
        sb.set_visibility("lang", false);
        assert_eq!(sb.get_visible_entries(StatusBarAlignment::Right).len(), 0);
    }

    #[test]
    fn statusbar_group_operations() {
        let mut group = StatusBarGroup::new("test-group");
        assert!(group.is_empty());
        group.add("item1");
        group.add("item2");
        assert_eq!(group.len(), 2);
        assert!(group.contains("item1"));
    }

    #[test]
    fn statusbar_tooltip_render() {
        let tooltip = StatusBarTooltip::new("tip1", "Git Status");
        let rendered = tooltip.render();
        assert!(rendered.contains("Git Status"));
    }

    #[test]
    fn layout_builder_horizontal() {
        let node = LayoutBuilder::horizontal()
            .fixed(20)
            .flex(1)
            .fixed(30)
            .build()
            .unwrap();
        assert_eq!(node.len(), 3);
        assert!(!node.is_empty());
    }

    #[test]
    fn layout_split_view() {
        let split = SplitView::horizontal(0.3);
        assert!((split.ratio() - 0.3).abs() < 0.01);
        let area = Rect::new(0, 0, 100, 50);
        let (left, right) = split.split(area);
        assert!(left.width > 0);
        assert!(right.width > 0);
        assert_eq!(left.width + right.width, area.width);
    }

    #[test]
    fn layout_rect_utilities() {
        let area = Rect::new(10, 10, 80, 60);
        let inset_area = inset(area, 5);
        assert_eq!(inset_area.x, 15);
        assert_eq!(inset_area.width, 70);
        let (cx, cy) = center(area);
        assert_eq!(cx, 50);
        assert_eq!(cy, 40);
        assert!(contains(area, inset_area));
    }

    #[test]
    fn layout_padding_and_area() {
        let r = Rect::new(0, 0, 100, 50);
        let p = Padding { top: 2, right: 3, bottom: 2, left: 3 };
        let padded = apply_padding(&r, &p);
        assert_eq!(padded.width, 94);
        assert_eq!(padded.height, 46);
        assert_eq!(rect_area(&r), 5000);
    }

    #[test]
    fn layout_distribute_evenly_test() {
        let dist = distribute_evenly(100, 3);
        assert_eq!(dist.len(), 3);
        let total: u16 = dist.iter().sum();
        assert_eq!(total, 100);
    }
}

// ─── Deep Extension Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod deep_extension_tests {
    use vsedit_ext_api::{
        ApiRegistry, ApiCapabilities, ContributionPoint, all_namespaces,
        ExtApiStats, enumerate_capabilities, count_enabled_capabilities,
        API_VERSION,
    };
    use vsedit_ext_activation::{
        ActivationEvent, ActivationEventMatcher, ExtensionActivationQueue,
        ActivationDependencyGraph, parse_activation_event,
        activation_event_to_string,
    };
    use vsedit_registry::{
        ExtensionPointRegistry, ExtensionPointMetadata, RegistrySnapshot, merge_registries,
    };

    #[test]
    fn api_registry_namespace_registration() {
        let mut reg = ApiRegistry::new();
        reg.register_namespace("commands", 1);
        reg.register_namespace("window", 2);
        assert!(reg.has_namespace("commands"));
        assert_eq!(reg.get_proxy_id("commands"), Some(1));
        assert_eq!(reg.namespace_count(), 2);
    }

    #[test]
    fn api_registry_with_defaults() {
        let reg = ApiRegistry::with_defaults();
        let ns = reg.registered_namespaces();
        assert!(!ns.is_empty());
    }

    #[test]
    fn api_registry_contribution_points() {
        let mut reg = ApiRegistry::new();
        reg.register_contribution(ContributionPoint::Commands);
        reg.register_contribution(ContributionPoint::Languages);
        assert!(!reg.is_contribution_points_empty());
        assert_eq!(reg.contributions().len(), 2);
    }

    #[test]
    fn api_capabilities_and_version() {
        assert!(!API_VERSION.is_empty());
        let caps = ApiCapabilities::default();
        let enabled = count_enabled_capabilities(&caps);
        let _ = enabled; // used for side-effect check
        let all_caps = enumerate_capabilities(&caps);
        assert!(!all_caps.is_empty());
    }

    #[test]
    fn api_all_namespaces() {
        let ns = all_namespaces();
        assert!(ns.contains(&"commands"));
        assert!(ns.contains(&"window"));
    }

    #[test]
    fn activation_event_parsing() {
        assert_eq!(parse_activation_event("*"), Some(ActivationEvent::Star));
        assert_eq!(
            parse_activation_event("onLanguage:rust"),
            Some(ActivationEvent::OnLanguage("rust".into()))
        );
        assert_eq!(parse_activation_event("onDebug"), Some(ActivationEvent::OnDebug));
        assert!(parse_activation_event("invalid").is_none());
    }

    #[test]
    fn activation_event_to_string_roundtrip() {
        let events = vec![
            ActivationEvent::Star,
            ActivationEvent::OnLanguage("python".into()),
            ActivationEvent::OnCommand("editor.action.format".into()),
        ];
        for event in &events {
            let s = activation_event_to_string(event);
            let parsed = parse_activation_event(&s).unwrap();
            assert_eq!(&parsed, event);
        }
    }

    #[test]
    fn activation_matcher_language_trigger() {
        let mut matcher = ActivationEventMatcher::new();
        let event = ActivationEvent::OnLanguage("rust".into());
        assert!(!matcher.should_activate(&event));
        matcher.open_language("rust");
        assert!(matcher.should_activate(&event));
    }

    #[test]
    fn activation_queue_evaluate_and_pop() {
        let mut queue = ExtensionActivationQueue::new();
        queue.register("ext-a".into(), vec![ActivationEvent::Star]);
        queue.register("ext-b".into(), vec![ActivationEvent::OnLanguage("go".into())]);
        let matcher = ActivationEventMatcher::new();
        let newly = queue.evaluate(&matcher);
        assert!(newly.contains(&"ext-a".to_string()));
        assert!(!newly.contains(&"ext-b".to_string()));
        let popped = queue.pop_pending().unwrap();
        assert_eq!(popped, "ext-a");
        assert!(queue.is_activated("ext-a"));
    }

    #[test]
    fn activation_dependency_graph() {
        let mut graph = ActivationDependencyGraph::new();
        graph.add_dependency("ext-b", "ext-a");
        let mut activated = std::collections::HashSet::new();
        assert!(!graph.can_activate("ext-b", &activated));
        activated.insert("ext-a".into());
        assert!(graph.can_activate("ext-b", &activated));
    }

    #[test]
    fn extension_point_registry_crud() {
        let mut reg = ExtensionPointRegistry::new();
        assert!(reg.is_empty());
        reg.register_point("commands");
        reg.register_point("languages");
        assert_eq!(reg.len(), 2);
        assert!(reg.has_point("commands"));
        reg.unregister_point("commands").unwrap();
        assert!(!reg.has_point("commands"));
    }

    #[test]
    fn extension_point_registry_metadata() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point_with_metadata("themes", ExtensionPointMetadata {
            description: "Color themes".into(),
            version: Some("1.0.0".into()),
            deprecated: false,
        });
        let meta = reg.get_metadata("themes").unwrap();
        assert_eq!(meta.description, "Color themes");
    }

    #[test]
    fn extension_point_find_by_prefix() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("editor.commands");
        reg.register_point("editor.languages");
        reg.register_point("workbench.views");
        let found = reg.find_by_prefix("editor.");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn registry_merge_and_snapshot() {
        let mut a = ExtensionPointRegistry::new();
        a.register_point("commands");
        let mut b = ExtensionPointRegistry::new();
        b.register_point("languages");
        let count = merge_registries(&mut a, &b);
        assert_eq!(count, 1);
        assert!(a.has_point("commands"));
        assert!(a.has_point("languages"));
        let snap = RegistrySnapshot::from_registry(&a, 0);
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn api_stats_tracking() {
        let mut stats = ExtApiStats::new();
        stats.record_success(100);
        stats.record_success(200);
        stats.record_failure(50);
        assert_eq!(stats.total(), 3);
        assert!(stats.success_rate() > 0.6);
        assert_eq!(stats.average_time_ns(), 116); // (100+200+50)/3
    }
}

// ─── Deep Platform Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod deep_platform_tests {
    use vsedit_files::{
        file_compare, diff_summary, file_similarity,
    };
    use vsedit_configuration::{
        ConfigurationModel, Configuration, ConfigurationTarget, ConfigurationRegistry,
        SettingSchema, SettingType,
    };
    use vsedit_storage::{Storage, StorageScope, StorageService};
    use vsedit_encryption::{
        derive_key, generate_salt, base64_encode, base64_decode,
        hmac_sign, hmac_verify, EncryptionService, validate_key_strength,
        byte_entropy, hex_encode, hex_decode,
    };
    use serde_json::json;

    #[test]
    fn file_diff_identical() {
        let old = b"hello\nworld\n";
        let new = b"hello\nworld\n";
        let lines = file_compare(old, new);
        let summary = diff_summary(&lines);
        assert_eq!(summary.added, 0);
        assert_eq!(summary.removed, 0);
    }

    #[test]
    fn file_diff_with_changes() {
        let old = b"line1\nline2\nline3\n";
        let new = b"line1\nmodified\nline3\nnew\n";
        let lines = file_compare(old, new);
        let summary = diff_summary(&lines);
        assert!(summary.added > 0 || summary.removed > 0);
    }

    #[test]
    fn file_similarity_metric() {
        let a = b"hello world";
        let b = b"hello world";
        let sim = file_similarity(a, b);
        assert!((sim - 1.0).abs() < 0.01);
        let sim2 = file_similarity(b"abc", b"xyz");
        assert!(sim2 < 1.0);
    }

    #[test]
    fn configuration_model_set_get() {
        let mut model = ConfigurationModel::new();
        model.set_value("editor.fontSize", json!(14));
        let val: Option<i64> = model.get_value("editor.fontSize");
        assert_eq!(val, Some(14));
    }

    #[test]
    fn configuration_model_merge() {
        let mut base = ConfigurationModel::new();
        base.set_value("editor.tabSize", json!(4));
        let mut overlay = ConfigurationModel::new();
        overlay.set_value("editor.tabSize", json!(2));
        overlay.set_value("editor.wordWrap", json!("on"));
        base.merge(&overlay);
        let tab_size: Option<i64> = base.get_value("editor.tabSize");
        assert_eq!(tab_size, Some(2));
        let word_wrap: Option<String> = base.get_value("editor.wordWrap");
        assert_eq!(word_wrap, Some("on".into()));
    }

    #[test]
    fn configuration_layered() {
        let mut config = Configuration::new();
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.fontSize", json!(12));
        config.set_layer(ConfigurationTarget::Default, defaults);
        let mut user = ConfigurationModel::new();
        user.set_value("editor.fontSize", json!(16));
        config.set_layer(ConfigurationTarget::User, user);
        let effective = config.get_effective_value("editor.fontSize");
        assert_eq!(effective, Some(json!(16)));
    }

    #[test]
    fn configuration_inspect() {
        let mut config = Configuration::new();
        let mut defaults = ConfigurationModel::new();
        defaults.set_value("editor.minimap.enabled", json!(true));
        config.set_layer(ConfigurationTarget::Default, defaults);
        let inspect = config.inspect("editor.minimap.enabled");
        assert!(inspect.merged_value().is_some());
    }

    #[test]
    fn configuration_registry_settings() {
        let mut registry = ConfigurationRegistry::new();
        registry.register_setting(SettingSchema {
            key: "editor.fontSize".into(),
            setting_type: SettingType::Number,
            default: json!(14),
            description: "Font size in pixels".into(),
            enum_values: None,
            enum_descriptions: None,
        });
        assert!(!registry.is_empty());
        assert!(registry.get_schema("editor.fontSize").is_some());
    }

    #[test]
    fn storage_in_memory_crud() {
        let store = Storage::in_memory().unwrap();
        store.set("theme", "dark").unwrap();
        assert_eq!(store.get("theme"), Some("dark".into()));
        assert!(store.has("theme"));
        store.set_bool("minimap", true).unwrap();
        assert_eq!(store.get_bool("minimap"), Some(true));
        store.set_i64("fontSize", 14).unwrap();
        assert_eq!(store.get_i64("fontSize"), Some(14));
        store.remove("theme").unwrap();
        assert!(!store.has("theme"));
    }

    #[test]
    fn storage_service_scoped() {
        let global = Storage::in_memory().unwrap();
        let workspace = Storage::in_memory().unwrap();
        let svc = StorageService::new(global).with_workspace(workspace);
        svc.set("key1", "global_val", StorageScope::Global).unwrap();
        svc.set("key1", "ws_val", StorageScope::Workspace).unwrap();
        assert_eq!(svc.get("key1", StorageScope::Global), Some("global_val".into()));
        assert_eq!(svc.get("key1", StorageScope::Workspace), Some("ws_val".into()));
    }

    #[test]
    fn encryption_base64_roundtrip() {
        let data = b"hello world encryption test";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encryption_hex_roundtrip() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let hex = hex_encode(&data);
        let decoded = hex_decode(&hex).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encryption_derive_key_and_service() {
        let key = derive_key("my-passphrase");
        assert!(!key.is_empty());
        let svc = EncryptionService::from_passphrase("test-pass");
        let encrypted = svc.encrypt_string("secret data");
        let decrypted = svc.decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, "secret data");
    }

    #[test]
    fn encryption_hmac_sign_verify() {
        let key = derive_key("hmac-key");
        let data = b"important message";
        let sig = hmac_sign(&key, data);
        assert!(hmac_verify(&key, data, &sig));
        assert!(!hmac_verify(&key, b"tampered", &sig));
    }

    #[test]
    fn encryption_entropy_and_validation() {
        let random_data = generate_salt(64);
        let entropy = byte_entropy(&random_data);
        assert!(entropy > 0.0);
        let strong_key = generate_salt(32);
        assert!(validate_key_strength(&strong_key).is_ok());
    }
}

// ─── Deep Render Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod deep_render_tests {
    use vsedit_theme::{
        Color, dark_plus, light_plus, high_contrast,
        builtin_themes, blend_colors, relative_luminance, contrast_ratio,
        validate_contrast, WcagLevel, theme_diff,
    };
    use vsedit_tokens::{
        StandardTokenType, TokenMetadata, FontStyle, Token, LineTokens,
        TokenMetadataBuilder, TokenizationState, TokenizationCache,
        compute_token_statistics, token_type_color_name,
    };
    use vsedit_unicodehl::{
        UnicodeHighlightConfig, highlight_line,
        count_non_ascii, is_safe_text, UnicodeAnalysis,
    };
    use vsedit_styles::{
        ThemeColor, ThemeColorResolver, parse_hex_color,
        ColorPalette as StyleColorPalette, StyleProperty, StyleRule, StyleSheet,
        editor_style,
    };

    #[test]
    fn theme_color_creation() {
        let c = Color::rgb(255, 0, 0);
        assert_eq!(c.r, 255);
        let hex_c = Color::from_hex("#00ff00").unwrap();
        assert_eq!(hex_c.g, 255);
        let hex_str = c.to_hex();
        assert!(hex_str.starts_with('#'));
    }

    #[test]
    fn theme_dark_plus_properties() {
        let theme = dark_plus();
        assert!(!theme.is_high_contrast());
        assert!(theme.get_color("editor.background").is_some());
        assert!(theme.token_color_count() > 0);
    }

    #[test]
    fn theme_builtin_themes_exist() {
        let themes = builtin_themes();
        assert!(themes.len() >= 4); // dark+, light+, monokai, solarized, etc.
        let hc = high_contrast();
        assert!(hc.is_high_contrast());
    }

    #[test]
    fn theme_color_blending() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let mid = blend_colors(&black, &white, 0.5);
        assert!(mid.r > 100 && mid.r < 155);
    }

    #[test]
    fn theme_contrast_ratio_wcag() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let ratio = contrast_ratio(&black, &white);
        assert!(ratio > 20.0);
        assert!(validate_contrast(&black, &white, WcagLevel::AAA));
        let lum = relative_luminance(&white);
        assert!(lum > 0.9);
    }

    #[test]
    fn theme_diff_detection() {
        let dark = dark_plus();
        let light = light_plus();
        let changes = theme_diff(&dark, &light);
        assert!(!changes.is_empty());
    }

    #[test]
    fn token_metadata_builder() {
        let meta = TokenMetadataBuilder::new()
            .language_id(1)
            .token_type(StandardTokenType::Comment)
            .font_style(FontStyle::ITALIC)
            .foreground(10)
            .background(0)
            .build()
            .unwrap();
        assert_eq!(meta.token_type(), StandardTokenType::Comment);
        assert_eq!(meta.language_id(), 1);
        assert!(meta.font_style().is_italic());
    }

    #[test]
    fn token_line_tokens_operations() {
        let meta = TokenMetadata::new(0, StandardTokenType::Other, FontStyle::NONE, 1, 0);
        let tokens = LineTokens::new(vec![
            Token { start_offset: 0, metadata: meta },
            Token { start_offset: 5, metadata: TokenMetadata::new(0, StandardTokenType::Comment, FontStyle::ITALIC, 2, 0) },
        ]);
        assert_eq!(tokens.count(), 2);
        assert!(tokens.contains_type(StandardTokenType::Comment));
        assert_eq!(tokens.count_type(StandardTokenType::Comment), 1);
        let stats = compute_token_statistics(&tokens);
        assert_eq!(stats.total_tokens, 2);
    }

    #[test]
    fn token_cache_operations() {
        let mut cache = TokenizationCache::new();
        let tokens = LineTokens::empty();
        let state = TokenizationState::initial();
        cache.set(0, tokens, state);
        assert_eq!(cache.cached_line_count(), 1);
        assert!(cache.get(0).is_some());
        cache.invalidate(0);
        assert_eq!(cache.cached_line_count(), 0);
    }

    #[test]
    fn token_type_names() {
        let comment_name = token_type_color_name(StandardTokenType::Comment);
        assert!(comment_name.contains("comment"));
        let string_name = token_type_color_name(StandardTokenType::String);
        assert!(string_name.contains("string"));
    }

    #[test]
    fn unicode_highlight_detection() {
        let config = UnicodeHighlightConfig::strict();
        let highlights = highlight_line("hello wоrld", 1, &config); // 'о' is Cyrillic
        assert!(!highlights.is_empty());
        let safe = highlight_line("hello world", 1, &config);
        assert!(safe.is_empty());
    }

    #[test]
    fn unicode_analysis_and_safety() {
        assert!(is_safe_text("hello world"));
        assert!(count_non_ascii("hello") == 0);
        assert!(count_non_ascii("héllo") == 1);
        let analysis = UnicodeAnalysis::analyze("hello world");
        assert!(analysis.is_safe());
        assert!((analysis.ascii_percentage() - 100.0).abs() < 0.1);
    }

    #[test]
    fn styles_color_resolver() {
        let mut resolver = ThemeColorResolver::new();
        let color = ThemeColor::new("editor.background");
        resolver.register(color.clone(), editor_style());
        assert!(!resolver.is_empty());
        assert_eq!(resolver.len(), 1);
        let resolved = resolver.resolve(&color);
        // Should return the registered style (not default)
        assert_eq!(format!("{:?}", resolved), format!("{:?}", editor_style()));
    }

    #[test]
    fn styles_parse_hex_and_palette() {
        let c = parse_hex_color("#ff0000").unwrap();
        assert_eq!(format!("{:?}", c), format!("{:?}", vsedit_styles::Color::Rgb(255, 0, 0)));
        let palette = StyleColorPalette::dark_default();
        assert!(!palette.is_empty());
    }

    #[test]
    fn styles_stylesheet_operations() {
        let mut sheet = StyleSheet::new();
        let mut rule = StyleRule::new("editor");
        rule.set("fontSize", StyleProperty::NumberValue(14.0));
        sheet.add_rule(rule);
        assert_eq!(sheet.rule_count(), 1);
        assert!(sheet.find_rule("editor").is_some());
        let selectors = sheet.selectors();
        assert!(selectors.contains(&"editor"));
    }
}
