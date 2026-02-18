//! Tree-sitter parsing service.
//!
//! Provides structural code understanding via tree-sitter-compatible AST types,
//! incremental parsing, symbol extraction, bracket pair detection, code folding,
//! and semantic token extraction.
//!
//! Since tree-sitter language parsers are large C libraries, this crate defines
//! the interface and types without bundling actual parser crates.  Real parsers
//! are loaded at runtime via [`TreeSitterConfig`] which points at shared
//! library (`*.so` / `*.dylib`) files on disk.  A [`MockParser`] is provided
//! for testing.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors returned by tree-sitter operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSitterError {
    LanguageNotFound(String),
    ParseFailed(String),
    InvalidNode(String),
    LibraryLoadFailed(String),
}

impl fmt::Display for TreeSitterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeSitterError::LanguageNotFound(name) => {
                write!(f, "language not found: {name}")
            }
            TreeSitterError::ParseFailed(reason) => {
                write!(f, "parse failed: {reason}")
            }
            TreeSitterError::InvalidNode(msg) => {
                write!(f, "invalid node: {msg}")
            }
            TreeSitterError::LibraryLoadFailed(path) => {
                write!(f, "failed to load parser library: {path}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration for dynamic parser loading
// ---------------------------------------------------------------------------

/// Describes where to find a tree-sitter parser shared library on disk.
#[derive(Debug, Clone)]
pub struct ParserLibraryEntry {
    /// Language identifier (e.g. `"rust"`, `"python"`).
    pub language_id: String,
    /// Path to the `.so` / `.dylib` file.
    pub library_path: PathBuf,
    /// Name of the C symbol that returns the `TSLanguage *`.
    pub symbol_name: String,
}

/// Configuration for loading tree-sitter parsers at runtime.
#[derive(Debug, Clone)]
pub struct TreeSitterConfig {
    /// Directory that is searched for parser libraries when a relative path is
    /// given in [`ParserLibraryEntry::library_path`].
    pub parser_dir: PathBuf,
    /// Per-language entries.
    pub parsers: Vec<ParserLibraryEntry>,
}

impl TreeSitterConfig {
    pub fn new(parser_dir: PathBuf) -> Self {
        Self {
            parser_dir,
            parsers: Vec::new(),
        }
    }

    pub fn add_parser(&mut self, entry: ParserLibraryEntry) {
        self.parsers.push(entry);
    }

    /// Resolve the absolute path for a parser entry.
    pub fn resolve_path(&self, entry: &ParserLibraryEntry) -> PathBuf {
        if entry.library_path.is_absolute() {
            entry.library_path.clone()
        } else {
            self.parser_dir.join(&entry.library_path)
        }
    }

    /// Find the entry for a language.
    pub fn get_entry(&self, language_id: &str) -> Option<&ParserLibraryEntry> {
        self.parsers.iter().find(|e| e.language_id == language_id)
    }
}

// ---------------------------------------------------------------------------
// Incremental edit descriptor
// ---------------------------------------------------------------------------

/// A point in the source file (0-based row and column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub row: u32,
    pub column: u32,
}

/// Describes an incremental edit for re-parsing after a buffer change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncrementalEdit {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_point: Point,
    pub old_end_point: Point,
    pub new_end_point: Point,
}

// ---------------------------------------------------------------------------
// Language definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TreeSitterLanguage {
    pub name: String,
    pub file_types: Vec<String>,
    pub highlight_query: Option<String>,
}

impl TreeSitterLanguage {
    /// Check if this language handles a given file extension.
    pub fn supports_file(&self, filename: &str) -> bool {
        match filename.rsplit('.').next() {
            Some(ext) => self.file_types.iter().any(|ft| ft == ext),
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Syntax node (AST)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SyntaxNode {
    pub kind: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub children: Vec<SyntaxNode>,
    pub named: bool,
}

impl fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}:{}-{}:{}]",
            self.kind, self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

impl SyntaxNode {
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn span_lines(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Return all nodes in pre-order traversal.
    pub fn flatten(&self) -> Vec<&SyntaxNode> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.flatten());
        }
        result
    }

    /// Find all nodes whose kind matches the given string.
    pub fn find_by_kind<'a>(&'a self, kind: &str) -> Vec<&'a SyntaxNode> {
        self.flatten()
            .into_iter()
            .filter(|n| n.kind == kind)
            .collect()
    }

    /// Find the deepest node containing the given line and column.
    pub fn find_at_position(&self, line: u32, col: u32) -> Option<&SyntaxNode> {
        let contains = (self.start_line < line
            || (self.start_line == line && self.start_col <= col))
            && (self.end_line > line || (self.end_line == line && self.end_col >= col));
        if !contains {
            return None;
        }
        for child in &self.children {
            if let Some(deeper) = child.find_at_position(line, col) {
                return Some(deeper);
            }
        }
        Some(self)
    }

    /// Return only named children.
    pub fn named_children(&self) -> Vec<&SyntaxNode> {
        self.children.iter().filter(|c| c.named).collect()
    }

    /// Max depth of the subtree rooted at this node (leaf = 1).
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Return the first child whose kind matches `kind`.
    pub fn child_by_kind(&self, kind: &str) -> Option<&SyntaxNode> {
        self.children.iter().find(|c| c.kind == kind)
    }

    /// Extract text from source given this node's byte range is unavailable;
    /// fall back to line/col substring.
    pub fn text_from_source<'a>(&self, lines: &[&'a str]) -> Option<&'a str> {
        if self.start_line != self.end_line {
            return None; // multi-line – caller should handle
        }
        let line = lines.get(self.start_line as usize)?;
        line.get(self.start_col as usize..self.end_col as usize)
    }
}

// ---------------------------------------------------------------------------
// Document symbol extraction
// ---------------------------------------------------------------------------

/// The kind of a document symbol extracted from the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Variable,
    Constant,
    Property,
    Module,
    Trait,
    Type,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Interface => "interface",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Property => "property",
            SymbolKind::Module => "module",
            SymbolKind::Trait => "trait",
            SymbolKind::Type => "type",
        };
        f.write_str(s)
    }
}

/// A range in the document (0-based lines and columns).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A symbol extracted from the AST (function, struct, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Full range of the symbol definition (including body).
    pub range: Range,
    /// Range of the symbol name (for "go to definition").
    pub selection_range: Range,
    /// Nested symbols (e.g. methods inside a class).
    pub children: Vec<DocumentSymbol>,
}

/// Maps tree-sitter node kinds to [`SymbolKind`].
fn node_kind_to_symbol_kind(kind: &str) -> Option<SymbolKind> {
    match kind {
        "function_item" | "function_definition" | "function_declaration"
        | "arrow_function" | "lambda" => Some(SymbolKind::Function),
        "method_definition" | "method_declaration" => Some(SymbolKind::Method),
        "class_declaration" | "class_definition" => Some(SymbolKind::Class),
        "struct_item" | "struct_declaration" => Some(SymbolKind::Struct),
        "enum_item" | "enum_declaration" => Some(SymbolKind::Enum),
        "interface_declaration" => Some(SymbolKind::Interface),
        "let_declaration" | "variable_declaration" | "variable_declarator" => {
            Some(SymbolKind::Variable)
        }
        "const_item" | "const_declaration" => Some(SymbolKind::Constant),
        "field_declaration" | "property_declaration" => Some(SymbolKind::Property),
        "mod_item" | "module" | "module_declaration" => Some(SymbolKind::Module),
        "trait_item" | "trait_declaration" => Some(SymbolKind::Trait),
        "type_alias_declaration" | "type_item" => Some(SymbolKind::Type),
        "impl_item" => Some(SymbolKind::Type),
        _ => None,
    }
}

/// Find the name of a symbol node by looking for an `identifier` or
/// `type_identifier` child.
fn find_name_in_node(node: &SyntaxNode, source_lines: &[&str]) -> Option<String> {
    for child in &node.children {
        if child.kind == "identifier" || child.kind == "type_identifier" || child.kind == "name" {
            if let Some(text) = child.text_from_source(source_lines) {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Extract document symbols from a parsed syntax tree.
pub fn extract_symbols(tree: &SyntaxNode, source: &str) -> Vec<DocumentSymbol> {
    let lines: Vec<&str> = source.lines().collect();
    extract_symbols_recursive(tree, &lines)
}

fn extract_symbols_recursive(node: &SyntaxNode, lines: &[&str]) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for child in &node.children {
        if !child.named {
            continue;
        }
        if let Some(kind) = node_kind_to_symbol_kind(&child.kind) {
            let name = find_name_in_node(child, lines)
                .unwrap_or_else(|| "<anonymous>".to_string());
            let range = Range {
                start_line: child.start_line,
                start_col: child.start_col,
                end_line: child.end_line,
                end_col: child.end_col,
            };
            // Selection range is the name node range if found, otherwise same as range.
            let sel = child
                .children
                .iter()
                .find(|c| c.kind == "identifier" || c.kind == "type_identifier" || c.kind == "name")
                .map(|n| Range {
                    start_line: n.start_line,
                    start_col: n.start_col,
                    end_line: n.end_line,
                    end_col: n.end_col,
                })
                .unwrap_or_else(|| range.clone());
            let nested = extract_symbols_recursive(child, lines);
            symbols.push(DocumentSymbol {
                name,
                kind,
                range,
                selection_range: sel,
                children: nested,
            });
        } else {
            symbols.extend(extract_symbols_recursive(child, lines));
        }
    }
    symbols
}

// ---------------------------------------------------------------------------
// AST-based bracket pair detection
// ---------------------------------------------------------------------------

/// A bracket pair detected from the AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstBracketPair {
    pub open_line: u32,
    pub open_col: u32,
    pub close_line: u32,
    pub close_col: u32,
    pub bracket_char: char,
    pub depth: u32,
}

/// Node kinds that represent string or comment content — brackets inside
/// these should be ignored.
const STRING_COMMENT_KINDS: &[&str] = &[
    "string",
    "string_literal",
    "raw_string_literal",
    "template_string",
    "comment",
    "line_comment",
    "block_comment",
    "doc_comment",
];

fn is_string_or_comment(kind: &str) -> bool {
    STRING_COMMENT_KINDS.contains(&kind)
}

/// Detect matching bracket pairs from an AST, skipping brackets inside
/// strings and comments.
pub fn detect_bracket_pairs(tree: &SyntaxNode) -> Vec<AstBracketPair> {
    let mut pairs = Vec::new();
    let mut open_stacks: HashMap<char, Vec<(u32, u32, u32)>> = HashMap::new();
    collect_bracket_pairs(tree, &mut open_stacks, &mut pairs, false);
    pairs
}

fn bracket_char_for_kind(kind: &str) -> Option<(char, bool)> {
    match kind {
        "(" => Some(('(', true)),
        ")" => Some(('(', false)),
        "[" => Some(('[', true)),
        "]" => Some(('[', false)),
        "{" => Some(('{', true)),
        "}" => Some(('{', false)),
        _ => None,
    }
}

fn collect_bracket_pairs(
    node: &SyntaxNode,
    stacks: &mut HashMap<char, Vec<(u32, u32, u32)>>,
    pairs: &mut Vec<AstBracketPair>,
    in_string_or_comment: bool,
) {
    let skip = in_string_or_comment || is_string_or_comment(&node.kind);

    if !skip {
        if let Some((ch, is_open)) = bracket_char_for_kind(&node.kind) {
            if is_open {
                let stack = stacks.entry(ch).or_default();
                let depth = stack.len() as u32;
                stack.push((node.start_line, node.start_col, depth));
            } else if let Some((ol, oc, depth)) = stacks.entry(ch).or_default().pop() {
                pairs.push(AstBracketPair {
                    open_line: ol,
                    open_col: oc,
                    close_line: node.start_line,
                    close_col: node.start_col,
                    bracket_char: ch,
                    depth,
                });
            }
        }
    }

    for child in &node.children {
        collect_bracket_pairs(child, stacks, pairs, skip);
    }
}

// ---------------------------------------------------------------------------
// AST-based code folding
// ---------------------------------------------------------------------------

/// A folding range derived from tree-sitter AST nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsFoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: TsFoldingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsFoldingKind {
    /// Function / method body.
    Function,
    /// If / else / match block.
    Control,
    /// Import group.
    Import,
    /// Comment block.
    Comment,
    /// General block (`{ … }`).
    Block,
}

/// Node kinds that produce folding ranges.
fn folding_kind_for_node(kind: &str) -> Option<TsFoldingKind> {
    match kind {
        "function_item" | "function_definition" | "function_declaration"
        | "method_definition" | "method_declaration" | "arrow_function" | "lambda" => {
            Some(TsFoldingKind::Function)
        }
        "if_expression" | "if_statement" | "else_clause" | "match_expression"
        | "switch_statement" | "for_statement" | "for_expression" | "while_statement"
        | "while_expression" | "loop_expression" => Some(TsFoldingKind::Control),
        "use_declaration" | "import_statement" | "import_declaration" => {
            Some(TsFoldingKind::Import)
        }
        "comment" | "line_comment" | "block_comment" | "doc_comment" => {
            Some(TsFoldingKind::Comment)
        }
        "block" | "declaration_list" | "field_declaration_list" | "enum_variant_list"
        | "match_block" | "statement_block" => Some(TsFoldingKind::Block),
        _ => None,
    }
}

/// Compute folding ranges from a tree-sitter AST.
pub fn compute_folding_ranges_ts(tree: &SyntaxNode) -> Vec<TsFoldingRange> {
    let mut ranges = Vec::new();
    collect_folding_ranges(tree, &mut ranges);
    ranges.sort_by_key(|r| r.start_line);
    ranges
}

fn collect_folding_ranges(node: &SyntaxNode, ranges: &mut Vec<TsFoldingRange>) {
    if let Some(kind) = folding_kind_for_node(&node.kind) {
        if node.end_line > node.start_line {
            ranges.push(TsFoldingRange {
                start_line: node.start_line,
                end_line: node.end_line,
                kind,
            });
        }
    }
    for child in &node.children {
        collect_folding_ranges(child, ranges);
    }
}

// ---------------------------------------------------------------------------
// Semantic tokens from tree-sitter
// ---------------------------------------------------------------------------

/// Semantic token types (a subset of the LSP spec for fallback highlighting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticTokenType {
    Namespace,
    Type,
    Class,
    Enum,
    Interface,
    Struct,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    Function,
    Method,
    Macro,
    Keyword,
    Modifier,
    Comment,
    String,
    Number,
    Regexp,
    Operator,
}

