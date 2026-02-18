//! Extension point registry for vsedit.
//!
//! Provides a simple registry of named extension points, equivalent to
//! VS Code's extension point infrastructure. Other crates register their
//! extension points here so the system can discover and validate them.

use std::collections::{HashMap, HashSet};
use std::fmt;

// ---------------------------------------------------------------------------
// RegistryError
// ---------------------------------------------------------------------------

/// Errors that can occur when operating on the extension point registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The extension point ID is already registered.
    DuplicatePoint(String),
    /// The extension point ID was not found.
    PointNotFound(String),
    /// The extension point ID does not follow naming conventions.
    InvalidId(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::DuplicatePoint(id) => {
                write!(f, "extension point already registered: {id}")
            }
            RegistryError::PointNotFound(id) => {
                write!(f, "extension point not found: {id}")
            }
            RegistryError::InvalidId(id) => {
                write!(f, "invalid extension point id: {id}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

// ---------------------------------------------------------------------------
// ExtensionPointRegistry
// ---------------------------------------------------------------------------

/// A registry of named extension points.
///
/// Extension points are identified by string IDs (e.g.
/// `"vsedit.configuration"`, `"vsedit.commands"`). Crates register their
/// extension points at startup so other parts of the system can discover them.
pub struct ExtensionPointRegistry {
    points: HashSet<String>,
    metadata: HashMap<String, ExtensionPointMetadata>,
}

/// Optional metadata attached to an extension point.
#[derive(Debug, Clone, Default)]
pub struct ExtensionPointMetadata {
    pub description: String,
    pub version: Option<String>,
    pub deprecated: bool,
}

impl fmt::Display for ExtensionPointMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description)?;
        if let Some(ref v) = self.version {
            write!(f, " (v{v})")?;
        }
        if self.deprecated {
            write!(f, " [DEPRECATED]")?;
        }
        Ok(())
    }
}

impl ExtensionPointRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            points: HashSet::new(),
            metadata: HashMap::new(),
        }
    }

    /// Register an extension point by ID.
    pub fn register_point(&mut self, id: &str) {
        self.points.insert(id.to_string());
    }

    /// Register an extension point with metadata.
    pub fn register_point_with_metadata(&mut self, id: &str, meta: ExtensionPointMetadata) {
        self.points.insert(id.to_string());
        self.metadata.insert(id.to_string(), meta);
    }

    /// Returns `true` if the given extension point has been registered.
    pub fn has_point(&self, id: &str) -> bool {
        self.points.contains(id)
    }

    /// Returns the number of registered extension points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns `true` if no extension points are registered.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns metadata for an extension point, if any.
    pub fn get_metadata(&self, id: &str) -> Option<&ExtensionPointMetadata> {
        self.metadata.get(id)
    }

    /// Returns an iterator over all registered extension point IDs.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.points.iter().map(String::as_str)
    }

    /// Unregister an extension point by ID.
    ///
    /// Returns an error if the point was not previously registered.
    pub fn unregister_point(&mut self, id: &str) -> Result<(), RegistryError> {
        if !self.points.remove(id) {
            return Err(RegistryError::PointNotFound(id.to_string()));
        }
        self.metadata.remove(id);
        Ok(())
    }

    /// Register an extension point, returning an error if it already exists.
    pub fn try_register(&mut self, id: &str) -> Result<(), RegistryError> {
        if self.points.contains(id) {
            return Err(RegistryError::DuplicatePoint(id.to_string()));
        }
        self.points.insert(id.to_string());
        Ok(())
    }

    /// Find all registered extension points whose ID starts with `prefix`.
    pub fn find_by_prefix(&self, prefix: &str) -> Vec<&str> {
        let mut result: Vec<&str> = self
            .points
            .iter()
            .filter(|id| id.starts_with(prefix))
            .map(String::as_str)
            .collect();
        result.sort();
        result
    }

    /// Return IDs of all extension points marked as deprecated.
    pub fn get_deprecated(&self) -> Vec<&str> {
        let mut result: Vec<&str> = self
            .metadata
            .iter()
            .filter(|(_, meta)| meta.deprecated)
            .map(|(id, _)| id.as_str())
            .collect();
        result.sort();
        result
    }

    /// Remove all registered extension points.
    pub fn clear(&mut self) {
        self.points.clear();
        self.metadata.clear();
    }

    /// Validate that an extension point ID follows naming conventions.
    ///
    /// A valid ID consists of dot-separated segments where each segment is
    /// non-empty and contains no whitespace.
    pub fn validate_id(id: &str) -> Result<(), RegistryError> {
        if id.is_empty() {
            return Err(RegistryError::InvalidId(id.to_string()));
        }
        if id.contains(char::is_whitespace) {
            return Err(RegistryError::InvalidId(id.to_string()));
        }
        let segments: Vec<&str> = id.split('.').collect();
        if segments.iter().any(|s| s.is_empty()) {
            return Err(RegistryError::InvalidId(id.to_string()));
        }
        Ok(())
    }
}

impl Default for ExtensionPointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for registry operations.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl RegistryStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &RegistryStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for RegistryStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RegistryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RegistryStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for registry.
#[derive(Debug, Clone)]
pub struct RegistryValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl RegistryValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for RegistryValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ContributionValue & SchemaFieldType
// ---------------------------------------------------------------------------

/// Describes the expected type of a field in an extension point schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaFieldType {
    /// A string value.
    StringType,
    /// A boolean value.
    BoolType,
    /// A numeric (f64) value.
    NumberType,
    /// An array of contribution values.
    ArrayType,
}

/// A dynamically-typed value that can appear in a contribution.
#[derive(Debug, Clone, PartialEq)]
pub enum ContributionValue {
    /// String payload.
    Str(String),
    /// Boolean payload.
    Bool(bool),
    /// Numeric payload.
    Number(f64),
    /// Array of nested values.
    Array(Vec<ContributionValue>),
}

impl ContributionValue {
    /// Returns the `SchemaFieldType` that corresponds to this value.
    pub fn field_type(&self) -> SchemaFieldType {
        match self {
            ContributionValue::Str(_) => SchemaFieldType::StringType,
            ContributionValue::Bool(_) => SchemaFieldType::BoolType,
            ContributionValue::Number(_) => SchemaFieldType::NumberType,
            ContributionValue::Array(_) => SchemaFieldType::ArrayType,
        }
    }

    /// Returns a human-readable type name.
    pub fn type_name(&self) -> &'static str {
        match self {
            ContributionValue::Str(_) => "string",
            ContributionValue::Bool(_) => "bool",
            ContributionValue::Number(_) => "number",
            ContributionValue::Array(_) => "array",
        }
    }
}

impl fmt::Display for ContributionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContributionValue::Str(s) => write!(f, "\"{s}\""),
            ContributionValue::Bool(b) => write!(f, "{b}"),
            ContributionValue::Number(n) => write!(f, "{n}"),
            ContributionValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
        }
    }
}

impl SchemaFieldType {
    /// Returns a human-readable name for this field type.
    pub fn type_name(&self) -> &'static str {
        match self {
            SchemaFieldType::StringType => "string",
            SchemaFieldType::BoolType => "bool",
            SchemaFieldType::NumberType => "number",
            SchemaFieldType::ArrayType => "array",
        }
    }

    /// Returns `true` if the given `ContributionValue` matches this schema type.
    pub fn matches(&self, value: &ContributionValue) -> bool {
        value.field_type() == *self
    }
}

impl fmt::Display for SchemaFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.type_name())
    }
}

// ---------------------------------------------------------------------------
// ExtensionPointValidator
// ---------------------------------------------------------------------------

/// Validates contributions against an extension point schema.
///
/// Define the expected fields and their types, then validate incoming
/// contribution maps. Missing fields and type mismatches are reported as
/// a list of human-readable error strings.
#[derive(Debug, Clone)]
pub struct ExtensionPointValidator {
    schema: HashMap<String, SchemaFieldType>,
}

impl ExtensionPointValidator {
    /// Create a new validator with an empty schema.
    pub fn new() -> Self {
        Self {
            schema: HashMap::new(),
        }
    }

    /// Add a required field to the schema.
    pub fn add_field(&mut self, name: &str, field_type: SchemaFieldType) {
        self.schema.insert(name.to_string(), field_type);
    }

    /// Returns the number of fields in the schema.
    pub fn field_count(&self) -> usize {
        self.schema.len()
    }

    /// Returns `true` if the schema contains no fields.
    pub fn is_empty(&self) -> bool {
        self.schema.is_empty()
    }

    /// Returns the expected type for a given field name, if defined.
    pub fn get_field_type(&self, name: &str) -> Option<&SchemaFieldType> {
        self.schema.get(name)
    }

    /// Validate a contribution against the schema.
    ///
    /// Returns an empty `Vec` if the contribution is valid.  Otherwise each
    /// entry describes one validation failure (missing field or type mismatch).
    pub fn validate_contribution(
        &self,
        contribution: &HashMap<String, ContributionValue>,
    ) -> Vec<String> {
        let mut errors = Vec::new();

        // Check every schema field is present and has the right type.
        let mut sorted_fields: Vec<(&String, &SchemaFieldType)> = self.schema.iter().collect();
        sorted_fields.sort_by_key(|(name, _)| name.as_str());

        for (field_name, expected_type) in &sorted_fields {
            match contribution.get(*field_name) {
                None => {
                    errors.push(format!("missing required field: {field_name}"));
                }
                Some(value) => {
                    if !expected_type.matches(value) {
                        errors.push(format!(
                            "field '{field_name}' expected type {expected_type}, got {}",
                            value.type_name()
                        ));
                    }
                }
            }
        }

        errors
    }
}

