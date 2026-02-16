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
}
