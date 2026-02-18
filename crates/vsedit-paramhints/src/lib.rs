//! Function signature help.

use std::fmt;
/// Information about a single parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInformation {
    pub label: String,
    pub documentation: Option<String>,
}

/// Information about a function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<ParameterInformation>,
    pub active_parameter: Option<u32>,
}

/// The result of a signature help request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub signatures: Vec<SignatureInformation>,
    pub active_signature: u32,
    pub active_parameter: u32,
}

impl SignatureHelp {
    /// Returns the currently active signature, if any.
    pub fn active_signature_info(&self) -> Option<&SignatureInformation> {
        self.signatures.get(self.active_signature as usize)
    }

    /// Returns the currently active parameter of the active signature, if any.
    pub fn active_param_info(&self) -> Option<&ParameterInformation> {
        let sig = self.active_signature_info()?;
        let idx = sig.active_parameter.unwrap_or(self.active_parameter);
        sig.parameters.get(idx as usize)
    }
}

/// How signature help was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureHelpTriggerKind {
    Invoke,
    TriggerCharacter,
    ContentChange,
}

/// Context for a signature help request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpContext {
    pub trigger_kind: SignatureHelpTriggerKind,
    pub trigger_character: Option<char>,
    pub is_retrigger: bool,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during signature help operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureHelpError {
    /// No signatures are available.
    NoSignatures,
    /// The requested index is out of range.
    InvalidIndex,
    /// The underlying provider failed.
    ProviderFailed(String),
}

impl std::fmt::Display for SignatureHelpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSignatures => write!(f, "no signatures available"),
            Self::InvalidIndex => write!(f, "index out of range"),
            Self::ProviderFailed(msg) => write!(f, "provider failed: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls
// ---------------------------------------------------------------------------

impl std::fmt::Display for SignatureInformation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.label)?;
        for (i, p) in self.parameters.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", p.label)?;
        }
        write!(f, ")")
    }
}

impl std::fmt::Display for SignatureHelpTriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invoke => write!(f, "Invoke"),
            Self::TriggerCharacter => write!(f, "TriggerCharacter"),
            Self::ContentChange => write!(f, "ContentChange"),
        }
    }
}

// ---------------------------------------------------------------------------
// Extra helpers on SignatureInformation
// ---------------------------------------------------------------------------

impl SignatureInformation {
    /// Returns the number of parameters in this signature.
    pub fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    /// Returns `true` if this signature has documentation.
    pub fn has_documentation(&self) -> bool {
        self.documentation.is_some()
    }
}

// ---------------------------------------------------------------------------
// Navigation helpers on SignatureHelp
// ---------------------------------------------------------------------------

impl SignatureHelp {
    /// Move to the next signature, wrapping around if `cycle` is true.
    pub fn next_signature(&mut self, cycle: bool) {
        if self.signatures.is_empty() {
            return;
        }
        let len = self.signatures.len() as u32;
        if self.active_signature + 1 < len {
            self.active_signature += 1;
        } else if cycle {
            self.active_signature = 0;
        }
    }

    /// Move to the previous signature, wrapping around if `cycle` is true.
    pub fn prev_signature(&mut self, cycle: bool) {
        if self.signatures.is_empty() {
            return;
        }
        if self.active_signature > 0 {
            self.active_signature -= 1;
        } else if cycle {
            self.active_signature = self.signatures.len() as u32 - 1;
        }
    }

    /// Move to the next parameter of the active signature, wrapping if `cycle`.
    pub fn next_parameter(&mut self, cycle: bool) {
        if let Some(sig) = self.signatures.get(self.active_signature as usize) {
            let len = sig.parameters.len() as u32;
            if len == 0 {
                return;
            }
            if self.active_parameter + 1 < len {
                self.active_parameter += 1;
            } else if cycle {
                self.active_parameter = 0;
            }
        }
    }

    /// Move to the previous parameter of the active signature, wrapping if `cycle`.
    pub fn prev_parameter(&mut self, cycle: bool) {
        if let Some(sig) = self.signatures.get(self.active_signature as usize) {
            let len = sig.parameters.len() as u32;
            if len == 0 {
                return;
            }
            if self.active_parameter > 0 {
                self.active_parameter -= 1;
            } else if cycle {
                self.active_parameter = len - 1;
            }
        }
    }

    /// Convenience: returns the label of the active signature, if any.
    pub fn active_signature_label(&self) -> Option<&str> {
        self.active_signature_info().map(|s| s.label.as_str())
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for signature help behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelpConfig {
    /// Whether signature help is enabled.
    pub enabled: bool,
    /// Characters that trigger signature help.
    pub trigger_characters: Vec<char>,
    /// Characters that re-trigger signature help.
    pub retrigger_characters: Vec<char>,
    /// Whether navigation should cycle around the list.
    pub cycle: bool,
}

impl Default for SignatureHelpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_characters: vec!['(', ','],
            retrigger_characters: vec![','],
            cycle: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Provider trait & registry
// ---------------------------------------------------------------------------

/// Provides signature help for function calls.
pub trait SignatureHelpProvider {
    fn provide_signature_help(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &SignatureHelpContext,
    ) -> Option<SignatureHelp>;
}

/// A registry that stores multiple providers and queries them in order.
///
/// The first provider that returns `Some` wins.
pub struct SignatureHelpRegistry {
    providers: Vec<Box<dyn SignatureHelpProvider>>,
}

impl SignatureHelpRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Box<dyn SignatureHelpProvider>) {
        self.providers.push(provider);
    }

    /// Query all providers in registration order; return the first `Some`.
    pub fn provide_signature_help(
        &self,
        uri: &str,
        line: u32,
        col: u32,
        context: &SignatureHelpContext,
    ) -> Option<SignatureHelp> {
        for provider in &self.providers {
            if let Some(help) = provider.provide_signature_help(uri, line, col, context) {
                return Some(help);
            }
        }
        None
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }
}

impl Default for SignatureHelpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Aggregated statistics about a [`SignatureHelp`] instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamHintStats {
    /// Total number of signatures available.
    pub total_signatures: usize,
    /// Sum of parameters across all signatures.
    pub total_parameters: usize,
    /// Number of signatures whose active parameter is set (i.e. currently
    /// presenting a highlighted hint to the user).
    pub active_hints: usize,
}

/// Compute aggregated statistics for the given [`SignatureHelp`].
pub fn compute_param_hint_stats(help: &SignatureHelp) -> ParamHintStats {
    let total_signatures = help.signatures.len();
    let total_parameters: usize = help.signatures.iter().map(|s| s.parameters.len()).sum();
    let active_hints = help
        .signatures
        .iter()
        .filter(|s| s.active_parameter.is_some())
        .count();
    ParamHintStats {
        total_signatures,
        total_parameters,
        active_hints,
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpWidget — rendering helpers
// ---------------------------------------------------------------------------

/// Render a signature help result to displayable lines.
///
/// The active parameter is wrapped in `[brackets]` for emphasis.
/// Includes overload navigation hint when multiple signatures exist.
pub fn render_signature_help(help: &SignatureHelp, max_width: u16) -> Vec<String> {
    let mut output = Vec::new();

    if help.signatures.is_empty() {
        return output;
    }

    let sig = match help.active_signature_info() {
        Some(s) => s,
        None => return output,
    };

    // Overload indicator
    if help.signatures.len() > 1 {
        output.push(format!(
            "{}/{} overloads (↑/↓ to switch)",
            help.active_signature + 1,
            help.signatures.len()
        ));
    }

    // Build the signature line with active parameter highlighted
    let active_idx = sig.active_parameter.unwrap_or(help.active_parameter) as usize;
    let mut sig_line = String::new();
    sig_line.push_str(&sig.label);
    sig_line.push('(');
    for (i, param) in sig.parameters.iter().enumerate() {
        if i > 0 {
            sig_line.push_str(", ");
        }
        if i == active_idx {
            sig_line.push('[');
            sig_line.push_str(&param.label);
            sig_line.push(']');
        } else {
            sig_line.push_str(&param.label);
        }
    }
    sig_line.push(')');

    // Word-wrap the signature line
    let max_w = max_width as usize;
    if sig_line.len() > max_w && max_w > 0 {
        let mut remaining = sig_line.as_str();
        while remaining.len() > max_w {
            output.push(remaining[..max_w].to_string());
            remaining = &remaining[max_w..];
        }
        if !remaining.is_empty() {
            output.push(remaining.to_string());
        }
    } else {
        output.push(sig_line);
    }

    // Show active parameter documentation if available
    if let Some(param) = sig.parameters.get(active_idx) {
        if let Some(ref doc) = param.documentation {
            output.push(format!("  {}", doc));
        }
    }

    // Show signature documentation
    if let Some(ref doc) = sig.documentation {
        output.push(String::new());
        output.push(doc.clone());
    }

    output
}

/// Check whether a character should trigger signature help.
pub fn should_trigger(ch: char, config: &SignatureHelpConfig) -> bool {
    config.enabled && config.trigger_characters.contains(&ch)
}

/// Check whether a character should re-trigger signature help.
pub fn should_retrigger(ch: char, config: &SignatureHelpConfig) -> bool {
    config.enabled && config.retrigger_characters.contains(&ch)
}

/// Check whether a character should dismiss signature help.
pub fn should_dismiss(ch: char) -> bool {
    ch == ')' || ch == ';'
}

/// Computed layout for the signature help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureHelpWidget {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl SignatureHelpWidget {
    /// Compute overlay position above the cursor.
    pub fn compute(
        lines: &[String],
        cursor_x: u16,
        cursor_y: u16,
        max_width: u16,
        max_height: u16,
    ) -> Self {
        let content_width = lines
            .iter()
            .map(|l| l.len() as u16)
            .max()
            .unwrap_or(0)
            .min(max_width.saturating_sub(2))
            .max(10);
        let content_height = (lines.len() as u16)
            .min(max_height.saturating_sub(2))
            .max(1);

        let width = content_width + 2;
        let height = content_height + 2;

        let x = if cursor_x + width <= max_width {
            cursor_x
        } else {
            max_width.saturating_sub(width)
        };

        // Prefer showing above cursor
        let y = if cursor_y >= height + 1 {
            cursor_y - height - 1
        } else {
            cursor_y + 1
        };

        Self { x, y, width, height }
    }
}

// ---------------------------------------------------------------------------
// Parameter type extraction
// ---------------------------------------------------------------------------

/// Extract the type portion from a parameter label like "x: i32" → "i32".
pub fn extract_parameter_type(label: &str) -> Option<&str> {
    let colon_pos = label.find(':')?;
    let type_part = label[colon_pos + 1..].trim();
    if type_part.is_empty() { None } else { Some(type_part) }
}

/// Extract the name portion from a parameter label like "x: i32" → "x".
pub fn extract_parameter_name(label: &str) -> &str {
    match label.find(':') {
        Some(pos) => label[..pos].trim(),
        None => label.trim(),
    }
}

// ---------------------------------------------------------------------------
// Overload ranking
// ---------------------------------------------------------------------------

/// Rank an overload based on how many parameters match the provided argument count.
/// Returns a score where higher is better.
pub fn rank_overload(sig: &SignatureInformation, arg_count: usize) -> i32 {
    let param_count = sig.parameters.len();
    if param_count == arg_count {
        100
    } else if arg_count < param_count {
        50 - (param_count as i32 - arg_count as i32)
    } else {
        0
    }
}

/// Sort signatures by relevance to the given argument count.
/// Returns indices sorted from best to worst match.
pub fn rank_overloads(signatures: &[SignatureInformation], arg_count: usize) -> Vec<usize> {
    let mut indexed: Vec<(usize, i32)> = signatures.iter()
        .enumerate()
        .map(|(i, s)| (i, rank_overload(s, arg_count)))
        .collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));
    indexed.into_iter().map(|(i, _)| i).collect()
}

