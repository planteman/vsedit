//! Built-in language registrations and editing configurations.

use regex::Regex;

use crate::config::*;
use crate::definition::LanguageDefinition;
use crate::registry::LanguageService;

/// Register 30+ common languages with the given service.
pub fn register_default_languages(svc: &mut LanguageService) {
    for def in built_in_definitions() {
        svc.register(def);
    }
    svc.register_default_edit_configs();
}

// ---------------------------------------------------------------------------
// Language definitions
// ---------------------------------------------------------------------------

fn built_in_definitions() -> Vec<LanguageDefinition> {
    vec![
        LanguageDefinition {
            id: "rust".into(),
            name: "Rust".into(),
            extensions: vec![".rs".into()],
            filenames: vec![],
            aliases: vec!["Rust".into(), "rust".into()],
            mime_types: vec!["text/x-rust".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "typescript".into(),
            name: "TypeScript".into(),
            extensions: vec![".ts".into(), ".mts".into(), ".cts".into()],
            filenames: vec![],
            aliases: vec!["TypeScript".into(), "ts".into()],
            mime_types: vec!["text/typescript".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "javascript".into(),
            name: "JavaScript".into(),
            extensions: vec![".js".into(), ".mjs".into(), ".cjs".into(), ".jsx".into()],
            filenames: vec![],
            aliases: vec!["JavaScript".into(), "js".into()],
            mime_types: vec!["text/javascript".into()],
            first_line: Some(r"^#!.*\bnode\b".into()),
        },
        LanguageDefinition {
            id: "python".into(),
            name: "Python".into(),
            extensions: vec![".py".into(), ".pyi".into()],
            filenames: vec![],
            aliases: vec!["Python".into(), "py".into()],
            mime_types: vec!["text/x-python".into()],
            first_line: Some(r"^#!.*\bpython[23]?\b".into()),
        },
        LanguageDefinition {
            id: "go".into(),
            name: "Go".into(),
            extensions: vec![".go".into()],
            filenames: vec![],
            aliases: vec!["Go".into(), "golang".into()],
            mime_types: vec!["text/x-go".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "java".into(),
            name: "Java".into(),
            extensions: vec![".java".into()],
            filenames: vec![],
            aliases: vec!["Java".into()],
            mime_types: vec!["text/x-java".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "c".into(),
            name: "C".into(),
            extensions: vec![".c".into(), ".h".into()],
            filenames: vec![],
            aliases: vec!["C".into()],
            mime_types: vec!["text/x-c".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "cpp".into(),
            name: "C++".into(),
            extensions: vec![".cpp".into(), ".cc".into(), ".cxx".into(), ".hpp".into(), ".hxx".into()],
            filenames: vec![],
            aliases: vec!["C++".into(), "cpp".into()],
            mime_types: vec!["text/x-c++src".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "csharp".into(),
            name: "C#".into(),
            extensions: vec![".cs".into()],
            filenames: vec![],
            aliases: vec!["C#".into(), "csharp".into()],
            mime_types: vec!["text/x-csharp".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "html".into(),
            name: "HTML".into(),
            extensions: vec![".html".into(), ".htm".into()],
            filenames: vec![],
            aliases: vec!["HTML".into()],
            mime_types: vec!["text/html".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "css".into(),
            name: "CSS".into(),
            extensions: vec![".css".into()],
            filenames: vec![],
            aliases: vec!["CSS".into()],
            mime_types: vec!["text/css".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "json".into(),
            name: "JSON".into(),
            extensions: vec![".json".into()],
            filenames: vec![],
            aliases: vec!["JSON".into()],
            mime_types: vec!["application/json".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "jsonc".into(),
            name: "JSON with Comments".into(),
            extensions: vec![".jsonc".into()],
            filenames: vec![],
            aliases: vec!["JSONC".into(), "jsonc".into()],
            mime_types: vec!["application/json".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "yaml".into(),
            name: "YAML".into(),
            extensions: vec![".yml".into(), ".yaml".into()],
            filenames: vec![],
            aliases: vec!["YAML".into(), "yml".into()],
            mime_types: vec!["text/yaml".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "toml".into(),
            name: "TOML".into(),
            extensions: vec![".toml".into()],
            filenames: vec!["Cargo.toml".into()],
            aliases: vec!["TOML".into()],
            mime_types: vec!["text/x-toml".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "markdown".into(),
            name: "Markdown".into(),
            extensions: vec![".md".into(), ".markdown".into()],
            filenames: vec![],
            aliases: vec!["Markdown".into(), "md".into()],
            mime_types: vec!["text/markdown".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "shellscript".into(),
            name: "Shell Script".into(),
            extensions: vec![".sh".into(), ".bash".into(), ".zsh".into()],
            filenames: vec![".bashrc".into(), ".zshrc".into(), ".profile".into()],
            aliases: vec!["Shell".into(), "bash".into(), "sh".into()],
            mime_types: vec!["text/x-shellscript".into()],
            first_line: Some(r"^#!.*\b(bash|sh|zsh)\b".into()),
        },
        LanguageDefinition {
            id: "xml".into(),
            name: "XML".into(),
            extensions: vec![".xml".into(), ".xsl".into(), ".xsd".into()],
            filenames: vec![],
            aliases: vec!["XML".into()],
            mime_types: vec!["text/xml".into(), "application/xml".into()],
            first_line: Some(r"^<\?xml\b".into()),
        },
        LanguageDefinition {
            id: "sql".into(),
            name: "SQL".into(),
            extensions: vec![".sql".into()],
            filenames: vec![],
            aliases: vec!["SQL".into()],
            mime_types: vec!["text/x-sql".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "dockerfile".into(),
            name: "Dockerfile".into(),
            extensions: vec![".dockerfile".into()],
            filenames: vec!["Dockerfile".into(), "Containerfile".into()],
            aliases: vec!["Dockerfile".into(), "docker".into()],
            mime_types: vec!["text/x-dockerfile".into()],
            first_line: None,
        },
        // -- Additional languages (11 more to reach 30+) --------------------
        LanguageDefinition {
            id: "ruby".into(),
            name: "Ruby".into(),
            extensions: vec![".rb".into(), ".rake".into(), ".gemspec".into()],
            filenames: vec!["Gemfile".into(), "Rakefile".into()],
            aliases: vec!["Ruby".into(), "rb".into()],
            mime_types: vec!["text/x-ruby".into()],
            first_line: Some(r"^#!.*\bruby\b".into()),
        },
        LanguageDefinition {
            id: "php".into(),
            name: "PHP".into(),
            extensions: vec![".php".into(), ".phtml".into()],
            filenames: vec![],
            aliases: vec!["PHP".into()],
            mime_types: vec!["text/x-php".into()],
            first_line: Some(r"^<\?php\b".into()),
        },
        LanguageDefinition {
            id: "powershell".into(),
            name: "PowerShell".into(),
            extensions: vec![".ps1".into(), ".psm1".into(), ".psd1".into()],
            filenames: vec![],
            aliases: vec!["PowerShell".into(), "pwsh".into()],
            mime_types: vec!["text/x-powershell".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "makefile".into(),
            name: "Makefile".into(),
            extensions: vec![".mk".into()],
            filenames: vec!["Makefile".into(), "GNUmakefile".into(), "makefile".into()],
            aliases: vec!["Makefile".into(), "make".into()],
            mime_types: vec!["text/x-makefile".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "ini".into(),
            name: "INI".into(),
            extensions: vec![".ini".into(), ".cfg".into(), ".conf".into()],
            filenames: vec![],
            aliases: vec!["INI".into(), "ini".into()],
            mime_types: vec!["text/x-ini".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "lua".into(),
            name: "Lua".into(),
            extensions: vec![".lua".into()],
            filenames: vec![],
            aliases: vec!["Lua".into()],
            mime_types: vec!["text/x-lua".into()],
            first_line: Some(r"^#!.*\blua\b".into()),
        },
        LanguageDefinition {
            id: "perl".into(),
            name: "Perl".into(),
            extensions: vec![".pl".into(), ".pm".into()],
            filenames: vec![],
            aliases: vec!["Perl".into()],
            mime_types: vec!["text/x-perl".into()],
            first_line: Some(r"^#!.*\bperl\b".into()),
        },
        LanguageDefinition {
            id: "swift".into(),
            name: "Swift".into(),
            extensions: vec![".swift".into()],
            filenames: vec![],
            aliases: vec!["Swift".into()],
            mime_types: vec!["text/x-swift".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "kotlin".into(),
            name: "Kotlin".into(),
            extensions: vec![".kt".into(), ".kts".into()],
            filenames: vec![],
            aliases: vec!["Kotlin".into(), "kt".into()],
            mime_types: vec!["text/x-kotlin".into()],
            first_line: None,
        },
        LanguageDefinition {
            id: "scala".into(),
            name: "Scala".into(),
            extensions: vec![".scala".into(), ".sc".into()],
            filenames: vec![],
            aliases: vec!["Scala".into()],
            mime_types: vec!["text/x-scala".into()],
            first_line: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Built-in editing configurations
// ---------------------------------------------------------------------------

/// Build the map of built-in editing configurations.
pub(crate) fn build_edit_configs() -> Vec<(String, LanguageEditConfig)> {
    let mut out = Vec::new();

    // C-like languages share the same base config.
    let c_like_ids = [
        "rust", "go", "java", "c", "cpp", "csharp", "swift", "kotlin", "scala", "jsonc", "php",
    ];
    for id in c_like_ids {
        out.push((id.to_string(), c_like_edit_config()));
    }

    // JS/TS get a word pattern that includes `$`.
    out.push(("javascript".into(), js_edit_config()));
    out.push(("typescript".into(), js_edit_config()));

    out.push(("python".into(), python_edit_config()));
    out.push(("ruby".into(), ruby_edit_config()));
    out.push(("html".into(), html_edit_config()));
    out.push(("xml".into(), html_edit_config()));
    out.push(("css".into(), css_edit_config()));
    out.push(("json".into(), json_edit_config()));
    out.push(("yaml".into(), hash_comment_edit_config()));
    out.push(("toml".into(), hash_comment_edit_config()));
    out.push(("ini".into(), hash_comment_edit_config()));
    out.push(("makefile".into(), hash_comment_edit_config()));
    out.push(("dockerfile".into(), hash_comment_edit_config()));
    out.push(("shellscript".into(), hash_comment_edit_config()));
    out.push(("powershell".into(), hash_comment_edit_config()));
    out.push(("perl".into(), hash_comment_edit_config()));
    out.push(("markdown".into(), markdown_edit_config()));
    out.push(("sql".into(), sql_edit_config()));
    out.push(("lua".into(), lua_edit_config()));

    out
}

// -- Helpers ----------------------------------------------------------------

fn default_brackets() -> Vec<BracketPair> {
    vec![
        BracketPair { open: "(".into(), close: ")".into() },
        BracketPair { open: "[".into(), close: "]".into() },
        BracketPair { open: "{".into(), close: "}".into() },
    ]
}

fn default_auto_closing() -> Vec<AutoClosingPair> {
    vec![
        AutoClosingPair { open: "(".into(), close: ")".into(), not_in: vec![] },
        AutoClosingPair { open: "[".into(), close: "]".into(), not_in: vec![] },
        AutoClosingPair { open: "{".into(), close: "}".into(), not_in: vec![] },
        AutoClosingPair {
            open: "\"".into(),
            close: "\"".into(),
            not_in: vec!["string".into()],
        },
        AutoClosingPair {
            open: "'".into(),
            close: "'".into(),
            not_in: vec!["string".into(), "comment".into()],
        },
    ]
}

fn default_surrounding() -> Vec<BracketPair> {
    vec![
        BracketPair { open: "(".into(), close: ")".into() },
        BracketPair { open: "[".into(), close: "]".into() },
        BracketPair { open: "{".into(), close: "}".into() },
        BracketPair { open: "\"".into(), close: "\"".into() },
        BracketPair { open: "'".into(), close: "'".into() },
        BracketPair { open: "`".into(), close: "`".into() },
    ]
}

fn region_folding() -> Option<FoldingMarkers> {
    Some(FoldingMarkers {
        start: Regex::new(r"(?i)#\s*region\b").unwrap(),
        end: Regex::new(r"(?i)#\s*endregion\b").unwrap(),
    })
}

fn c_like_indent_rules() -> Option<IndentationRules> {
    Some(IndentationRules {
        increase_indent_pattern: Regex::new(r"[{(\[]").unwrap(),
        decrease_indent_pattern: Regex::new(r"^\s*[})\]]").unwrap(),
        indent_next_line_pattern: None,
        unindented_line_pattern: None,
    })
}

fn c_like_on_enter_rules() -> Vec<OnEnterRule> {
    vec![
        OnEnterRule {
            before_text: Regex::new(r"/\*\*").unwrap(),
            after_text: None,
            action: IndentAction::IndentOutdent,
        },
        OnEnterRule {
            before_text: Regex::new(r"^\s*\*\s").unwrap(),
            after_text: None,
            action: IndentAction::None,
        },
    ]
}

// -- Per-language configs ---------------------------------------------------

fn c_like_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("//".into()),
            block_comment: Some(("/*".into(), "*/".into())),
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: region_folding(),
        indentation_rules: c_like_indent_rules(),
        word_pattern: None,
        on_enter_rules: c_like_on_enter_rules(),
    }
}

fn js_edit_config() -> LanguageEditConfig {
    let mut cfg = c_like_edit_config();
    cfg.word_pattern = Some(Regex::new(r"[\w$][\w$]*").unwrap());
    cfg.auto_closing_pairs.push(AutoClosingPair {
        open: "`".into(),
        close: "`".into(),
        not_in: vec!["string".into()],
    });
    cfg
}

fn python_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("#".into()),
            block_comment: None,
        },
        brackets: default_brackets(),
        auto_closing_pairs: {
            let mut pairs = default_auto_closing();
            pairs.push(AutoClosingPair {
                open: "\"\"\"".into(),
                close: "\"\"\"".into(),
                not_in: vec!["string".into(), "comment".into()],
            });
            pairs
        },
        surrounding_pairs: default_surrounding(),
        folding_markers: Some(FoldingMarkers {
            start: Regex::new(r"(?i)#\s*region\b").unwrap(),
            end: Regex::new(r"(?i)#\s*endregion\b").unwrap(),
        }),
        indentation_rules: Some(IndentationRules {
            increase_indent_pattern: Regex::new(r":\s*(#.*)?$").unwrap(),
            decrease_indent_pattern: Regex::new(
                r"^\s*(elif|else|except|finally|return|pass|break|continue)\b",
            )
            .unwrap(),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }),
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn ruby_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("#".into()),
            block_comment: Some(("=begin".into(), "=end".into())),
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: None,
        indentation_rules: Some(IndentationRules {
            increase_indent_pattern: Regex::new(
                r"\b(def|class|module|if|unless|while|until|for|do|begin|case)\b",
            )
            .unwrap(),
            decrease_indent_pattern: Regex::new(r"^\s*(end|else|elsif|when|rescue|ensure)\b")
                .unwrap(),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }),
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn html_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: None,
            block_comment: Some(("<!--".into(), "-->".into())),
        },
        brackets: vec![
            BracketPair { open: "<!--".into(), close: "-->".into() },
            BracketPair { open: "<".into(), close: ">".into() },
            BracketPair { open: "{".into(), close: "}".into() },
            BracketPair { open: "(".into(), close: ")".into() },
        ],
        auto_closing_pairs: vec![
            AutoClosingPair { open: "{".into(), close: "}".into(), not_in: vec![] },
            AutoClosingPair { open: "[".into(), close: "]".into(), not_in: vec![] },
            AutoClosingPair { open: "(".into(), close: ")".into(), not_in: vec![] },
            AutoClosingPair {
                open: "\"".into(),
                close: "\"".into(),
                not_in: vec!["string".into()],
            },
            AutoClosingPair {
                open: "'".into(),
                close: "'".into(),
                not_in: vec!["string".into()],
            },
            AutoClosingPair {
                open: "<!--".into(),
                close: "-->".into(),
                not_in: vec!["comment".into()],
            },
        ],
        surrounding_pairs: default_surrounding(),
        folding_markers: Some(FoldingMarkers {
            start: Regex::new(r"(?i)<!--\s*#?region\b").unwrap(),
            end: Regex::new(r"(?i)<!--\s*#?endregion\b").unwrap(),
        }),
        indentation_rules: None,
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn css_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: None,
            block_comment: Some(("/*".into(), "*/".into())),
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: region_folding(),
        indentation_rules: c_like_indent_rules(),
        word_pattern: Some(Regex::new(r"[\w][\w-]*").unwrap()),
        on_enter_rules: Vec::new(),
    }
}

fn json_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: None,
            block_comment: None,
        },
        brackets: vec![
            BracketPair { open: "[".into(), close: "]".into() },
            BracketPair { open: "{".into(), close: "}".into() },
        ],
        auto_closing_pairs: vec![
            AutoClosingPair { open: "[".into(), close: "]".into(), not_in: vec![] },
            AutoClosingPair { open: "{".into(), close: "}".into(), not_in: vec![] },
            AutoClosingPair {
                open: "\"".into(),
                close: "\"".into(),
                not_in: vec!["string".into()],
            },
        ],
        surrounding_pairs: default_surrounding(),
        folding_markers: None,
        indentation_rules: c_like_indent_rules(),
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn hash_comment_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("#".into()),
            block_comment: None,
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: region_folding(),
        indentation_rules: None,
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn markdown_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: None,
            block_comment: Some(("<!--".into(), "-->".into())),
        },
        brackets: vec![
            BracketPair { open: "(".into(), close: ")".into() },
            BracketPair { open: "[".into(), close: "]".into() },
        ],
        auto_closing_pairs: vec![
            AutoClosingPair { open: "(".into(), close: ")".into(), not_in: vec![] },
            AutoClosingPair { open: "[".into(), close: "]".into(), not_in: vec![] },
            AutoClosingPair {
                open: "`".into(),
                close: "`".into(),
                not_in: vec!["string".into()],
            },
        ],
        surrounding_pairs: default_surrounding(),
        folding_markers: Some(FoldingMarkers {
            start: Regex::new(r"(?i)<!--\s*#?region\b").unwrap(),
            end: Regex::new(r"(?i)<!--\s*#?endregion\b").unwrap(),
        }),
        indentation_rules: None,
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn sql_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("--".into()),
            block_comment: Some(("/*".into(), "*/".into())),
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: None,
        indentation_rules: None,
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}

fn lua_edit_config() -> LanguageEditConfig {
    LanguageEditConfig {
        comments: CommentConfig {
            line_comment: Some("--".into()),
            block_comment: Some(("--[[".into(), "]]".into())),
        },
        brackets: default_brackets(),
        auto_closing_pairs: default_auto_closing(),
        surrounding_pairs: default_surrounding(),
        folding_markers: None,
        indentation_rules: Some(IndentationRules {
            increase_indent_pattern: Regex::new(
                r"\b(function|if|for|while|repeat|else|elseif|do)\b",
            )
            .unwrap(),
            decrease_indent_pattern: Regex::new(r"^\s*(end|else|elseif|until)\b").unwrap(),
            indent_next_line_pattern: None,
            unindented_line_pattern: None,
        }),
        word_pattern: None,
        on_enter_rules: Vec::new(),
    }
}