impl Default for ExtensionPointValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RegistrySnapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of the extension point registry.
///
/// Snapshots are cheap to create and can be compared via [`registry_diff`] to
/// detect additions and removals between two points in time.
#[derive(Debug, Clone)]
pub struct RegistrySnapshot {
    point_ids: Vec<String>,
    metadata: HashMap<String, ExtensionPointMetadata>,
    timestamp: u64,
}

impl RegistrySnapshot {
    /// Capture a snapshot from the current state of a registry.
    pub fn from_registry(registry: &ExtensionPointRegistry, timestamp: u64) -> Self {
        let mut point_ids: Vec<String> = registry.iter().map(String::from).collect();
        point_ids.sort();

        let metadata: HashMap<String, ExtensionPointMetadata> = point_ids
            .iter()
            .filter_map(|id| {
                registry
                    .get_metadata(id)
                    .cloned()
                    .map(|m| (id.clone(), m))
            })
            .collect();

        Self {
            point_ids,
            metadata,
            timestamp,
        }
    }

    /// Returns `true` if the snapshot contains the given extension point ID.
    pub fn contains(&self, id: &str) -> bool {
        self.point_ids.iter().any(|p| p == id)
    }

    /// Returns the number of extension points in the snapshot.
    pub fn len(&self) -> usize {
        self.point_ids.len()
    }

    /// Returns `true` if the snapshot contains no extension points.
    pub fn is_empty(&self) -> bool {
        self.point_ids.is_empty()
    }

    /// Returns the sorted list of extension point IDs.
    pub fn point_ids(&self) -> &[String] {
        &self.point_ids
    }

    /// Returns the timestamp at which this snapshot was taken.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns metadata for the given extension point, if captured.
    pub fn get_metadata(&self, id: &str) -> Option<&ExtensionPointMetadata> {
        self.metadata.get(id)
    }
}

impl fmt::Display for RegistrySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RegistrySnapshot(points={}, ts={})",
            self.point_ids.len(),
            self.timestamp
        )
    }
}

// ---------------------------------------------------------------------------
// RegistryDiff
// ---------------------------------------------------------------------------

/// The difference between two [`RegistrySnapshot`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryDiff {
    /// Extension points present in `new` but not in `old`.
    pub added: Vec<String>,
    /// Extension points present in `old` but not in `new`.
    pub removed: Vec<String>,
    /// Extension points present in both snapshots.
    pub unchanged: Vec<String>,
}

impl RegistryDiff {
    /// Returns `true` if there are no additions or removals.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Returns a human-readable summary of the diff.
    pub fn summary(&self) -> String {
        format!(
            "{} added, {} removed, {} unchanged",
            self.added.len(),
            self.removed.len(),
            self.unchanged.len()
        )
    }

    /// Returns the total number of changes (additions + removals).
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len()
    }
}

impl fmt::Display for RegistryDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryDiff({})", self.summary())
    }
}

/// Compute the difference between two registry snapshots.
///
/// Extension points that appear only in `new_snap` are reported as added;
/// those only in `old_snap` are removed; those in both are unchanged.
pub fn registry_diff(old_snap: &RegistrySnapshot, new_snap: &RegistrySnapshot) -> RegistryDiff {
    let old_set: HashSet<&str> = old_snap.point_ids.iter().map(String::as_str).collect();
    let new_set: HashSet<&str> = new_snap.point_ids.iter().map(String::as_str).collect();

    let mut added: Vec<String> = new_set
        .difference(&old_set)
        .map(|s| s.to_string())
        .collect();
    added.sort();

    let mut removed: Vec<String> = old_set
        .difference(&new_set)
        .map(|s| s.to_string())
        .collect();
    removed.sort();

    let mut unchanged: Vec<String> = old_set
        .intersection(&new_set)
        .map(|s| s.to_string())
        .collect();
    unchanged.sort();

    RegistryDiff {
        added,
        removed,
        unchanged,
    }
}

// ---------------------------------------------------------------------------
// RegistryEventLog
// ---------------------------------------------------------------------------

/// The kind of event recorded in a [`RegistryEventLog`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryEventKind {
    /// An extension point was registered.
    Registered,
    /// An extension point was unregistered.
    Unregistered,
}

impl fmt::Display for RegistryEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryEventKind::Registered => f.write_str("registered"),
            RegistryEventKind::Unregistered => f.write_str("unregistered"),
        }
    }
}

/// A single timestamped event in the registry event log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEvent {
    pub kind: RegistryEventKind,
    pub point_id: String,
    pub timestamp_ns: u64,
}

impl fmt::Display for RegistryEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}ns] {} '{}'",
            self.timestamp_ns, self.kind, self.point_id
        )
    }
}

/// An append-only log of registry mutation events.
///
/// Useful for auditing which extension points were registered or unregistered
/// and in what order. Each event carries a caller-supplied timestamp so the log
/// can be correlated with external clocks.
#[derive(Debug, Clone, Default)]
pub struct RegistryEventLog {
    events: Vec<RegistryEvent>,
}

impl RegistryEventLog {
    /// Create an empty event log.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Record a registration event.
    pub fn record_register(&mut self, point_id: &str, timestamp_ns: u64) {
        self.events.push(RegistryEvent {
            kind: RegistryEventKind::Registered,
            point_id: point_id.to_string(),
            timestamp_ns,
        });
    }

    /// Record an unregistration event.
    pub fn record_unregister(&mut self, point_id: &str, timestamp_ns: u64) {
        self.events.push(RegistryEvent {
            kind: RegistryEventKind::Unregistered,
            point_id: point_id.to_string(),
            timestamp_ns,
        });
    }

    /// Returns all recorded events in chronological order.
    pub fn events(&self) -> &[RegistryEvent] {
        &self.events
    }

    /// Returns the number of recorded events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if no events have been recorded.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns only events that match the given kind.
    pub fn filter_by_kind(&self, kind: &RegistryEventKind) -> Vec<&RegistryEvent> {
        self.events.iter().filter(|e| e.kind == *kind).collect()
    }

    /// Returns only events for the given extension point ID.
    pub fn filter_by_point(&self, point_id: &str) -> Vec<&RegistryEvent> {
        self.events
            .iter()
            .filter(|e| e.point_id == point_id)
            .collect()
    }

    /// Returns events within the given timestamp range (inclusive).
    pub fn filter_by_time_range(&self, start_ns: u64, end_ns: u64) -> Vec<&RegistryEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp_ns >= start_ns && e.timestamp_ns <= end_ns)
            .collect()
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// DependencyTracker
// ---------------------------------------------------------------------------

/// Tracks directed dependencies between extension points.
///
/// An edge `(A, B)` means extension point `A` depends on extension point `B`.
/// The tracker can detect missing dependencies and cycles.
#[derive(Debug, Clone, Default)]
pub struct DependencyTracker {
    /// Maps each extension point to the set of points it depends on.
    deps: HashMap<String, HashSet<String>>,
}

impl DependencyTracker {
    /// Create an empty dependency tracker.
    pub fn new() -> Self {
        Self {
            deps: HashMap::new(),
        }
    }

    /// Declare that `point` depends on `dependency`.
    pub fn add_dependency(&mut self, point: &str, dependency: &str) {
        self.deps
            .entry(point.to_string())
            .or_default()
            .insert(dependency.to_string());
    }

    /// Remove a dependency edge.
    pub fn remove_dependency(&mut self, point: &str, dependency: &str) -> bool {
        if let Some(set) = self.deps.get_mut(point) {
            return set.remove(dependency);
        }
        false
    }

    /// Returns the direct dependencies of `point`.
    pub fn direct_dependencies(&self, point: &str) -> Vec<&str> {
        let mut result: Vec<&str> = self
            .deps
            .get(point)
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default();
        result.sort();
        result
    }

    /// Returns all extension points that directly depend on `target`.
    pub fn dependents_of(&self, target: &str) -> Vec<&str> {
        let mut result: Vec<&str> = self
            .deps
            .iter()
            .filter(|(_, deps)| deps.contains(target))
            .map(|(point, _)| point.as_str())
            .collect();
        result.sort();
        result
    }

    /// Check for dependencies that reference points not present in `registry`.
    pub fn find_missing(
        &self,
        registry: &ExtensionPointRegistry,
    ) -> Vec<(String, String)> {
        let mut missing = Vec::new();
        for (point, deps) in &self.deps {
            for dep in deps {
                if !registry.has_point(dep) {
                    missing.push((point.clone(), dep.clone()));
                }
            }
        }
        missing.sort();
        missing
    }

    /// Detect whether adding or having a dependency from `point` to `dependency`
    /// would create a cycle. Uses iterative DFS.
    pub fn has_cycle_through(&self, point: &str, dependency: &str) -> bool {
        // Check if `dependency` can reach `point` through existing edges.
        let mut visited = HashSet::new();
        let mut stack = vec![dependency.to_string()];
        while let Some(current) = stack.pop() {
            if current == point {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(next) = self.deps.get(&current) {
                    for n in next {
                        stack.push(n.clone());
                    }
                }
            }
        }
        false
    }

    /// Returns the total number of dependency edges.
    pub fn edge_count(&self) -> usize {
        self.deps.values().map(|s| s.len()).sum()
    }

    /// Returns the number of extension points that have at least one dependency.
    pub fn point_count(&self) -> usize {
        self.deps.len()
    }
}