// ---------------------------------------------------------------------------
// Signature formatting
// ---------------------------------------------------------------------------

/// Format a signature with the active parameter highlighted using brackets.
pub fn format_signature_with_highlight(sig: &SignatureInformation, active_param: u32) -> String {
    let mut result = format!("{}(", sig.label);
    for (i, p) in sig.parameters.iter().enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        if i as u32 == active_param {
            result.push('[');
            result.push_str(&p.label);
            result.push(']');
        } else {
            result.push_str(&p.label);
        }
    }
    result.push(')');
    result
}

/// Compute the character range of the active parameter within the signature label.
pub fn active_parameter_range(sig: &SignatureInformation, active_param: u32) -> Option<(usize, usize)> {
    let param = sig.parameters.get(active_param as usize)?;
    let label_str = format!("{}", sig);
    let start = label_str.find(&param.label)?;
    Some((start, start + param.label.len()))
}

// ---------------------------------------------------------------------------
// ParameterHintCycle – cycling through overloads
// ---------------------------------------------------------------------------

/// Manages cycling through multiple signature overloads.
#[derive(Debug, Clone)]
pub struct ParameterHintCycle {
    total_signatures: usize,
    current_index: usize,
}

impl ParameterHintCycle {
    /// Create a new cycle with the given number of signatures.
    pub fn new(total_signatures: usize) -> Self {
        Self {
            total_signatures,
            current_index: 0,
        }
    }

    /// Advance to the next overload, wrapping around.
    pub fn next(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        self.current_index = (self.current_index + 1) % self.total_signatures;
        self.current_index
    }

    /// Go to the previous overload, wrapping around.
    pub fn prev(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        if self.current_index == 0 {
            self.current_index = self.total_signatures - 1;
        } else {
            self.current_index -= 1;
        }
        self.current_index
    }

    /// Jump to a specific index. Returns `false` if out of range.
    pub fn set_index(&mut self, idx: usize) -> bool {
        if idx < self.total_signatures {
            self.current_index = idx;
            true
        } else {
            false
        }
    }

    /// Current signature index.
    pub fn current(&self) -> usize {
        self.current_index
    }

    /// Total number of signatures.
    pub fn total(&self) -> usize {
        self.total_signatures
    }

    /// Format the cycle indicator, e.g., "2/5".
    pub fn display_indicator(&self) -> String {
        if self.total_signatures == 0 {
            return String::new();
        }
        format!("{}/{}", self.current_index + 1, self.total_signatures)
    }

    /// Apply this cycle to a `SignatureHelp`, updating its `active_signature`.
    pub fn apply_to(&self, help: &mut SignatureHelp) {
        help.active_signature = self.current_index as u32;
    }

    /// Update the total and reset to 0 if the total changed.
    pub fn update_total(&mut self, new_total: usize) {
        if new_total != self.total_signatures {
            self.total_signatures = new_total;
            self.current_index = 0;
        }
    }
}

impl fmt::Display for ParameterHintCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_indicator())
    }
}

// ---------------------------------------------------------------------------
// ParameterInformation extensions
// ---------------------------------------------------------------------------

impl ParameterInformation {
    pub fn has_documentation(&self) -> bool {
        self.documentation.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.label.is_empty()
    }

    pub fn label_length(&self) -> usize {
        self.label.len()
    }
}