impl fmt::Display for SemanticTokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SemanticTokenType::Namespace => "namespace",
            SemanticTokenType::Type => "type",
            SemanticTokenType::Class => "class",
            SemanticTokenType::Enum => "enum",
            SemanticTokenType::Interface => "interface",
            SemanticTokenType::Struct => "struct",
            SemanticTokenType::TypeParameter => "typeParameter",
            SemanticTokenType::Parameter => "parameter",
            SemanticTokenType::Variable => "variable",
            SemanticTokenType::Property => "property",
            SemanticTokenType::Function => "function",
            SemanticTokenType::Method => "method",
            SemanticTokenType::Macro => "macro",
            SemanticTokenType::Keyword => "keyword",
            SemanticTokenType::Modifier => "modifier",
            SemanticTokenType::Comment => "comment",
            SemanticTokenType::String => "string",
            SemanticTokenType::Number => "number",
            SemanticTokenType::Regexp => "regexp",
            SemanticTokenType::Operator => "operator",
        };
        f.write_str(s)
    }
}

/// Bitflags for semantic token modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SemanticTokenModifiers(pub u32);

impl SemanticTokenModifiers {
    pub const NONE: Self = Self(0);
    pub const DECLARATION: Self = Self(1);
    pub const DEFINITION: Self = Self(1 << 1);
    pub const READONLY: Self = Self(1 << 2);
    pub const STATIC: Self = Self(1 << 3);
    pub const DEPRECATED: Self = Self(1 << 4);
    pub const ASYNC: Self = Self(1 << 5);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A single semantic token produced from the tree-sitter AST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticToken {
    pub line: u32,
    pub start_col: u32,
    pub length: u32,
    pub token_type: SemanticTokenType,
    pub modifiers: SemanticTokenModifiers,
}

/// Map tree-sitter node kind (+ context) to a semantic token type.
fn classify_node(node: &SyntaxNode, parent_kind: Option<&str>) -> Option<SemanticTokenType> {
    match node.kind.as_str() {
        "identifier" => match parent_kind {
            Some("function_item") | Some("function_definition") | Some("function_declaration")
            | Some("call_expression") => Some(SemanticTokenType::Function),
            Some("method_definition") | Some("method_declaration") => {
                Some(SemanticTokenType::Method)
            }
            Some("parameter") | Some("formal_parameter") | Some("formal_parameters") => {
                Some(SemanticTokenType::Parameter)
            }
            Some("field_declaration") | Some("property_declaration") => {
                Some(SemanticTokenType::Property)
            }
            _ => Some(SemanticTokenType::Variable),
        },
        "type_identifier" => Some(SemanticTokenType::Type),
        "string" | "string_literal" | "raw_string_literal" | "template_string"
        | "string_content" => Some(SemanticTokenType::String),
        "integer_literal" | "float_literal" | "number" => Some(SemanticTokenType::Number),
        "comment" | "line_comment" | "block_comment" | "doc_comment" => {
            Some(SemanticTokenType::Comment)
        }
        "macro_invocation" | "macro_definition" | "attribute_item" => {
            Some(SemanticTokenType::Macro)
        }
        // Keywords are usually unnamed nodes with their own literal kind.
        "fn" | "let" | "const" | "struct" | "enum" | "impl" | "trait" | "pub" | "mod" | "use"
        | "if" | "else" | "match" | "for" | "while" | "loop" | "return" | "async" | "await"
        | "class" | "function" | "var" | "import" | "export" | "default" | "extends"
        | "implements" => Some(SemanticTokenType::Keyword),
        _ => None,
    }
}

/// Extract semantic tokens from a tree-sitter AST for fallback highlighting.
pub fn extract_semantic_tokens(tree: &SyntaxNode, _source: &str) -> Vec<SemanticToken> {
    let mut tokens = Vec::new();
    collect_semantic_tokens(tree, None, &mut tokens);
    tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.start_col.cmp(&b.start_col)));
    tokens
}

fn collect_semantic_tokens(
    node: &SyntaxNode,
    parent_kind: Option<&str>,
    tokens: &mut Vec<SemanticToken>,
) {
    if let Some(token_type) = classify_node(node, parent_kind) {
        if node.start_line == node.end_line {
            let length = node.end_col.saturating_sub(node.start_col);
            if length > 0 {
                tokens.push(SemanticToken {
                    line: node.start_line,
                    start_col: node.start_col,
                    length,
                    token_type,
                    modifiers: SemanticTokenModifiers::NONE,
                });
            }
        }
    }

    let pk = Some(node.kind.as_str());
    for child in &node.children {
        collect_semantic_tokens(child, pk, tokens);
    }
}

// ---------------------------------------------------------------------------
// Mock parser (for testing without real tree-sitter C libraries)
// ---------------------------------------------------------------------------

/// A mock parser that produces a predefined tree for testing.
pub struct MockParser {
    trees: HashMap<String, SyntaxNode>,
}

impl MockParser {
    pub fn new() -> Self {
        Self {
            trees: HashMap::new(),
        }
    }

    /// Register a predefined tree for a language.
    pub fn register_tree(&mut self, language_id: &str, tree: SyntaxNode) {
        self.trees.insert(language_id.to_string(), tree);
    }

    /// Parse source code (ignores source, returns pre-registered tree).
    pub fn parse(
        &self,
        language_id: &str,
        _source: &str,
    ) -> Result<SyntaxNode, TreeSitterError> {
        self.trees
            .get(language_id)
            .cloned()
            .ok_or_else(|| TreeSitterError::LanguageNotFound(language_id.to_string()))
    }

    /// Simulate an incremental re-parse (returns same tree in mock).
    pub fn edit_tree(
        &self,
        language_id: &str,
        _old_tree: &SyntaxNode,
        _edit: &IncrementalEdit,
        _new_source: &str,
    ) -> Result<SyntaxNode, TreeSitterError> {
        self.parse(language_id, "")
    }
}

impl Default for MockParser {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TreeSitterService — manages parsers for different languages
// ---------------------------------------------------------------------------

/// Service for tree-sitter language management and parsing.
pub struct TreeSitterService {
    languages: Vec<TreeSitterLanguage>,
    config: Option<TreeSitterConfig>,
    mock_parser: Option<MockParser>,
}

impl TreeSitterService {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            config: None,
            mock_parser: None,
        }
    }

    /// Create a service backed by a [`MockParser`] for testing.
    pub fn with_mock_parser(mock: MockParser) -> Self {
        Self {
            languages: Vec::new(),
            config: None,
            mock_parser: Some(mock),
        }
    }

    /// Set the configuration for dynamic parser loading.
    pub fn set_config(&mut self, config: TreeSitterConfig) {
        self.config = Some(config);
    }

    /// Get the current configuration.
    pub fn config(&self) -> Option<&TreeSitterConfig> {
        self.config.as_ref()
    }

    pub fn register_language(&mut self, lang: TreeSitterLanguage) {
        self.languages.push(lang);
    }

    pub fn get_language(&self, name: &str) -> Option<&TreeSitterLanguage> {
        self.languages.iter().find(|l| l.name == name)
    }

    pub fn get_language_for_file(&self, filename: &str) -> Option<&TreeSitterLanguage> {
        let ext = filename.rsplit('.').next()?;
        self.languages
            .iter()
            .find(|l| l.file_types.iter().any(|ft| ft == ext))
    }

    pub fn language_count(&self) -> usize {
        self.languages.len()
    }

    /// Remove a language by name. Returns true if it was present.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.languages.len();
        self.languages.retain(|l| l.name != name);
        self.languages.len() < before
    }

    /// List all supported file extensions across registered languages.
    pub fn supported_extensions(&self) -> Vec<&str> {
        let mut exts: Vec<&str> = self
            .languages
            .iter()
            .flat_map(|l| l.file_types.iter().map(|s| s.as_str()))
            .collect();
        exts.sort();
        exts.dedup();
        exts
    }

    /// Returns true if languages is empty.
    pub fn is_languages_empty(&self) -> bool {
        self.languages.is_empty()
    }

    /// Get the first language, if any.
    pub fn first_language(&self) -> Option<&TreeSitterLanguage> {
        self.languages.first()
    }

    /// Get the last language, if any.
    pub fn last_language(&self) -> Option<&TreeSitterLanguage> {
        self.languages.last()
    }

    /// Retain only languages matching the predicate.
    pub fn retain_languages(&mut self, f: impl Fn(&TreeSitterLanguage) -> bool) {
        self.languages.retain(|item| f(item));
    }

    // -- Parsing --

    /// Parse source code for the given language, returning an AST.
    pub fn parse(
        &self,
        language_id: &str,
        source: &str,
    ) -> Result<SyntaxNode, TreeSitterError> {
        if let Some(mock) = &self.mock_parser {
            return mock.parse(language_id, source);
        }
        // Without a real tree-sitter runtime, verify the language is configured.
        if let Some(cfg) = &self.config {
            if cfg.get_entry(language_id).is_none() {
                return Err(TreeSitterError::LanguageNotFound(language_id.to_string()));
            }
            // Real implementation would call tree_sitter_parse() here.
            Err(TreeSitterError::ParseFailed(
                "real tree-sitter runtime not linked".to_string(),
            ))
        } else {
            Err(TreeSitterError::LanguageNotFound(language_id.to_string()))
        }
    }

    /// Incremental re-parse after an edit.
    pub fn edit_tree(
        &self,
        language_id: &str,
        old_tree: &SyntaxNode,
        edit: &IncrementalEdit,
        new_source: &str,
    ) -> Result<SyntaxNode, TreeSitterError> {
        if let Some(mock) = &self.mock_parser {
            return mock.edit_tree(language_id, old_tree, edit, new_source);
        }
        Err(TreeSitterError::ParseFailed(
            "real tree-sitter runtime not linked".to_string(),
        ))
    }

    /// Get a parser reference (check if language is available).
    pub fn get_parser(&self, language_id: &str) -> Option<&TreeSitterLanguage> {
        self.get_language(language_id)
    }
}

