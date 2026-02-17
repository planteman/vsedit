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
}