impl fmt::Display for ParameterInformation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)?;
        if let Some(ref doc) = self.documentation {
            write!(f, " — {doc}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SignatureInformation extensions
// ---------------------------------------------------------------------------

impl SignatureInformation {
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    pub fn find_parameter(&self, name: &str) -> Option<&ParameterInformation> {
        self.parameters.iter().find(|p| {
            let param_name = extract_parameter_name(&p.label);
            param_name == name
        })
    }
}

// ---------------------------------------------------------------------------
// SignatureHelp extensions
// ---------------------------------------------------------------------------

impl SignatureHelp {
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }

    pub fn has_active_parameter(&self) -> bool {
        self.active_signature_info()
            .map(|sig| {
                let idx = sig.active_parameter.unwrap_or(self.active_parameter) as usize;
                idx < sig.parameters.len()
            })
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpContext extensions
// ---------------------------------------------------------------------------

impl SignatureHelpContext {
    pub fn is_manual_trigger(&self) -> bool {
        self.trigger_kind == SignatureHelpTriggerKind::Invoke
    }

    pub fn is_auto_trigger(&self) -> bool {
        matches!(
            self.trigger_kind,
            SignatureHelpTriggerKind::TriggerCharacter | SignatureHelpTriggerKind::ContentChange
        )
    }

    pub fn has_active_signature(&self) -> bool {
        self.is_retrigger
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpConfig extensions
// ---------------------------------------------------------------------------

impl SignatureHelpConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn summary(&self) -> String {
        format!(
            "enabled={}, triggers={:?}, retriggers={:?}, cycle={}",
            self.enabled, self.trigger_characters, self.retrigger_characters, self.cycle,
        )
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpRegistry extensions
// ---------------------------------------------------------------------------

impl SignatureHelpRegistry {
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    pub fn clear(&mut self) {
        self.providers.clear();
    }
}

// ---------------------------------------------------------------------------
// ParamHintStats extensions
// ---------------------------------------------------------------------------

impl ParamHintStats {
    pub fn merge(&self, other: &ParamHintStats) -> ParamHintStats {
        ParamHintStats {
            total_signatures: self.total_signatures + other.total_signatures,
            total_parameters: self.total_parameters + other.total_parameters,
            active_hints: self.active_hints + other.active_hints,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "sigs={}, params={}, active={}",
            self.total_signatures, self.total_parameters, self.active_hints,
        )
    }
}

impl fmt::Display for ParamHintStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} signature(s), {} parameter(s), {} active",
            self.total_signatures, self.total_parameters, self.active_hints,
        )
    }
}

// ---------------------------------------------------------------------------
// ParameterHintCycle extensions
// ---------------------------------------------------------------------------

impl ParameterHintCycle {
    pub fn is_first(&self) -> bool {
        self.current_index == 0
    }

    pub fn is_last(&self) -> bool {
        self.total_signatures > 0 && self.current_index == self.total_signatures - 1
    }
}

// ---------------------------------------------------------------------------
// SignatureHelpWidget extensions
// ---------------------------------------------------------------------------

impl SignatureHelpWidget {
    pub fn is_visible(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    pub fn area(&self) -> u32 {
        self.width as u32 * self.height as u32
    }
}

// ---------------------------------------------------------------------------
// Active parameter tracking via cursor position & comma counting
// ---------------------------------------------------------------------------

/// Determine which parameter is active based on the cursor position within
/// a call expression.  The function counts top-level commas (respecting
/// nested parentheses, brackets, braces, and string literals) to the left
/// of `cursor_offset` within `text`.
///
/// Returns `None` if the cursor is not inside a parenthesised argument list.
pub fn active_parameter_from_cursor(text: &str, cursor_offset: usize) -> Option<u32> {
    let bytes = text.as_bytes();
    let len = bytes.len().min(cursor_offset);

    // Walk backwards to find the matching open-paren for the innermost call.
    let mut depth: i32 = 0;
    let mut open_paren_pos: Option<usize> = None;
    for i in (0..len).rev() {
        match bytes[i] {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                if depth == 0 {
                    open_paren_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let start = open_paren_pos? + 1;

    // Count top-level commas between open_paren_pos+1 and cursor_offset.
    let mut commas: u32 = 0;
    let mut nest: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for &b in &bytes[start..len] {
        if escape {
            escape = false;
            continue;
        }
        if b == b'\\' && in_string {
            escape = true;
            continue;
        }
        if b == b'"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match b {
            b'(' | b'[' | b'{' => nest += 1,
            b')' | b']' | b'}' => nest -= 1,
            b',' if nest == 0 => commas += 1,
            _ => {}
        }
    }
    Some(commas)
}

// ---------------------------------------------------------------------------
// Markdown documentation → plain-text stripping
// ---------------------------------------------------------------------------

/// Strip common markdown formatting from documentation text, producing a
/// plain-text approximation suitable for a minimal terminal overlay.
///
/// Handles: bold/italic markers, inline code backticks, code fences,
/// heading `#` prefixes, link syntax `[text](url)`, and HTML `<tags>`.
pub fn strip_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_fence = false;

    for line in md.lines() {
        let trimmed = line.trim();

        // Toggle fenced code blocks.
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // Strip heading markers.
        let stripped = trimmed.trim_start_matches('#').trim_start();
        // Strip bold/italic markers.
        let stripped = stripped.replace("**", "").replace("__", "");
        let stripped = stripped.replace('*', "").replace('_', " ");
        // Strip inline backticks.
        let stripped = stripped.replace('`', "");
        // Strip link syntax [text](url) → text
        let stripped = strip_markdown_links(&stripped);
        // Strip simple HTML tags.
        let stripped = strip_html_tags(&stripped);

        if !stripped.is_empty() {
            out.push_str(&stripped);
            out.push('\n');
        }
    }

    // Trim trailing newline.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Replace `[text](url)` with just `text`.
fn strip_markdown_links(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '[' {
            let mut link_text = String::new();
            let mut found_close = false;
            for c in chars.by_ref() {
                if c == ']' {
                    found_close = true;
                    break;
                }
                link_text.push(c);
            }
            if found_close && chars.peek() == Some(&'(') {
                chars.next(); // skip '('
                for c in chars.by_ref() {
                    if c == ')' {
                        break;
                    }
                }
                result.push_str(&link_text);
            } else {
                result.push('[');
                result.push_str(&link_text);
                if found_close {
                    result.push(']');
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Remove HTML tags like `<br>`, `<code>`, etc.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' && in_tag {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Signature context extraction for nested calls
// ---------------------------------------------------------------------------

/// Information about the call site surrounding the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallContext {
    /// Name of the function being called (text immediately before the `(`).
    pub function_name: String,
    /// Byte offset of the opening parenthesis in the source text.
    pub open_paren_offset: usize,
    /// The active parameter index (0-based) at the cursor.
    pub active_parameter: u32,
}

/// Extract the innermost [`CallContext`] at `cursor_offset` within `text`.
///
/// This is useful for nested calls like `foo(bar(1, |), 3)` where the cursor
/// `|` is inside the inner `bar(…)` call.
pub fn extract_call_context(text: &str, cursor_offset: usize) -> Option<CallContext> {
    let bytes = text.as_bytes();
    let len = bytes.len().min(cursor_offset);

    // Walk backwards to find the innermost unmatched open-paren.
    let mut depth: i32 = 0;
    let mut open_pos: Option<usize> = None;
    for i in (0..len).rev() {
        match bytes[i] {
            b')' | b']' | b'}' => depth += 1,
            b'(' => {
                if depth == 0 {
                    open_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            b'[' | b'{' => {
                if depth == 0 {
                    // Not a function call paren.
                    return None;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let paren_pos = open_pos?;

    // Extract the function name: scan backwards from paren_pos skipping
    // whitespace, then collect identifier characters.
    let mut name_end = paren_pos;
    while name_end > 0 && bytes[name_end - 1] == b' ' {
        name_end -= 1;
    }
    let mut name_start = name_end;
    while name_start > 0
        && (bytes[name_start - 1].is_ascii_alphanumeric()
            || bytes[name_start - 1] == b'_'
            || bytes[name_start - 1] == b'.'
            || bytes[name_start - 1] == b':'
            || bytes[name_start - 1] == b'!')
    {
        name_start -= 1;
    }

    let function_name = text[name_start..name_end].to_string();
    if function_name.is_empty() {
        return None;
    }

    let active_parameter = active_parameter_from_cursor(text, cursor_offset)?;

    Some(CallContext {
        function_name,
        open_paren_offset: paren_pos,
        active_parameter,
    })
}

// ---------------------------------------------------------------------------
// Parameter type annotation display formatting
// ---------------------------------------------------------------------------

/// Format a list of parameter labels into a compact type signature string.
///
/// Example: `["x: i32", "y: &str"]` → `"(i32, &str)"`.
/// Parameters without type annotations are rendered as `_`.
pub fn format_type_signature(params: &[ParameterInformation]) -> String {
    let mut out = String::from("(");
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        match extract_parameter_type(&p.label) {
            Some(ty) => out.push_str(ty),
            None => out.push('_'),
        }
    }
    out.push(')');
    out
}

/// Build a one-line summary of a signature: `name(type1, type2) -> …`
/// The return type is extracted from the label if it contains `->`.
pub fn format_signature_summary(sig: &SignatureInformation) -> String {
    let types = format_type_signature(&sig.parameters);
    let ret = sig
        .label
        .find("->")
        .map(|pos| sig.label[pos..].trim())
        .unwrap_or("");
    let name = sig
        .label
        .split('(')
        .next()
        .unwrap_or(&sig.label)
        .trim_start_matches("fn ")
        .trim();
    if ret.is_empty() {
        format!("{name}{types}")
    } else {
        format!("{name}{types} {ret}")
    }
}

// ---------------------------------------------------------------------------
// Best-overload selection
// ---------------------------------------------------------------------------

/// Select the best overload index for the given number of arguments so far.
/// Updates `help.active_signature` in place and returns the chosen index.
pub fn select_best_overload(help: &mut SignatureHelp, arg_count: usize) -> Option<usize> {
    if help.signatures.is_empty() {
        return None;
    }
    let ranked = rank_overloads(&help.signatures, arg_count);
    let best = *ranked.first()?;
    help.active_signature = best as u32;
    Some(best)
}

// ---------------------------------------------------------------------------
// ParameterHintCycler – lightweight overload navigator
// ---------------------------------------------------------------------------

/// A lightweight navigator for cycling through signature overloads.
///
/// Unlike [`ParameterHintCycle`] which couples to [`SignatureHelp`], this struct
/// is self-contained and can be used in any UI that needs wrap-around indexing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHintCycler {
    pub total_signatures: usize,
    pub current_index: usize,
}

impl ParameterHintCycler {
    /// Create a new cycler for the given number of overloads.
    pub fn new(total: usize) -> Self {
        Self {
            total_signatures: total,
            current_index: 0,
        }
    }

    /// Advance to the next overload, wrapping around to 0 after the last.
    pub fn next(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        self.current_index = (self.current_index + 1) % self.total_signatures;
        self.current_index
    }

    /// Move to the previous overload, wrapping to the last after 0.
    pub fn prev(&mut self) -> usize {
        if self.total_signatures == 0 {
            return 0;
        }
        if self.current_index == 0 {
            self.current_index = self.total_signatures - 1;
        } else {
            self.current_index -= 1;
        }
        self.current_index
    }

    /// Return the current index without advancing.
    pub fn current(&self) -> usize {
        self.current_index
    }

    /// Jump directly to an index. Returns `false` if out of range.
    pub fn jump_to(&mut self, index: usize) -> bool {
        if index >= self.total_signatures {
            return false;
        }
        self.current_index = index;
        true
    }

    /// Returns `true` when there is exactly one signature (no cycling needed).
    pub fn is_single(&self) -> bool {
        self.total_signatures == 1
    }
}

// ---------------------------------------------------------------------------
// ParameterBoldHighlighter – markup the active parameter
// ---------------------------------------------------------------------------

/// Produces a highlighted version of a signature label by wrapping the active
/// parameter in bold markers (`**...**`).
pub struct ParameterBoldHighlighter;

impl ParameterBoldHighlighter {
    /// Wrap the active parameter's text in `**...**` inside the label.
    ///
    /// If `param_index` is out of range or the parameter text is not found in
    /// `label`, the original label is returned unchanged.
    pub fn highlight(label: &str, param_index: usize, params: &[ParameterInformation]) -> String {
        let ranges = Self::extract_parameter_ranges(label, params);
        if let Some(&(start, end)) = ranges.get(param_index) {
            Self::format_with_marker(label, start, end)
        } else {
            label.to_string()
        }
    }

    /// Find the byte-offset `(start, end)` of each parameter label inside the
    /// signature label. Searches left-to-right, skipping already-matched spans.
    pub fn extract_parameter_ranges(
        label: &str,
        params: &[ParameterInformation],
    ) -> Vec<(usize, usize)> {
        let mut ranges = Vec::with_capacity(params.len());
        let mut search_from = 0;
        for p in params {
            if let Some(rel) = label[search_from..].find(&p.label) {
                let abs_start = search_from + rel;
                let abs_end = abs_start + p.label.len();
                ranges.push((abs_start, abs_end));
                search_from = abs_end;
            }
        }
        ranges
    }

    /// Insert bold markers around `label[active_start..active_end]`.
    pub fn format_with_marker(label: &str, active_start: usize, active_end: usize) -> String {
        let mut out = String::with_capacity(label.len() + 4);
        out.push_str(&label[..active_start]);
        out.push_str("**");
        out.push_str(&label[active_start..active_end]);
        out.push_str("**");
        out.push_str(&label[active_end..]);
        out
    }
}

// ---------------------------------------------------------------------------
// TriggerCharDetector – configurable trigger / retrigger character sets
// ---------------------------------------------------------------------------

/// Holds the set of characters that trigger or retrigger signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerCharDetector {
    pub trigger_chars: Vec<char>,
    pub retrigger_chars: Vec<char>,
}

impl TriggerCharDetector {
    /// Create an empty detector (no triggers).
    pub fn new() -> Self {
        Self {
            trigger_chars: Vec::new(),
            retrigger_chars: Vec::new(),
        }
    }

    /// Create a detector pre-loaded with the common defaults: `(` triggers,
    /// `,` retriggers.
    pub fn with_defaults() -> Self {
        Self {
            trigger_chars: vec!['('],
            retrigger_chars: vec![','],
        }
    }

    /// Register an additional trigger character.
    pub fn add_trigger(&mut self, ch: char) {
        if !self.trigger_chars.contains(&ch) {
            self.trigger_chars.push(ch);
        }
    }

    /// Register an additional retrigger character.
    pub fn add_retrigger(&mut self, ch: char) {
        if !self.retrigger_chars.contains(&ch) {
            self.retrigger_chars.push(ch);
        }
    }

    /// Returns `true` if `ch` is a trigger character.
    pub fn should_trigger(&self, ch: char) -> bool {
        self.trigger_chars.contains(&ch)
    }

    /// Returns `true` if `ch` is a retrigger character.
    pub fn should_retrigger(&self, ch: char) -> bool {
        self.retrigger_chars.contains(&ch)
    }

    /// Returns `true` if `ch` is either a trigger or retrigger character.
    pub fn is_trigger_or_retrigger(&self, ch: char) -> bool {
        self.should_trigger(ch) || self.should_retrigger(ch)
    }
}

// ---------------------------------------------------------------------------
// RetriggerAction / RetriggerState – track paren-depth and retrigger events
// ---------------------------------------------------------------------------

/// The action produced by [`RetriggerState::on_char`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetriggerAction {
    /// No signature-help action needed.
    None,
    /// A new parameter list was opened.
    Open,
    /// The user moved to the next parameter (e.g. typed `,`).
    Retrigger,
    /// The parameter list was closed.
    Close,
}

impl fmt::Display for RetriggerAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetriggerAction::None => write!(f, "None"),
            RetriggerAction::Open => write!(f, "Open"),
            RetriggerAction::Retrigger => write!(f, "Retrigger"),
            RetriggerAction::Close => write!(f, "Close"),
        }
    }
}

/// Tracks the nesting depth of parentheses to decide whether a character
/// should open, retrigger, or close signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetriggerState {
    pub active: bool,
    pub depth: u32,
    pub last_trigger_char: Option<char>,
}

impl RetriggerState {
    /// Create an inactive state with zero depth.
    pub fn new() -> Self {
        Self {
            active: false,
            depth: 0,
            last_trigger_char: Option::None,
        }
    }

    /// Feed a character and return the resulting action.
    ///
    /// Logic:
    /// * `(` → depth += 1, active = true → `Open`
    /// * `)` → depth -= 1 (saturating), if depth reaches 0 → `Close`
    /// * `,` when active → `Retrigger`
    /// * anything else → `None`
    pub fn on_char(&mut self, ch: char, detector: &TriggerCharDetector) -> RetriggerAction {
        if ch == '(' {
            self.depth += 1;
            self.active = true;
            self.last_trigger_char = Some(ch);
            return RetriggerAction::Open;
        }
        if ch == ')' {
            self.depth = self.depth.saturating_sub(1);
            self.last_trigger_char = Some(ch);
            if self.depth == 0 {
                self.active = false;
                return RetriggerAction::Close;
            }
            return RetriggerAction::None;
        }
        if self.active && detector.should_retrigger(ch) {
            self.last_trigger_char = Some(ch);
            return RetriggerAction::Retrigger;
        }
        RetriggerAction::None
    }
}


// ---------------------------------------------------------------------------
// ParameterHintAnimator
// ---------------------------------------------------------------------------

/// Animation phase for the parameter hint widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationPhase {
    /// Widget is hidden.
    Hidden,
    /// Widget is fading in.
    FadingIn,
    /// Widget is fully visible.
    Visible,
    /// Widget is fading out.
    FadingOut,
}

impl fmt::Display for AnimationPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Hidden => "Hidden",
            Self::FadingIn => "FadingIn",
            Self::Visible => "Visible",
            Self::FadingOut => "FadingOut",
        };
        write!(f, "{label}")
    }
}

/// Manages animation state for the parameter hint widget.
#[derive(Debug, Clone)]
pub struct ParameterHintAnimator {
    phase: AnimationPhase,
    /// Progress within the current phase (0.0 to 1.0).
    progress: f64,
    /// Duration of fade-in in milliseconds.
    fade_in_ms: u64,
    /// Duration of fade-out in milliseconds.
    fade_out_ms: u64,
    /// Minimum display time before fade-out can start.
    min_visible_ms: u64,
    /// Time at which the current phase started (epoch ms).
    phase_start_ms: u64,
    /// Whether animation is enabled.
    enabled: bool,
    /// Number of show/hide transitions.
    transition_count: u64,
}

impl ParameterHintAnimator {
    /// Create a new animator with default durations.
    pub fn new() -> Self {
        Self {
            phase: AnimationPhase::Hidden,
            progress: 0.0,
            fade_in_ms: 150,
            fade_out_ms: 100,
            min_visible_ms: 500,
            phase_start_ms: 0,
            enabled: true,
            transition_count: 0,
        }
    }

    /// Create an animator with custom durations.
    pub fn with_durations(fade_in_ms: u64, fade_out_ms: u64, min_visible_ms: u64) -> Self {
        Self {
            phase: AnimationPhase::Hidden,
            progress: 0.0,
            fade_in_ms,
            fade_out_ms,
            min_visible_ms,
            phase_start_ms: 0,
            enabled: true,
            transition_count: 0,
        }
    }

    /// Start showing the widget (begin fade-in).
    pub fn show(&mut self, now_ms: u64) {
        if !self.enabled {
            self.phase = AnimationPhase::Visible;
            self.progress = 1.0;
            return;
        }
        self.phase = AnimationPhase::FadingIn;
        self.progress = 0.0;
        self.phase_start_ms = now_ms;
        self.transition_count += 1;
    }

    /// Start hiding the widget (begin fade-out).
    pub fn hide(&mut self, now_ms: u64) {
        if !self.enabled {
            self.phase = AnimationPhase::Hidden;
            self.progress = 0.0;
            return;
        }
        // Enforce minimum visible time.
        if self.phase == AnimationPhase::Visible {
            let elapsed = now_ms.saturating_sub(self.phase_start_ms);
            if elapsed < self.min_visible_ms {
                return;
            }
        }
        self.phase = AnimationPhase::FadingOut;
        self.progress = 1.0;
        self.phase_start_ms = now_ms;
        self.transition_count += 1;
    }

    /// Update the animation state given current time.
    pub fn update(&mut self, now_ms: u64) {
        let elapsed = now_ms.saturating_sub(self.phase_start_ms);
        match self.phase {
            AnimationPhase::FadingIn => {
                if self.fade_in_ms == 0 {
                    self.phase = AnimationPhase::Visible;
                    self.progress = 1.0;
                    self.phase_start_ms = now_ms;
                } else {
                    self.progress = (elapsed as f64 / self.fade_in_ms as f64).min(1.0);
                    if self.progress >= 1.0 {
                        self.phase = AnimationPhase::Visible;
                        self.phase_start_ms = now_ms;
                    }
                }
            }
            AnimationPhase::FadingOut => {
                if self.fade_out_ms == 0 {
                    self.phase = AnimationPhase::Hidden;
                    self.progress = 0.0;
                } else {
                    self.progress = 1.0 - (elapsed as f64 / self.fade_out_ms as f64).min(1.0);
                    if self.progress <= 0.0 {
                        self.phase = AnimationPhase::Hidden;
                    }
                }
            }
            _ => {}
        }
    }

    /// Current phase.
    pub fn phase(&self) -> AnimationPhase {
        self.phase
    }

    /// Current opacity (0.0 = invisible, 1.0 = fully visible).
    pub fn opacity(&self) -> f64 {
        self.progress
    }

    /// Whether the widget is at least partially visible.
    pub fn is_visible(&self) -> bool {
        self.phase != AnimationPhase::Hidden
    }

    /// Whether animation is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable animation.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Number of transitions performed.
    pub fn transition_count(&self) -> u64 {
        self.transition_count
    }

    /// Reset to hidden state.
    pub fn reset(&mut self) {
        self.phase = AnimationPhase::Hidden;
        self.progress = 0.0;
        self.transition_count = 0;
    }
}

impl fmt::Display for ParameterHintAnimator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HintAnimator({}, opacity={:.2}, transitions={})",
            self.phase, self.progress, self.transition_count
        )
    }
}