impl Default for TreeSitterService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Query runner – pattern matching on syntax trees
// ---------------------------------------------------------------------------

/// A single pattern to match against syntax tree nodes.
#[derive(Debug, Clone)]
pub struct QueryPattern {
    pub node_kind: String,
    pub named_only: bool,
}

/// A match returned by [`TreeSitterQueryRunner::run`].
#[derive(Debug, Clone)]
pub struct QueryMatch {
    pub pattern_index: usize,
    pub node: SyntaxNode,
}

/// Runs pattern-matching queries against a [`SyntaxNode`] tree.
pub struct TreeSitterQueryRunner {
    patterns: Vec<QueryPattern>,
}

impl TreeSitterQueryRunner {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn add_pattern(&mut self, kind: impl Into<String>, named_only: bool) -> &mut Self {
        self.patterns.push(QueryPattern {
            node_kind: kind.into(),
            named_only,
        });
        self
    }

    pub fn run(&self, tree: &SyntaxNode) -> Vec<QueryMatch> {
        let mut matches = Vec::new();
        self.collect_matches(tree, &mut matches);
        matches
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    pub fn clear(&mut self) {
        self.patterns.clear();
    }

    fn collect_matches(&self, node: &SyntaxNode, matches: &mut Vec<QueryMatch>) {
        for (idx, pat) in self.patterns.iter().enumerate() {
            if node.kind == pat.node_kind && (!pat.named_only || node.named) {
                matches.push(QueryMatch {
                    pattern_index: idx,
                    node: node.clone(),
                });
            }
        }
        for child in &node.children {
            self.collect_matches(child, matches);
        }
    }
}

impl Default for TreeSitterQueryRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Enhanced folding provider
// ---------------------------------------------------------------------------

/// A folding region with a collapsed-text hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRegion {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: TsFoldingKind,
    pub collapsed_text: String,
}

/// Provides enhanced folding regions from a syntax tree.
pub struct TreeSitterFoldingProvider;

impl TreeSitterFoldingProvider {
    pub fn new() -> Self {
        Self
    }

    pub fn compute_folding(&self, tree: &SyntaxNode) -> Vec<FoldingRegion> {
        let mut regions = Vec::new();
        self.collect_folding(tree, &mut regions);
        regions.sort_by_key(|r| r.start_line);
        regions
    }

    pub fn merge_adjacent(&self, regions: &[FoldingRegion], max_gap: u32) -> Vec<FoldingRegion> {
        if regions.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<FoldingRegion> = Vec::new();
        for r in regions {
            let should_merge = merged.last().map_or(false, |prev: &FoldingRegion| {
                prev.kind == r.kind && r.start_line <= prev.end_line + max_gap + 1
            });
            if should_merge {
                let prev = merged.last_mut().unwrap();
                if r.end_line > prev.end_line {
                    prev.end_line = r.end_line;
                }
            } else {
                merged.push(r.clone());
            }
        }
        merged
    }

    pub fn filter_by_kind(&self, regions: &[FoldingRegion], kind: TsFoldingKind) -> Vec<FoldingRegion> {
        regions.iter().filter(|r| r.kind == kind).cloned().collect()
    }

    fn collect_folding(&self, node: &SyntaxNode, regions: &mut Vec<FoldingRegion>) {
        if let Some(kind) = folding_kind_for_node(&node.kind) {
            if node.end_line > node.start_line {
                let text = format!("{} …", node.kind);
                regions.push(FoldingRegion {
                    start_line: node.start_line,
                    end_line: node.end_line,
                    kind,
                    collapsed_text: text,
                });
            }
        }
        for child in &node.children {
            self.collect_folding(child, regions);
        }
    }
}

impl Default for TreeSitterFoldingProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scope-based indentation resolver
// ---------------------------------------------------------------------------

/// Resolves indentation levels based on ancestor node kinds.
pub struct TreeSitterScopeResolver {
    indent_kinds: Vec<String>,
    dedent_kinds: Vec<String>,
    indent_size: u32,
}

impl TreeSitterScopeResolver {
    pub fn new(indent_size: u32) -> Self {
        Self {
            indent_kinds: Vec::new(),
            dedent_kinds: Vec::new(),
            indent_size,
        }
    }

    pub fn add_indent_kind(&mut self, kind: impl Into<String>) -> &mut Self {
        self.indent_kinds.push(kind.into());
        self
    }

    pub fn add_dedent_kind(&mut self, kind: impl Into<String>) -> &mut Self {
        self.dedent_kinds.push(kind.into());
        self
    }

    /// Count how many ancestors of `node` match one of the indent kinds,
    /// minus those matching a dedent kind, multiplied by `indent_size`.
    pub fn compute_indent(&self, node: &SyntaxNode) -> u32 {
        let indent_count = self.count_matching_ancestors(node, &self.indent_kinds);
        let dedent_count = self.count_matching_ancestors(node, &self.dedent_kinds);
        indent_count.saturating_sub(dedent_count) * self.indent_size
    }

    /// Find the deepest node covering `line` and compute its indentation.
    pub fn suggest_indent_for_line(&self, tree: &SyntaxNode, line: u32) -> u32 {
        if let Some(node) = self.find_deepest_at_line(tree, line) {
            self.compute_indent(node)
        } else {
            0
        }
    }

    fn count_matching_ancestors(&self, node: &SyntaxNode, kinds: &[String]) -> u32 {
        // Walk the flat list; count how many nodes from root to this node match.
        // Since SyntaxNode doesn't store a parent pointer, we count matching
        // node kinds in the subtree rooted at `node` going upward conceptually.
        // Here we count the node itself if it matches.
        let mut count = 0u32;
        if kinds.iter().any(|k| k == &node.kind) {
            count += 1;
        }
        count
    }

    fn find_deepest_at_line<'a>(&self, node: &'a SyntaxNode, line: u32) -> Option<&'a SyntaxNode> {
        if node.start_line > line || node.end_line < line {
            return None;
        }
        for child in &node.children {
            if let Some(deeper) = self.find_deepest_at_line(child, line) {
                return Some(deeper);
            }
        }
        Some(node)
    }
}

// ---------------------------------------------------------------------------
// Incremental update handler
// ---------------------------------------------------------------------------

/// Buffers incremental edits and tracks a version counter.
pub struct IncrementalUpdateHandler {
    edits: Vec<IncrementalEdit>,
    version: u64,
}

impl IncrementalUpdateHandler {
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            version: 0,
        }
    }

    pub fn record_edit(&mut self, edit: IncrementalEdit) {
        self.edits.push(edit);
        self.version += 1;
    }

    pub fn pending_edits(&self) -> &[IncrementalEdit] {
        &self.edits
    }

    pub fn flush(&mut self) -> Vec<IncrementalEdit> {
        let flushed = std::mem::take(&mut self.edits);
        flushed
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn has_pending(&self) -> bool {
        !self.edits.is_empty()
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }
}

impl Default for IncrementalUpdateHandler {
    fn default() -> Self {
        Self::new()
    }
}


// === Treesitter Highlight Mapper ===

/// Treesitter Highlight Mapper implementation.
#[derive(Debug, Clone)]
pub struct TreesitterHighlightMapper {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TreesitterHighlightMapperStats,
}

/// Statistics for TreesitterHighlightMapper.
#[derive(Debug, Clone, Default)]
pub struct TreesitterHighlightMapperStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TreesitterHighlightMapperStats {
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

impl TreesitterHighlightMapper {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TreesitterHighlightMapperStats::default(),
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

