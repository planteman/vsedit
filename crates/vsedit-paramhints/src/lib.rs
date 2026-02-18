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


/// A probabilistic sorted list using a skip-list structure (variant 135).
pub struct Xh135SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh135SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 177 as u64,
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

/// A compact bit set supporting boolean operations (variant 135).
pub struct Xh135BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh135BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 135).
pub struct Xi135Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi135Deque<T> {
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
pub struct Xi135Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi135Interval {
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

/// A simple interval tree (variant 135).
pub struct Xi135IntervalTree {
    xi_intervals: Vec<Xi135Interval>,
}

impl Xi135IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi135Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi135Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi135Interval) -> Vec<&Xi135Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi135Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi135Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi135Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi135Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi135Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi135Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 135) ---

/// Disjoint set / union-find for crate 135.
pub struct Xj135UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj135UnionFind {
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

const XJ135_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 135.
pub struct Xj135BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj135BTreeNode<K, V>>>,
    len: usize,
}

struct Xj135BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj135BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj135BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ135_BTREE_ORDER - 1
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
        let mid = XJ135_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj135BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj135BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj135BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj135BTreeNode::xj_new_leaf();
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


// --- xk_135 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk135SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk135SegmentTree {
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
pub struct Xk135DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk135DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_135).
#[derive(Debug, Clone)]
pub struct Xl135Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl135Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_135).
#[derive(Debug, Clone)]
pub struct Xl135SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl135SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm135MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm135MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm135Tokenizer {
    text: String,
}

impl Xm135Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 135.
pub struct Xn135Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn135Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 135 -----

#[derive(Debug, Clone)]
struct Xn135AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn135AvlNode<K, V>>>,
    right: Option<Box<Xn135AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 135.
#[derive(Debug, Clone)]
pub struct Xn135AVL<K, V> {
    root: Option<Box<Xn135AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn135AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn135AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn135AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn135AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn135AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn135AvlNode<K, V>>) -> Box<Xn135AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn135AvlNode<K, V>>) -> Box<Xn135AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn135AvlNode<K, V>>) -> Box<Xn135AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn135AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn135AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn135AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn135AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn135AvlNode<K, V>>) -> &Xn135AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn135AvlNode<K, V>>) -> (Box<Xn135AvlNode<K, V>>, Option<Box<Xn135AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn135AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn135AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn135AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn135AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn135AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn135AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn135AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo135RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo135Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo135RBNode<K, V> {
    key: K,
    value: V,
    color: Xo135Color,
    left: Option<Box<Xo135RBNode<K, V>>>,
    right: Option<Box<Xo135RBNode<K, V>>>,
}

/// A red-black tree map for crate 135.
#[derive(Debug, Clone)]
pub struct Xo135RedBlack<K, V> {
    root: Option<Box<Xo135RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo135RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo135Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo135RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo135RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo135RBNode {
                    key, value, color: Xo135Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo135RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo135Color::Red)
    }

    fn xo_balance(mut h: Box<Xo135RBNode<K, V>>) -> Box<Xo135RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo135Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo135RBNode<K, V>>) -> Box<Xo135RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo135Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo135RBNode<K, V>>) -> Box<Xo135RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo135Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo135RBNode<K, V>>) {
        h.color = Xo135Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo135Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo135Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo135Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo135RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo135RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo135RBNode<K, V>) -> (K, V, Option<Box<Xo135RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo135RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo135Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo135RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo135ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 135.
#[derive(Debug, Clone)]
pub struct Xo135ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo135ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo135#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo135#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 135).
#[derive(Debug)]
pub struct Xp135SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp135Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp135Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp135Node<K, V>>>,
    xp_right: Option<Box<Xp135Node<K, V>>>,
}

impl<K: Ord, V> Xp135Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp135SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp135SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp135Node<K, V>>>, key: &K) -> Option<Box<Xp135Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp135Node<K, V>>) -> Box<Xp135Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp135Node<K, V>>) -> Box<Xp135Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp135Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp135Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp135Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq135Treap ---------------

use std::cmp::Ordering as Xq135Ord;