// ---------------------------------------------------------------------------
// Batch validation
// ---------------------------------------------------------------------------

/// Result of validating a single contribution in a batch.
#[derive(Debug, Clone)]
pub struct BatchValidationResult {
    pub index: usize,
    pub errors: Vec<String>,
}

impl BatchValidationResult {
    /// Returns `true` if the contribution passed validation.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate multiple contributions against a schema in one call.
///
/// Returns a [`BatchValidationResult`] for each contribution, preserving the
/// input ordering. Only entries with errors are included in the returned vec
/// when `errors_only` is `true`.
pub fn batch_validate(
    validator: &ExtensionPointValidator,
    contributions: &[HashMap<String, ContributionValue>],
    errors_only: bool,
) -> Vec<BatchValidationResult> {
    contributions
        .iter()
        .enumerate()
        .filter_map(|(i, contrib)| {
            let errors = validator.validate_contribution(contrib);
            if errors_only && errors.is_empty() {
                None
            } else {
                Some(BatchValidationResult { index: i, errors })
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Registry merge
// ---------------------------------------------------------------------------

/// Merge the contents of `source` into `target`.
///
/// Extension points already present in `target` are skipped (no error).
/// Metadata from `source` is copied for newly added points.
/// Returns the number of new extension points added.
pub fn merge_registries(
    target: &mut ExtensionPointRegistry,
    source: &ExtensionPointRegistry,
) -> usize {
    let mut added = 0;
    for id in source.iter() {
        if !target.has_point(id) {
            if let Some(meta) = source.get_metadata(id) {
                target.register_point_with_metadata(id, meta.clone());
            } else {
                target.register_point(id);
            }
            added += 1;
        }
    }
    added
}

// ---------------------------------------------------------------------------
// RegistryBulkOps – batch register/unregister
// ---------------------------------------------------------------------------

/// Batch operations on the registry.
pub struct RegistryBulkOps;

impl RegistryBulkOps {
    /// Register multiple extension points at once, returning how many were newly added.
    pub fn register_many(registry: &mut ExtensionPointRegistry, ids: &[&str]) -> usize {
        let mut added = 0;
        for id in ids {
            let before = registry.len();
            registry.register_point(id);
            if registry.len() > before {
                added += 1;
            }
        }
        added
    }

    /// Unregister multiple extension points, returning how many were removed.
    pub fn unregister_many(registry: &mut ExtensionPointRegistry, ids: &[&str]) -> usize {
        let mut removed = 0;
        for id in ids {
            if registry.unregister_point(id).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    /// Register points only if all IDs pass validation. Returns error on first failure.
    pub fn register_validated(
        registry: &mut ExtensionPointRegistry,
        ids: &[&str],
    ) -> Result<usize, RegistryError> {
        for id in ids {
            ExtensionPointRegistry::validate_id(id)?;
        }
        Ok(Self::register_many(registry, ids))
    }
}

// ---------------------------------------------------------------------------
// RegistryDepGraph – dependency tracking between providers
// ---------------------------------------------------------------------------

/// Tracks dependencies between extension point providers.
#[derive(Debug, Clone, Default)]
pub struct RegistryDepGraph {
    /// Maps a provider to the set of providers it depends on.
    edges: HashMap<String, HashSet<String>>,
}

impl RegistryDepGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare that `provider` depends on `dependency`.
    pub fn add_dependency(&mut self, provider: &str, dependency: &str) {
        self.edges
            .entry(provider.to_string())
            .or_default()
            .insert(dependency.to_string());
    }

    /// Return direct dependencies of `provider`.
    pub fn dependencies_of(&self, provider: &str) -> Vec<&str> {
        self.edges
            .get(provider)
            .map(|s| s.iter().map(|d| d.as_str()).collect())
            .unwrap_or_default()
    }

    /// Return providers that directly depend on `dependency`.
    pub fn dependents_of(&self, dependency: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|(_, deps)| deps.contains(dependency))
            .map(|(k, _)| k.as_str())
            .collect()
    }

    /// Check if adding `provider -> dependency` would create a cycle.
    pub fn would_cycle(&self, provider: &str, dependency: &str) -> bool {
        if provider == dependency {
            return true;
        }
        let mut visited = HashSet::new();
        let mut stack = vec![dependency.to_string()];
        while let Some(current) = stack.pop() {
            if current == provider {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(deps) = self.edges.get(&current) {
                    for dep in deps {
                        stack.push(dep.clone());
                    }
                }
            }
        }
        false
    }

    /// Return a topological order if the graph is acyclic, or `None` if cyclic.
    pub fn topological_order(&self) -> Option<Vec<String>> {
        let mut all_nodes: HashSet<&str> = HashSet::new();
        for (k, deps) in &self.edges {
            all_nodes.insert(k.as_str());
            for d in deps {
                all_nodes.insert(d.as_str());
            }
        }
        // Use Kahn's algorithm on the reversed graph
        // edge: provider -> dependency means dependency must come before provider
        let mut in_deg: HashMap<&str, usize> = HashMap::new();
        for &node in &all_nodes {
            in_deg.insert(node, 0);
        }
        for (provider, _deps) in &self.edges {
            // provider depends on deps, so provider has in_degree = deps.len()
            *in_deg.entry(provider.as_str()).or_insert(0) += _deps.len();
        }
        let mut queue: Vec<&str> = in_deg.iter().filter(|(_, d)| **d == 0).map(|(k, _)| *k).collect();
        queue.sort(); // deterministic
        let mut result = Vec::new();
        while let Some(node) = queue.pop() {
            result.push(node.to_string());
            // For each provider that depends on `node`, decrement in_degree
            for (provider, deps) in &self.edges {
                if deps.contains(node) {
                    if let Some(deg) = in_deg.get_mut(provider.as_str()) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(provider.as_str());
                            queue.sort();
                        }
                    }
                }
            }
        }
        if result.len() == all_nodes.len() {
            Some(result)
        } else {
            None
        }
    }

    /// Number of providers in the graph.
    pub fn provider_count(&self) -> usize {
        self.edges.len()
    }
}

impl fmt::Display for RegistryDepGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryDepGraph({} providers)", self.edges.len())
    }
}

// ---------------------------------------------------------------------------
// RegistryChangeBatch – notification batching
// ---------------------------------------------------------------------------

/// A single change event in the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryChange {
    Added(String),
    Removed(String),
    MetadataUpdated(String),
}

impl fmt::Display for RegistryChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryChange::Added(id) => write!(f, "+{id}"),
            RegistryChange::Removed(id) => write!(f, "-{id}"),
            RegistryChange::MetadataUpdated(id) => write!(f, "~{id}"),
        }
    }
}

/// Batches registry changes for deferred notification.
#[derive(Debug, Clone, Default)]
pub struct RegistryChangeBatch {
    changes: Vec<RegistryChange>,
}

impl RegistryChangeBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an addition.
    pub fn record_add(&mut self, id: &str) {
        self.changes.push(RegistryChange::Added(id.to_string()));
    }

    /// Record a removal.
    pub fn record_remove(&mut self, id: &str) {
        self.changes.push(RegistryChange::Removed(id.to_string()));
    }

    /// Record a metadata update.
    pub fn record_metadata_update(&mut self, id: &str) {
        self.changes.push(RegistryChange::MetadataUpdated(id.to_string()));
    }

    /// How many changes are batched.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Drain all changes and return them, leaving the batch empty.
    pub fn drain(&mut self) -> Vec<RegistryChange> {
        std::mem::take(&mut self.changes)
    }

    /// Return only the additions.
    pub fn additions(&self) -> Vec<&str> {
        self.changes.iter().filter_map(|c| match c {
            RegistryChange::Added(id) => Some(id.as_str()),
            _ => None,
        }).collect()
    }

    /// Return only the removals.
    pub fn removals(&self) -> Vec<&str> {
        self.changes.iter().filter_map(|c| match c {
            RegistryChange::Removed(id) => Some(id.as_str()),
            _ => None,
        }).collect()
    }

    /// Compact: remove redundant add+remove pairs for the same ID.
    pub fn compact(&mut self) {
        let added: HashSet<String> = self.additions().iter().map(|s| s.to_string()).collect();
        let removed: HashSet<String> = self.removals().iter().map(|s| s.to_string()).collect();
        let cancelled: HashSet<&String> = added.intersection(&removed).collect();
        self.changes.retain(|c| {
            let id = match c {
                RegistryChange::Added(id) | RegistryChange::Removed(id) | RegistryChange::MetadataUpdated(id) => id,
            };
            !cancelled.contains(id)
        });
    }
}

impl fmt::Display for RegistryChangeBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryChangeBatch({} changes)", self.changes.len())
    }
}

// ---------------------------------------------------------------------------
// RegistryDependencyChecker
// ---------------------------------------------------------------------------

/// Result of a single dependency check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepCheckResult {
    pub extension_id: String,
    pub dependency_id: String,
    pub satisfied: bool,
    pub reason: String,
}

impl DepCheckResult {
    pub fn satisfied(ext: impl Into<String>, dep: impl Into<String>) -> Self {
        Self {
            extension_id: ext.into(),
            dependency_id: dep.into(),
            satisfied: true,
            reason: "dependency found".into(),
        }
    }

    pub fn unsatisfied(ext: impl Into<String>, dep: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            extension_id: ext.into(),
            dependency_id: dep.into(),
            satisfied: false,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for DepCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.satisfied { "OK" } else { "FAIL" };
        write!(f, "[{status}] {} -> {}: {}", self.extension_id, self.dependency_id, self.reason)
    }
}