// ---------------------------------------------------------------------------
// ParameterSignatureFormatter
// ---------------------------------------------------------------------------

/// Options for formatting a function signature.
#[derive(Debug, Clone)]
pub struct SignatureFormatOptions {
    /// Maximum width before wrapping.
    pub max_width: usize,
    /// Whether to include parameter types.
    pub show_types: bool,
    /// Whether to include return type.
    pub show_return_type: bool,
    /// Indentation for wrapped parameters.
    pub indent: String,
    /// Whether to number parameters.
    pub number_params: bool,
}

impl Default for SignatureFormatOptions {
    fn default() -> Self {
        Self {
            max_width: 80,
            show_types: true,
            show_return_type: true,
            indent: "    ".to_string(),
            number_params: false,
        }
    }
}

/// Formats function signatures for display in the hint widget.
#[derive(Debug, Clone)]
pub struct ParameterSignatureFormatter {
    options: SignatureFormatOptions,
    format_count: u64,
}

impl ParameterSignatureFormatter {
    /// Create a new formatter with default options.
    pub fn new() -> Self {
        Self {
            options: SignatureFormatOptions::default(),
            format_count: 0,
        }
    }

    /// Create with custom options.
    pub fn with_options(options: SignatureFormatOptions) -> Self {
        Self { options, format_count: 0 }
    }

    /// Format a `SignatureInformation` into display lines.
    pub fn format(&mut self, sig: &SignatureInformation) -> Vec<String> {
        self.format_count += 1;
        let mut lines = Vec::new();

        // Build parameter list.
        let params: Vec<String> = sig.parameters.iter().enumerate().map(|(i, p)| {
            if self.options.number_params {
                format!("{}. {}", i + 1, p.label)
            } else {
                p.label.clone()
            }
        }).collect();

        let one_line = format!("{}({})", sig.label, params.join(", "));

        if one_line.len() <= self.options.max_width {
            lines.push(one_line);
        } else {
            // Multi-line format.
            lines.push(format!("{}(", sig.label));
            for (i, param) in params.iter().enumerate() {
                let suffix = if i < params.len() - 1 { "," } else { "" };
                lines.push(format!("{}{}{}", self.options.indent, param, suffix));
            }
            lines.push(")".to_string());
        }

        // Add documentation if present.
        if let Some(ref doc) = sig.documentation {
            lines.push(String::new());
            lines.push(doc.clone());
        }

        lines
    }

    /// Format a signature into a single compact string.
    pub fn format_compact(&mut self, sig: &SignatureInformation) -> String {
        self.format_count += 1;
        let params: Vec<&str> = sig.parameters.iter().map(|p| p.label.as_str()).collect();
        format!("{}({})", sig.label, params.join(", "))
    }

    /// Format only the parameter list.
    pub fn format_params(&self, sig: &SignatureInformation) -> String {
        sig.parameters.iter().map(|p| p.label.as_str()).collect::<Vec<_>>().join(", ")
    }

    /// Highlight the active parameter by wrapping it with markers.
    pub fn highlight_active(&mut self, sig: &SignatureInformation, active: u32) -> String {
        self.format_count += 1;
        let params: Vec<String> = sig.parameters.iter().enumerate().map(|(i, p)| {
            if i as u32 == active {
                format!(">>{}<<", p.label)
            } else {
                p.label.clone()
            }
        }).collect();
        format!("{}({})", sig.label, params.join(", "))
    }

    /// Number of format operations performed.
    pub fn format_count(&self) -> u64 {
        self.format_count
    }

    /// Get a reference to the current options.
    pub fn options(&self) -> &SignatureFormatOptions {
        &self.options
    }

    /// Set max width.
    pub fn set_max_width(&mut self, width: usize) {
        self.options.max_width = width;
    }

    /// Reset the format counter.
    pub fn reset_count(&mut self) {
        self.format_count = 0;
    }
}