struct Xq135TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq135TreapNode<K, V>>>,
    right: Option<Box<Xq135TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq135Treap<K, V> {
    root: Option<Box<Xq135TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq135TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_135_size<K, V>(node: &Option<Box<Xq135TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_135_update_size<K, V>(node: &mut Xq135TreapNode<K, V>) {
    node.size = 1 + xq_135_size(&node.left) + xq_135_size(&node.right);
}

fn xq_135_rotate_right<K, V>(mut node: Box<Xq135TreapNode<K, V>>) -> Box<Xq135TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_135_update_size(&mut node);
    left.right = Some(node);
    xq_135_update_size(&mut left);
    left
}

fn xq_135_rotate_left<K, V>(mut node: Box<Xq135TreapNode<K, V>>) -> Box<Xq135TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_135_update_size(&mut node);
    right.left = Some(node);
    xq_135_update_size(&mut right);
    right
}

fn xq_135_insert_node<K: Ord, V>(
    node: Option<Box<Xq135TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq135TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq135TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq135Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq135Ord::Less => {
                let (new_left, old) = xq_135_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_135_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_135_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq135Ord::Greater => {
                let (new_right, old) = xq_135_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_135_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_135_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_135_remove_node<K: Ord, V>(
    node: Option<Box<Xq135TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq135TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq135Ord::Less => {
                let (new_left, old) = xq_135_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_135_update_size(&mut n);
                (Some(n), old)
            }
            Xq135Ord::Greater => {
                let (new_right, old) = xq_135_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_135_update_size(&mut n);
                (Some(n), old)
            }
            Xq135Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_135_rotate_right(n);
                    let (new_right, old) = xq_135_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_135_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_135_rotate_left(n);
                    let (new_left, old) = xq_135_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_135_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_135_find_min<K, V>(node: &Option<Box<Xq135TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_135_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_135_find_max<K, V>(node: &Option<Box<Xq135TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_135_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_135_rank<K: Ord, V>(node: &Option<Box<Xq135TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq135Ord::Less => xq_135_rank(&n.left, key),
            Xq135Ord::Equal => xq_135_size(&n.left),
            Xq135Ord::Greater => 1 + xq_135_size(&n.left) + xq_135_rank(&n.right, key),
        },
    }
}

fn xq_135_kth<K, V>(node: &Option<Box<Xq135TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_135_size(&n.left);
        if k < left_size {
            xq_135_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_135_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_135_in_order<K: Clone, V>(node: &Option<Box<Xq135TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_135_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_135_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq135Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 135 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_135_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq135Ord::Equal => return Some(&n.value),
                Xq135Ord::Less => cur = &n.left,
                Xq135Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_135_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_135_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_135_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_135_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_135_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_135_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_135_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq135VEBTree ---------------

pub struct Xq135VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq135VEBTree>>,
    clusters: Vec<Option<Box<Xq135VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq135VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq135VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq135VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
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


    #[test]
    fn xh135_skip_insert_contains() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh135_skip_remove() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh135_skip_len() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh135_skip_range_query() {
        let mut sl = super::Xh135SkipList::xh_new(4);
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
    fn xh135_skip_floor_ceiling() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh135_skip_rank() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh135_skip_empty() {
        let sl = super::Xh135SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh135_skip_duplicates() {
        let mut sl = super::Xh135SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh135_bitset_set_test() {
        let mut bs = super::Xh135BitSet::xh_new(256);
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
    fn xh135_bitset_clear_count() {
        let mut bs = super::Xh135BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh135_bitset_and_or_xor() {
        let mut a = super::Xh135BitSet::xh_new(128);
        let mut b = super::Xh135BitSet::xh_new(128);
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
    fn xh135_bitset_iter_ones() {
        let mut bs = super::Xh135BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh135_bitset_first_last() {
        let mut bs = super::Xh135BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh135_bitset_empty() {
        let bs = super::Xh135BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi135_deque_push_pop_back() {
        let mut dq = super::Xi135Deque::xi_new(4);
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
    fn xi135_deque_push_pop_front() {
        let mut dq = super::Xi135Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi135_deque_mixed_ops() {
        let mut dq = super::Xi135Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi135_deque_get_and_split() {
        let mut dq = super::Xi135Deque::xi_new(8);
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
    fn xi135_deque_rotate_left() {
        let mut dq = super::Xi135Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi135_deque_rotate_right() {
        let mut dq = super::Xi135Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi135_deque_grow() {
        let mut dq = super::Xi135Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi135_deque_empty() {
        let dq = super::Xi135Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi135_interval_tree_insert_query() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi135Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi135Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi135_interval_tree_overlap() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi135Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi135Interval::xi_new(12, 20));
        let q = super::Xi135Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi135_interval_tree_remove() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi135Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi135_interval_tree_gaps() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi135Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi135Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi135Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi135Interval::xi_new(8, 10));
    }

    #[test]
    fn xi135_interval_tree_merge() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi135Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi135Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi135Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi135Interval::xi_new(10, 15));
    }

    #[test]
    fn xi135_interval_tree_all() {
        let mut tree = super::Xi135IntervalTree::xi_new();
        tree.xi_insert(super::Xi135Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi135Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi135_interval_tree_empty() {
        let tree = super::Xi135IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi135_interval_tree_contains_point() {
        let iv = super::Xi135Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 135) ---

    #[test]
    fn xj_135_uf_make_and_find() {
        let mut uf = super::Xj135UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_135_uf_union_connected() {
        let mut uf = super::Xj135UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_135_uf_component_count() {
        let mut uf = super::Xj135UnionFind::xj_new();
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
    fn xj_135_uf_component_size() {
        let mut uf = super::Xj135UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_135_uf_largest_component() {
        let mut uf = super::Xj135UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_135_uf_many_elements() {
        let mut uf = super::Xj135UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_135_uf_separate_components() {
        let mut uf = super::Xj135UnionFind::xj_new();
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
    fn xj_135_uf_path_compression() {
        let mut uf = super::Xj135UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_135_bt_insert_get() {
        let mut bt = super::Xj135BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_135_bt_contains_len() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_135_bt_replace() {
        let mut bt = super::Xj135BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_135_bt_remove() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_135_bt_keys_values() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_135_bt_range() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_135_bt_min_max() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_135_bt_many_inserts() {
        let mut bt = super::Xj135BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_135 segment tree tests ---

    #[test]
    fn xk_135_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_135_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk135SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_135_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_135_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_135_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_135_st_single_element() {
        let data = vec![42];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_135_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk135SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_135_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk135SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_135 disjoint intervals tests ---

    #[test]
    fn xk_135_di_add_and_count() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_135_di_merge_overlap() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_135_di_contains() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_135_di_remove() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_135_di_covered_length() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_135_di_gaps() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_135_di_merge_adjacent() {
        let mut di = super::Xk135DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_135_di_empty() {
        let di = super::Xk135DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_135_rope_new_empty() {
        let rope = super::Xl135Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_135_rope_from_str() {
        let rope = super::Xl135Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_135_rope_insert_at() {
        let mut rope = super::Xl135Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_135_rope_delete_range() {
        let mut rope = super::Xl135Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_135_rope_char_at() {
        let rope = super::Xl135Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_135_rope_split_concat() {
        let rope = super::Xl135Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_135_rope_line_count() {
        let rope = super::Xl135Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_135_rope_line_at() {
        let rope = super::Xl135Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_135_sa_build_and_search() {
        let sa = super::Xl135SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_135_sa_count() {
        let sa = super::Xl135SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_135_sa_longest_repeated() {
        let sa = super::Xl135SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_135_sa_all_positions() {
        let sa = super::Xl135SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_135_sa_len() {
        let sa = super::Xl135SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_135_sa_empty() {
        let sa = super::Xl135SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_135_rope_slice() {
        let rope = super::Xl135Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_135_sa_search_start() {
        let sa = super::Xl135SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_135_sparse_set_get() {
        let mut m = super::Xm135MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_135_sparse_row_col() {
        let mut m = super::Xm135MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_135_sparse_transpose() {
        let mut m = super::Xm135MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_135_sparse_multiply_vec() {
        let mut m = super::Xm135MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_135_sparse_nnz_density() {
        let mut m = super::Xm135MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_135_sparse_clear() {
        let mut m = super::Xm135MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_135_sparse_overwrite_zero() {
        let mut m = super::Xm135MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_135_tokenizer_basic() {
        let t = super::Xm135Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_135_tokenizer_count() {
        let t = super::Xm135Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_135_tokenizer_unique() {
        let t = super::Xm135Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_135_tokenizer_frequency() {
        let t = super::Xm135Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_135_tokenizer_delimiter() {
        let t = super::Xm135Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_135_tokenizer_whitespace() {
        let t = super::Xm135Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_135_tokenizer_empty() {
        let t = super::Xm135Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 135 ----

    #[test]
    fn xn_135_fenwick_prefix_sum() {
        let mut ft = super::Xn135Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_135_fenwick_range_sum() {
        let mut ft = super::Xn135Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_135_fenwick_point_query() {
        let mut ft = super::Xn135Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_135_fenwick_len() {
        let ft = super::Xn135Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_135_fenwick_multiple_updates() {
        let mut ft = super::Xn135Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_135_fenwick_single_element() {
        let mut ft = super::Xn135Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_135_fenwick_find_kth() {
        let mut ft = super::Xn135Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_135_fenwick_negative_delta() {
        let mut ft = super::Xn135Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 135 ----

    #[test]
    fn xn_135_avl_insert_get() {
        let mut m = super::Xn135AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_135_avl_remove() {
        let mut m = super::Xn135AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_135_avl_in_order() {
        let mut m = super::Xn135AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_135_avl_min_max() {
        let mut m = super::Xn135AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_135_avl_floor_ceiling() {
        let mut m = super::Xn135AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_135_avl_height_balanced() {
        let mut m = super::Xn135AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_135_avl_overwrite() {
        let mut m = super::Xn135AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_135_avl_empty() {
        let m: super::Xn135AVL<i32, i32> = super::Xn135AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo135RedBlack tests ---

    #[test]
    fn xo_135_rb_insert_and_get() {
        let mut tree = super::Xo135RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_135_rb_len_and_empty() {
        let mut tree = super::Xo135RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_135_rb_min_max() {
        let mut tree = super::Xo135RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_135_rb_contains() {
        let mut tree = super::Xo135RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_135_rb_remove() {
        let mut tree = super::Xo135RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_135_rb_in_order() {
        let mut tree = super::Xo135RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_135_rb_black_height() {
        let mut tree = super::Xo135RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_135_rb_overwrite() {
        let mut tree = super::Xo135RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo135ConsistentHash tests ---

    #[test]
    fn xo_135_ch_add_and_count() {
        let mut ring = super::Xo135ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_135_ch_remove_node() {
        let mut ring = super::Xo135ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_135_ch_get_node() {
        let mut ring = super::Xo135ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_135_ch_empty_ring() {
        let ring = super::Xo135ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_135_ch_distribution() {
        let mut ring = super::Xo135ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_135_ch_rebalance() {
        let mut ring = super::Xo135ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_135_ch_virtual_nodes() {
        let mut ring = super::Xo135ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_135_ch_consistent_lookup() {
        let mut ring = super::Xo135ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_135_splay_insert_get() {
        let mut t = super::Xp135SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_135_splay_remove() {
        let mut t = super::Xp135SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_135_splay_count_increases() {
        let mut t = super::Xp135SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_135_splay_depth() {
        let mut t = super::Xp135SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_135_splay_len_empty() {
        let t = super::Xp135SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_135_splay_min_max() {
        let mut t = super::Xp135SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_135_splay_overwrite() {
        let mut t = super::Xp135SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_135_splay_remove_missing() {
        let mut t = super::Xp135SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_135 treap tests ----
    #[test]
    fn xq_135_treap_empty() {
        let t = super::Xq135Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_135_treap_insert_get() {
        let mut t = super::Xq135Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_135_treap_overwrite() {
        let mut t = super::Xq135Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_135_treap_remove() {
        let mut t = super::Xq135Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_135_treap_min_max() {
        let mut t = super::Xq135Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_135_treap_rank() {
        let mut t = super::Xq135Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_135_treap_kth() {
        let mut t = super::Xq135Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_135_treap_in_order() {
        let mut t = super::Xq135Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_135 VEB tree tests ----
    #[test]
    fn xq_135_veb_empty() {
        let v = super::Xq135VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_135_veb_insert_contains() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_135_veb_min_max() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_135_veb_delete() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_135_veb_successor() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_135_veb_predecessor() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_135_veb_count() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_135_veb_duplicate_insert() {
        let mut v = super::Xq135VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}