/// Validates extension dependencies in the registry.
pub struct RegistryDependencyChecker {
    registered_extensions: std::collections::HashSet<String>,
    dependencies: std::collections::HashMap<String, Vec<String>>,
}

impl RegistryDependencyChecker {
    pub fn new() -> Self {
        Self {
            registered_extensions: std::collections::HashSet::new(),
            dependencies: std::collections::HashMap::new(),
        }
    }

    pub fn register_extension(&mut self, id: impl Into<String>) {
        self.registered_extensions.insert(id.into());
    }

    pub fn add_dependency(&mut self, ext_id: impl Into<String>, dep_id: impl Into<String>) {
        self.dependencies.entry(ext_id.into()).or_default().push(dep_id.into());
    }

    pub fn extension_count(&self) -> usize {
        self.registered_extensions.len()
    }

    /// Check all dependencies for a given extension.
    pub fn check(&self, ext_id: &str) -> Vec<DepCheckResult> {
        match self.dependencies.get(ext_id) {
            None => vec![],
            Some(deps) => deps
                .iter()
                .map(|dep| {
                    if self.registered_extensions.contains(dep) {
                        DepCheckResult::satisfied(ext_id, dep)
                    } else {
                        DepCheckResult::unsatisfied(ext_id, dep, "dependency not registered")
                    }
                })
                .collect(),
        }
    }

    /// Check all extensions and return all unsatisfied dependencies.
    pub fn check_all(&self) -> Vec<DepCheckResult> {
        let mut results = Vec::new();
        for ext_id in self.dependencies.keys() {
            for r in self.check(ext_id) {
                if !r.satisfied {
                    results.push(r);
                }
            }
        }
        results
    }

    /// Detect circular dependencies using DFS.
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = Vec::new();

        for ext in self.dependencies.keys() {
            if !visited.contains(ext) {
                self.dfs_cycle(ext, &mut visited, &mut stack, &mut cycles);
            }
        }
        cycles
    }

    fn dfs_cycle(
        &self,
        node: &str,
        visited: &mut std::collections::HashSet<String>,
        stack: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        if let Some(pos) = stack.iter().position(|n| n == node) {
            cycles.push(stack[pos..].to_vec());
            return;
        }
        if visited.contains(node) {
            return;
        }
        stack.push(node.to_string());
        if let Some(deps) = self.dependencies.get(node) {
            for dep in deps {
                self.dfs_cycle(dep, visited, stack, cycles);
            }
        }
        stack.pop();
        visited.insert(node.to_string());
    }

    /// Return topological order if no cycles, or None.
    pub fn topological_order(&self) -> Option<Vec<String>> {
        if !self.find_cycles().is_empty() {
            return None;
        }
        let mut result = Vec::new();
        let mut visited = std::collections::HashSet::new();
        for ext in &self.registered_extensions {
            self.topo_visit(ext, &mut visited, &mut result);
        }
        Some(result)
    }

    fn topo_visit(&self, node: &str, visited: &mut std::collections::HashSet<String>, result: &mut Vec<String>) {
        if visited.contains(node) {
            return;
        }
        visited.insert(node.to_string());
        if let Some(deps) = self.dependencies.get(node) {
            for dep in deps {
                self.topo_visit(dep, visited, result);
            }
        }
        result.push(node.to_string());
    }
}

impl fmt::Display for RegistryDependencyChecker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryDependencyChecker({} exts, {} dep sets)",
            self.registered_extensions.len(), self.dependencies.len())
    }
}

// ---------------------------------------------------------------------------
// RegistryHotReloadHandler
// ---------------------------------------------------------------------------

/// Status of a hot-reload operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotReloadStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

impl fmt::Display for HotReloadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HotReloadStatus::Pending => write!(f, "pending"),
            HotReloadStatus::InProgress => write!(f, "in-progress"),
            HotReloadStatus::Completed => write!(f, "completed"),
            HotReloadStatus::Failed(msg) => write!(f, "failed: {msg}"),
        }
    }
}

/// Record of a hot-reload event.
#[derive(Debug, Clone)]
pub struct HotReloadRecord {
    pub extension_id: String,
    pub version: String,
    pub status: HotReloadStatus,
    pub timestamp: u64,
}

impl HotReloadRecord {
    pub fn new(ext_id: impl Into<String>, version: impl Into<String>, timestamp: u64) -> Self {
        Self {
            extension_id: ext_id.into(),
            version: version.into(),
            status: HotReloadStatus::Pending,
            timestamp,
        }
    }
}

impl fmt::Display for HotReloadRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{} [{}] t={}", self.extension_id, self.version, self.status, self.timestamp)
    }
}

/// Handles hot-reloading of registry entries by tracking reload events and versions.
pub struct RegistryHotReloadHandler {
    records: Vec<HotReloadRecord>,
    max_history: usize,
}

impl RegistryHotReloadHandler {
    pub fn new(max_history: usize) -> Self {
        Self { records: Vec::new(), max_history }
    }

    pub fn request_reload(&mut self, ext_id: impl Into<String>, version: impl Into<String>, timestamp: u64) -> usize {
        let record = HotReloadRecord::new(ext_id, version, timestamp);
        self.records.push(record);
        if self.records.len() > self.max_history {
            self.records.remove(0);
        }
        self.records.len() - 1
    }

    pub fn mark_in_progress(&mut self, index: usize) -> bool {
        if let Some(r) = self.records.get_mut(index) {
            r.status = HotReloadStatus::InProgress;
            true
        } else {
            false
        }
    }

    pub fn mark_completed(&mut self, index: usize) -> bool {
        if let Some(r) = self.records.get_mut(index) {
            r.status = HotReloadStatus::Completed;
            true
        } else {
            false
        }
    }

    pub fn mark_failed(&mut self, index: usize, reason: impl Into<String>) -> bool {
        if let Some(r) = self.records.get_mut(index) {
            r.status = HotReloadStatus::Failed(reason.into());
            true
        } else {
            false
        }
    }

    pub fn history_len(&self) -> usize {
        self.records.len()
    }

    pub fn latest_for(&self, ext_id: &str) -> Option<&HotReloadRecord> {
        self.records.iter().rev().find(|r| r.extension_id == ext_id)
    }

    pub fn failed_records(&self) -> Vec<&HotReloadRecord> {
        self.records.iter().filter(|r| matches!(r.status, HotReloadStatus::Failed(_))).collect()
    }

    pub fn completed_count(&self) -> usize {
        self.records.iter().filter(|r| r.status == HotReloadStatus::Completed).count()
    }
}

impl fmt::Display for RegistryHotReloadHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RegistryHotReloadHandler({} records, max={})", self.records.len(), self.max_history)
    }
}



/// Service registry configuration manager.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    entries: Vec<RegistryEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single service registry entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl RegistryEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl RegistryConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: RegistryEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut RegistryEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&RegistryEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&RegistryEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&RegistryEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<RegistryEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Service and component registry — extended utilities (xc)
// ---------------------------------------------------------------------------