impl fmt::Display for ParameterSignatureFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SignatureFormatter(max_width={}, formatted={})",
            self.options.max_width, self.format_count
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-paramhints: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamhintsXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ParamhintsXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ParamhintsXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ParamhintsXRegistry {
    entries: Vec<ParamhintsXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ParamhintsXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ParamhintsXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ParamhintsXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ParamhintsXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ParamhintsXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ParamhintsXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ParamhintsXConfig> {
        let mut sorted: Vec<&ParamhintsXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ParamhintsXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ParamhintsXIterator<'_> {
        ParamhintsXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ParamhintsXIterator<'a> {
    inner: std::slice::Iter<'a, ParamhintsXConfig>,
}

impl<'a> Iterator for ParamhintsXIterator<'a> {
    type Item = &'a ParamhintsXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ParamhintsXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ParamhintsXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ParamhintsXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ParamhintsXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ParamhintsXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ParamhintsXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ParamhintsXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ParamhintsXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ParamhintsXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ParamhintsXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ParamhintsXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ParamhintsXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ParamhintsXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 27
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer27 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer27 {
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
pub fn xb_fnv1a_27(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_27<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_27<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_27(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_27(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 136
// ---------------------------------------------------------------------------

/// Generic object pool `Xc136Pool<T>`.
pub struct Xc136Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc136Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc136PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc136Pool<T> {
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
    pub fn stats(&self) -> Xc136PoolStats {
        Xc136PoolStats {
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

impl<T> Default for Xc136Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc136Scheduler`.
pub struct Xc136Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc136Scheduler {
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

impl Default for Xc136Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_136 hash for the given byte slice.
pub fn xc_136_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_136 convention.
pub fn xc_136_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe39 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe39Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe39PipelineError {
    pub stage: Xe39Stage,
    pub message: String,
}

impl std::fmt::Display for Xe39PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe39Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe39Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError>>>,
    stage_names: Vec<Xe39Stage>,
}

impl Xe39Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe39Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe39Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe39Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe39Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe39Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe39CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe39CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe39Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe39CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe39CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe39Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe39CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_39_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe39CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_39_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe39CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_39_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
    Ok(data)
}

pub fn xe_39_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_39_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_39_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_39_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe39PipelineError> {
    Err(Xe39PipelineError {
        stage: Xe39Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_6: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg6Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg6Graph {
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

impl Default for Xg6Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_6: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg6Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg6Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg6Heap<T>) {
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

impl<T: Ord> Default for Xg6Heap<T> {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signature() -> SignatureInformation {
        SignatureInformation {
            label: "fn foo(x: i32, y: &str)".into(),
            documentation: Some("Does foo things.".into()),
            parameters: vec![
                ParameterInformation {
                    label: "x: i32".into(),
                    documentation: Some("The x value.".into()),
                },
                ParameterInformation {
                    label: "y: &str".into(),
                    documentation: None,
                },
            ],
            active_parameter: None,
        }
    }

    #[test]
    fn active_signature_info() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let sig = help.active_signature_info().unwrap();
        assert_eq!(sig.label, "fn foo(x: i32, y: &str)");
    }

    #[test]
    fn active_param_info() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 1,
        };
        let param = help.active_param_info().unwrap();
        assert_eq!(param.label, "y: &str");
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        assert!(help.active_signature_info().is_none());
        assert!(help.active_param_info().is_none());
    }

    struct DummyProvider;

    impl SignatureHelpProvider for DummyProvider {
        fn provide_signature_help(
            &self,
            _uri: &str,
            _line: u32,
            _col: u32,
            context: &SignatureHelpContext,
        ) -> Option<SignatureHelp> {
            if context.trigger_kind == SignatureHelpTriggerKind::Invoke {
                Some(SignatureHelp {
                    signatures: vec![sample_signature()],
                    active_signature: 0,
                    active_parameter: 0,
                })
            } else {
                None
            }
        }
    }

    #[test]
    fn provider_returns_help_on_invoke() {
        let provider = DummyProvider;
        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::Invoke,
            trigger_character: None,
            is_retrigger: false,
        };
        let help = provider
            .provide_signature_help("file:///main.rs", 5, 10, &ctx)
            .unwrap();
        assert_eq!(help.signatures.len(), 1);
    }

    // -----------------------------------------------------------------------
    // New tests
    // -----------------------------------------------------------------------

    fn two_signature_help() -> SignatureHelp {
        SignatureHelp {
            signatures: vec![
                sample_signature(),
                SignatureInformation {
                    label: "fn bar(a: bool)".into(),
                    documentation: None,
                    parameters: vec![ParameterInformation {
                        label: "a: bool".into(),
                        documentation: None,
                    }],
                    active_parameter: None,
                },
            ],
            active_signature: 0,
            active_parameter: 0,
        }
    }

    #[test]
    fn next_signature_cycles() {
        let mut help = two_signature_help();
        help.next_signature(true);
        assert_eq!(help.active_signature, 1);
        help.next_signature(true);
        assert_eq!(help.active_signature, 0); // wrapped
    }

    #[test]
    fn next_signature_no_cycle_clamps() {
        let mut help = two_signature_help();
        help.next_signature(false);
        assert_eq!(help.active_signature, 1);
        help.next_signature(false);
        assert_eq!(help.active_signature, 1); // clamped
    }

    #[test]
    fn prev_signature_cycles() {
        let mut help = two_signature_help();
        assert_eq!(help.active_signature, 0);
        help.prev_signature(true);
        assert_eq!(help.active_signature, 1); // wrapped
        help.prev_signature(true);
        assert_eq!(help.active_signature, 0);
    }

    #[test]
    fn prev_signature_no_cycle_clamps() {
        let mut help = two_signature_help();
        help.prev_signature(false);
        assert_eq!(help.active_signature, 0); // clamped at 0
    }

    #[test]
    fn next_prev_parameter() {
        let mut help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        help.next_parameter(false);
        assert_eq!(help.active_parameter, 1);
        help.next_parameter(false);
        assert_eq!(help.active_parameter, 1); // clamped
        help.next_parameter(true);
        assert_eq!(help.active_parameter, 0); // cycled

        help.prev_parameter(false);
        assert_eq!(help.active_parameter, 0); // clamped
        help.prev_parameter(true);
        assert_eq!(help.active_parameter, 1); // cycled
    }

    #[test]
    fn active_signature_label_returns_label() {
        let help = two_signature_help();
        assert_eq!(
            help.active_signature_label(),
            Some("fn foo(x: i32, y: &str)")
        );
    }

    #[test]
    fn display_signature_information() {
        let sig = sample_signature();
        let text = format!("{sig}");
        assert_eq!(text, "fn foo(x: i32, y: &str)(x: i32, y: &str)");
    }

    #[test]
    fn display_trigger_kind() {
        assert_eq!(format!("{}", SignatureHelpTriggerKind::Invoke), "Invoke");
        assert_eq!(
            format!("{}", SignatureHelpTriggerKind::TriggerCharacter),
            "TriggerCharacter"
        );
        assert_eq!(
            format!("{}", SignatureHelpTriggerKind::ContentChange),
            "ContentChange"
        );
    }

    #[test]
    fn parameter_count_and_has_documentation() {
        let sig = sample_signature();
        assert_eq!(sig.parameter_count(), 2);
        assert!(sig.has_documentation());

        let bare = SignatureInformation {
            label: "fn bare()".into(),
            documentation: None,
            parameters: vec![],
            active_parameter: None,
        };
        assert_eq!(bare.parameter_count(), 0);
        assert!(!bare.has_documentation());
    }

    #[test]
    fn config_defaults() {
        let cfg = SignatureHelpConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.trigger_characters, vec!['(', ',']);
        assert_eq!(cfg.retrigger_characters, vec![',']);
        assert!(cfg.cycle);
    }

    #[test]
    fn registry_queries_in_order() {
        struct NullProvider;
        impl SignatureHelpProvider for NullProvider {
            fn provide_signature_help(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _ctx: &SignatureHelpContext,
            ) -> Option<SignatureHelp> {
                None
            }
        }

        let mut registry = SignatureHelpRegistry::new();
        registry.register(Box::new(NullProvider));
        registry.register(Box::new(DummyProvider));
        assert_eq!(registry.provider_count(), 2);

        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::Invoke,
            trigger_character: None,
            is_retrigger: false,
        };
        // NullProvider returns None, DummyProvider returns Some
        let help = registry
            .provide_signature_help("file:///main.rs", 1, 1, &ctx)
            .unwrap();
        assert_eq!(help.signatures.len(), 1);
    }

    #[test]
    fn registry_returns_none_when_all_fail() {
        struct NullProvider;
        impl SignatureHelpProvider for NullProvider {
            fn provide_signature_help(
                &self,
                _uri: &str,
                _line: u32,
                _col: u32,
                _ctx: &SignatureHelpContext,
            ) -> Option<SignatureHelp> {
                None
            }
        }

        let mut registry = SignatureHelpRegistry::new();
        registry.register(Box::new(NullProvider));

        let ctx = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::ContentChange,
            trigger_character: None,
            is_retrigger: false,
        };
        assert!(registry
            .provide_signature_help("file:///x.rs", 0, 0, &ctx)
            .is_none());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            SignatureHelpError::NoSignatures.to_string(),
            "no signatures available"
        );
        assert_eq!(
            SignatureHelpError::InvalidIndex.to_string(),
            "index out of range"
        );
        assert_eq!(
            SignatureHelpError::ProviderFailed("timeout".into()).to_string(),
            "provider failed: timeout"
        );
    }

    #[test]
    fn navigation_on_empty_signatures() {
        let mut help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        help.next_signature(true);
        assert_eq!(help.active_signature, 0);
        help.prev_signature(true);
        assert_eq!(help.active_signature, 0);
        help.next_parameter(true);
        assert_eq!(help.active_parameter, 0);
        help.prev_parameter(true);
        assert_eq!(help.active_parameter, 0);
    }

    // -----------------------------------------------------------------------
    // ParamHintStats tests
    // -----------------------------------------------------------------------

    #[test]
    fn stats_empty_signatures() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(
            stats,
            ParamHintStats {
                total_signatures: 0,
                total_parameters: 0,
                active_hints: 0,
            }
        );
    }

    #[test]
    fn stats_single_signature_no_active_hint() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 1);
        assert_eq!(stats.total_parameters, 2);
        assert_eq!(stats.active_hints, 0);
    }

    #[test]
    fn stats_multiple_signatures_with_active_hints() {
        let help = SignatureHelp {
            signatures: vec![
                SignatureInformation {
                    label: "fn a(x: i32)".into(),
                    documentation: None,
                    parameters: vec![ParameterInformation {
                        label: "x: i32".into(),
                        documentation: None,
                    }],
                    active_parameter: Some(0),
                },
                SignatureInformation {
                    label: "fn b(a: u8, b: u8, c: u8)".into(),
                    documentation: None,
                    parameters: vec![
                        ParameterInformation { label: "a: u8".into(), documentation: None },
                        ParameterInformation { label: "b: u8".into(), documentation: None },
                        ParameterInformation { label: "c: u8".into(), documentation: None },
                    ],
                    active_parameter: None,
                },
                SignatureInformation {
                    label: "fn c()".into(),
                    documentation: None,
                    parameters: vec![],
                    active_parameter: Some(0),
                },
            ],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 3);
        assert_eq!(stats.total_parameters, 4); // 1 + 3 + 0
        assert_eq!(stats.active_hints, 2); // first and third
    }

    #[test]
    fn stats_all_signatures_active() {
        let make_sig = |n: usize| SignatureInformation {
            label: format!("fn s{n}()"),
            documentation: None,
            parameters: vec![ParameterInformation {
                label: "p".into(),
                documentation: None,
            }],
            active_parameter: Some(0),
        };
        let help = SignatureHelp {
            signatures: vec![make_sig(0), make_sig(1), make_sig(2)],
            active_signature: 0,
            active_parameter: 0,
        };
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 3);
        assert_eq!(stats.total_parameters, 3);
        assert_eq!(stats.active_hints, 3);
    }

    #[test]
    fn stats_two_signature_help_helper() {
        let help = two_signature_help();
        let stats = compute_param_hint_stats(&help);
        assert_eq!(stats.total_signatures, 2);
        assert_eq!(stats.total_parameters, 3); // 2 + 1
        assert_eq!(stats.active_hints, 0);
    }

    // -----------------------------------------------------------------------
    // Rendering & trigger tests
    // -----------------------------------------------------------------------

    #[test]
    fn render_signature_help_basic() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        assert!(!lines.is_empty());
        // Active param should be highlighted with brackets
        assert!(lines.iter().any(|l| l.contains("[x: i32]")));
    }

    #[test]
    fn render_signature_help_second_param() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 1,
        };
        let lines = render_signature_help(&help, 80);
        assert!(lines.iter().any(|l| l.contains("[y: &str]")));
    }

    #[test]
    fn render_signature_help_overloads() {
        let help = two_signature_help();
        let lines = render_signature_help(&help, 80);
        assert!(lines.iter().any(|l| l.contains("1/2 overloads")));
    }

    #[test]
    fn render_signature_help_empty() {
        let help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_signature_help_with_docs() {
        let help = SignatureHelp {
            signatures: vec![sample_signature()],
            active_signature: 0,
            active_parameter: 0,
        };
        let lines = render_signature_help(&help, 80);
        // sample_signature has documentation for param 0
        assert!(lines.iter().any(|l| l.contains("The x value")));
    }

    #[test]
    fn should_trigger_open_paren() {
        let cfg = SignatureHelpConfig::default();
        assert!(should_trigger('(', &cfg));
        assert!(should_trigger(',', &cfg));
        assert!(!should_trigger(')', &cfg));
        assert!(!should_trigger('a', &cfg));
    }

    #[test]
    fn should_retrigger_comma() {
        let cfg = SignatureHelpConfig::default();
        assert!(should_retrigger(',', &cfg));
        assert!(!should_retrigger('(', &cfg));
    }

    #[test]
    fn should_dismiss_chars() {
        assert!(should_dismiss(')'));
        assert!(should_dismiss(';'));
        assert!(!should_dismiss(','));
        assert!(!should_dismiss('a'));
    }

    #[test]
    fn signature_help_widget_compute() {
        let lines = vec!["fn foo(x: i32, y: &str)".to_string()];
        let widget = SignatureHelpWidget::compute(&lines, 10, 15, 80, 24);
        assert!(widget.width > 0);
        assert!(widget.height > 0);
        assert!(widget.y < 15); // should be above cursor
    }

    #[test]
    fn should_trigger_disabled() {
        let cfg = SignatureHelpConfig {
            enabled: false,
            ..SignatureHelpConfig::default()
        };
        assert!(!should_trigger('(', &cfg));
        assert!(!should_retrigger(',', &cfg));
    }

    #[test]
    fn hint_cycle_next_wraps() {
        let mut cycle = ParameterHintCycle::new(3);
        assert_eq!(cycle.current(), 0);
        assert_eq!(cycle.next(), 1);
        assert_eq!(cycle.next(), 2);
        assert_eq!(cycle.next(), 0); // wraps
    }

    #[test]
    fn hint_cycle_prev_wraps() {
        let mut cycle = ParameterHintCycle::new(3);
        assert_eq!(cycle.prev(), 2); // wraps backward
        assert_eq!(cycle.prev(), 1);
        assert_eq!(cycle.prev(), 0);
    }

    #[test]
    fn hint_cycle_set_index() {
        let mut cycle = ParameterHintCycle::new(5);
        assert!(cycle.set_index(3));
        assert_eq!(cycle.current(), 3);
        assert!(!cycle.set_index(10)); // out of range
        assert_eq!(cycle.current(), 3); // unchanged
    }

    #[test]
    fn hint_cycle_display_indicator() {
        let mut cycle = ParameterHintCycle::new(5);
        assert_eq!(cycle.display_indicator(), "1/5");
        cycle.next();
        assert_eq!(cycle.display_indicator(), "2/5");
    }

    #[test]
    fn hint_cycle_empty() {
        let mut cycle = ParameterHintCycle::new(0);
        assert_eq!(cycle.next(), 0);
        assert_eq!(cycle.prev(), 0);
        assert_eq!(cycle.display_indicator(), "");
    }

    #[test]
    fn hint_cycle_apply_to_help() {
        let mut cycle = ParameterHintCycle::new(3);
        cycle.next();
        cycle.next();
        let mut help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        cycle.apply_to(&mut help);
        assert_eq!(help.active_signature, 2);
    }

    #[test]
    fn hint_cycle_update_total_resets() {
        let mut cycle = ParameterHintCycle::new(5);
        cycle.set_index(3);
        cycle.update_total(10);
        assert_eq!(cycle.current(), 0);
        assert_eq!(cycle.total(), 10);
    }

    // -----------------------------------------------------------------------
    // New extension tests
    // -----------------------------------------------------------------------

    #[test]
    fn parameter_information_extensions() {
        let p = ParameterInformation {
            label: "x: i32".into(),
            documentation: Some("an integer".into()),
        };
        assert!(p.has_documentation());
        assert!(!p.is_empty());
        assert_eq!(p.label_length(), 6);

        let empty = ParameterInformation {
            label: String::new(),
            documentation: None,
        };
        assert!(!empty.has_documentation());
        assert!(empty.is_empty());
        assert_eq!(empty.label_length(), 0);
    }

    #[test]
    fn display_parameter_information() {
        let p = ParameterInformation {
            label: "x: i32".into(),
            documentation: Some("an integer".into()),
        };
        assert_eq!(format!("{p}"), "x: i32 — an integer");

        let no_doc = ParameterInformation {
            label: "y: bool".into(),
            documentation: None,
        };
        assert_eq!(format!("{no_doc}"), "y: bool");
    }

    #[test]
    fn signature_information_find_parameter() {
        let sig = sample_signature();
        let found = sig.find_parameter("x").unwrap();
        assert_eq!(found.label, "x: i32");
        assert!(sig.find_parameter("nonexistent").is_none());
        assert!(!sig.is_empty());

        let empty_sig = SignatureInformation {
            label: "fn nop()".into(),
            documentation: None,
            parameters: vec![],
            active_parameter: None,
        };
        assert!(empty_sig.is_empty());
    }

    #[test]
    fn signature_help_extensions() {
        let help = two_signature_help();
        assert_eq!(help.signature_count(), 2);
        assert!(!help.is_empty());
        assert!(help.has_active_parameter());

        let empty_help = SignatureHelp {
            signatures: vec![],
            active_signature: 0,
            active_parameter: 0,
        };
        assert!(empty_help.is_empty());
        assert!(!empty_help.has_active_parameter());
    }

    #[test]
    fn signature_help_context_extensions() {
        let manual = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::Invoke,
            trigger_character: None,
            is_retrigger: false,
        };
        assert!(manual.is_manual_trigger());
        assert!(!manual.is_auto_trigger());
        assert!(!manual.has_active_signature());

        let auto = SignatureHelpContext {
            trigger_kind: SignatureHelpTriggerKind::TriggerCharacter,
            trigger_character: Some('('),
            is_retrigger: true,
        };
        assert!(!auto.is_manual_trigger());
        assert!(auto.is_auto_trigger());
        assert!(auto.has_active_signature());
    }

    #[test]
    fn config_extensions_and_registry_clear() {
        let cfg = SignatureHelpConfig::default();
        assert!(cfg.is_enabled());
        let s = cfg.summary();
        assert!(s.contains("enabled=true"));
        assert!(s.contains("cycle=true"));

        let mut registry = SignatureHelpRegistry::new();
        assert!(registry.is_empty());
        registry.register(Box::new(DummyProvider));
        assert!(!registry.is_empty());
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.provider_count(), 0);
    }

    #[test]
    fn param_hint_stats_merge_and_display() {
        let a = ParamHintStats {
            total_signatures: 2,
            total_parameters: 5,
            active_hints: 1,
        };
        let b = ParamHintStats {
            total_signatures: 3,
            total_parameters: 4,
            active_hints: 2,
        };
        let merged = a.merge(&b);
        assert_eq!(merged.total_signatures, 5);
        assert_eq!(merged.total_parameters, 9);
        assert_eq!(merged.active_hints, 3);
        assert_eq!(merged.summary(), "sigs=5, params=9, active=3");
        assert_eq!(
            format!("{merged}"),
            "5 signature(s), 9 parameter(s), 3 active"
        );
    }

    #[test]
    fn hint_cycle_is_first_is_last() {
        let mut cycle = ParameterHintCycle::new(3);
        assert!(cycle.is_first());
        assert!(!cycle.is_last());
        cycle.set_index(2);
        assert!(!cycle.is_first());
        assert!(cycle.is_last());
    }

    #[test]
    fn widget_is_visible_and_area() {
        let w = SignatureHelpWidget { x: 0, y: 0, width: 10, height: 5 };
        assert!(w.is_visible());
        assert_eq!(w.area(), 50);

        let zero = SignatureHelpWidget { x: 0, y: 0, width: 0, height: 5 };
        assert!(!zero.is_visible());
        assert_eq!(zero.area(), 0);
    }

    // -----------------------------------------------------------------------
    // Active parameter from cursor tests
    // -----------------------------------------------------------------------

    #[test]
    fn active_param_from_cursor_simple() {
        // foo(a, b, c)  — cursor after 'a' → param 0
        let text = "foo(a, b, c)";
        assert_eq!(active_parameter_from_cursor(text, 5), Some(0)); // after 'a'
        assert_eq!(active_parameter_from_cursor(text, 7), Some(1)); // after first comma+space
        assert_eq!(active_parameter_from_cursor(text, 10), Some(2)); // after second comma+space
    }

    #[test]
    fn active_param_from_cursor_nested() {
        // foo(bar(1, 2), c)  — cursor inside bar → param 1 of bar
        let text = "foo(bar(1, 2), c)";
        assert_eq!(active_parameter_from_cursor(text, 10), Some(1)); // inside bar, after comma
        // cursor in outer call after the inner call closes
        assert_eq!(active_parameter_from_cursor(text, 16), Some(1)); // at 'c' in outer foo
    }

    #[test]
    fn active_param_from_cursor_strings() {
        // Commas inside string literals should be ignored.
        let text = r#"foo("a,b", c)"#;
        assert_eq!(active_parameter_from_cursor(text, 11), Some(1)); // at 'c'
        assert_eq!(active_parameter_from_cursor(text, 6), Some(0)); // inside string
    }

    #[test]
    fn active_param_from_cursor_no_paren() {
        // No open paren → None
        assert_eq!(active_parameter_from_cursor("hello world", 5), None);
    }

    // -----------------------------------------------------------------------
    // Markdown stripping tests
    // -----------------------------------------------------------------------

    #[test]
    fn strip_markdown_basic() {
        let md = "## Hello\n\nThis is **bold** and `code`.";
        let plain = strip_markdown(md);
        assert!(plain.contains("Hello"));
        assert!(plain.contains("bold"));
        assert!(plain.contains("code"));
        assert!(!plain.contains("**"));
        assert!(!plain.contains('`'));
        assert!(!plain.contains("##"));
    }

    #[test]
    fn strip_markdown_links_and_fences() {
        let md = "See [docs](https://example.com) for info.\n```\ncode block\n```";
        let plain = strip_markdown(md);
        assert!(plain.contains("See docs for info."));
        assert!(plain.contains("code block"));
        assert!(!plain.contains("https://"));
    }

    // -----------------------------------------------------------------------
    // Call context extraction tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_call_context_simple() {
        let text = "println!(hello, world)";
        let ctx = extract_call_context(text, 20).unwrap();
        assert_eq!(ctx.function_name, "println!");
        assert_eq!(ctx.active_parameter, 1);
    }

    #[test]
    fn extract_call_context_nested_inner() {
        let text = "foo(bar(x, y), z)";
        // cursor inside bar(…) at position 12 (after "bar(x, y")
        let ctx = extract_call_context(text, 12).unwrap();
        assert_eq!(ctx.function_name, "bar");
        assert_eq!(ctx.active_parameter, 1);
    }

    // -----------------------------------------------------------------------
    // Type annotation formatting tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_type_signature_basic() {
        let params = vec![
            ParameterInformation { label: "x: i32".into(), documentation: None },
            ParameterInformation { label: "y: &str".into(), documentation: None },
        ];
        assert_eq!(format_type_signature(&params), "(i32, &str)");
    }

    #[test]
    fn format_type_signature_no_types() {
        let params = vec![
            ParameterInformation { label: "x".into(), documentation: None },
        ];
        assert_eq!(format_type_signature(&params), "(_)");
    }

    #[test]
    fn format_signature_summary_with_return() {
        let sig = SignatureInformation {
            label: "fn add(a: i32, b: i32) -> i32".into(),
            documentation: None,
            parameters: vec![
                ParameterInformation { label: "a: i32".into(), documentation: None },
                ParameterInformation { label: "b: i32".into(), documentation: None },
            ],
            active_parameter: None,
        };
        assert_eq!(format_signature_summary(&sig), "add(i32, i32) -> i32");
    }

    // -----------------------------------------------------------------------
    // Best-overload selection test
    // -----------------------------------------------------------------------

    #[test]
    fn select_best_overload_picks_exact_match() {
        let mut help = SignatureHelp {
            signatures: vec![
                SignatureInformation {
                    label: "fn f(a: i32)".into(),
                    documentation: None,
                    parameters: vec![
                        ParameterInformation { label: "a: i32".into(), documentation: None },
                    ],
                    active_parameter: None,
                },
                SignatureInformation {
                    label: "fn f(a: i32, b: i32)".into(),
                    documentation: None,
                    parameters: vec![
                        ParameterInformation { label: "a: i32".into(), documentation: None },
                        ParameterInformation { label: "b: i32".into(), documentation: None },
                    ],
                    active_parameter: None,
                },
            ],
            active_signature: 0,
            active_parameter: 0,
        };
        // With 2 args the second overload should be selected.
        let best = select_best_overload(&mut help, 2).unwrap();
        assert_eq!(best, 1);
        assert_eq!(help.active_signature, 1);
    }

    // -----------------------------------------------------------------------
    // ParameterHintCycler tests
    // -----------------------------------------------------------------------

    #[test]
    fn cycler_next_wraps() {
        let mut c = ParameterHintCycler::new(3);
        assert_eq!(c.current(), 0);
        assert_eq!(c.next(), 1);
        assert_eq!(c.next(), 2);
        assert_eq!(c.next(), 0); // wrap
    }

    #[test]
    fn cycler_prev_wraps() {
        let mut c = ParameterHintCycler::new(3);
        assert_eq!(c.prev(), 2); // wrap backward
        assert_eq!(c.prev(), 1);
        assert_eq!(c.prev(), 0);
    }

    #[test]
    fn cycler_jump_to_valid_and_invalid() {
        let mut c = ParameterHintCycler::new(4);
        assert!(c.jump_to(3));
        assert_eq!(c.current(), 3);
        assert!(!c.jump_to(4)); // out of range
        assert_eq!(c.current(), 3); // unchanged
        assert!(!c.jump_to(100));
    }

    #[test]
    fn cycler_is_single() {
        assert!(ParameterHintCycler::new(1).is_single());
        assert!(!ParameterHintCycler::new(0).is_single());
        assert!(!ParameterHintCycler::new(2).is_single());
    }

    #[test]
    fn cycler_zero_total() {
        let mut c = ParameterHintCycler::new(0);
        assert_eq!(c.next(), 0);
        assert_eq!(c.prev(), 0);
        assert!(!c.jump_to(0));
    }

    // -----------------------------------------------------------------------
    // ParameterBoldHighlighter tests
    // -----------------------------------------------------------------------

    #[test]
    fn highlighter_basic() {
        let params = vec![
            ParameterInformation { label: "x: i32".into(), documentation: None },
            ParameterInformation { label: "y: &str".into(), documentation: None },
        ];
        let label = "fn foo(x: i32, y: &str)";
        let h = ParameterBoldHighlighter::highlight(label, 0, &params);
        assert!(h.contains("**x: i32**"));
        assert!(!h.contains("**y: &str**"));
    }

    #[test]
    fn highlighter_second_param() {
        let params = vec![
            ParameterInformation { label: "x: i32".into(), documentation: None },
            ParameterInformation { label: "y: &str".into(), documentation: None },
        ];
        let label = "fn foo(x: i32, y: &str)";
        let h = ParameterBoldHighlighter::highlight(label, 1, &params);
        assert!(h.contains("**y: &str**"));
    }

    #[test]
    fn highlighter_out_of_range_returns_original() {
        let params = vec![
            ParameterInformation { label: "x: i32".into(), documentation: None },
        ];
        let label = "fn foo(x: i32)";
        assert_eq!(ParameterBoldHighlighter::highlight(label, 5, &params), label);
    }

    #[test]
    fn extract_parameter_ranges_correct() {
        let params = vec![
            ParameterInformation { label: "a: u8".into(), documentation: None },
            ParameterInformation { label: "b: u8".into(), documentation: None },
        ];
        let label = "fn f(a: u8, b: u8)";
        let ranges = ParameterBoldHighlighter::extract_parameter_ranges(label, &params);
        assert_eq!(ranges.len(), 2);
        assert_eq!(&label[ranges[0].0..ranges[0].1], "a: u8");
        assert_eq!(&label[ranges[1].0..ranges[1].1], "b: u8");
    }

    #[test]
    fn format_with_marker_works() {
        let label = "fn f(x: i32)";
        let out = ParameterBoldHighlighter::format_with_marker(label, 5, 11);
        assert_eq!(out, "fn f(**x: i32**)");
    }

    // -----------------------------------------------------------------------
    // TriggerCharDetector tests
    // -----------------------------------------------------------------------

    #[test]
    fn trigger_detector_defaults() {
        let d = TriggerCharDetector::with_defaults();
        assert!(d.should_trigger('('));
        assert!(!d.should_trigger(','));
        assert!(d.should_retrigger(','));
        assert!(!d.should_retrigger('('));
        assert!(d.is_trigger_or_retrigger('('));
        assert!(d.is_trigger_or_retrigger(','));
        assert!(!d.is_trigger_or_retrigger('x'));
    }

    #[test]
    fn trigger_detector_add_chars() {
        let mut d = TriggerCharDetector::new();
        assert!(!d.should_trigger('<'));
        d.add_trigger('<');
        assert!(d.should_trigger('<'));
        // duplicate add is idempotent
        d.add_trigger('<');
        assert_eq!(d.trigger_chars.len(), 1);

        d.add_retrigger(';');
        assert!(d.should_retrigger(';'));
    }

    // -----------------------------------------------------------------------
    // RetriggerState tests
    // -----------------------------------------------------------------------

    #[test]
    fn retrigger_state_open_retrigger_close() {
        let det = TriggerCharDetector::with_defaults();
        let mut st = RetriggerState::new();
        assert!(!st.active);

        assert_eq!(st.on_char('(', &det), RetriggerAction::Open);
        assert!(st.active);
        assert_eq!(st.depth, 1);

        assert_eq!(st.on_char(',', &det), RetriggerAction::Retrigger);

        assert_eq!(st.on_char(')', &det), RetriggerAction::Close);
        assert!(!st.active);
        assert_eq!(st.depth, 0);
    }

    #[test]
    fn retrigger_state_nested_parens() {
        let det = TriggerCharDetector::with_defaults();
        let mut st = RetriggerState::new();

        assert_eq!(st.on_char('(', &det), RetriggerAction::Open);
        assert_eq!(st.on_char('(', &det), RetriggerAction::Open);
        assert_eq!(st.depth, 2);

        // closing inner paren doesn't close signature help
        assert_eq!(st.on_char(')', &det), RetriggerAction::None);
        assert!(st.active);
        assert_eq!(st.depth, 1);

        // closing outer paren closes
        assert_eq!(st.on_char(')', &det), RetriggerAction::Close);
        assert!(!st.active);
    }

    #[test]
    fn retrigger_action_display() {
        assert_eq!(format!("{}", RetriggerAction::None), "None");
        assert_eq!(format!("{}", RetriggerAction::Open), "Open");
        assert_eq!(format!("{}", RetriggerAction::Retrigger), "Retrigger");
        assert_eq!(format!("{}", RetriggerAction::Close), "Close");
    }

    #[test]
    fn retrigger_state_ignores_comma_when_inactive() {
        let det = TriggerCharDetector::with_defaults();
        let mut st = RetriggerState::new();
        assert_eq!(st.on_char(',', &det), RetriggerAction::None);
    }

    #[test]
    fn animator_starts_hidden() {
        let anim = ParameterHintAnimator::new();
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
        assert!(!anim.is_visible());
    }

    #[test]
    fn animator_show_starts_fading_in() {
        let mut anim = ParameterHintAnimator::new();
        anim.show(0);
        assert_eq!(anim.phase(), AnimationPhase::FadingIn);
        assert!(anim.is_visible());
    }

    #[test]
    fn animator_fade_in_completes() {
        let mut anim = ParameterHintAnimator::new();
        anim.show(0);
        anim.update(200);
        assert_eq!(anim.phase(), AnimationPhase::Visible);
        assert!((anim.opacity() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn animator_hide_after_min_visible() {
        let mut anim = ParameterHintAnimator::with_durations(0, 100, 500);
        anim.show(0);
        anim.update(0);
        anim.hide(600);
        assert_eq!(anim.phase(), AnimationPhase::FadingOut);
    }

    #[test]
    fn animator_hide_blocked_before_min_visible() {
        let mut anim = ParameterHintAnimator::with_durations(0, 100, 500);
        anim.show(0);
        anim.update(0);
        anim.hide(100);
        assert_eq!(anim.phase(), AnimationPhase::Visible);
    }

    #[test]
    fn animator_fade_out_completes() {
        let mut anim = ParameterHintAnimator::with_durations(0, 100, 0);
        anim.show(0);
        anim.update(0);
        anim.hide(0);
        anim.update(200);
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
    }

    #[test]
    fn animator_disabled_shows_immediately() {
        let mut anim = ParameterHintAnimator::new();
        anim.set_enabled(false);
        anim.show(0);
        assert_eq!(anim.phase(), AnimationPhase::Visible);
    }

    #[test]
    fn animator_transition_count() {
        let mut anim = ParameterHintAnimator::new();
        anim.show(0);
        anim.update(200);
        anim.hide(800);
        assert_eq!(anim.transition_count(), 2);
    }

    #[test]
    fn animator_reset() {
        let mut anim = ParameterHintAnimator::new();
        anim.show(0);
        anim.reset();
        assert_eq!(anim.phase(), AnimationPhase::Hidden);
        assert_eq!(anim.transition_count(), 0);
    }

    #[test]
    fn animator_display() {
        let anim = ParameterHintAnimator::new();
        let s = format!("{anim}");
        assert!(s.contains("Hidden"));
        assert!(s.contains("transitions=0"));
    }

    #[test]
    fn sig_formatter_compact() {
        let mut fmt = ParameterSignatureFormatter::new();
        let sig = SignatureInformation {
            label: "add".into(),
            documentation: None,
            parameters: vec![
                ParameterInformation { label: "a: i32".into(), documentation: None },
                ParameterInformation { label: "b: i32".into(), documentation: None },
            ],
            active_parameter: None,
        };
        let result = fmt.format_compact(&sig);
        assert_eq!(result, "add(a: i32, b: i32)");
    }

    #[test]
    fn sig_formatter_multiline() {
        let mut fmt = ParameterSignatureFormatter::new();
        fmt.set_max_width(10);
        let sig = SignatureInformation {
            label: "long_function_name".into(),
            documentation: None,
            parameters: vec![
                ParameterInformation { label: "param1".into(), documentation: None },
                ParameterInformation { label: "param2".into(), documentation: None },
            ],
            active_parameter: None,
        };
        let lines = fmt.format(&sig);
        assert!(lines.len() > 1);
    }

    #[test]
    fn sig_formatter_highlight_active() {
        let mut fmt = ParameterSignatureFormatter::new();
        let sig = SignatureInformation {
            label: "foo".into(),
            documentation: None,
            parameters: vec![
                ParameterInformation { label: "x".into(), documentation: None },
                ParameterInformation { label: "y".into(), documentation: None },
            ],
            active_parameter: None,
        };
        let result = fmt.highlight_active(&sig, 1);
        assert!(result.contains(">>y<<"));
        assert!(!result.contains(">>x<<"));
    }

    #[test]
    fn sig_formatter_with_documentation() {
        let mut fmt = ParameterSignatureFormatter::new();
        let sig = SignatureInformation {
            label: "bar".into(),
            documentation: Some("Does bar things".into()),
            parameters: vec![],
            active_parameter: None,
        };
        let lines = fmt.format(&sig);
        assert!(lines.iter().any(|l| l.contains("Does bar things")));
    }

    #[test]
    fn sig_formatter_format_count() {
        let mut fmt = ParameterSignatureFormatter::new();
        let sig = SignatureInformation {
            label: "f".into(), documentation: None,
            parameters: vec![], active_parameter: None,
        };
        fmt.format_compact(&sig);
        fmt.format_compact(&sig);
        assert_eq!(fmt.format_count(), 2);
    }

    #[test]
    fn sig_formatter_display() {
        let fmt = ParameterSignatureFormatter::new();
        let s = format!("{fmt}");
        assert!(s.contains("max_width=80"));
    }



    #[test]
    fn paramhints_x_config_new() {
        let c = ParamhintsXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn paramhints_x_config_builder() {
        let c = ParamhintsXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn paramhints_x_config_display() {
        let c = ParamhintsXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn paramhints_x_registry_insert_get() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn paramhints_x_registry_duplicate() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a")).unwrap();
        assert!(reg.insert(ParamhintsXConfig::new("a")).is_err());
    }

    #[test]
    fn paramhints_x_registry_remove() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a")).unwrap();
        reg.insert(ParamhintsXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn paramhints_x_registry_active_entries() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a")).unwrap();
        reg.insert(ParamhintsXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn paramhints_x_registry_by_weight() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ParamhintsXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn paramhints_x_registry_tags() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ParamhintsXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn paramhints_x_registry_total_weight() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ParamhintsXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn paramhints_x_registry_iterator() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a")).unwrap();
        reg.insert(ParamhintsXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn paramhints_x_cache_put_get() {
        let mut cache = ParamhintsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn paramhints_x_cache_eviction() {
        let mut cache = ParamhintsXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn paramhints_x_cache_lru_order() {
        let mut cache = ParamhintsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn paramhints_x_cache_most_least_recent() {
        let mut cache = ParamhintsXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn paramhints_x_formatter_entry() {
        let e = ParamhintsXConfig::new("k").with_value("v");
        let fmt = ParamhintsXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn paramhints_x_formatter_summary() {
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ParamhintsXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn paramhints_x_validator_valid() {
        let v = ParamhintsXValidator::new();
        let c = ParamhintsXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn paramhints_x_validator_empty_key() {
        let v = ParamhintsXValidator::new();
        let c = ParamhintsXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn paramhints_x_validator_require_value() {
        let v = ParamhintsXValidator::new().require_value(true);
        let c = ParamhintsXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn paramhints_x_validator_allowed_tags() {
        let v = ParamhintsXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ParamhintsXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn paramhints_x_validator_validate_all() {
        let v = ParamhintsXValidator::new();
        let mut reg = ParamhintsXRegistry::new();
        reg.insert(ParamhintsXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn xb_ring_buffer_27_push_and_len() {
        let mut rb = super::XbRingBuffer27::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_27_overwrite() {
        let mut rb = super::XbRingBuffer27::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_27_get_out_of_bounds() {
        let rb = super::XbRingBuffer27::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_27_drain_all() {
        let mut rb = super::XbRingBuffer27::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_27_peek_front_back() {
        let mut rb = super::XbRingBuffer27::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_27_clear() {
        let mut rb = super::XbRingBuffer27::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_27_capacity() {
        let rb = super::XbRingBuffer27::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_27_basic() {
        let h = super::xb_fnv1a_27(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_27(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_27_different_inputs() {
        let h1 = super::xb_fnv1a_27(b"abc");
        let h2 = super::xb_fnv1a_27(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_27_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_27(&data);
        let dec = super::xb_rle_decode_27(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_27_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_27(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_27(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_27_values() {
        assert!((super::xb_clamp_27(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_27(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_27(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_27_values() {
        assert!((super::xb_lerp_27(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_27(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_27(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_27_wrap_around_twice() {
        let mut rb = super::XbRingBuffer27::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 136 ----

    #[test]
    fn xc_136_pool_new_empty() {
        let pool: super::Xc136Pool<i32> = super::Xc136Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_136_pool_release_acquire() {
        let mut pool = super::Xc136Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_136_pool_acquire_empty() {
        let mut pool: super::Xc136Pool<i32> = super::Xc136Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_136_pool_full() {
        let mut pool = super::Xc136Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_136_pool_drain() {
        let mut pool = super::Xc136Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_136_pool_stats() {
        let mut pool = super::Xc136Pool::new(8);
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
    fn xc_136_pool_clear() {
        let mut pool = super::Xc136Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_136_pool_shrink() {
        let mut pool = super::Xc136Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_136_pool_default() {
        let pool: super::Xc136Pool<String> = super::Xc136Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_136_pool_extend() {
        let mut pool = super::Xc136Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_136_pool_retain() {
        let mut pool = super::Xc136Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_136_scheduler_round_robin() {
        let mut sched = super::Xc136Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_136_scheduler_empty() {
        let mut sched = super::Xc136Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_136_scheduler_reset() {
        let mut sched = super::Xc136Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_136_scheduler_add_remove() {
        let mut sched = super::Xc136Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_136_scheduler_targets() {
        let sched = super::Xc136Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_136_hash_empty() {
        assert_eq!(super::xc_136_hash(b""), 5381);
    }

    #[test]
    fn xc_136_hash_data() {
        let h = super::xc_136_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_136_hash(b"hello"), h);
    }

    #[test]
    fn xc_136_reverse_str() {
        assert_eq!(super::xc_136_reverse("abc"), "cba");
        assert_eq!(super::xc_136_reverse(""), "");
    }


    #[test]
    fn xe_39_pipeline_empty() {
        let p = super::Xe39Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_39_pipeline_parse_stage() {
        let p = super::Xe39Pipeline::new()
            .add_parse(super::xe_39_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_39_pipeline_transform_double() {
        let p = super::Xe39Pipeline::new()
            .add_transform(super::xe_39_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_39_pipeline_validate_reverse() {
        let p = super::Xe39Pipeline::new()
            .add_validate(super::xe_39_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_39_pipeline_emit_filter() {
        let p = super::Xe39Pipeline::new()
            .add_emit(super::xe_39_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_39_pipeline_multi_stage() {
        let p = super::Xe39Pipeline::new()
            .add_parse(super::xe_39_pipeline_identity)
            .add_transform(super::xe_39_pipeline_double)
            .add_validate(super::xe_39_pipeline_reverse)
            .add_emit(super::xe_39_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_39_pipeline_error_propagation() {
        let p = super::Xe39Pipeline::new()
            .add_parse(super::xe_39_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe39Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_39_pipeline_compose() {
        let p1 = super::Xe39Pipeline::new()
            .add_parse(super::xe_39_pipeline_identity);
        let p2 = super::Xe39Pipeline::new()
            .add_transform(super::xe_39_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_39_pipeline_error_display() {
        let e = super::Xe39PipelineError {
            stage: super::Xe39Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_39_cache_put_get() {
        let mut c = super::Xe39Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_39_cache_miss() {
        let mut c: super::Xe39Cache<&str, i32> = super::Xe39Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_39_cache_ttl_expiry() {
        let mut c = super::Xe39Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_39_cache_evict() {
        let mut c = super::Xe39Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_39_cache_capacity() {
        let mut c = super::Xe39Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_39_cache_stats() {
        let mut c = super::Xe39Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_39_cache_clear() {
        let mut c = super::Xe39Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_6 graph tests ------------------------------------------------

    #[test]
    fn xg_6_graph_empty() {
        let g = super::Xg6Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_6_graph_add_node() {
        let mut g = super::Xg6Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_6_graph_add_edge() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_6_graph_neighbors() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_6_graph_has_path() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_6_graph_self_path() {
        let g = super::Xg6Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_6_graph_topo_sort() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_6_graph_cycle_detect_false() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_6_graph_cycle_detect_true() {
        let mut g = super::Xg6Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_6 heap tests -------------------------------------------------

    #[test]
    fn xg_6_heap_empty() {
        let h: super::Xg6Heap<i32> = super::Xg6Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_6_heap_push_pop() {
        let mut h = super::Xg6Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_6_heap_peek() {
        let mut h = super::Xg6Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_6_heap_drain_sorted() {
        let mut h = super::Xg6Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_6_heap_merge() {
        let mut a = super::Xg6Heap::new();
        let mut b = super::Xg6Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_6_heap_default() {
        let h: super::Xg6Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_6_graph_default() {
        let g: super::Xg6Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }

}