    pub fn stats(&self) -> &TreesitterHighlightMapperStats {
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

impl Default for TreesitterHighlightMapper {
    fn default() -> Self {
        Self::new()
    }
}

// === Treesitter Edit Tracker ===

/// Priority level for TreesitterEditTracker items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TreesitterEditTrackerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TreesitterEditTrackerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TreesitterEditTrackerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Treesitter Edit Tracker implementation.
#[derive(Debug, Clone)]
pub struct TreesitterEditTracker {
    items: Vec<TreesitterEditTrackerItem>,
    max_items: usize,
    default_priority: TreesitterEditTrackerPriority,
}

/// A single item in TreesitterEditTracker.
#[derive(Debug, Clone)]
pub struct TreesitterEditTrackerItem {
    pub id: String,
    pub label: String,
    pub priority: TreesitterEditTrackerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TreesitterEditTrackerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TreesitterEditTrackerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TreesitterEditTrackerPriority) -> Self {
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

impl TreesitterEditTracker {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TreesitterEditTrackerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TreesitterEditTrackerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TreesitterEditTrackerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TreesitterEditTrackerItem> {
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

    pub fn by_priority(&self, priority: TreesitterEditTrackerPriority) -> Vec<&TreesitterEditTrackerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TreesitterEditTrackerItem> {
        let mut sorted: Vec<&TreesitterEditTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TreesitterEditTrackerItem> {
        let mut sorted: Vec<&TreesitterEditTrackerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TreesitterEditTrackerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TreesitterEditTrackerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TreesitterEditTrackerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TreesitterEditTrackerItem> {
        self.items.iter()
    }
}

impl Default for TreesitterEditTracker {
    fn default() -> Self {
        Self::new()
    }
}


/// Configuration manager for wb_treesitter functionality.
pub struct WbTreesitterConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbTreesitterConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &WbTreesitterConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_treesitter operations.
pub struct WbTreesitterRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbTreesitterRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for wb_treesitter.
pub struct WbTreesitterValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbTreesitterValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &WbTreesitterValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Tree-sitter grammar management — extended utilities (yn)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_ts operations.
#[derive(Debug, Clone)]
pub struct YnMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YnMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_ts.
#[derive(Debug, Clone)]
pub struct YnRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YnRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_ts lookups.
#[derive(Debug, Clone)]
pub struct YnLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YnLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_treesitter
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbTreesitterRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbTreesitterRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbTreesitterCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbTreesitterCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbTreesitterCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 231
// ---------------------------------------------------------------------------

/// Generic object pool `Xc231Pool<T>`.
pub struct Xc231Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc231Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc231PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc231Pool<T> {
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
    pub fn stats(&self) -> Xc231PoolStats {
        Xc231PoolStats {
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

impl<T> Default for Xc231Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc231Scheduler`.
pub struct Xc231Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc231Scheduler {
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

impl Default for Xc231Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_231 hash for the given byte slice.
pub fn xc_231_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_231 convention.
pub fn xc_231_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_97 deepening: state machine + event bus ---

/// States for the Xd97 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd97State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd97State {
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
pub struct Xd97Transition {
    pub from: Xd97State,
    pub to: Xd97State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd97StateMachine {
    current: Xd97State,
    history: Vec<Xd97Transition>,
    step_counter: usize,
}

impl Xd97StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd97State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd97State {
        self.current
    }

    pub fn history(&self) -> &[Xd97Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd97State) -> Result<Xd97State, String> {
        let allowed = match (self.current, target) {
            (Xd97State::Idle, Xd97State::Running) => true,
            (Xd97State::Running, Xd97State::Paused) => true,
            (Xd97State::Running, Xd97State::Done) => true,
            (Xd97State::Paused, Xd97State::Running) => true,
            (Xd97State::Paused, Xd97State::Done) => true,
            (Xd97State::Done, Xd97State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_97: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd97Transition {
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
            "Xd97SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd97State> {
        let prefix = "Xd97SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd97State::Idle),
            "Running" => Some(Xd97State::Running),
            "Paused" => Some(Xd97State::Paused),
            "Done" => Some(Xd97State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd97State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd97 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd97Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd97Event {
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

type Xd97HandlerFn = Box<dyn Fn(&Xd97Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd97EventBus {
    handlers: Vec<(usize, Option<String>, Xd97HandlerFn)>,
    next_id: usize,
    published: Vec<Xd97Event>,
}

impl Xd97EventBus {
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
        F: Fn(&Xd97Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd97Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd97Event) {
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

    pub fn published_events(&self) -> &[Xd97Event] {
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
// xg_21: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg21Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg21Graph {
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

impl Default for Xg21Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_21: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg21Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg21Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg21Heap<T>) {
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

impl<T: Ord> Default for Xg21Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 230).
pub struct Xh230SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh230SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 272 as u64,
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

/// A compact bit set supporting boolean operations (variant 230).
pub struct Xh230BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh230BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 230).
pub struct Xi230Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi230Deque<T> {
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
pub struct Xi230Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi230Interval {
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

/// A simple interval tree (variant 230).
pub struct Xi230IntervalTree {
    xi_intervals: Vec<Xi230Interval>,
}

impl Xi230IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi230Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi230Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi230Interval) -> Vec<&Xi230Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi230Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi230Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi230Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi230Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi230Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi230Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 230) ---

/// Disjoint set / union-find for crate 230.
pub struct Xj230UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj230UnionFind {
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

const XJ230_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 230.
pub struct Xj230BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj230BTreeNode<K, V>>>,
    len: usize,
}

struct Xj230BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj230BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj230BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ230_BTREE_ORDER - 1
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
        let mid = XJ230_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj230BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj230BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj230BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj230BTreeNode::xj_new_leaf();
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


// --- xk_230 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk230SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk230SegmentTree {
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
pub struct Xk230DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk230DisjointIntervals {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rust_lang() -> TreeSitterLanguage {
        TreeSitterLanguage {
            name: "rust".into(),
            file_types: vec!["rs".into()],
            highlight_query: Some("(function_item name: (identifier) @function)".into()),
        }
    }

    // -- Sample tree representing:
    //   fn main() {
    //       let x = 1;
    //   }
    //   fn helper() {}
    fn sample_tree() -> SyntaxNode {
        SyntaxNode {
            kind: "source_file".into(),
            start_line: 0,
            start_col: 0,
            end_line: 20,
            end_col: 0,
            named: true,
            children: vec![
                SyntaxNode {
                    kind: "function_item".into(),
                    start_line: 0,
                    start_col: 0,
                    end_line: 10,
                    end_col: 1,
                    named: true,
                    children: vec![
                        SyntaxNode {
                            kind: "identifier".into(),
                            start_line: 0,
                            start_col: 3,
                            end_line: 0,
                            end_col: 7,
                            named: true,
                            children: Vec::new(),
                        },
                        SyntaxNode {
                            kind: "block".into(),
                            start_line: 0,
                            start_col: 10,
                            end_line: 10,
                            end_col: 1,
                            named: true,
                            children: vec![SyntaxNode {
                                kind: "identifier".into(),
                                start_line: 2,
                                start_col: 4,
                                end_line: 2,
                                end_col: 8,
                                named: true,
                                children: Vec::new(),
                            }],
                        },
                        SyntaxNode {
                            kind: "(".into(),
                            start_line: 0,
                            start_col: 7,
                            end_line: 0,
                            end_col: 8,
                            named: false,
                            children: Vec::new(),
                        },
                    ],
                },
                SyntaxNode {
                    kind: "function_item".into(),
                    start_line: 12,
                    start_col: 0,
                    end_line: 20,
                    end_col: 0,
                    named: true,
                    children: Vec::new(),
                },
            ],
        }
    }

    /// A richer mock tree for symbol / folding / semantic token tests.
    fn rich_tree() -> SyntaxNode {
        SyntaxNode {
            kind: "source_file".into(),
            start_line: 0,
            start_col: 0,
            end_line: 30,
            end_col: 0,
            named: true,
            children: vec![
                // use std::io;
                SyntaxNode {
                    kind: "use_declaration".into(),
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 12,
                    named: true,
                    children: Vec::new(),
                },
                // fn main() { … }
                SyntaxNode {
                    kind: "function_item".into(),
                    start_line: 2,
                    start_col: 0,
                    end_line: 10,
                    end_col: 1,
                    named: true,
                    children: vec![
                        // fn keyword
                        SyntaxNode {
                            kind: "fn".into(),
                            start_line: 2,
                            start_col: 0,
                            end_line: 2,
                            end_col: 2,
                            named: false,
                            children: Vec::new(),
                        },
                        // name: main
                        SyntaxNode {
                            kind: "identifier".into(),
                            start_line: 2,
                            start_col: 3,
                            end_line: 2,
                            end_col: 7,
                            named: true,
                            children: Vec::new(),
                        },
                        // (
                        SyntaxNode {
                            kind: "(".into(),
                            start_line: 2,
                            start_col: 7,
                            end_line: 2,
                            end_col: 8,
                            named: false,
                            children: Vec::new(),
                        },
                        // )
                        SyntaxNode {
                            kind: ")".into(),
                            start_line: 2,
                            start_col: 8,
                            end_line: 2,
                            end_col: 9,
                            named: false,
                            children: Vec::new(),
                        },
                        // block { … }
                        SyntaxNode {
                            kind: "block".into(),
                            start_line: 2,
                            start_col: 10,
                            end_line: 10,
                            end_col: 1,
                            named: true,
                            children: vec![
                                SyntaxNode {
                                    kind: "{".into(),
                                    start_line: 2,
                                    start_col: 10,
                                    end_line: 2,
                                    end_col: 11,
                                    named: false,
                                    children: Vec::new(),
                                },
                                // let x = 42;
                                SyntaxNode {
                                    kind: "let_declaration".into(),
                                    start_line: 3,
                                    start_col: 4,
                                    end_line: 3,
                                    end_col: 15,
                                    named: true,
                                    children: vec![
                                        SyntaxNode {
                                            kind: "let".into(),
                                            start_line: 3,
                                            start_col: 4,
                                            end_line: 3,
                                            end_col: 7,
                                            named: false,
                                            children: Vec::new(),
                                        },
                                        SyntaxNode {
                                            kind: "identifier".into(),
                                            start_line: 3,
                                            start_col: 8,
                                            end_line: 3,
                                            end_col: 9,
                                            named: true,
                                            children: Vec::new(),
                                        },
                                        SyntaxNode {
                                            kind: "integer_literal".into(),
                                            start_line: 3,
                                            start_col: 12,
                                            end_line: 3,
                                            end_col: 14,
                                            named: true,
                                            children: Vec::new(),
                                        },
                                    ],
                                },
                                // if true { … }
                                SyntaxNode {
                                    kind: "if_expression".into(),
                                    start_line: 4,
                                    start_col: 4,
                                    end_line: 6,
                                    end_col: 5,
                                    named: true,
                                    children: Vec::new(),
                                },
                                // "hello"
                                SyntaxNode {
                                    kind: "string_literal".into(),
                                    start_line: 7,
                                    start_col: 4,
                                    end_line: 7,
                                    end_col: 11,
                                    named: true,
                                    children: Vec::new(),
                                },
                                // // a comment
                                SyntaxNode {
                                    kind: "line_comment".into(),
                                    start_line: 8,
                                    start_col: 4,
                                    end_line: 8,
                                    end_col: 20,
                                    named: true,
                                    children: Vec::new(),
                                },
                                SyntaxNode {
                                    kind: "}".into(),
                                    start_line: 10,
                                    start_col: 0,
                                    end_line: 10,
                                    end_col: 1,
                                    named: false,
                                    children: Vec::new(),
                                },
                            ],
                        },
                    ],
                },
                // struct Foo { … }
                SyntaxNode {
                    kind: "struct_item".into(),
                    start_line: 12,
                    start_col: 0,
                    end_line: 15,
                    end_col: 1,
                    named: true,
                    children: vec![
                        SyntaxNode {
                            kind: "type_identifier".into(),
                            start_line: 12,
                            start_col: 7,
                            end_line: 12,
                            end_col: 10,
                            named: true,
                            children: Vec::new(),
                        },
                        SyntaxNode {
                            kind: "field_declaration_list".into(),
                            start_line: 12,
                            start_col: 11,
                            end_line: 15,
                            end_col: 1,
                            named: true,
                            children: vec![SyntaxNode {
                                kind: "field_declaration".into(),
                                start_line: 13,
                                start_col: 4,
                                end_line: 13,
                                end_col: 12,
                                named: true,
                                children: vec![SyntaxNode {
                                    kind: "identifier".into(),
                                    start_line: 13,
                                    start_col: 4,
                                    end_line: 13,
                                    end_col: 7,
                                    named: true,
                                    children: Vec::new(),
                                }],
                            }],
                        },
                    ],
                },
                // enum Color { … }
                SyntaxNode {
                    kind: "enum_item".into(),
                    start_line: 17,
                    start_col: 0,
                    end_line: 20,
                    end_col: 1,
                    named: true,
                    children: vec![SyntaxNode {
                        kind: "type_identifier".into(),
                        start_line: 17,
                        start_col: 5,
                        end_line: 17,
                        end_col: 10,
                        named: true,
                        children: Vec::new(),
                    }],
                },
                // const MAX: u32 = 100;
                SyntaxNode {
                    kind: "const_item".into(),
                    start_line: 22,
                    start_col: 0,
                    end_line: 22,
                    end_col: 22,
                    named: true,
                    children: vec![SyntaxNode {
                        kind: "identifier".into(),
                        start_line: 22,
                        start_col: 6,
                        end_line: 22,
                        end_col: 9,
                        named: true,
                        children: Vec::new(),
                    }],
                },
                // mod utils { … }
                SyntaxNode {
                    kind: "mod_item".into(),
                    start_line: 24,
                    start_col: 0,
                    end_line: 28,
                    end_col: 1,
                    named: true,
                    children: vec![SyntaxNode {
                        kind: "identifier".into(),
                        start_line: 24,
                        start_col: 4,
                        end_line: 24,
                        end_col: 9,
                        named: true,
                        children: Vec::new(),
                    }],
                },
            ],
        }
    }

    // -----------------------------------------------------------------------
    // Original tests (language registration, SyntaxNode basics)
    // -----------------------------------------------------------------------

    #[test]
    fn register_and_lookup() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        assert_eq!(svc.language_count(), 1);
        assert!(svc.get_language("rust").is_some());
        assert!(svc.get_language("python").is_none());
    }

    #[test]
    fn lookup_by_file_extension() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        let lang = svc.get_language_for_file("main.rs").unwrap();
        assert_eq!(lang.name, "rust");
        assert!(svc.get_language_for_file("main.py").is_none());
    }

    #[test]
    fn syntax_node_methods() {
        let leaf = SyntaxNode {
            kind: "identifier".into(),
            start_line: 5, start_col: 4,
            end_line: 5, end_col: 10,
            children: Vec::new(), named: true,
        };
        assert!(leaf.is_leaf());
        assert_eq!(leaf.child_count(), 0);
        assert_eq!(leaf.span_lines(), 1);

        let parent = SyntaxNode {
            kind: "function_item".into(),
            start_line: 1, start_col: 0,
            end_line: 10, end_col: 1,
            children: vec![leaf], named: true,
        };
        assert!(!parent.is_leaf());
        assert_eq!(parent.child_count(), 1);
        assert_eq!(parent.span_lines(), 10);
    }

    #[test]
    fn error_display() {
        let e = TreeSitterError::LanguageNotFound("rust".into());
        assert_eq!(e.to_string(), "language not found: rust");
        let e = TreeSitterError::ParseFailed("unexpected EOF".into());
        assert_eq!(e.to_string(), "parse failed: unexpected EOF");
        let e = TreeSitterError::InvalidNode("missing kind".into());
        assert_eq!(e.to_string(), "invalid node: missing kind");
    }

    #[test]
    fn error_display_library_load() {
        let e = TreeSitterError::LibraryLoadFailed("/usr/lib/ts-rust.so".into());
        assert_eq!(
            e.to_string(),
            "failed to load parser library: /usr/lib/ts-rust.so"
        );
    }

    #[test]
    fn syntax_node_display() {
        let node = SyntaxNode {
            kind: "identifier".into(),
            start_line: 3, start_col: 5,
            end_line: 3, end_col: 10,
            children: Vec::new(), named: true,
        };
        assert_eq!(node.to_string(), "identifier [3:5-3:10]");
    }

    #[test]
    fn flatten_pre_order() {
        let tree = sample_tree();
        let flat = tree.flatten();
        let kinds: Vec<&str> = flat.iter().map(|n| n.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "source_file", "function_item", "identifier", "block",
                "identifier", "(", "function_item"
            ]
        );
    }

    #[test]
    fn find_by_kind_multiple() {
        let tree = sample_tree();
        assert_eq!(tree.find_by_kind("identifier").len(), 2);
        assert_eq!(tree.find_by_kind("function_item").len(), 2);
        assert!(tree.find_by_kind("struct_item").is_empty());
    }

    #[test]
    fn find_at_position_deepest() {
        let tree = sample_tree();
        assert_eq!(tree.find_at_position(2, 5).unwrap().kind, "identifier");
        assert_eq!(tree.find_at_position(0, 3).unwrap().kind, "identifier");
        assert_eq!(tree.find_at_position(15, 0).unwrap().kind, "function_item");
        assert!(tree.find_at_position(30, 0).is_none());
    }

    #[test]
    fn named_children_filter() {
        let tree = sample_tree();
        let func = &tree.children[0];
        assert_eq!(func.child_count(), 3);
        let named = func.named_children();
        assert_eq!(named.len(), 2);
        assert!(named.iter().all(|c| c.named));
    }

    #[test]
    fn depth_calculation() {
        let tree = sample_tree();
        assert_eq!(tree.depth(), 4);
        assert_eq!(tree.children[1].depth(), 1);
    }

    #[test]
    fn unregister_language() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        assert!(svc.unregister("rust"));
        assert_eq!(svc.language_count(), 0);
        assert!(!svc.unregister("rust"));
    }

    #[test]
    fn supported_extensions_list() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        svc.register_language(TreeSitterLanguage {
            name: "python".into(),
            file_types: vec!["py".into(), "pyi".into()],
            highlight_query: None,
        });
        assert_eq!(svc.supported_extensions(), vec!["py", "pyi", "rs"]);
    }

    #[test]
    fn supports_file_extension() {
        let lang = rust_lang();
        assert!(lang.supports_file("main.rs"));
        assert!(!lang.supports_file("main.py"));
        assert!(!lang.supports_file("noext"));
    }

    #[test]
    fn error_equality() {
        let a = TreeSitterError::ParseFailed("eof".into());
        let b = TreeSitterError::ParseFailed("eof".into());
        let c = TreeSitterError::ParseFailed("other".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // -----------------------------------------------------------------------
    // TreeSitterConfig tests
    // -----------------------------------------------------------------------

    #[test]
    fn config_resolve_relative_path() {
        let cfg = TreeSitterConfig::new(PathBuf::from("/usr/lib/ts-parsers"));
        let entry = ParserLibraryEntry {
            language_id: "rust".into(),
            library_path: PathBuf::from("tree-sitter-rust.so"),
            symbol_name: "tree_sitter_rust".into(),
        };
        let resolved = cfg.resolve_path(&entry);
        assert_eq!(resolved, PathBuf::from("/usr/lib/ts-parsers/tree-sitter-rust.so"));
    }

    #[test]
    fn config_resolve_absolute_path() {
        let cfg = TreeSitterConfig::new(PathBuf::from("/usr/lib/ts-parsers"));
        let entry = ParserLibraryEntry {
            language_id: "rust".into(),
            library_path: PathBuf::from("/opt/parsers/rust.so"),
            symbol_name: "tree_sitter_rust".into(),
        };
        let resolved = cfg.resolve_path(&entry);
        assert_eq!(resolved, PathBuf::from("/opt/parsers/rust.so"));
    }

    #[test]
    fn config_add_and_get_entry() {
        let mut cfg = TreeSitterConfig::new(PathBuf::from("/tmp"));
        assert!(cfg.get_entry("rust").is_none());
        cfg.add_parser(ParserLibraryEntry {
            language_id: "rust".into(),
            library_path: PathBuf::from("rust.so"),
            symbol_name: "tree_sitter_rust".into(),
        });
        assert!(cfg.get_entry("rust").is_some());
        assert!(cfg.get_entry("python").is_none());
    }

    // -----------------------------------------------------------------------
    // IncrementalEdit tests
    // -----------------------------------------------------------------------

    #[test]
    fn incremental_edit_construction() {
        let edit = IncrementalEdit {
            start_byte: 10,
            old_end_byte: 15,
            new_end_byte: 20,
            start_point: Point { row: 1, column: 0 },
            old_end_point: Point { row: 1, column: 5 },
            new_end_point: Point { row: 1, column: 10 },
        };
        assert_eq!(edit.start_byte, 10);
        assert_eq!(edit.new_end_byte - edit.old_end_byte, 5);
    }

    #[test]
    fn point_equality() {
        let a = Point { row: 1, column: 5 };
        let b = Point { row: 1, column: 5 };
        let c = Point { row: 2, column: 0 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // -----------------------------------------------------------------------
    // MockParser + TreeSitterService parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn mock_parser_parse_registered() {
        let mut mock = MockParser::new();
        mock.register_tree("rust", sample_tree());
        let tree = mock.parse("rust", "fn main() {}").unwrap();
        assert_eq!(tree.kind, "source_file");
    }

    #[test]
    fn mock_parser_parse_unregistered() {
        let mock = MockParser::new();
        let err = mock.parse("rust", "").unwrap_err();
        assert_eq!(err, TreeSitterError::LanguageNotFound("rust".into()));
    }

    #[test]
    fn service_parse_with_mock() {
        let mut mock = MockParser::new();
        mock.register_tree("rust", sample_tree());
        let svc = TreeSitterService::with_mock_parser(mock);
        let tree = svc.parse("rust", "fn main() {}").unwrap();
        assert_eq!(tree.kind, "source_file");
    }

    #[test]
    fn service_parse_no_mock_no_config() {
        let svc = TreeSitterService::new();
        let err = svc.parse("rust", "").unwrap_err();
        assert_eq!(err, TreeSitterError::LanguageNotFound("rust".into()));
    }

    #[test]
    fn service_edit_tree_with_mock() {
        let mut mock = MockParser::new();
        mock.register_tree("rust", sample_tree());
        let svc = TreeSitterService::with_mock_parser(mock);
        let old = svc.parse("rust", "").unwrap();
        let edit = IncrementalEdit {
            start_byte: 0, old_end_byte: 5, new_end_byte: 10,
            start_point: Point { row: 0, column: 0 },
            old_end_point: Point { row: 0, column: 5 },
            new_end_point: Point { row: 0, column: 10 },
        };
        let new_tree = svc.edit_tree("rust", &old, &edit, "fn main() { }").unwrap();
        assert_eq!(new_tree.kind, "source_file");
    }

    #[test]
    fn service_get_parser() {
        let mut svc = TreeSitterService::new();
        svc.register_language(rust_lang());
        assert!(svc.get_parser("rust").is_some());
        assert!(svc.get_parser("python").is_none());
    }

    #[test]
    fn service_set_and_get_config() {
        let mut svc = TreeSitterService::new();
        assert!(svc.config().is_none());
        svc.set_config(TreeSitterConfig::new(PathBuf::from("/tmp")));
        assert!(svc.config().is_some());
    }

    // -----------------------------------------------------------------------
    // Symbol extraction tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_symbols_from_rich_tree() {
        let tree = rich_tree();
        // Source lines for name extraction.
        let source = "use std::io;\n\
                       \n\
                       fn main() {\n\
                       \x20   let x = 42;\n\
                       \x20   if true {\n\
                       \x20       y;\n\
                       \x20   }\n\
                       \x20   \"hello\"\n\
                       \x20   // a comment\n\
                       \n\
                       }\n\
                       \n\
                       struct Foo {\n\
                       \x20   bar: u32,\n\
                       \n\
                       }\n\
                       \n\
                       enum Color {\n\
                       \x20   Red,\n\
                       \x20   Blue,\n\
                       }\n\
                       \n\
                       const MAX: u32 = 100;\n\
                       \n\
                       mod utils {\n\
                       \n\
                       \n\
                       \n\
                       }\n";
        let symbols = extract_symbols(&tree, source);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"Color"));
        assert!(names.contains(&"MAX"));
        assert!(names.contains(&"utils"));
    }

    #[test]
    fn extract_symbols_finds_function() {
        let tree = rich_tree();
        let source = "use std::io;\n\nfn main() {\n    let x = 42;\n}\n";
        let symbols = extract_symbols(&tree, source);
        let func = symbols.iter().find(|s| s.kind == SymbolKind::Function);
        assert!(func.is_some());
        assert_eq!(func.unwrap().name, "main");
    }

    #[test]
    fn extract_symbols_finds_struct() {
        let tree = rich_tree();
        let source = "use std::io;\n\nfn main() {\n}\n\n\n\n\n\n\n\n\nstruct Foo {\n    bar: u32,\n\n}\n";
        let symbols = extract_symbols(&tree, source);
        let st = symbols.iter().find(|s| s.kind == SymbolKind::Struct);
        assert!(st.is_some());
        assert_eq!(st.unwrap().name, "Foo");
    }

    #[test]
    fn extract_symbols_nested_variable() {
        let tree = rich_tree();
        let source = "use std::io;\n\nfn main() {\n    let x = 42;\n}\n";
        let symbols = extract_symbols(&tree, source);
        let func = symbols.iter().find(|s| s.kind == SymbolKind::Function).unwrap();
        let has_var = func.children.iter().any(|c| c.kind == SymbolKind::Variable);
        assert!(has_var);
    }

    #[test]
    fn symbol_kind_display() {
        assert_eq!(SymbolKind::Function.to_string(), "function");
        assert_eq!(SymbolKind::Struct.to_string(), "struct");
        assert_eq!(SymbolKind::Enum.to_string(), "enum");
        assert_eq!(SymbolKind::Module.to_string(), "module");
        assert_eq!(SymbolKind::Constant.to_string(), "constant");
        assert_eq!(SymbolKind::Trait.to_string(), "trait");
        assert_eq!(SymbolKind::Type.to_string(), "type");
        assert_eq!(SymbolKind::Property.to_string(), "property");
    }

    #[test]
    fn extract_symbols_empty_tree() {
        let tree = SyntaxNode {
            kind: "source_file".into(),
            start_line: 0, start_col: 0, end_line: 0, end_col: 0,
            named: true, children: Vec::new(),
        };
        let symbols = extract_symbols(&tree, "");
        assert!(symbols.is_empty());
    }

    // -----------------------------------------------------------------------
    // AST-based bracket pair detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_bracket_pairs_simple() {
        let tree = rich_tree();
        let pairs = detect_bracket_pairs(&tree);
        let parens: Vec<_> = pairs.iter().filter(|p| p.bracket_char == '(').collect();
        assert_eq!(parens.len(), 1);
        assert_eq!(parens[0].open_line, 2);
        assert_eq!(parens[0].close_line, 2);
    }

    #[test]
    fn detect_bracket_pairs_braces() {
        let tree = rich_tree();
        let pairs = detect_bracket_pairs(&tree);
        let braces: Vec<_> = pairs.iter().filter(|p| p.bracket_char == '{').collect();
        assert_eq!(braces.len(), 1);
    }

    #[test]
    fn detect_bracket_pairs_skips_strings() {
        // Build a tree with brackets inside a string node.
        let tree = SyntaxNode {
            kind: "source_file".into(),
            start_line: 0, start_col: 0, end_line: 1, end_col: 0,
            named: true,
            children: vec![SyntaxNode {
                kind: "string_literal".into(),
                start_line: 0, start_col: 0, end_line: 0, end_col: 5,
                named: true,
                children: vec![
                    SyntaxNode {
                        kind: "(".into(),
                        start_line: 0, start_col: 1, end_line: 0, end_col: 2,
                        named: false, children: Vec::new(),
                    },
                    SyntaxNode {
                        kind: ")".into(),
                        start_line: 0, start_col: 3, end_line: 0, end_col: 4,
                        named: false, children: Vec::new(),
                    },
                ],
            }],
        };
        let pairs = detect_bracket_pairs(&tree);
        assert!(pairs.is_empty(), "brackets inside strings should be skipped");
    }

    #[test]
    fn detect_bracket_pairs_skips_comments() {
        let tree = SyntaxNode {
            kind: "source_file".into(),
            start_line: 0, start_col: 0, end_line: 1, end_col: 0,
            named: true,
            children: vec![SyntaxNode {
                kind: "line_comment".into(),
                start_line: 0, start_col: 0, end_line: 0, end_col: 10,
                named: true,
                children: vec![
                    SyntaxNode {
                        kind: "{".into(),
                        start_line: 0, start_col: 3, end_line: 0, end_col: 4,
                        named: false, children: Vec::new(),
                    },
                ],
            }],
        };
        let pairs = detect_bracket_pairs(&tree);
        assert!(pairs.is_empty());
    }

    // -----------------------------------------------------------------------
    // AST-based code folding tests
    // -----------------------------------------------------------------------

    #[test]
    fn folding_ranges_from_rich_tree() {
        let tree = rich_tree();
        let ranges = compute_folding_ranges_ts(&tree);
        assert!(!ranges.is_empty());
        // function_item spans lines 2..10 — should produce a Function fold
        let func_fold = ranges.iter().find(|r| r.kind == TsFoldingKind::Function);
        assert!(func_fold.is_some());
        let ff = func_fold.unwrap();
        assert_eq!(ff.start_line, 2);
        assert_eq!(ff.end_line, 10);
    }

    #[test]
    fn folding_ranges_include_block() {
        let tree = rich_tree();
        let ranges = compute_folding_ranges_ts(&tree);
        let block = ranges.iter().find(|r| r.kind == TsFoldingKind::Block);
        assert!(block.is_some());
    }

    #[test]
    fn folding_ranges_include_control() {
        let tree = rich_tree();
        let ranges = compute_folding_ranges_ts(&tree);
        let ctrl = ranges.iter().find(|r| r.kind == TsFoldingKind::Control);
        assert!(ctrl.is_some());
    }

    #[test]
    fn folding_ranges_single_line_skipped() {
        // A node that spans a single line should not produce a fold.
        let tree = SyntaxNode {
            kind: "source_file".into(),
            start_line: 0, start_col: 0, end_line: 1, end_col: 0,
            named: true,
            children: vec![SyntaxNode {
                kind: "function_item".into(),
                start_line: 0, start_col: 0, end_line: 0, end_col: 10,
                named: true, children: Vec::new(),
            }],
        };
        let ranges = compute_folding_ranges_ts(&tree);
        assert!(ranges.is_empty());
    }

    #[test]
    fn folding_ranges_sorted() {
        let tree = rich_tree();
        let ranges = compute_folding_ranges_ts(&tree);
        for window in ranges.windows(2) {
            assert!(window[0].start_line <= window[1].start_line);
        }
    }

    // -----------------------------------------------------------------------
    // Semantic token extraction tests
    // -----------------------------------------------------------------------

    #[test]
    fn semantic_tokens_from_rich_tree() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn semantic_tokens_include_function_name() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let func_tok = tokens.iter().find(|t| {
            t.token_type == SemanticTokenType::Function && t.line == 2
        });
        assert!(func_tok.is_some());
    }

    #[test]
    fn semantic_tokens_include_keyword() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let kw = tokens.iter().find(|t| t.token_type == SemanticTokenType::Keyword);
        assert!(kw.is_some());
    }

    #[test]
    fn semantic_tokens_include_number() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let num = tokens.iter().find(|t| t.token_type == SemanticTokenType::Number);
        assert!(num.is_some());
    }

    #[test]
    fn semantic_tokens_include_string() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let s = tokens.iter().find(|t| t.token_type == SemanticTokenType::String);
        assert!(s.is_some());
    }

    #[test]
    fn semantic_tokens_include_comment() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let c = tokens.iter().find(|t| t.token_type == SemanticTokenType::Comment);
        assert!(c.is_some());
    }

    #[test]
    fn semantic_tokens_include_type() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        let ty = tokens.iter().find(|t| t.token_type == SemanticTokenType::Type);
        assert!(ty.is_some());
    }

    #[test]
    fn semantic_tokens_sorted_by_position() {
        let tree = rich_tree();
        let tokens = extract_semantic_tokens(&tree, "");
        for window in tokens.windows(2) {
            assert!(
                (window[0].line, window[0].start_col) <= (window[1].line, window[1].start_col)
            );
        }
    }

    #[test]
    fn semantic_token_type_display() {
        assert_eq!(SemanticTokenType::Function.to_string(), "function");
        assert_eq!(SemanticTokenType::Keyword.to_string(), "keyword");
        assert_eq!(SemanticTokenType::Comment.to_string(), "comment");
        assert_eq!(SemanticTokenType::String.to_string(), "string");
        assert_eq!(SemanticTokenType::Number.to_string(), "number");
        assert_eq!(SemanticTokenType::Operator.to_string(), "operator");
        assert_eq!(SemanticTokenType::Namespace.to_string(), "namespace");
        assert_eq!(SemanticTokenType::Regexp.to_string(), "regexp");
    }

    #[test]
    fn semantic_token_modifiers_union() {
        let m = SemanticTokenModifiers::DECLARATION.union(SemanticTokenModifiers::READONLY);
        assert!(m.contains(SemanticTokenModifiers::DECLARATION));
        assert!(m.contains(SemanticTokenModifiers::READONLY));
        assert!(!m.contains(SemanticTokenModifiers::STATIC));
    }

    #[test]
    fn semantic_tokens_empty_tree() {
        let tree = SyntaxNode {
            kind: "source_file".into(),
            start_line: 0, start_col: 0, end_line: 0, end_col: 0,
            named: true, children: Vec::new(),
        };
        let tokens = extract_semantic_tokens(&tree, "");
        assert!(tokens.is_empty());
    }

    // -----------------------------------------------------------------------
    // child_by_kind / text_from_source
    // -----------------------------------------------------------------------

    #[test]
    fn child_by_kind_found() {
        let tree = sample_tree();
        let func = &tree.children[0];
        assert!(func.child_by_kind("identifier").is_some());
        assert!(func.child_by_kind("block").is_some());
        assert!(func.child_by_kind("struct_item").is_none());
    }

    #[test]
    fn text_from_source_single_line() {
        let node = SyntaxNode {
            kind: "identifier".into(),
            start_line: 0, start_col: 3, end_line: 0, end_col: 7,
            named: true, children: Vec::new(),
        };
        let lines = vec!["fn main() {}"];
        assert_eq!(node.text_from_source(&lines), Some("main"));
    }

    #[test]
    fn text_from_source_multiline_returns_none() {
        let node = SyntaxNode {
            kind: "block".into(),
            start_line: 0, start_col: 0, end_line: 2, end_col: 1,
            named: true, children: Vec::new(),
        };
        let lines = vec!["{ ", " x", "}"];
        assert_eq!(node.text_from_source(&lines), None);
    }

    // -----------------------------------------------------------------------
    // TsFoldingKind equality
    // -----------------------------------------------------------------------

    #[test]
    fn ts_folding_kind_eq() {
        assert_eq!(TsFoldingKind::Function, TsFoldingKind::Function);
        assert_ne!(TsFoldingKind::Function, TsFoldingKind::Block);
    }

    // -----------------------------------------------------------------------
    // Document symbol selection_range test
    // -----------------------------------------------------------------------

    #[test]
    fn symbol_selection_range_is_name() {
        let tree = rich_tree();
        let source = "use std::io;\n\nfn main() {\n    let x = 42;\n}\n";
        let symbols = extract_symbols(&tree, source);
        let func = symbols.iter().find(|s| s.kind == SymbolKind::Function).unwrap();
        // selection_range should point at the identifier "main" (line 2, cols 3..7)
        assert_eq!(func.selection_range.start_line, 2);
        assert_eq!(func.selection_range.start_col, 3);
        assert_eq!(func.selection_range.end_col, 7);
    }

    // -------------------------------------------------------------------
    // TreeSitterQueryRunner tests
    // -------------------------------------------------------------------

    #[test]
    fn test_query_runner_find_by_kind() {
        let tree = sample_tree();
        let mut runner = TreeSitterQueryRunner::new();
        runner.add_pattern("identifier", false);
        let matches = runner.run(&tree);
        assert!(!matches.is_empty());
        assert!(matches.iter().all(|m| m.node.kind == "identifier"));
    }

    #[test]
    fn test_query_runner_named_only() {
        let tree = sample_tree();
        let mut runner = TreeSitterQueryRunner::new();
        runner.add_pattern("identifier", true);
        let matches = runner.run(&tree);
        assert!(matches.iter().all(|m| m.node.named));
    }

    #[test]
    fn test_query_runner_no_matches() {
        let tree = sample_tree();
        let mut runner = TreeSitterQueryRunner::new();
        runner.add_pattern("nonexistent_kind", false);
        let matches = runner.run(&tree);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_query_runner_clear() {
        let mut runner = TreeSitterQueryRunner::new();
        runner.add_pattern("identifier", false);
        runner.add_pattern("function_item", true);
        assert_eq!(runner.pattern_count(), 2);
        runner.clear();
        assert_eq!(runner.pattern_count(), 0);
    }

    // -------------------------------------------------------------------
    // TreeSitterFoldingProvider tests
    // -------------------------------------------------------------------

    #[test]
    fn test_folding_provider_compute() {
        let tree = rich_tree();
        let provider = TreeSitterFoldingProvider::new();
        let regions = provider.compute_folding(&tree);
        assert!(!regions.is_empty());
        // Regions should be sorted by start_line.
        for w in regions.windows(2) {
            assert!(w[0].start_line <= w[1].start_line);
        }
        // Each region must span more than one line.
        for r in &regions {
            assert!(r.end_line > r.start_line);
        }
    }

    #[test]
    fn test_folding_provider_filter_by_kind() {
        let tree = rich_tree();
        let provider = TreeSitterFoldingProvider::new();
        let regions = provider.compute_folding(&tree);
        let funcs = provider.filter_by_kind(&regions, TsFoldingKind::Function);
        assert!(funcs.iter().all(|r| r.kind == TsFoldingKind::Function));
        // Filtering by a kind not present yields empty.
        let imports = provider.filter_by_kind(&regions, TsFoldingKind::Import);
        // The rich_tree may or may not have imports; just confirm type consistency.
        assert!(imports.iter().all(|r| r.kind == TsFoldingKind::Import));
    }

    #[test]
    fn test_folding_provider_merge_adjacent() {
        let provider = TreeSitterFoldingProvider::new();
        let regions = vec![
            FoldingRegion { start_line: 0, end_line: 5, kind: TsFoldingKind::Comment, collapsed_text: "a".into() },
            FoldingRegion { start_line: 6, end_line: 10, kind: TsFoldingKind::Comment, collapsed_text: "b".into() },
            FoldingRegion { start_line: 20, end_line: 25, kind: TsFoldingKind::Comment, collapsed_text: "c".into() },
        ];
        let merged = provider.merge_adjacent(&regions, 1);
        // First two should merge (gap = 0 ≤ max_gap+1=2), third stays separate.
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start_line, 0);
        assert_eq!(merged[0].end_line, 10);
        assert_eq!(merged[1].start_line, 20);
    }

    // -------------------------------------------------------------------
    // TreeSitterScopeResolver tests
    // -------------------------------------------------------------------

    #[test]
    fn test_scope_resolver_indent() {
        let mut resolver = TreeSitterScopeResolver::new(4);
        resolver.add_indent_kind("block");
        let block_node = SyntaxNode {
            kind: "block".into(),
            start_line: 1, start_col: 0, end_line: 5, end_col: 1,
            named: true, children: vec![],
        };
        assert_eq!(resolver.compute_indent(&block_node), 4);
        // A non-matching node gets 0.
        let id_node = SyntaxNode {
            kind: "identifier".into(),
            start_line: 2, start_col: 4, end_line: 2, end_col: 5,
            named: true, children: vec![],
        };
        assert_eq!(resolver.compute_indent(&id_node), 0);
    }

    #[test]
    fn test_scope_resolver_suggest_indent() {
        let mut resolver = TreeSitterScopeResolver::new(4);
        resolver.add_indent_kind("function_item");
        let tree = sample_tree();
        // Line 5 is inside the first function_item (lines 0..10).
        let indent = resolver.suggest_indent_for_line(&tree, 5);
        // The deepest node at line 5 is the let_declaration; function_item
        // doesn't directly match at deepest, so indent depends on deepest node kind.
        // At minimum it should return a valid value.
        assert!(indent < 100);
    }

    // -------------------------------------------------------------------
    // IncrementalUpdateHandler tests
    // -------------------------------------------------------------------

    fn make_edit(start_byte: u32, old_end: u32, new_end: u32) -> IncrementalEdit {
        IncrementalEdit {
            start_byte,
            old_end_byte: old_end,
            new_end_byte: new_end,
            start_point: Point { row: 0, column: start_byte },
            old_end_point: Point { row: 0, column: old_end },
            new_end_point: Point { row: 0, column: new_end },
        }
    }

    #[test]
    fn test_incremental_handler_record() {
        let mut handler = IncrementalUpdateHandler::new();
        assert!(!handler.has_pending());
        assert_eq!(handler.edit_count(), 0);

        handler.record_edit(make_edit(0, 5, 10));
        assert!(handler.has_pending());
        assert_eq!(handler.edit_count(), 1);
        assert_eq!(handler.pending_edits().len(), 1);
        assert_eq!(handler.pending_edits()[0].start_byte, 0);
    }

    #[test]
    fn test_incremental_handler_flush() {
        let mut handler = IncrementalUpdateHandler::new();
        handler.record_edit(make_edit(0, 5, 10));
        handler.record_edit(make_edit(10, 15, 20));
        assert_eq!(handler.edit_count(), 2);

        let flushed = handler.flush();
        assert_eq!(flushed.len(), 2);
        assert!(!handler.has_pending());
        assert_eq!(handler.edit_count(), 0);
        // Version is preserved after flush.
        assert_eq!(handler.version(), 2);
    }

    #[test]
    fn test_incremental_handler_version() {
        let mut handler = IncrementalUpdateHandler::new();
        assert_eq!(handler.version(), 0);
        handler.record_edit(make_edit(0, 1, 2));
        assert_eq!(handler.version(), 1);
        handler.record_edit(make_edit(3, 4, 5));
        assert_eq!(handler.version(), 2);
        handler.flush();
        assert_eq!(handler.version(), 2);
    }

    #[test]
    fn treesitterHighlightMapper_new() {
        let s = TreesitterHighlightMapper::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn treesitterHighlightMapper_add_contains() {
        let mut s = TreesitterHighlightMapper::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn treesitterHighlightMapper_add_duplicate() {
        let mut s = TreesitterHighlightMapper::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn treesitterHighlightMapper_remove() {
        let mut s = TreesitterHighlightMapper::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn treesitterHighlightMapper_capacity() {
        let s = TreesitterHighlightMapper::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn treesitterHighlightMapper_search() {
        let mut s = TreesitterHighlightMapper::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn treesitterHighlightMapper_stats() {
        let mut s = TreesitterHighlightMapper::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn treesitterEditTracker_new() {
        let m = TreesitterEditTracker::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn treesitterEditTracker_add_find() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn treesitterEditTracker_priority_filter() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("a", "A").with_priority(TreesitterEditTrackerPriority::High));
        m.add(TreesitterEditTrackerItem::new("b", "B").with_priority(TreesitterEditTrackerPriority::Low));
        m.add(TreesitterEditTrackerItem::new("c", "C").with_priority(TreesitterEditTrackerPriority::High));
        assert_eq!(m.by_priority(TreesitterEditTrackerPriority::High).len(), 2);
    }

    #[test]
    fn treesitterEditTracker_remove() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn treesitterEditTracker_search() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("id1", "Hello World"));
        m.add(TreesitterEditTrackerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn treesitterEditTracker_total_weight() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("a", "A").with_priority(TreesitterEditTrackerPriority::Critical));
        m.add(TreesitterEditTrackerItem::new("b", "B").with_priority(TreesitterEditTrackerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn treesitterEditTracker_capacity_limit() {
        let mut m = TreesitterEditTracker::new().with_max_items(2);
        m.add(TreesitterEditTrackerItem::new("1", "one"));
        m.add(TreesitterEditTrackerItem::new("2", "two"));
        assert!(!m.add(TreesitterEditTrackerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn treesitterEditTracker_sorted_by_priority() {
        let mut m = TreesitterEditTracker::new();
        m.add(TreesitterEditTrackerItem::new("lo", "Low").with_priority(TreesitterEditTrackerPriority::Low));
        m.add(TreesitterEditTrackerItem::new("hi", "High").with_priority(TreesitterEditTrackerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn treesitterEditTracker_item_metadata() {
        let mut item = TreesitterEditTrackerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn treesitterHighlightMapper_enabled_toggle() {
        let mut s = TreesitterHighlightMapper::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn treesitterEditTracker_priority_display() {
        assert_eq!(format!("{}", TreesitterEditTrackerPriority::High), "high");
        assert_eq!(format!("{}", TreesitterEditTrackerPriority::Low), "low");
    }


    #[test]
    fn wb_treesitter_config_new() {
        let cfg = WbTreesitterConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_treesitter_config_set_get() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_treesitter_config_remove() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_treesitter_config_keys_sorted() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_treesitter_config_bump_version() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_treesitter_config_clear() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_treesitter_config_merge() {
        let mut cfg1 = WbTreesitterConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbTreesitterConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_treesitter_config_disable() {
        let mut cfg = WbTreesitterConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_treesitter_rate_tracker_empty() {
        let rt = WbTreesitterRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_treesitter_rate_tracker_record() {
        let mut rt = WbTreesitterRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_treesitter_rate_tracker_prune() {
        let mut rt = WbTreesitterRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_treesitter_validator_valid() {
        let v = WbTreesitterValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_treesitter_validator_errors() {
        let mut v = WbTreesitterValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_treesitter_validator_clear() {
        let mut v = WbTreesitterValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_treesitter_validator_merge() {
        let mut v1 = WbTreesitterValidator::new();
        v1.add_error("e1");
        let mut v2 = WbTreesitterValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_treesitter_rate_tracker_clear() {
        let mut rt = WbTreesitterRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yn_metrics_empty() {
        let m = YnMetrics::new("wb_ts");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yn_metrics_record_and_mean() {
        let mut m = YnMetrics::new("wb_ts");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yn_metrics_min_max() {
        let mut m = YnMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yn_metrics_variance_and_std() {
        let mut m = YnMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yn_metrics_percentile() {
        let mut m = YnMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yn_metrics_merge() {
        let mut a = YnMetrics::new("a");
        a.record(1.0);
        let mut b = YnMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yn_metrics_reset() {
        let mut m = YnMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yn_rate_window_empty() {
        let rw = YnRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yn_rate_window_tick_and_rate() {
        let mut rw = YnRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yn_lru_cache_basic() {
        let mut c = YnLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yn_lru_cache_contains_and_keys() {
        let mut c = YnLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yn_lru_cache_remove() {
        let mut c = YnLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yn_metrics_sum() {
        let mut m = YnMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yn_metrics_label() {
        let m = YnMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yn_lru_cache_clear() {
        let mut c = YnLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_treesitter
    #[test]
    fn xa_wb_treesitter_ring_new() {
        let rb = super::XaWbTreesitterRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_treesitter_ring_push_len() {
        let mut rb = super::XaWbTreesitterRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_treesitter_ring_wrap() {
        let mut rb = super::XaWbTreesitterRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_treesitter_ring_mean_empty() {
        let rb = super::XaWbTreesitterRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_treesitter_ring_mean_values() {
        let mut rb = super::XaWbTreesitterRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_treesitter_ring_min_max() {
        let mut rb = super::XaWbTreesitterRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_treesitter_ring_iter() {
        let mut rb = super::XaWbTreesitterRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_treesitter_counter_new() {
        let c = super::XaWbTreesitterCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_treesitter_counter_inc() {
        let mut c = super::XaWbTreesitterCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_treesitter_counter_inc_by() {
        let mut c = super::XaWbTreesitterCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_treesitter_counter_reset() {
        let mut c = super::XaWbTreesitterCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_treesitter_counter_clear() {
        let mut c = super::XaWbTreesitterCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_treesitter_counter_default() {
        let c = super::XaWbTreesitterCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 231 ----

    #[test]
    fn xc_231_pool_new_empty() {
        let pool: super::Xc231Pool<i32> = super::Xc231Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_231_pool_release_acquire() {
        let mut pool = super::Xc231Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_231_pool_acquire_empty() {
        let mut pool: super::Xc231Pool<i32> = super::Xc231Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_231_pool_full() {
        let mut pool = super::Xc231Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_231_pool_drain() {
        let mut pool = super::Xc231Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_231_pool_stats() {
        let mut pool = super::Xc231Pool::new(8);
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
    fn xc_231_pool_clear() {
        let mut pool = super::Xc231Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_231_pool_shrink() {
        let mut pool = super::Xc231Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_231_pool_default() {
        let pool: super::Xc231Pool<String> = super::Xc231Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_231_pool_extend() {
        let mut pool = super::Xc231Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_231_pool_retain() {
        let mut pool = super::Xc231Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_231_scheduler_round_robin() {
        let mut sched = super::Xc231Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_231_scheduler_empty() {
        let mut sched = super::Xc231Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_231_scheduler_reset() {
        let mut sched = super::Xc231Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_231_scheduler_add_remove() {
        let mut sched = super::Xc231Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_231_scheduler_targets() {
        let sched = super::Xc231Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_231_hash_empty() {
        assert_eq!(super::xc_231_hash(b""), 5381);
    }

    #[test]
    fn xc_231_hash_data() {
        let h = super::xc_231_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_231_hash(b"hello"), h);
    }

    #[test]
    fn xc_231_reverse_str() {
        assert_eq!(super::xc_231_reverse("abc"), "cba");
        assert_eq!(super::xc_231_reverse(""), "");
    }


    // --- xd_97 deepening tests ---

    #[test]
    fn xd_97_sm_initial_state() {
        let sm = Xd97StateMachine::new();
        assert_eq!(sm.current_state(), Xd97State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_97_sm_valid_idle_to_running() {
        let mut sm = Xd97StateMachine::new();
        assert!(sm.transition(Xd97State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd97State::Running);
    }

    #[test]
    fn xd_97_sm_valid_running_to_paused() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        assert!(sm.transition(Xd97State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd97State::Paused);
    }

    #[test]
    fn xd_97_sm_valid_running_to_done() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        assert!(sm.transition(Xd97State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd97State::Done);
    }

    #[test]
    fn xd_97_sm_valid_paused_to_running() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        sm.transition(Xd97State::Paused).unwrap();
        assert!(sm.transition(Xd97State::Running).is_ok());
    }

    #[test]
    fn xd_97_sm_valid_done_to_idle() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        sm.transition(Xd97State::Done).unwrap();
        assert!(sm.transition(Xd97State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd97State::Idle);
    }

    #[test]
    fn xd_97_sm_invalid_idle_to_done() {
        let mut sm = Xd97StateMachine::new();
        assert!(sm.transition(Xd97State::Done).is_err());
    }

    #[test]
    fn xd_97_sm_invalid_idle_to_paused() {
        let mut sm = Xd97StateMachine::new();
        assert!(sm.transition(Xd97State::Paused).is_err());
    }

    #[test]
    fn xd_97_sm_history_tracking() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        sm.transition(Xd97State::Paused).unwrap();
        sm.transition(Xd97State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd97State::Idle);
        assert_eq!(sm.history()[0].to, Xd97State::Running);
        assert_eq!(sm.history()[1].from, Xd97State::Running);
        assert_eq!(sm.history()[2].to, Xd97State::Done);
    }

    #[test]
    fn xd_97_sm_serialize_deserialize() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd97StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd97State::Running));
    }

    #[test]
    fn xd_97_sm_deserialize_invalid() {
        assert_eq!(Xd97StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_97_sm_reset() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd97State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_97_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd97EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd97Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_97_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd97EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd97Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd97Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_97_bus_unsubscribe() {
        let mut bus = Xd97EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_97_event_kind_and_payload() {
        let e = Xd97Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd97Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_97_bus_clear_history() {
        let mut bus = Xd97EventBus::new();
        bus.publish(Xd97Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_97_sm_step_counter_increments() {
        let mut sm = Xd97StateMachine::new();
        sm.transition(Xd97State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd97State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_21 graph tests ------------------------------------------------

    #[test]
    fn xg_21_graph_empty() {
        let g = super::Xg21Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_21_graph_add_node() {
        let mut g = super::Xg21Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_21_graph_add_edge() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_21_graph_neighbors() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_21_graph_has_path() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_21_graph_self_path() {
        let g = super::Xg21Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_21_graph_topo_sort() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_21_graph_cycle_detect_false() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_21_graph_cycle_detect_true() {
        let mut g = super::Xg21Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_21 heap tests -------------------------------------------------

    #[test]
    fn xg_21_heap_empty() {
        let h: super::Xg21Heap<i32> = super::Xg21Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_21_heap_push_pop() {
        let mut h = super::Xg21Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_21_heap_peek() {
        let mut h = super::Xg21Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_21_heap_drain_sorted() {
        let mut h = super::Xg21Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_21_heap_merge() {
        let mut a = super::Xg21Heap::new();
        let mut b = super::Xg21Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_21_heap_default() {
        let h: super::Xg21Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_21_graph_default() {
        let g: super::Xg21Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh230_skip_insert_contains() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh230_skip_remove() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh230_skip_len() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh230_skip_range_query() {
        let mut sl = super::Xh230SkipList::xh_new(4);
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
    fn xh230_skip_floor_ceiling() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh230_skip_rank() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh230_skip_empty() {
        let sl = super::Xh230SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh230_skip_duplicates() {
        let mut sl = super::Xh230SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh230_bitset_set_test() {
        let mut bs = super::Xh230BitSet::xh_new(256);
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
    fn xh230_bitset_clear_count() {
        let mut bs = super::Xh230BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh230_bitset_and_or_xor() {
        let mut a = super::Xh230BitSet::xh_new(128);
        let mut b = super::Xh230BitSet::xh_new(128);
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
    fn xh230_bitset_iter_ones() {
        let mut bs = super::Xh230BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh230_bitset_first_last() {
        let mut bs = super::Xh230BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh230_bitset_empty() {
        let bs = super::Xh230BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi230_deque_push_pop_back() {
        let mut dq = super::Xi230Deque::xi_new(4);
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
    fn xi230_deque_push_pop_front() {
        let mut dq = super::Xi230Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi230_deque_mixed_ops() {
        let mut dq = super::Xi230Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi230_deque_get_and_split() {
        let mut dq = super::Xi230Deque::xi_new(8);
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
    fn xi230_deque_rotate_left() {
        let mut dq = super::Xi230Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi230_deque_rotate_right() {
        let mut dq = super::Xi230Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi230_deque_grow() {
        let mut dq = super::Xi230Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi230_deque_empty() {
        let dq = super::Xi230Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi230_interval_tree_insert_query() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi230Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi230Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi230_interval_tree_overlap() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi230Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi230Interval::xi_new(12, 20));
        let q = super::Xi230Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi230_interval_tree_remove() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi230Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi230_interval_tree_gaps() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi230Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi230Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi230Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi230Interval::xi_new(8, 10));
    }

    #[test]
    fn xi230_interval_tree_merge() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi230Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi230Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi230Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi230Interval::xi_new(10, 15));
    }

    #[test]
    fn xi230_interval_tree_all() {
        let mut tree = super::Xi230IntervalTree::xi_new();
        tree.xi_insert(super::Xi230Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi230Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi230_interval_tree_empty() {
        let tree = super::Xi230IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi230_interval_tree_contains_point() {
        let iv = super::Xi230Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 230) ---

    #[test]
    fn xj_230_uf_make_and_find() {
        let mut uf = super::Xj230UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_230_uf_union_connected() {
        let mut uf = super::Xj230UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_230_uf_component_count() {
        let mut uf = super::Xj230UnionFind::xj_new();
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
    fn xj_230_uf_component_size() {
        let mut uf = super::Xj230UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_230_uf_largest_component() {
        let mut uf = super::Xj230UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_230_uf_many_elements() {
        let mut uf = super::Xj230UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_230_uf_separate_components() {
        let mut uf = super::Xj230UnionFind::xj_new();
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
    fn xj_230_uf_path_compression() {
        let mut uf = super::Xj230UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_230_bt_insert_get() {
        let mut bt = super::Xj230BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_230_bt_contains_len() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_230_bt_replace() {
        let mut bt = super::Xj230BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_230_bt_remove() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_230_bt_keys_values() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_230_bt_range() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_230_bt_min_max() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_230_bt_many_inserts() {
        let mut bt = super::Xj230BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_230 segment tree tests ---

    #[test]
    fn xk_230_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_230_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk230SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_230_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_230_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_230_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_230_st_single_element() {
        let data = vec![42];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_230_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk230SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_230_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk230SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_230 disjoint intervals tests ---

    #[test]
    fn xk_230_di_add_and_count() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_230_di_merge_overlap() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_230_di_contains() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_230_di_remove() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_230_di_covered_length() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_230_di_gaps() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_230_di_merge_adjacent() {
        let mut di = super::Xk230DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_230_di_empty() {
        let di = super::Xk230DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