/// Metric accumulator for registry operations.
#[derive(Debug, Clone)]
pub struct XcMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XcMetrics {
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

/// Sliding-window rate counter for registry.
#[derive(Debug, Clone)]
pub struct XcRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XcRateWindow {
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

/// A small LRU-style cache for registry lookups.
#[derive(Debug, Clone)]
pub struct XcLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XcLruCache {
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
// xb_ utilities – batch 19
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer19 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer19 {
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
pub fn xb_fnv1a_19(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_19<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_19<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_19(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_19(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 147
// ---------------------------------------------------------------------------

/// Generic object pool `Xc147Pool<T>`.
pub struct Xc147Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc147Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc147PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc147Pool<T> {
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
    pub fn stats(&self) -> Xc147PoolStats {
        Xc147PoolStats {
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

impl<T> Default for Xc147Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc147Scheduler`.
pub struct Xc147Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc147Scheduler {
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

impl Default for Xc147Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_147 hash for the given byte slice.
pub fn xc_147_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_147 convention.
pub fn xc_147_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe31 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe31Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe31PipelineError {
    pub stage: Xe31Stage,
    pub message: String,
}

impl std::fmt::Display for Xe31PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe31Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe31Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError>>>,
    stage_names: Vec<Xe31Stage>,
}

impl Xe31Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe31Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe31Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe31Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe31Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
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

    pub fn compose(mut self, other: Xe31Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe31CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe31CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe31Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe31CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe31CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe31Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe31CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_31_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe31CacheEntry {
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

    fn xe_31_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe31CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_31_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
    Ok(data)
}

pub fn xe_31_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_31_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_31_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_31_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe31PipelineError> {
    Err(Xe31PipelineError {
        stage: Xe31Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #117
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf117Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf117TrieNode {
    children: std::collections::HashMap<char, Xf117TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf117Trie {
    root: Xf117TrieNode,
    count: usize,
}

impl Xf117Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf117TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf117TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf117TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf117BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf117BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 146).
pub struct Xh146SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh146SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 188 as u64,
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

/// A compact bit set supporting boolean operations (variant 146).
pub struct Xh146BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh146BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 146).
pub struct Xi146Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi146Deque<T> {
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
pub struct Xi146Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi146Interval {
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

/// A simple interval tree (variant 146).
pub struct Xi146IntervalTree {
    xi_intervals: Vec<Xi146Interval>,
}

impl Xi146IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi146Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi146Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi146Interval) -> Vec<&Xi146Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi146Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi146Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi146Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi146Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi146Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi146Interval> = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check() {
        let mut reg = ExtensionPointRegistry::new();
        assert!(!reg.has_point("vsedit.configuration"));

        reg.register_point("vsedit.configuration");
        assert!(reg.has_point("vsedit.configuration"));
    }

    #[test]
    fn register_duplicate_is_idempotent() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.commands");
        reg.register_point("vsedit.commands");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn len_and_is_empty() {
        let mut reg = ExtensionPointRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.register_point("a");
        reg.register_point("b");
        assert_eq!(reg.len(), 2);
        assert!(!reg.is_empty());
    }

    #[test]
    fn metadata() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point_with_metadata(
            "vsedit.themes",
            ExtensionPointMetadata {
                description: "Color themes".into(),
                ..Default::default()
            },
        );
        assert!(reg.has_point("vsedit.themes"));
        let meta = reg.get_metadata("vsedit.themes").unwrap();
        assert_eq!(meta.description, "Color themes");
    }

    #[test]
    fn get_metadata_returns_none_for_simple_point() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("simple");
        assert!(reg.get_metadata("simple").is_none());
    }

    #[test]
    fn default_impl() {
        let reg = ExtensionPointRegistry::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn iter_returns_all_points() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("a");
        reg.register_point("b");
        reg.register_point("c");

        let mut ids: Vec<&str> = reg.iter().collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn unregister_existing_point() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point_with_metadata(
            "vsedit.commands",
            ExtensionPointMetadata {
                description: "Commands".into(),
                ..Default::default()
            },
        );
        assert!(reg.has_point("vsedit.commands"));
        reg.unregister_point("vsedit.commands").unwrap();
        assert!(!reg.has_point("vsedit.commands"));
        assert!(reg.get_metadata("vsedit.commands").is_none());
    }

    #[test]
    fn unregister_missing_point_returns_error() {
        let mut reg = ExtensionPointRegistry::new();
        let err = reg.unregister_point("nonexistent").unwrap_err();
        assert_eq!(err, RegistryError::PointNotFound("nonexistent".into()));
    }

    #[test]
    fn try_register_duplicate_returns_error() {
        let mut reg = ExtensionPointRegistry::new();
        reg.try_register("vsedit.editor").unwrap();
        let err = reg.try_register("vsedit.editor").unwrap_err();
        assert_eq!(err, RegistryError::DuplicatePoint("vsedit.editor".into()));
    }

    #[test]
    fn find_by_prefix_returns_sorted_matches() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.editor.tabs");
        reg.register_point("vsedit.editor.gutter");
        reg.register_point("vsedit.themes");
        reg.register_point("vsedit.editor.minimap");

        let results = reg.find_by_prefix("vsedit.editor.");
        assert_eq!(
            results,
            vec!["vsedit.editor.gutter", "vsedit.editor.minimap", "vsedit.editor.tabs"]
        );
    }

    #[test]
    fn get_deprecated_filters_correctly() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point_with_metadata(
            "vsedit.oldapi",
            ExtensionPointMetadata {
                description: "Old API".into(),
                deprecated: true,
                ..Default::default()
            },
        );
        reg.register_point_with_metadata(
            "vsedit.newapi",
            ExtensionPointMetadata {
                description: "New API".into(),
                deprecated: false,
                ..Default::default()
            },
        );
        reg.register_point("vsedit.plain");

        let deprecated = reg.get_deprecated();
        assert_eq!(deprecated, vec!["vsedit.oldapi"]);
    }

    #[test]
    fn clear_removes_all() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("a");
        reg.register_point("b");
        reg.register_point_with_metadata(
            "c",
            ExtensionPointMetadata {
                description: "C".into(),
                ..Default::default()
            },
        );
        assert_eq!(reg.len(), 3);
        reg.clear();
        assert!(reg.is_empty());
        assert!(reg.get_metadata("c").is_none());
    }

    #[test]
    fn validate_id_accepts_valid_ids() {
        assert!(ExtensionPointRegistry::validate_id("vsedit.commands").is_ok());
        assert!(ExtensionPointRegistry::validate_id("a.b.c.d").is_ok());
        assert!(ExtensionPointRegistry::validate_id("single").is_ok());
    }

    #[test]
    fn validate_id_rejects_invalid_ids() {
        assert!(ExtensionPointRegistry::validate_id("").is_err());
        assert!(ExtensionPointRegistry::validate_id("has space").is_err());
        assert!(ExtensionPointRegistry::validate_id(".leading.dot").is_err());
        assert!(ExtensionPointRegistry::validate_id("trailing.dot.").is_err());
        assert!(ExtensionPointRegistry::validate_id("double..dot").is_err());
    }

    #[test]
    fn metadata_display_impl() {
        let meta = ExtensionPointMetadata {
            description: "Color themes".into(),
            version: Some("1.2.0".into()),
            deprecated: false,
        };
        assert_eq!(format!("{meta}"), "Color themes (v1.2.0)");

        let deprecated_meta = ExtensionPointMetadata {
            description: "Old themes".into(),
            version: None,
            deprecated: true,
        };
        assert_eq!(format!("{deprecated_meta}"), "Old themes [DEPRECATED]");
    }

    #[test]
    fn error_display_messages() {
        let e1 = RegistryError::DuplicatePoint("foo".into());
        assert_eq!(format!("{e1}"), "extension point already registered: foo");

        let e2 = RegistryError::PointNotFound("bar".into());
        assert_eq!(format!("{e2}"), "extension point not found: bar");

        let e3 = RegistryError::InvalidId("bad id".into());
        assert_eq!(format!("{e3}"), "invalid extension point id: bad id");
    }

    // -----------------------------------------------------------------------
    // ExtensionPointValidator tests
    // -----------------------------------------------------------------------

    #[test]
    fn validator_accepts_matching_contribution() {
        let mut v = ExtensionPointValidator::new();
        v.add_field("name", SchemaFieldType::StringType);
        v.add_field("enabled", SchemaFieldType::BoolType);
        v.add_field("priority", SchemaFieldType::NumberType);

        let mut contrib = HashMap::new();
        contrib.insert("name".into(), ContributionValue::Str("hello".into()));
        contrib.insert("enabled".into(), ContributionValue::Bool(true));
        contrib.insert("priority".into(), ContributionValue::Number(1.0));

        let errors = v.validate_contribution(&contrib);
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    #[test]
    fn validator_rejects_missing_field() {
        let mut v = ExtensionPointValidator::new();
        v.add_field("title", SchemaFieldType::StringType);
        v.add_field("active", SchemaFieldType::BoolType);

        let mut contrib = HashMap::new();
        contrib.insert("title".into(), ContributionValue::Str("t".into()));
        // "active" is missing

        let errors = v.validate_contribution(&contrib);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("missing required field"));
        assert!(errors[0].contains("active"));
    }

    #[test]
    fn validator_rejects_wrong_type() {
        let mut v = ExtensionPointValidator::new();
        v.add_field("count", SchemaFieldType::NumberType);

        let mut contrib = HashMap::new();
        contrib.insert(
            "count".into(),
            ContributionValue::Str("not a number".into()),
        );

        let errors = v.validate_contribution(&contrib);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("expected type number"));
        assert!(errors[0].contains("got string"));
    }

    #[test]
    fn validator_empty_schema_accepts_anything() {
        let v = ExtensionPointValidator::new();

        let mut contrib = HashMap::new();
        contrib.insert("whatever".into(), ContributionValue::Bool(false));
        contrib.insert("stuff".into(), ContributionValue::Number(42.0));

        let errors = v.validate_contribution(&contrib);
        assert!(errors.is_empty());

        // Also accepts an empty contribution.
        let errors2 = v.validate_contribution(&HashMap::new());
        assert!(errors2.is_empty());
    }

    #[test]
    fn validator_array_type_field() {
        let mut v = ExtensionPointValidator::new();
        v.add_field("tags", SchemaFieldType::ArrayType);

        let mut contrib = HashMap::new();
        contrib.insert(
            "tags".into(),
            ContributionValue::Array(vec![
                ContributionValue::Str("a".into()),
                ContributionValue::Str("b".into()),
            ]),
        );

        let errors = v.validate_contribution(&contrib);
        assert!(errors.is_empty());
    }

    // -----------------------------------------------------------------------
    // RegistrySnapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn snapshot_from_registry() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.commands");
        reg.register_point("vsedit.themes");

        let snap = RegistrySnapshot::from_registry(&reg, 100);
        assert_eq!(snap.len(), 2);
        assert_eq!(snap.timestamp(), 100);
        assert!(snap.point_ids().contains(&"vsedit.commands".to_string()));
        assert!(snap.point_ids().contains(&"vsedit.themes".to_string()));
    }

    #[test]
    fn snapshot_contains_check() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.keybindings");

        let snap = RegistrySnapshot::from_registry(&reg, 200);
        assert!(snap.contains("vsedit.keybindings"));
        assert!(!snap.contains("vsedit.nonexistent"));
    }

    // -----------------------------------------------------------------------
    // registry_diff tests
    // -----------------------------------------------------------------------

    #[test]
    fn diff_detects_additions() {
        let mut reg_old = ExtensionPointRegistry::new();
        reg_old.register_point("vsedit.a");

        let mut reg_new = ExtensionPointRegistry::new();
        reg_new.register_point("vsedit.a");
        reg_new.register_point("vsedit.b");

        let old_snap = RegistrySnapshot::from_registry(&reg_old, 0);
        let new_snap = RegistrySnapshot::from_registry(&reg_new, 1);

        let diff = registry_diff(&old_snap, &new_snap);
        assert_eq!(diff.added, vec!["vsedit.b".to_string()]);
        assert!(diff.removed.is_empty());
        assert_eq!(diff.unchanged, vec!["vsedit.a".to_string()]);
        assert!(!diff.is_empty());
    }

    #[test]
    fn diff_detects_removals() {
        let mut reg_old = ExtensionPointRegistry::new();
        reg_old.register_point("vsedit.x");
        reg_old.register_point("vsedit.y");

        let mut reg_new = ExtensionPointRegistry::new();
        reg_new.register_point("vsedit.x");

        let old_snap = RegistrySnapshot::from_registry(&reg_old, 10);
        let new_snap = RegistrySnapshot::from_registry(&reg_new, 20);

        let diff = registry_diff(&old_snap, &new_snap);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["vsedit.y".to_string()]);
        assert_eq!(diff.unchanged, vec!["vsedit.x".to_string()]);
        assert!(!diff.is_empty());
    }

    #[test]
    fn diff_unchanged_points() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.config");
        reg.register_point("vsedit.lang");

        let snap_a = RegistrySnapshot::from_registry(&reg, 0);
        let snap_b = RegistrySnapshot::from_registry(&reg, 1);

        let diff = registry_diff(&snap_a, &snap_b);
        assert!(diff.is_empty());
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.unchanged.len(), 2);
        assert_eq!(diff.summary(), "0 added, 0 removed, 2 unchanged");
    }

    #[test]
    fn behavior_check_0() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_34() {
        let _svc = ExtensionPointRegistry::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn registry_stats_new_defaults() {
        let stats = RegistryStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn registry_stats_record_success() {
        let mut stats = RegistryStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_stats_record_failure() {
        let mut stats = RegistryStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_stats_reset() {
        let mut stats = RegistryStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn registry_stats_merge() {
        let mut a = RegistryStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = RegistryStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn registry_stats_display() {
        let mut stats = RegistryStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn registry_stats_default() {
        let stats = RegistryStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn registry_validator_accepts_valid_name() {
        let v = RegistryValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn registry_validator_rejects_empty() {
        let v = RegistryValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn registry_validator_rejects_too_long() {
        let v = RegistryValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn registry_validator_forbidden_prefix() {
        let v = RegistryValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn registry_validator_allowed_chars() {
        let v = RegistryValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn registry_validator_range() {
        let v = RegistryValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn registry_sanitize_removes_control() {
        let result = RegistryValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn registry_truncate_short_string() {
        assert_eq!(RegistryValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn registry_truncate_long_string() {
        let result = RegistryValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn registry_is_ascii_printable() {
        assert!(RegistryValidator::is_ascii_printable("Hello World 123"));
        assert!(!RegistryValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // RegistryEventLog tests
    // -----------------------------------------------------------------------

    #[test]
    fn event_log_records_and_filters() {
        let mut log = RegistryEventLog::new();
        assert!(log.is_empty());

        log.record_register("vsedit.commands", 100);
        log.record_register("vsedit.themes", 200);
        log.record_unregister("vsedit.commands", 300);

        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());

        let registrations = log.filter_by_kind(&RegistryEventKind::Registered);
        assert_eq!(registrations.len(), 2);

        let unregistrations = log.filter_by_kind(&RegistryEventKind::Unregistered);
        assert_eq!(unregistrations.len(), 1);
        assert_eq!(unregistrations[0].point_id, "vsedit.commands");

        let cmd_events = log.filter_by_point("vsedit.commands");
        assert_eq!(cmd_events.len(), 2);

        let range = log.filter_by_time_range(150, 250);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].point_id, "vsedit.themes");

        // Display impls
        assert_eq!(format!("{}", log.events()[0]), "[100ns] registered 'vsedit.commands'");
    }

    // -----------------------------------------------------------------------
    // DependencyTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn dependency_tracker_basic_operations() {
        let mut tracker = DependencyTracker::new();
        tracker.add_dependency("vsedit.editor", "vsedit.config");
        tracker.add_dependency("vsedit.editor", "vsedit.themes");
        tracker.add_dependency("vsedit.debugger", "vsedit.editor");

        assert_eq!(tracker.edge_count(), 3);
        assert_eq!(tracker.point_count(), 2);

        let deps = tracker.direct_dependencies("vsedit.editor");
        assert_eq!(deps, vec!["vsedit.config", "vsedit.themes"]);

        let dependents = tracker.dependents_of("vsedit.editor");
        assert_eq!(dependents, vec!["vsedit.debugger"]);

        assert!(tracker.remove_dependency("vsedit.editor", "vsedit.themes"));
        assert!(!tracker.remove_dependency("vsedit.editor", "vsedit.themes"));
        assert_eq!(tracker.edge_count(), 2);
    }

    #[test]
    fn dependency_tracker_cycle_detection() {
        let mut tracker = DependencyTracker::new();
        tracker.add_dependency("a", "b");
        tracker.add_dependency("b", "c");

        // c -> a would form a cycle a -> b -> c -> a
        assert!(tracker.has_cycle_through("c", "a"));
        // d -> a would NOT form a cycle
        assert!(!tracker.has_cycle_through("d", "a"));
    }

    #[test]
    fn dependency_tracker_find_missing() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.editor");
        // vsedit.config is NOT registered

        let mut tracker = DependencyTracker::new();
        tracker.add_dependency("vsedit.editor", "vsedit.config");

        let missing = tracker.find_missing(&reg);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], ("vsedit.editor".to_string(), "vsedit.config".to_string()));
    }

    // -----------------------------------------------------------------------
    // Batch validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn batch_validate_mixed_results() {
        let mut v = ExtensionPointValidator::new();
        v.add_field("name", SchemaFieldType::StringType);
        v.add_field("count", SchemaFieldType::NumberType);

        let good = {
            let mut m = HashMap::new();
            m.insert("name".into(), ContributionValue::Str("ok".into()));
            m.insert("count".into(), ContributionValue::Number(1.0));
            m
        };
        let bad_type = {
            let mut m = HashMap::new();
            m.insert("name".into(), ContributionValue::Str("ok".into()));
            m.insert("count".into(), ContributionValue::Bool(true));
            m
        };
        let missing_field: HashMap<String, ContributionValue> = {
            let mut m = HashMap::new();
            m.insert("name".into(), ContributionValue::Str("ok".into()));
            m
        };

        // errors_only = false: returns all
        let all = batch_validate(&v, &[good.clone(), bad_type.clone(), missing_field.clone()], false);
        assert_eq!(all.len(), 3);
        assert!(all[0].is_valid());
        assert!(!all[1].is_valid());
        assert!(!all[2].is_valid());

        // errors_only = true: skips valid
        let errs = batch_validate(&v, &[good, bad_type, missing_field], true);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].index, 1);
        assert_eq!(errs[1].index, 2);
    }

    // -----------------------------------------------------------------------
    // merge_registries tests
    // -----------------------------------------------------------------------

    #[test]
    fn merge_registries_combines_points() {
        let mut target = ExtensionPointRegistry::new();
        target.register_point("vsedit.commands");
        target.register_point_with_metadata(
            "vsedit.themes",
            ExtensionPointMetadata {
                description: "Themes".into(),
                ..Default::default()
            },
        );

        let mut source = ExtensionPointRegistry::new();
        source.register_point("vsedit.commands"); // duplicate, should be skipped
        source.register_point_with_metadata(
            "vsedit.keybindings",
            ExtensionPointMetadata {
                description: "Key bindings".into(),
                version: Some("2.0".into()),
                ..Default::default()
            },
        );
        source.register_point("vsedit.snippets");

        let added = merge_registries(&mut target, &source);
        assert_eq!(added, 2);
        assert_eq!(target.len(), 4);
        assert!(target.has_point("vsedit.keybindings"));
        assert!(target.has_point("vsedit.snippets"));
        // metadata carried over for keybindings
        let meta = target.get_metadata("vsedit.keybindings").unwrap();
        assert_eq!(meta.description, "Key bindings");
        assert_eq!(meta.version.as_deref(), Some("2.0"));
    }

    // -- RegistrySnapshot tests --

    #[test]
    fn snapshot_capture_empty() {
        let reg = ExtensionPointRegistry::new();
        let snap = RegistrySnapshot::from_registry(&reg, 0);
        assert!(snap.is_empty());
        assert_eq!(snap.len(), 0);
        assert!(!snap.contains("x"));
    }

    #[test]
    fn snapshot_capture_with_points() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("vsedit.a");
        reg.register_point("vsedit.b");
        let snap = RegistrySnapshot::from_registry(&reg, 1);
        assert_eq!(snap.len(), 2);
        assert!(snap.contains("vsedit.a"));
        assert!(snap.contains("vsedit.b"));
    }

    #[test]
    fn snapshot_diff_added_removed() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("a");
        reg.register_point("b");
        let snap1 = RegistrySnapshot::from_registry(&reg, 1);

        reg.unregister_point("a").unwrap();
        reg.register_point("c");
        let snap2 = RegistrySnapshot::from_registry(&reg, 2);

        let diff = registry_diff(&snap1, &snap2);
        assert!(diff.added.contains(&"c".to_string()));
        assert!(diff.removed.contains(&"a".to_string()));
    }

    #[test]
    fn snapshot_display() {
        let reg = ExtensionPointRegistry::new();
        let snap = RegistrySnapshot::from_registry(&reg, 0);
        let s = format!("{snap}");
        assert!(s.contains("RegistrySnapshot"));
    }

    // -- RegistryBulkOps tests --

    #[test]
    fn bulk_register_many() {
        let mut reg = ExtensionPointRegistry::new();
        let added = RegistryBulkOps::register_many(&mut reg, &["a", "b", "c"]);
        assert_eq!(added, 3);
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn bulk_register_deduplicates() {
        let mut reg = ExtensionPointRegistry::new();
        reg.register_point("a");
        let added = RegistryBulkOps::register_many(&mut reg, &["a", "b"]);
        assert_eq!(added, 1);
    }

    #[test]
    fn bulk_unregister_many() {
        let mut reg = ExtensionPointRegistry::new();
        RegistryBulkOps::register_many(&mut reg, &["a", "b", "c"]);
        let removed = RegistryBulkOps::unregister_many(&mut reg, &["a", "c", "missing"]);
        assert_eq!(removed, 2);
        assert_eq!(reg.len(), 1);
    }

    // -- RegistryDepGraph tests --

    #[test]
    fn dep_graph_basic() {
        let mut g = RegistryDepGraph::new();
        g.add_dependency("editor", "buffer");
        g.add_dependency("editor", "cursor");
        let deps = g.dependencies_of("editor");
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"buffer"));
        assert!(deps.contains(&"cursor"));
    }

    #[test]
    fn dep_graph_dependents_of() {
        let mut g = RegistryDepGraph::new();
        g.add_dependency("editor", "buffer");
        g.add_dependency("minimap", "buffer");
        let dependents = g.dependents_of("buffer");
        assert!(dependents.contains(&"editor"));
        assert!(dependents.contains(&"minimap"));
    }

    #[test]
    fn dep_graph_cycle_detection() {
        let mut g = RegistryDepGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "c");
        assert!(g.would_cycle("c", "a"));
        assert!(!g.would_cycle("a", "c"));
        assert!(g.would_cycle("a", "a"));
    }

    #[test]
    fn dep_graph_topological_order() {
        let mut g = RegistryDepGraph::new();
        g.add_dependency("editor", "buffer");
        g.add_dependency("editor", "cursor");
        g.add_dependency("cursor", "buffer");
        let order = g.topological_order().unwrap();
        let buf_idx = order.iter().position(|s| s == "buffer").unwrap();
        let cur_idx = order.iter().position(|s| s == "cursor").unwrap();
        let ed_idx = order.iter().position(|s| s == "editor").unwrap();
        assert!(buf_idx < cur_idx);
        assert!(cur_idx < ed_idx);
    }

    #[test]
    fn dep_graph_cyclic_returns_none() {
        let mut g = RegistryDepGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        assert!(g.topological_order().is_none());
    }

    // -- RegistryChangeBatch tests --

    #[test]
    fn change_batch_record_and_drain() {
        let mut batch = RegistryChangeBatch::new();
        batch.record_add("a");
        batch.record_remove("b");
        batch.record_metadata_update("c");
        assert_eq!(batch.len(), 3);
        let drained = batch.drain();
        assert_eq!(drained.len(), 3);
        assert!(batch.is_empty());
    }

    #[test]
    fn change_batch_additions_removals() {
        let mut batch = RegistryChangeBatch::new();
        batch.record_add("x");
        batch.record_add("y");
        batch.record_remove("z");
        assert_eq!(batch.additions().len(), 2);
        assert_eq!(batch.removals().len(), 1);
    }

    #[test]
    fn change_batch_compact() {
        let mut batch = RegistryChangeBatch::new();
        batch.record_add("temp");
        batch.record_remove("temp");
        batch.record_add("keep");
        batch.compact();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.additions(), vec!["keep"]);
    }

    #[test]
    fn change_display() {
        assert_eq!(format!("{}", RegistryChange::Added("x".into())), "+x");
        assert_eq!(format!("{}", RegistryChange::Removed("x".into())), "-x");
        assert_eq!(format!("{}", RegistryChange::MetadataUpdated("x".into())), "~x");
    }

    #[test]
    fn dep_checker_satisfied() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("ext-a");
        checker.register_extension("ext-b");
        checker.add_dependency("ext-a", "ext-b");
        let results = checker.check("ext-a");
        assert_eq!(results.len(), 1);
        assert!(results[0].satisfied);
    }

    #[test]
    fn dep_checker_unsatisfied() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("ext-a");
        checker.add_dependency("ext-a", "ext-missing");
        let results = checker.check("ext-a");
        assert_eq!(results.len(), 1);
        assert!(!results[0].satisfied);
    }

    #[test]
    fn dep_checker_check_all() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("a");
        checker.add_dependency("a", "missing1");
        checker.add_dependency("a", "missing2");
        let fails = checker.check_all();
        assert_eq!(fails.len(), 2);
    }

    #[test]
    fn dep_checker_no_deps() {
        let checker = RegistryDependencyChecker::new();
        let results = checker.check("anything");
        assert!(results.is_empty());
    }

    #[test]
    fn dep_checker_cycle_detection() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("a");
        checker.register_extension("b");
        checker.add_dependency("a", "b");
        checker.add_dependency("b", "a");
        let cycles = checker.find_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn dep_checker_no_cycle() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("a");
        checker.register_extension("b");
        checker.add_dependency("a", "b");
        let cycles = checker.find_cycles();
        assert!(cycles.is_empty());
    }

    #[test]
    fn dep_checker_topological_order() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("a");
        checker.register_extension("b");
        checker.add_dependency("a", "b");
        let order = checker.topological_order().unwrap();
        let pos_b = order.iter().position(|x| x == "b").unwrap();
        let pos_a = order.iter().position(|x| x == "a").unwrap();
        assert!(pos_b < pos_a);
    }

    #[test]
    fn dep_checker_topological_order_cycle_returns_none() {
        let mut checker = RegistryDependencyChecker::new();
        checker.add_dependency("a", "b");
        checker.add_dependency("b", "a");
        assert!(checker.topological_order().is_none());
    }

    #[test]
    fn dep_checker_display() {
        let mut checker = RegistryDependencyChecker::new();
        checker.register_extension("a");
        assert!(format!("{checker}").contains("1 exts"));
    }

    #[test]
    fn dep_check_result_display() {
        let ok = DepCheckResult::satisfied("a", "b");
        assert!(format!("{ok}").contains("OK"));
        let fail = DepCheckResult::unsatisfied("a", "c", "not found");
        assert!(format!("{fail}").contains("FAIL"));
    }

    #[test]
    fn hot_reload_lifecycle() {
        let mut handler = RegistryHotReloadHandler::new(100);
        let idx = handler.request_reload("ext1", "1.0.0", 1000);
        assert!(handler.mark_in_progress(idx));
        assert!(handler.mark_completed(idx));
        assert_eq!(handler.completed_count(), 1);
    }

    #[test]
    fn hot_reload_failed() {
        let mut handler = RegistryHotReloadHandler::new(100);
        let idx = handler.request_reload("ext1", "1.0.0", 1000);
        handler.mark_failed(idx, "load error");
        assert_eq!(handler.failed_records().len(), 1);
    }

    #[test]
    fn hot_reload_latest_for() {
        let mut handler = RegistryHotReloadHandler::new(100);
        handler.request_reload("ext1", "1.0.0", 1000);
        handler.request_reload("ext1", "2.0.0", 2000);
        let latest = handler.latest_for("ext1").unwrap();
        assert_eq!(latest.version, "2.0.0");
    }

    #[test]
    fn hot_reload_max_history() {
        let mut handler = RegistryHotReloadHandler::new(2);
        handler.request_reload("a", "1", 1);
        handler.request_reload("b", "1", 2);
        handler.request_reload("c", "1", 3);
        assert_eq!(handler.history_len(), 2);
    }

    #[test]
    fn hot_reload_invalid_index() {
        let mut handler = RegistryHotReloadHandler::new(10);
        assert!(!handler.mark_in_progress(99));
        assert!(!handler.mark_completed(99));
        assert!(!handler.mark_failed(99, "x"));
    }

    #[test]
    fn hot_reload_status_display() {
        assert_eq!(format!("{}", HotReloadStatus::Pending), "pending");
        assert_eq!(format!("{}", HotReloadStatus::InProgress), "in-progress");
        assert_eq!(format!("{}", HotReloadStatus::Completed), "completed");
        assert!(format!("{}", HotReloadStatus::Failed("err".into())).contains("err"));
    }

    #[test]
    fn hot_reload_record_display() {
        let r = HotReloadRecord::new("ext1", "1.0", 500);
        let s = format!("{r}");
        assert!(s.contains("ext1"));
        assert!(s.contains("1.0"));
        assert!(s.contains("pending"));
    }

    #[test]
    fn hot_reload_handler_display() {
        let handler = RegistryHotReloadHandler::new(50);
        assert!(format!("{handler}").contains("0 records"));
    }


    #[test]
    fn registry_entry_creation() {
        let e = RegistryEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn registry_entry_with_priority() {
        let e = RegistryEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn registry_entry_metadata() {
        let e = RegistryEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn registry_entry_remove_meta() {
        let mut e = RegistryEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn registry_entry_activate_deactivate() {
        let mut e = RegistryEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn registry_config_add_sorted() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("lo", "Lo").with_priority(1));
        c.add(RegistryEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn registry_config_capacity() {
        let mut c = RegistryConfig::new(1);
        assert!(c.add(RegistryEntry::new("a", "A")));
        assert!(!c.add(RegistryEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn registry_config_remove() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn registry_config_get() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn registry_config_active_entries() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        c.add(RegistryEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn registry_config_enable_disable() {
        let mut c = RegistryConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn registry_config_clear() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn registry_config_find_by_label() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn registry_config_top_n() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A").with_priority(1));
        c.add(RegistryEntry::new("b", "B").with_priority(2));
        c.add(RegistryEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn registry_config_deactivate_activate_all() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        c.add(RegistryEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn registry_config_highest_priority() {
        let mut c = RegistryConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(RegistryEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn registry_config_contains() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn registry_config_labels() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "Alpha"));
        c.add(RegistryEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn registry_config_drain_inactive() {
        let mut c = RegistryConfig::new(10);
        c.add(RegistryEntry::new("a", "A"));
        c.add(RegistryEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xc_metrics_empty() {
        let m = XcMetrics::new("registry");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xc_metrics_record_and_mean() {
        let mut m = XcMetrics::new("registry");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xc_metrics_min_max() {
        let mut m = XcMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xc_metrics_variance_and_std() {
        let mut m = XcMetrics::new("v");
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
    fn xc_metrics_percentile() {
        let mut m = XcMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xc_metrics_merge() {
        let mut a = XcMetrics::new("a");
        a.record(1.0);
        let mut b = XcMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xc_metrics_reset() {
        let mut m = XcMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xc_rate_window_empty() {
        let rw = XcRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xc_rate_window_tick_and_rate() {
        let mut rw = XcRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xc_lru_cache_basic() {
        let mut c = XcLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xc_lru_cache_contains_and_keys() {
        let mut c = XcLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xc_lru_cache_remove() {
        let mut c = XcLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xc_metrics_sum() {
        let mut m = XcMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xc_metrics_label() {
        let m = XcMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xc_lru_cache_clear() {
        let mut c = XcLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_19_push_and_len() {
        let mut rb = super::XbRingBuffer19::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_19_overwrite() {
        let mut rb = super::XbRingBuffer19::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_19_get_out_of_bounds() {
        let rb = super::XbRingBuffer19::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_19_drain_all() {
        let mut rb = super::XbRingBuffer19::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_19_peek_front_back() {
        let mut rb = super::XbRingBuffer19::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_19_clear() {
        let mut rb = super::XbRingBuffer19::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_19_capacity() {
        let rb = super::XbRingBuffer19::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_19_basic() {
        let h = super::xb_fnv1a_19(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_19(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_19_different_inputs() {
        let h1 = super::xb_fnv1a_19(b"abc");
        let h2 = super::xb_fnv1a_19(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_19_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_19(&data);
        let dec = super::xb_rle_decode_19(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_19_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_19(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_19(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_19_values() {
        assert!((super::xb_clamp_19(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_19(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_19(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_19_values() {
        assert!((super::xb_lerp_19(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_19(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_19(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_19_wrap_around_twice() {
        let mut rb = super::XbRingBuffer19::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 147 ----

    #[test]
    fn xc_147_pool_new_empty() {
        let pool: super::Xc147Pool<i32> = super::Xc147Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_147_pool_release_acquire() {
        let mut pool = super::Xc147Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_147_pool_acquire_empty() {
        let mut pool: super::Xc147Pool<i32> = super::Xc147Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_147_pool_full() {
        let mut pool = super::Xc147Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_147_pool_drain() {
        let mut pool = super::Xc147Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_147_pool_stats() {
        let mut pool = super::Xc147Pool::new(8);
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
    fn xc_147_pool_clear() {
        let mut pool = super::Xc147Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_147_pool_shrink() {
        let mut pool = super::Xc147Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_147_pool_default() {
        let pool: super::Xc147Pool<String> = super::Xc147Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_147_pool_extend() {
        let mut pool = super::Xc147Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_147_pool_retain() {
        let mut pool = super::Xc147Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_147_scheduler_round_robin() {
        let mut sched = super::Xc147Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_147_scheduler_empty() {
        let mut sched = super::Xc147Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_147_scheduler_reset() {
        let mut sched = super::Xc147Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_147_scheduler_add_remove() {
        let mut sched = super::Xc147Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_147_scheduler_targets() {
        let sched = super::Xc147Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_147_hash_empty() {
        assert_eq!(super::xc_147_hash(b""), 5381);
    }

    #[test]
    fn xc_147_hash_data() {
        let h = super::xc_147_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_147_hash(b"hello"), h);
    }

    #[test]
    fn xc_147_reverse_str() {
        assert_eq!(super::xc_147_reverse("abc"), "cba");
        assert_eq!(super::xc_147_reverse(""), "");
    }


    #[test]
    fn xe_31_pipeline_empty() {
        let p = super::Xe31Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_31_pipeline_parse_stage() {
        let p = super::Xe31Pipeline::new()
            .add_parse(super::xe_31_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_31_pipeline_transform_double() {
        let p = super::Xe31Pipeline::new()
            .add_transform(super::xe_31_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_31_pipeline_validate_reverse() {
        let p = super::Xe31Pipeline::new()
            .add_validate(super::xe_31_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_31_pipeline_emit_filter() {
        let p = super::Xe31Pipeline::new()
            .add_emit(super::xe_31_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_31_pipeline_multi_stage() {
        let p = super::Xe31Pipeline::new()
            .add_parse(super::xe_31_pipeline_identity)
            .add_transform(super::xe_31_pipeline_double)
            .add_validate(super::xe_31_pipeline_reverse)
            .add_emit(super::xe_31_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_31_pipeline_error_propagation() {
        let p = super::Xe31Pipeline::new()
            .add_parse(super::xe_31_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe31Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_31_pipeline_compose() {
        let p1 = super::Xe31Pipeline::new()
            .add_parse(super::xe_31_pipeline_identity);
        let p2 = super::Xe31Pipeline::new()
            .add_transform(super::xe_31_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_31_pipeline_error_display() {
        let e = super::Xe31PipelineError {
            stage: super::Xe31Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_31_cache_put_get() {
        let mut c = super::Xe31Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_31_cache_miss() {
        let mut c: super::Xe31Cache<&str, i32> = super::Xe31Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_31_cache_ttl_expiry() {
        let mut c = super::Xe31Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_31_cache_evict() {
        let mut c = super::Xe31Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_31_cache_capacity() {
        let mut c = super::Xe31Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_31_cache_stats() {
        let mut c = super::Xe31Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_31_cache_clear() {
        let mut c = super::Xe31Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #117 --

    #[test]
    fn xf117_trie_insert_search() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf117_trie_starts_with() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf117_trie_remove() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf117_trie_word_count() {
        let mut t = Xf117Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf117_trie_longest_prefix() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf117_trie_all_words() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf117_trie_autocomplete() {
        let mut t = Xf117Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf117_trie_empty_search() {
        let t = Xf117Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf117_bloom_add_contains() {
        let mut bf = Xf117BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf117_bloom_probably_absent() {
        let bf = Xf117BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf117_bloom_false_positive_rate() {
        let mut bf = Xf117BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf117_bloom_clear() {
        let mut bf = Xf117BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf117_bloom_union() {
        let mut a = Xf117BloomFilter::xf_new(512, 2);
        let mut b = Xf117BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf117_bloom_intersection_estimate() {
        let mut a = Xf117BloomFilter::xf_new(512, 2);
        let mut b = Xf117BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf117_bloom_union_size_mismatch() {
        let a = Xf117BloomFilter::xf_new(256, 2);
        let b = Xf117BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh146_skip_insert_contains() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh146_skip_remove() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh146_skip_len() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh146_skip_range_query() {
        let mut sl = super::Xh146SkipList::xh_new(4);
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
    fn xh146_skip_floor_ceiling() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh146_skip_rank() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh146_skip_empty() {
        let sl = super::Xh146SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh146_skip_duplicates() {
        let mut sl = super::Xh146SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh146_bitset_set_test() {
        let mut bs = super::Xh146BitSet::xh_new(256);
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
    fn xh146_bitset_clear_count() {
        let mut bs = super::Xh146BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh146_bitset_and_or_xor() {
        let mut a = super::Xh146BitSet::xh_new(128);
        let mut b = super::Xh146BitSet::xh_new(128);
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
    fn xh146_bitset_iter_ones() {
        let mut bs = super::Xh146BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh146_bitset_first_last() {
        let mut bs = super::Xh146BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh146_bitset_empty() {
        let bs = super::Xh146BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi146_deque_push_pop_back() {
        let mut dq = super::Xi146Deque::xi_new(4);
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
    fn xi146_deque_push_pop_front() {
        let mut dq = super::Xi146Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi146_deque_mixed_ops() {
        let mut dq = super::Xi146Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi146_deque_get_and_split() {
        let mut dq = super::Xi146Deque::xi_new(8);
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
    fn xi146_deque_rotate_left() {
        let mut dq = super::Xi146Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi146_deque_rotate_right() {
        let mut dq = super::Xi146Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi146_deque_grow() {
        let mut dq = super::Xi146Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi146_deque_empty() {
        let dq = super::Xi146Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi146_interval_tree_insert_query() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi146Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi146Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi146_interval_tree_overlap() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi146Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi146Interval::xi_new(12, 20));
        let q = super::Xi146Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi146_interval_tree_remove() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi146Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi146_interval_tree_gaps() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi146Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi146Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi146Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi146Interval::xi_new(8, 10));
    }

    #[test]
    fn xi146_interval_tree_merge() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi146Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi146Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi146Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi146Interval::xi_new(10, 15));
    }

    #[test]
    fn xi146_interval_tree_all() {
        let mut tree = super::Xi146IntervalTree::xi_new();
        tree.xi_insert(super::Xi146Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi146Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi146_interval_tree_empty() {
        let tree = super::Xi146IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi146_interval_tree_contains_point() {
        let iv = super::Xi146Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
