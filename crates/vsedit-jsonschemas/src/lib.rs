//! JSON schema validation support – types, properties, and a schema registry.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
}

impl fmt::Display for SchemaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SchemaType::String => "string",
            SchemaType::Number => "number",
            SchemaType::Integer => "integer",
            SchemaType::Boolean => "boolean",
            SchemaType::Array => "array",
            SchemaType::Object => "object",
            SchemaType::Null => "null",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone)]
pub struct SchemaProperty {
    pub name: String,
    pub schema_type: SchemaType,
    pub description: Option<String>,
    pub required: bool,
    pub default_value: Option<String>,
}

impl SchemaProperty {
    pub fn is_required(&self) -> bool {
        self.required
    }

    pub fn has_default(&self) -> bool {
        self.default_value.is_some()
    }
}

impl fmt::Display for SchemaProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let req = if self.required { " (required)" } else { "" };
        write!(f, "{}: {}{}", self.name, self.schema_type, req)
    }
}

#[derive(Debug, Clone)]
pub struct JsonSchema {
    pub id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub schema_type: SchemaType,
    pub properties: Vec<SchemaProperty>,
    pub file_match: Vec<String>,
}

impl JsonSchema {
    pub fn get_required_properties(&self) -> Vec<&SchemaProperty> {
        self.properties.iter().filter(|p| p.required).collect()
    }

    pub fn get_property(&self, name: &str) -> Option<&SchemaProperty> {
        self.properties.iter().find(|p| p.name == name)
    }

    pub fn property_count(&self) -> usize {
        self.properties.len()
    }
}

pub struct SchemaRegistry {
    schemas: Vec<JsonSchema>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
        }
    }

    pub fn register(&mut self, schema: JsonSchema) {
        self.schemas.push(schema);
    }

    /// Find a schema whose `file_match` patterns match the given filename.
    pub fn find_for_file(&self, filename: &str) -> Option<&JsonSchema> {
        self.schemas.iter().find(|s| {
            s.file_match.iter().any(|pattern| {
                if let Some(suffix) = pattern.strip_prefix('*') {
                    filename.ends_with(suffix)
                } else {
                    filename == pattern
                }
            })
        })
    }

    pub fn get_schema_by_id(&self, id: &str) -> Option<&JsonSchema> {
        self.schemas.iter().find(|s| s.id.as_deref() == Some(id))
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let len_before = self.schemas.len();
        self.schemas.retain(|s| s.id.as_deref() != Some(id));
        self.schemas.len() < len_before
    }

    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    pub fn get_all_file_matches(&self) -> Vec<&str> {
        self.schemas
            .iter()
            .flat_map(|s| s.file_match.iter().map(|m| m.as_str()))
            .collect()
    }

    /// Find all schemas whose `file_match` patterns match the given filename.
    pub fn find_schemas_for_file(&self, filename: &str) -> Vec<&JsonSchema> {
        self.schemas
            .iter()
            .filter(|s| {
                s.file_match.iter().any(|pattern| {
                    if let Some(suffix) = pattern.strip_prefix('*') {
                        filename.ends_with(suffix)
                    } else {
                        filename == pattern
                    }
                })
            })
            .collect()
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a `$ref` reference in a JSON schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaRef {
    /// A reference to a definition within the same schema (e.g. `#/definitions/Foo`).
    Internal(String),
    /// A reference to an external schema by URI.
    External(String),
}

impl SchemaRef {
    /// Returns `true` if this is an internal (`#/…`) reference.
    pub fn is_internal(&self) -> bool {
        matches!(self, SchemaRef::Internal(_))
    }

    /// Returns `true` if this is an external reference.
    pub fn is_external(&self) -> bool {
        matches!(self, SchemaRef::External(_))
    }

    /// Returns the raw reference string regardless of variant.
    pub fn as_str(&self) -> &str {
        match self {
            SchemaRef::Internal(s) | SchemaRef::External(s) => s.as_str(),
        }
    }

    /// Parse a raw `$ref` string into the appropriate variant.
    pub fn parse(raw: &str) -> Self {
        if raw.starts_with('#') {
            SchemaRef::Internal(raw.to_string())
        } else {
            SchemaRef::External(raw.to_string())
        }
    }
}

impl fmt::Display for SchemaRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaRef::Internal(s) => write!(f, "$ref(internal): {}", s),
            SchemaRef::External(s) => write!(f, "$ref(external): {}", s),
        }
    }
}

/// Constraints that may be applied to a schema property value.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaConstraint {
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    Minimum(f64),
    Maximum(f64),
    Enum(Vec<String>),
}

impl SchemaConstraint {
    /// Check whether `value` satisfies this constraint.
    pub fn is_satisfied_by(&self, value: &str) -> bool {
        match self {
            SchemaConstraint::MinLength(min) => value.len() >= *min,
            SchemaConstraint::MaxLength(max) => value.len() <= *max,
            SchemaConstraint::Pattern(pat) => value.contains(pat.as_str()),
            SchemaConstraint::Minimum(min) => {
                value.parse::<f64>().map_or(false, |v| v >= *min)
            }
            SchemaConstraint::Maximum(max) => {
                value.parse::<f64>().map_or(false, |v| v <= *max)
            }
            SchemaConstraint::Enum(variants) => variants.iter().any(|v| v == value),
        }
    }
}

/// The result of validating a value (or object) against a schema.
#[derive(Debug, Clone, Default)]
pub struct SchemaValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl SchemaValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }

    pub fn add_warning(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Merge another result into this one.
    pub fn merge(&mut self, other: SchemaValidationResult) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

/// Validate a single string `value` against a `SchemaProperty`.
///
/// This performs basic type checking via [`validate_type`] and returns a
/// [`SchemaValidationResult`] with any errors collected.
pub fn validate_schema_property(
    property: &SchemaProperty,
    value: Option<&str>,
) -> SchemaValidationResult {
    let mut result = SchemaValidationResult::new();
    match value {
        None => {
            if property.required {
                result.add_error(format!("Missing required property '{}'", property.name));
            }
        }
        Some(v) => {
            if !validate_type(v, property.schema_type) {
                result.add_error(format!(
                    "Property '{}' expected type '{}' but got '{}'",
                    property.name, property.schema_type, v
                ));
            }
        }
    }
    result
}

// ── Additional methods on SchemaProperty ────────────────────────────────

impl SchemaProperty {
    /// Builder helper – set the description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Builder helper – set the default value.
    pub fn with_default(mut self, val: &str) -> Self {
        self.default_value = Some(val.to_string());
        self
    }

    /// Returns the effective value: the provided `value` if `Some`, otherwise
    /// the default, otherwise `None`.
    pub fn effective_value<'a>(&'a self, value: Option<&'a str>) -> Option<&'a str> {
        value.or(self.default_value.as_deref())
    }
}

impl PartialEq for SchemaProperty {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.schema_type == other.schema_type
    }
}

// ── Additional methods on JsonSchema ────────────────────────────────────

impl JsonSchema {
    /// Return all optional (non-required) properties.
    pub fn get_optional_properties(&self) -> Vec<&SchemaProperty> {
        self.properties.iter().filter(|p| !p.required).collect()
    }

    /// Validate a set of key/value pairs against this schema's properties.
    pub fn validate_object(&self, fields: &[(&str, &str)]) -> SchemaValidationResult {
        let mut result = SchemaValidationResult::new();
        for prop in &self.properties {
            let value = fields.iter().find(|(k, _)| *k == prop.name).map(|(_, v)| *v);
            let r = validate_schema_property(prop, value);
            result.merge(r);
        }
        result
    }

    /// Merge another schema's properties into this one. Properties already
    /// present (by name) are kept unchanged.
    pub fn merge_with(&mut self, other: &JsonSchema) {
        for prop in &other.properties {
            if self.get_property(&prop.name).is_none() {
                self.properties.push(prop.clone());
            }
        }
    }
}

impl PartialEq for JsonSchema {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.schema_type == other.schema_type
    }
}

impl fmt::Display for JsonSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.title.as_deref().unwrap_or("<untitled>");
        write!(f, "Schema({}, type={})", title, self.schema_type)
    }
}

// ── Additional methods on SchemaRegistry ────────────────────────────────

impl SchemaRegistry {
    /// Find the first schema whose title matches `title`.
    pub fn find_by_title(&self, title: &str) -> Option<&JsonSchema> {
        self.schemas.iter().find(|s| s.title.as_deref() == Some(title))
    }

    /// Return all schemas whose top-level type equals `schema_type`.
    pub fn get_schemas_by_type(&self, schema_type: SchemaType) -> Vec<&JsonSchema> {
        self.schemas
            .iter()
            .filter(|s| s.schema_type == schema_type)
            .collect()
    }

    /// Merge all schemas from `other` into this registry.
    /// Schemas with duplicate ids are skipped.
    pub fn merge_registries(&mut self, other: SchemaRegistry) {
        for schema in other.schemas {
            let dominated = schema
                .id
                .as_deref()
                .map_or(false, |id| self.get_schema_by_id(id).is_some());
            if !dominated {
                self.schemas.push(schema);
            }
        }
    }
}

/// Produce a simple JSON-like string representation of a [`JsonSchema`].
pub fn format_schema_as_json(schema: &JsonSchema) -> String {
    let mut out = String::from("{\n");
    if let Some(ref id) = schema.id {
        out.push_str(&format!("  \"$id\": \"{}\",\n", id));
    }
    if let Some(ref title) = schema.title {
        out.push_str(&format!("  \"title\": \"{}\",\n", title));
    }
    out.push_str(&format!("  \"type\": \"{}\",\n", schema.schema_type));
    out.push_str("  \"properties\": {\n");
    for (i, prop) in schema.properties.iter().enumerate() {
        let comma = if i + 1 < schema.properties.len() { "," } else { "" };
        out.push_str(&format!(
            "    \"{}\": {{ \"type\": \"{}\" }}{}\n",
            prop.name, prop.schema_type, comma
        ));
    }
    out.push_str("  }\n");
    out.push('}');
    out
}

/// Basic type validation: checks whether `value` looks like the expected type.
pub fn validate_type(value: &str, expected: SchemaType) -> bool {
    match expected {
        SchemaType::String => true,
        SchemaType::Number => value.parse::<f64>().is_ok(),
        SchemaType::Integer => value.parse::<i64>().is_ok(),
        SchemaType::Boolean => matches!(value, "true" | "false"),
        SchemaType::Null => value == "null",
        SchemaType::Array => value.starts_with('[') && value.ends_with(']'),
        SchemaType::Object => value.starts_with('{') && value.ends_with('}'),
    }
}

/// A JSON-like value for schema validation.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

/// An error produced when validating a `JsonValue` against a `JsonSchema`.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
    pub expected_type: Option<SchemaType>,
}

/// Validates `JsonValue` instances against a `JsonSchema`.
pub struct SchemaValidator;

impl SchemaValidator {
    /// Validate a value against a schema, returning all errors found.
    pub fn validate(schema: &JsonSchema, value: &JsonValue) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Top-level type check.
        if !Self::validate_type(value, schema.schema_type) {
            errors.push(ValidationError {
                path: String::new(),
                message: format!(
                    "expected type {}, got {}",
                    schema.schema_type,
                    Self::type_name(value)
                ),
                expected_type: Some(schema.schema_type),
            });
            return errors;
        }

        // For objects, validate required fields and per-property types / enums.
        if let JsonValue::Object(fields) = value {
            errors.extend(Self::validate_required(schema, fields));

            for (key, val) in fields {
                if let Some(prop) = schema.get_property(key) {
                    let prop_path = key.clone();

                    if !Self::validate_type(val, prop.schema_type) {
                        errors.push(ValidationError {
                            path: prop_path.clone(),
                            message: format!(
                                "expected type {}, got {}",
                                prop.schema_type,
                                Self::type_name(val)
                            ),
                            expected_type: Some(prop.schema_type),
                        });
                    }

                    // Enum validation would be invoked here when constraints
                    // are attached to properties. Use validate_enum directly.
                    let _ = &prop_path;
                }
            }
        }

        errors
    }

    /// Returns `true` if `value` matches the expected `SchemaType`.
    pub fn validate_type(value: &JsonValue, expected: SchemaType) -> bool {
        matches!(
            (value, expected),
            (JsonValue::Str(_), SchemaType::String)
                | (JsonValue::Number(_), SchemaType::Number)
                | (JsonValue::Number(_), SchemaType::Integer)
                | (JsonValue::Bool(_), SchemaType::Boolean)
                | (JsonValue::Array(_), SchemaType::Array)
                | (JsonValue::Object(_), SchemaType::Object)
                | (JsonValue::Null, SchemaType::Null)
        )
    }

    /// Check that every required property in `schema` is present in `fields`.
    pub fn validate_required(
        schema: &JsonSchema,
        fields: &[(String, JsonValue)],
    ) -> Vec<ValidationError> {
        schema
            .get_required_properties()
            .into_iter()
            .filter(|prop| !fields.iter().any(|(k, _)| k == &prop.name))
            .map(|prop| ValidationError {
                path: prop.name.clone(),
                message: format!("missing required property \"{}\"", prop.name),
                expected_type: Some(prop.schema_type),
            })
            .collect()
    }

    /// Check whether `value` is one of the `allowed` enum variants.
    /// Returns an error if `value` is not in the list.
    pub fn validate_enum(
        path: &str,
        value: &str,
        allowed: &[String],
    ) -> Vec<ValidationError> {
        if allowed.is_empty() || allowed.iter().any(|a| a == value) {
            return Vec::new();
        }
        vec![ValidationError {
            path: path.to_string(),
            message: format!(
                "value \"{}\" is not one of the allowed values: {:?}",
                value, allowed
            ),
            expected_type: Some(SchemaType::String),
        }]
    }

    fn type_name(value: &JsonValue) -> &'static str {
        match value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(_) => "number",
            JsonValue::Str(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }
}

// ---------------------------------------------------------------------------
// SchemaType helpers
// ---------------------------------------------------------------------------

impl SchemaType {
    /// Returns all type variants.
    pub fn all() -> &'static [SchemaType] {
        &[
            SchemaType::String,
            SchemaType::Number,
            SchemaType::Integer,
            SchemaType::Boolean,
            SchemaType::Array,
            SchemaType::Object,
            SchemaType::Null,
        ]
    }

    /// Returns true if this is a numeric type.
    pub fn is_numeric(&self) -> bool {
        matches!(self, SchemaType::Number | SchemaType::Integer)
    }

    /// Returns true if this is a primitive (non-container) type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            SchemaType::String | SchemaType::Number | SchemaType::Integer
                | SchemaType::Boolean | SchemaType::Null
        )
    }

    /// Parse from a JSON schema type string.
    pub fn from_json_name(name: &str) -> Option<Self> {
        match name {
            "string" => Some(Self::String),
            "number" => Some(Self::Number),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "array" => Some(Self::Array),
            "object" => Some(Self::Object),
            "null" => Some(Self::Null),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// SchemaProperty builder
// ---------------------------------------------------------------------------

impl SchemaProperty {
    /// Create a new required property.
    pub fn required_prop(name: &str, schema_type: SchemaType, description: &str) -> Self {
        Self {
            name: name.to_string(),
            schema_type,
            description: Some(description.to_string()),
            required: true,
            default_value: None,
        }
    }

    /// Create a new optional property with a default.
    pub fn optional_prop(name: &str, schema_type: SchemaType, default: &str) -> Self {
        Self {
            name: name.to_string(),
            schema_type,
            description: None,
            required: false,
            default_value: Some(default.to_string()),
        }
    }

    /// Returns the effective value (current or default).
    pub fn effective_default(&self) -> Option<&str> {
        self.default_value.as_deref()
    }
}

// ---------------------------------------------------------------------------
// JsonSchema helpers
// ---------------------------------------------------------------------------

impl JsonSchema {
    /// Returns the number of required properties.
    pub fn required_count(&self) -> usize {
        self.properties.iter().filter(|p| p.required).count()
    }

    /// Returns the number of optional properties.
    pub fn optional_count(&self) -> usize {
        self.properties.iter().filter(|p| !p.required).count()
    }

    /// Find a property by name.
    pub fn find_property(&self, name: &str) -> Option<&SchemaProperty> {
        self.properties.iter().find(|p| p.name == name)
    }

    /// Returns property names as a sorted vec.
    pub fn property_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.properties.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        names
    }
}

// ---------------------------------------------------------------------------
// Schema comparison
// ---------------------------------------------------------------------------

/// Describes a difference between two schemas.
#[derive(Debug, Clone)]
pub enum SchemaDiff {
    PropertyAdded(String),
    PropertyRemoved(String),
    TypeChanged { property: String, old: SchemaType, new: SchemaType },
}

impl fmt::Display for SchemaDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaDiff::PropertyAdded(name) => write!(f, "+ {name}"),
            SchemaDiff::PropertyRemoved(name) => write!(f, "- {name}"),
            SchemaDiff::TypeChanged { property, old, new } => {
                write!(f, "~ {property}: {old} -> {new}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Schema composition (allOf / anyOf / oneOf)
// ---------------------------------------------------------------------------

/// Describes how multiple sub-schemas should be composed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionKind {
    /// All sub-schemas must be satisfied (intersection of properties).
    AllOf,
    /// At least one sub-schema must be satisfied.
    AnyOf,
    /// Exactly one sub-schema must be satisfied.
    OneOf,
}

impl fmt::Display for CompositionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositionKind::AllOf => write!(f, "allOf"),
            CompositionKind::AnyOf => write!(f, "anyOf"),
            CompositionKind::OneOf => write!(f, "oneOf"),
        }
    }
}

/// A composite schema that combines several sub-schemas.
#[derive(Debug, Clone)]
pub struct CompositeSchema {
    pub kind: CompositionKind,
    pub schemas: Vec<JsonSchema>,
}

impl CompositeSchema {
    pub fn new(kind: CompositionKind) -> Self {
        Self {
            kind,
            schemas: Vec::new(),
        }
    }

    pub fn add(&mut self, schema: JsonSchema) {
        self.schemas.push(schema);
    }

    /// Merge all sub-schemas into a single `JsonSchema` using allOf semantics:
    /// properties from all sub-schemas are collected; if a property appears more
    /// than once the first occurrence wins (same behaviour as `merge_with`).
    /// The resulting schema is always of type `Object`.
    pub fn merge_all(&self) -> JsonSchema {
        let mut merged = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: Vec::new(),
            file_match: Vec::new(),
        };
        for sub in &self.schemas {
            merged.merge_with(sub);
            // Also merge file_match patterns.
            for pat in &sub.file_match {
                if !merged.file_match.contains(pat) {
                    merged.file_match.push(pat.clone());
                }
            }
        }
        merged
    }

    /// Validate a `JsonValue` according to the composition kind.
    ///
    /// * `AllOf` – the value must pass validation against **every** sub-schema.
    /// * `AnyOf` – the value must pass validation against **at least one**.
    /// * `OneOf` – the value must pass validation against **exactly one**.
    pub fn validate(&self, value: &JsonValue) -> Vec<ValidationError> {
        let results: Vec<Vec<ValidationError>> = self
            .schemas
            .iter()
            .map(|s| SchemaValidator::validate(s, value))
            .collect();

        match self.kind {
            CompositionKind::AllOf => results.into_iter().flatten().collect(),
            CompositionKind::AnyOf => {
                if results.iter().any(|r| r.is_empty()) {
                    Vec::new()
                } else {
                    vec![ValidationError {
                        path: String::new(),
                        message: "value does not satisfy any of the anyOf sub-schemas".into(),
                        expected_type: None,
                    }]
                }
            }
            CompositionKind::OneOf => {
                let passing = results.iter().filter(|r| r.is_empty()).count();
                if passing == 1 {
                    Vec::new()
                } else {
                    vec![ValidationError {
                        path: String::new(),
                        message: format!(
                            "value must satisfy exactly one oneOf sub-schema, but {} matched",
                            passing
                        ),
                        expected_type: None,
                    }]
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// $ref resolution helpers
// ---------------------------------------------------------------------------

/// A simple definition store: maps definition names to schemas so that
/// internal `$ref` pointers (e.g. `#/definitions/Address`) can be resolved.
#[derive(Debug, Clone, Default)]
pub struct SchemaDefinitions {
    defs: Vec<(String, JsonSchema)>,
}

impl SchemaDefinitions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, schema: JsonSchema) {
        self.defs.push((name.into(), schema));
    }

    pub fn get(&self, name: &str) -> Option<&JsonSchema> {
        self.defs.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Resolve a `SchemaRef` against this definition store.
    /// Internal refs of the form `#/definitions/<name>` are looked up; all
    /// other refs return `None`.
    pub fn resolve(&self, reference: &SchemaRef) -> Option<&JsonSchema> {
        match reference {
            SchemaRef::Internal(path) => {
                let name = path
                    .strip_prefix("#/definitions/")
                    .or_else(|| path.strip_prefix("#/$defs/"))?;
                self.get(name)
            }
            SchemaRef::External(_) => None,
        }
    }

    /// Return all definition names.
    pub fn names(&self) -> Vec<&str> {
        self.defs.iter().map(|(n, _)| n.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// Default value extraction
// ---------------------------------------------------------------------------

/// Extract all default values from a schema, returning `(property_name, default)` pairs.
pub fn extract_defaults(schema: &JsonSchema) -> Vec<(&str, &str)> {
    schema
        .properties
        .iter()
        .filter_map(|p| p.default_value.as_deref().map(|d| (p.name.as_str(), d)))
        .collect()
}

/// Build a skeleton object (as `JsonValue`) by filling every property that has
/// a default value. Properties without defaults are omitted.
pub fn build_default_object(schema: &JsonSchema) -> JsonValue {
    let fields: Vec<(String, JsonValue)> = schema
        .properties
        .iter()
        .filter_map(|p| {
            p.default_value
                .as_deref()
                .map(|d| (p.name.clone(), default_str_to_value(d, p.schema_type)))
        })
        .collect();
    JsonValue::Object(fields)
}

/// Best-effort conversion of a default-value string into a `JsonValue`.
fn default_str_to_value(s: &str, ty: SchemaType) -> JsonValue {
    match ty {
        SchemaType::Boolean => match s {
            "true" => JsonValue::Bool(true),
            "false" => JsonValue::Bool(false),
            _ => JsonValue::Str(s.to_string()),
        },
        SchemaType::Number | SchemaType::Integer => s
            .parse::<f64>()
            .map(JsonValue::Number)
            .unwrap_or_else(|_| JsonValue::Str(s.to_string())),
        SchemaType::Null => JsonValue::Null,
        _ => JsonValue::Str(s.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Code-completion hint generation
// ---------------------------------------------------------------------------

/// A hint for a code editor's auto-completion list.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionHint {
    /// The text to insert.
    pub label: String,
    /// A short description shown alongside the label.
    pub detail: Option<String>,
    /// The expected JSON type of the value.
    pub value_type: SchemaType,
    /// Whether this property is required.
    pub required: bool,
    /// An optional snippet with a placeholder, e.g. `"key": "$1"`.
    pub insert_text: Option<String>,
}

/// Generate completion hints for the top-level properties of a schema.
/// Hints that correspond to keys already present in `existing_keys` are excluded.
pub fn completion_hints(schema: &JsonSchema, existing_keys: &[&str]) -> Vec<CompletionHint> {
    schema
        .properties
        .iter()
        .filter(|p| !existing_keys.contains(&p.name.as_str()))
        .map(|p| {
            let placeholder = p
                .default_value
                .as_deref()
                .unwrap_or_else(|| type_placeholder(p.schema_type));
            CompletionHint {
                label: p.name.clone(),
                detail: p.description.clone(),
                value_type: p.schema_type,
                required: p.required,
                insert_text: Some(format!("\"{}\": {}", p.name, placeholder)),
            }
        })
        .collect()
}

/// A reasonable placeholder value for each schema type (used in snippets).
fn type_placeholder(ty: SchemaType) -> &'static str {
    match ty {
        SchemaType::String => "\"\"",
        SchemaType::Number | SchemaType::Integer => "0",
        SchemaType::Boolean => "false",
        SchemaType::Array => "[]",
        SchemaType::Object => "{}",
        SchemaType::Null => "null",
    }
}

/// Compare two schemas and return differences.
pub fn compare_schemas(old: &JsonSchema, new: &JsonSchema) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();
    let old_names: std::collections::HashSet<&str> = old.properties.iter().map(|p| p.name.as_str()).collect();
    let new_names: std::collections::HashSet<&str> = new.properties.iter().map(|p| p.name.as_str()).collect();

    for name in &new_names {
        if !old_names.contains(name) {
            diffs.push(SchemaDiff::PropertyAdded(name.to_string()));
        }
    }
    for name in &old_names {
        if !new_names.contains(name) {
            diffs.push(SchemaDiff::PropertyRemoved(name.to_string()));
        }
    }
    for old_prop in &old.properties {
        if let Some(new_prop) = new.properties.iter().find(|p| p.name == old_prop.name) {
            if old_prop.schema_type != new_prop.schema_type {
                diffs.push(SchemaDiff::TypeChanged {
                    property: old_prop.name.clone(),
                    old: old_prop.schema_type,
                    new: new_prop.schema_type,
                });
            }
        }
    }
    diffs
}

// ---------------------------------------------------------------------------
// Recursive JSON schema validation
// ---------------------------------------------------------------------------

/// Validates `JsonValue` instances against a `JsonSchema` recursively,
/// descending into nested objects and checking required fields and types.
/// Unlike `SchemaValidator` (a unit struct for flat validation), this struct
/// carries a reference to `SchemaDefinitions` so that `$ref` pointers inside
/// nested properties can be resolved during the walk.
pub struct JsonSchemaValidator<'a> {
    definitions: Option<&'a SchemaDefinitions>,
}

impl<'a> JsonSchemaValidator<'a> {
    /// Create a validator without definition resolution.
    pub fn new() -> Self {
        Self { definitions: None }
    }

    /// Create a validator that can resolve `$ref` pointers.
    pub fn with_definitions(defs: &'a SchemaDefinitions) -> Self {
        Self { definitions: Some(defs) }
    }

    /// Validate `value` against `schema`, collecting all errors with JSON-pointer paths.
    pub fn validate(&self, schema: &JsonSchema, value: &JsonValue) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        self.validate_inner(schema, value, String::new(), &mut errors);
        errors
    }

    fn validate_inner(
        &self,
        schema: &JsonSchema,
        value: &JsonValue,
        path: String,
        errors: &mut Vec<ValidationError>,
    ) {
        if !SchemaValidator::validate_type(value, schema.schema_type) {
            errors.push(ValidationError {
                path: path.clone(),
                message: format!(
                    "expected type {}, got {}",
                    schema.schema_type,
                    Self::type_name(value),
                ),
                expected_type: Some(schema.schema_type),
            });
            return;
        }

        if let JsonValue::Object(fields) = value {
            // Check required properties are present.
            for prop in schema.get_required_properties() {
                if !fields.iter().any(|(k, _)| k == &prop.name) {
                    let field_path = join_path(&path, &prop.name);
                    errors.push(ValidationError {
                        path: field_path,
                        message: format!("missing required property \"{}\"", prop.name),
                        expected_type: Some(prop.schema_type),
                    });
                }
            }

            // Validate each present field.
            for (key, val) in fields {
                let field_path = join_path(&path, key);
                if let Some(prop) = schema.get_property(key) {
                    if !SchemaValidator::validate_type(val, prop.schema_type) {
                        errors.push(ValidationError {
                            path: field_path.clone(),
                            message: format!(
                                "expected type {}, got {}",
                                prop.schema_type,
                                Self::type_name(val),
                            ),
                            expected_type: Some(prop.schema_type),
                        });
                    }
                    // Recurse into nested objects when a sub-schema is
                    // available via definitions.
                    if prop.schema_type == SchemaType::Object {
                        if let Some(defs) = self.definitions {
                            let ref_str = format!("#/definitions/{}", prop.name);
                            let schema_ref = SchemaRef::parse(&ref_str);
                            if let Some(sub_schema) = defs.resolve(&schema_ref) {
                                self.validate_inner(sub_schema, val, field_path, errors);
                            }
                        }
                    }
                }
            }
        }

        if let JsonValue::Array(items) = value {
            // When the schema type is Array and properties describe item schema,
            // validate each item's type against the first property (convention).
            if schema.schema_type == SchemaType::Array {
                if let Some(item_prop) = schema.properties.first() {
                    for (i, item) in items.iter().enumerate() {
                        let item_path = format!("{}[{}]", path, i);
                        if !SchemaValidator::validate_type(item, item_prop.schema_type) {
                            errors.push(ValidationError {
                                path: item_path,
                                message: format!(
                                    "expected item type {}, got {}",
                                    item_prop.schema_type,
                                    Self::type_name(item),
                                ),
                                expected_type: Some(item_prop.schema_type),
                            });
                        }
                    }
                }
            }
        }
    }

    fn type_name(value: &JsonValue) -> &'static str {
        match value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(_) => "number",
            JsonValue::Str(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }
}

/// Join two JSON-pointer segments.
fn join_path(base: &str, segment: &str) -> String {
    if base.is_empty() {
        segment.to_string()
    } else {
        format!("{}/{}", base, segment)
    }
}

// ---------------------------------------------------------------------------
// Default-value applicator
// ---------------------------------------------------------------------------

/// Fills in missing properties of a `JsonValue::Object` using the defaults
/// declared in a `JsonSchema`. Existing values are never overwritten.
pub struct JsonSchemaDefaultValues;

impl JsonSchemaDefaultValues {
    /// Apply defaults from `schema` to `value`, returning a new value.
    /// Non-object values are returned unchanged.
    pub fn apply(schema: &JsonSchema, value: &JsonValue) -> JsonValue {
        match value {
            JsonValue::Object(fields) => {
                let mut result: Vec<(String, JsonValue)> = fields.clone();
                for prop in &schema.properties {
                    let already_present = result.iter().any(|(k, _)| k == &prop.name);
                    if !already_present {
                        if let Some(ref default) = prop.default_value {
                            let val = default_str_to_value(default, prop.schema_type);
                            result.push((prop.name.clone(), val));
                        }
                    }
                }
                JsonValue::Object(result)
            }
            other => other.clone(),
        }
    }

    /// Returns the list of property names whose defaults were applied.
    pub fn applied_keys(schema: &JsonSchema, value: &JsonValue) -> Vec<String> {
        match value {
            JsonValue::Object(fields) => {
                schema
                    .properties
                    .iter()
                    .filter(|p| {
                        p.default_value.is_some()
                            && !fields.iter().any(|(k, _)| k == &p.name)
                    })
                    .map(|p| p.name.clone())
                    .collect()
            }
            _ => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Completion suggestion generator
// ---------------------------------------------------------------------------

/// Generates richer completion suggestions than `CompletionHint`, including
/// sort priority and documentation strings suitable for an LSP-style editor.
#[derive(Debug, Clone)]
pub struct JsonSchemaCompletion {
    pub label: String,
    pub kind: SchemaType,
    pub documentation: Option<String>,
    pub sort_priority: u32,
    pub snippet: String,
}

impl JsonSchemaCompletion {
    /// Generate completion items for all properties in `schema` that are not
    /// already present in `existing_keys`. Required properties sort first.
    pub fn from_schema(schema: &JsonSchema, existing_keys: &[&str]) -> Vec<Self> {
        let mut items: Vec<Self> = schema
            .properties
            .iter()
            .filter(|p| !existing_keys.contains(&p.name.as_str()))
            .map(|p| {
                let priority = if p.required { 0 } else { 1 };
                let placeholder = p
                    .default_value
                    .as_deref()
                    .unwrap_or_else(|| type_placeholder(p.schema_type));
                Self {
                    label: p.name.clone(),
                    kind: p.schema_type,
                    documentation: p.description.clone(),
                    sort_priority: priority,
                    snippet: format!("\"{}\": {}", p.name, placeholder),
                }
            })
            .collect();
        items.sort_by_key(|c| (c.sort_priority, c.label.clone()));
        items
    }

    /// Returns `true` if this is a required-property completion.
    pub fn is_required(&self) -> bool {
        self.sort_priority == 0
    }
}

// ---------------------------------------------------------------------------
// $ref resolver
// ---------------------------------------------------------------------------

/// Resolves `$ref` references within a `SchemaDefinitions` map, returning
/// the referenced `JsonSchema` or `None` if the reference is unresolvable.
pub struct SchemaRefResolver<'a> {
    definitions: &'a SchemaDefinitions,
}

impl<'a> SchemaRefResolver<'a> {
    pub fn new(definitions: &'a SchemaDefinitions) -> Self {
        Self { definitions }
    }

    /// Resolve a raw `$ref` string.
    pub fn resolve_ref(&self, raw: &str) -> Option<&'a JsonSchema> {
        let schema_ref = SchemaRef::parse(raw);
        self.definitions.resolve(&schema_ref)
    }

    /// Resolve all internal refs from a list of raw strings, skipping any
    /// that cannot be resolved. Returns `(raw_ref, &JsonSchema)` pairs.
    pub fn resolve_all<'b>(&self, refs: &'b [&str]) -> Vec<(&'b str, &'a JsonSchema)> {
        refs.iter()
            .filter_map(|r| self.resolve_ref(r).map(|s| (*r, s)))
            .collect()
    }

    /// Returns `true` if every reference in `refs` can be resolved.
    pub fn all_resolvable(&self, refs: &[&str]) -> bool {
        refs.iter().all(|r| self.resolve_ref(r).is_some())
    }

    /// Returns the names of definitions that are never referenced by any
    /// entry in `refs`.
    pub fn unreferenced_definitions(&self, refs: &[&str]) -> Vec<&'a str> {
        let resolved_names: std::collections::HashSet<String> = refs
            .iter()
            .filter_map(|r| {
                r.strip_prefix("#/definitions/")
                    .or_else(|| r.strip_prefix("#/$defs/"))
                    .map(|s| s.to_string())
            })
            .collect();
        self.definitions
            .names()
            .into_iter()
            .filter(|n| !resolved_names.contains(*n))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// SchemaConstraintChecker
// ---------------------------------------------------------------------------

/// Validates values against schema constraints (required fields, types, ranges).
pub struct SchemaConstraintChecker;

impl SchemaConstraintChecker {
    /// Check that all required properties are present in the given field names.
    pub fn check_required(schema: &JsonSchema, present_fields: &[&str]) -> Vec<String> {
        schema
            .get_required_properties()
            .iter()
            .filter(|p| !present_fields.contains(&p.name.as_str()))
            .map(|p| format!("missing required field: {}", p.name))
            .collect()
    }

    /// Check that a value string matches the expected type name.
    pub fn check_type(expected: SchemaType, actual: &str) -> bool {
        let type_name = format!("{}", expected);
        type_name == actual
    }

    /// Validate a numeric value against optional min/max bounds.
    pub fn check_number_range(value: f64, min: Option<f64>, max: Option<f64>) -> Option<String> {
        if let Some(m) = min {
            if value < m {
                return Some(format!("{} is below minimum {}", value, m));
            }
        }
        if let Some(m) = max {
            if value > m {
                return Some(format!("{} exceeds maximum {}", value, m));
            }
        }
        None
    }

    /// Check if a string value matches one of the allowed enum values.
    pub fn check_enum(value: &str, allowed: &[&str]) -> bool {
        allowed.contains(&value)
    }

    /// Check if a string matches a simple substring pattern.
    pub fn check_pattern(value: &str, pattern: &str) -> bool {
        value.contains(pattern)
    }
}

// ---------------------------------------------------------------------------
// SchemaComposer
// ---------------------------------------------------------------------------

/// Merges two schemas together.
pub struct SchemaComposer;

impl SchemaComposer {
    /// Merge properties from `other` into `base`, keeping base properties on conflict.
    pub fn merge_properties(base: &JsonSchema, other: &JsonSchema) -> Vec<SchemaProperty> {
        let mut merged = base.properties.clone();
        for prop in &other.properties {
            if !merged.iter().any(|p| p.name == prop.name) {
                merged.push(prop.clone());
            }
        }
        merged
    }

    /// Combine required field lists from two schemas.
    pub fn combine_required(base: &JsonSchema, other: &JsonSchema) -> Vec<String> {
        let mut required: Vec<String> = base
            .get_required_properties()
            .iter()
            .map(|p| p.name.clone())
            .collect();
        for p in other.get_required_properties() {
            if !required.contains(&p.name) {
                required.push(p.name.clone());
            }
        }
        required
    }
}

// ---------------------------------------------------------------------------
// SchemaDocGenerator
// ---------------------------------------------------------------------------

/// Generates human-readable documentation from a schema.
pub struct SchemaDocGenerator;

impl SchemaDocGenerator {
    /// Generate a property table as a vector of (name, type, required, default) rows.
    pub fn property_table(schema: &JsonSchema) -> Vec<(String, String, bool, String)> {
        schema
            .properties
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    format!("{}", p.schema_type),
                    p.required,
                    p.default_value.clone().unwrap_or_default(),
                )
            })
            .collect()
    }

    /// Generate a plain-text summary of the schema.
    pub fn summary(schema: &JsonSchema) -> String {
        let title = schema.title.as_deref().unwrap_or("Untitled");
        let desc = schema.description.as_deref().unwrap_or("No description");
        let props = schema.properties.len();
        format!("{} — {} ({} properties)", title, desc, props)
    }

    /// Generate markdown documentation.
    pub fn to_markdown(schema: &JsonSchema) -> String {
        let mut md = String::new();
        let title = schema.title.as_deref().unwrap_or("Schema");
        md.push_str(&format!("# {}\n\n", title));
        if let Some(ref d) = schema.description {
            md.push_str(&format!("{}\n\n", d));
        }
        md.push_str("| Property | Type | Required | Default |\n");
        md.push_str("|----------|------|----------|--------|\n");
        for p in &schema.properties {
            let req = if p.required { "yes" } else { "no" };
            let def = p.default_value.as_deref().unwrap_or("-");
            md.push_str(&format!("| {} | {} | {} | {} |\n", p.name, p.schema_type, req, def));
        }
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> JsonSchema {
        JsonSchema {
            id: Some("tsconfig".to_string()),
            title: Some("TypeScript Config".to_string()),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![SchemaProperty {
                name: "compilerOptions".to_string(),
                schema_type: SchemaType::Object,
                description: Some("Compiler options".to_string()),
                required: false,
                default_value: None,
            }],
            file_match: vec!["tsconfig.json".to_string(), "*.tsconfig.json".to_string()],
        }
    }

    #[test]
    fn register_and_find_by_id() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        let found = reg.get_schema_by_id("tsconfig");
        assert!(found.is_some());
        assert_eq!(found.unwrap().title.as_deref(), Some("TypeScript Config"));
    }

    #[test]
    fn find_for_file_exact() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        assert!(reg.find_for_file("tsconfig.json").is_some());
        assert!(reg.find_for_file("package.json").is_none());
    }

    #[test]
    fn find_for_file_wildcard() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        assert!(reg.find_for_file("app.tsconfig.json").is_some());
    }

    #[test]
    fn missing_schema() {
        let reg = SchemaRegistry::new();
        assert!(reg.get_schema_by_id("nonexistent").is_none());
    }

    #[test]
    fn schema_property_is_required() {
        let prop = SchemaProperty {
            name: "strict".to_string(),
            schema_type: SchemaType::Boolean,
            description: None,
            required: true,
            default_value: None,
        };
        assert!(prop.is_required());
        assert!(!prop.has_default());
    }

    #[test]
    fn schema_property_has_default() {
        let prop = SchemaProperty {
            name: "target".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: false,
            default_value: Some("es6".to_string()),
        };
        assert!(!prop.is_required());
        assert!(prop.has_default());
    }

    #[test]
    fn json_schema_get_required_and_property() {
        let schema = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "name".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: true,
                    default_value: None,
                },
                SchemaProperty {
                    name: "version".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: false,
                    default_value: Some("1.0.0".to_string()),
                },
            ],
            file_match: vec![],
        };
        assert_eq!(schema.property_count(), 2);
        let required = schema.get_required_properties();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].name, "name");
        assert!(schema.get_property("version").is_some());
        assert!(schema.get_property("missing").is_none());
    }

    #[test]
    fn unregister_schema() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        assert_eq!(reg.schema_count(), 1);
        assert!(reg.unregister("tsconfig"));
        assert_eq!(reg.schema_count(), 0);
        assert!(!reg.unregister("tsconfig"));
    }

    #[test]
    fn get_all_file_matches_and_find_schemas_for_file() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        reg.register(JsonSchema {
            id: Some("package".to_string()),
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![],
            file_match: vec!["package.json".to_string()],
        });
        let matches = reg.get_all_file_matches();
        assert_eq!(matches.len(), 3);
        assert!(matches.contains(&"tsconfig.json"));
        assert!(matches.contains(&"package.json"));

        let found = reg.find_schemas_for_file("tsconfig.json");
        assert_eq!(found.len(), 1);
        assert!(reg.find_schemas_for_file("unknown.yaml").is_empty());
    }

    #[test]
    fn validate_type_checks() {
        assert!(validate_type("hello", SchemaType::String));
        assert!(validate_type("42", SchemaType::Number));
        assert!(validate_type("3.14", SchemaType::Number));
        assert!(!validate_type("abc", SchemaType::Number));
        assert!(validate_type("7", SchemaType::Integer));
        assert!(!validate_type("7.5", SchemaType::Integer));
        assert!(validate_type("true", SchemaType::Boolean));
        assert!(validate_type("false", SchemaType::Boolean));
        assert!(!validate_type("yes", SchemaType::Boolean));
        assert!(validate_type("null", SchemaType::Null));
        assert!(!validate_type("none", SchemaType::Null));
        assert!(validate_type("[1,2]", SchemaType::Array));
        assert!(!validate_type("1,2", SchemaType::Array));
        assert!(validate_type("{}", SchemaType::Object));
        assert!(!validate_type("obj", SchemaType::Object));
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", SchemaType::String), "string");
        assert_eq!(format!("{}", SchemaType::Integer), "integer");
        assert_eq!(format!("{}", SchemaType::Null), "null");

        let prop = SchemaProperty {
            name: "host".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: false,
            default_value: None,
        };
        assert_eq!(format!("{}", prop), "host: string");
    }

    // ── New tests ───────────────────────────────────────────────────────

    #[test]
    fn schema_validation_result_basics() {
        let mut r = SchemaValidationResult::new();
        assert!(r.is_valid());
        assert_eq!(r.error_count(), 0);
        assert_eq!(r.warning_count(), 0);

        r.add_error("e1");
        r.add_warning("w1");
        assert!(!r.is_valid());
        assert_eq!(r.error_count(), 1);
        assert_eq!(r.warning_count(), 1);
    }

    #[test]
    fn schema_validation_result_merge() {
        let mut a = SchemaValidationResult::new();
        a.add_error("a-err");
        let mut b = SchemaValidationResult::new();
        b.add_error("b-err");
        b.add_warning("b-warn");
        a.merge(b);
        assert_eq!(a.error_count(), 2);
        assert_eq!(a.warning_count(), 1);
    }

    #[test]
    fn validate_schema_property_missing_required() {
        let prop = SchemaProperty {
            name: "id".to_string(),
            schema_type: SchemaType::Integer,
            description: None,
            required: true,
            default_value: None,
        };
        let r = validate_schema_property(&prop, None);
        assert!(!r.is_valid());
        assert!(r.errors[0].contains("Missing required"));
    }

    #[test]
    fn validate_schema_property_wrong_type() {
        let prop = SchemaProperty {
            name: "count".to_string(),
            schema_type: SchemaType::Integer,
            description: None,
            required: false,
            default_value: None,
        };
        let r = validate_schema_property(&prop, Some("abc"));
        assert!(!r.is_valid());
        assert!(r.errors[0].contains("expected type"));
    }

    #[test]
    fn validate_schema_property_ok() {
        let prop = SchemaProperty {
            name: "count".to_string(),
            schema_type: SchemaType::Integer,
            description: None,
            required: true,
            default_value: None,
        };
        let r = validate_schema_property(&prop, Some("42"));
        assert!(r.is_valid());
    }

    #[test]
    fn schema_ref_parse_and_display() {
        let internal = SchemaRef::parse("#/definitions/Foo");
        assert!(internal.is_internal());
        assert!(!internal.is_external());
        assert_eq!(internal.as_str(), "#/definitions/Foo");
        assert!(format!("{}", internal).contains("internal"));

        let external = SchemaRef::parse("https://example.com/schema.json");
        assert!(external.is_external());
        assert!(format!("{}", external).contains("external"));
    }

    #[test]
    fn schema_constraint_min_max_length() {
        let min = SchemaConstraint::MinLength(3);
        assert!(min.is_satisfied_by("abc"));
        assert!(!min.is_satisfied_by("ab"));

        let max = SchemaConstraint::MaxLength(5);
        assert!(max.is_satisfied_by("hello"));
        assert!(!max.is_satisfied_by("toolong"));
    }

    #[test]
    fn schema_constraint_pattern_and_enum() {
        let pat = SchemaConstraint::Pattern("foo".to_string());
        assert!(pat.is_satisfied_by("foobar"));
        assert!(!pat.is_satisfied_by("bazqux"));

        let en = SchemaConstraint::Enum(vec!["a".into(), "b".into()]);
        assert!(en.is_satisfied_by("a"));
        assert!(!en.is_satisfied_by("c"));
    }

    #[test]
    fn schema_constraint_minimum_maximum() {
        let min = SchemaConstraint::Minimum(1.0);
        assert!(min.is_satisfied_by("1"));
        assert!(min.is_satisfied_by("2.5"));
        assert!(!min.is_satisfied_by("0.5"));
        assert!(!min.is_satisfied_by("not_a_number"));

        let max = SchemaConstraint::Maximum(10.0);
        assert!(max.is_satisfied_by("10"));
        assert!(!max.is_satisfied_by("11"));
    }

    #[test]
    fn schema_property_effective_value() {
        let prop = SchemaProperty {
            name: "target".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: false,
            default_value: Some("es6".to_string()),
        };
        assert_eq!(prop.effective_value(Some("es2020")), Some("es2020"));
        assert_eq!(prop.effective_value(None), Some("es6"));

        let no_default = SchemaProperty {
            name: "x".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: false,
            default_value: None,
        };
        assert_eq!(no_default.effective_value(None), None);
    }

    #[test]
    fn schema_property_partial_eq() {
        let a = SchemaProperty {
            name: "x".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: true,
            default_value: None,
        };
        let b = SchemaProperty {
            name: "x".to_string(),
            schema_type: SchemaType::String,
            description: Some("desc".to_string()),
            required: false,
            default_value: Some("v".to_string()),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn json_schema_get_optional_properties() {
        let schema = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "a".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: true,
                    default_value: None,
                },
                SchemaProperty {
                    name: "b".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: false,
                    default_value: None,
                },
            ],
            file_match: vec![],
        };
        let opt = schema.get_optional_properties();
        assert_eq!(opt.len(), 1);
        assert_eq!(opt[0].name, "b");
    }

    #[test]
    fn json_schema_validate_object() {
        let schema = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "name".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: true,
                    default_value: None,
                },
                SchemaProperty {
                    name: "age".to_string(),
                    schema_type: SchemaType::Integer,
                    description: None,
                    required: true,
                    default_value: None,
                },
            ],
            file_match: vec![],
        };
        let ok = schema.validate_object(&[("name", "Alice"), ("age", "30")]);
        assert!(ok.is_valid());

        let missing = schema.validate_object(&[("name", "Alice")]);
        assert!(!missing.is_valid());
    }

    #[test]
    fn json_schema_merge_with() {
        let mut base = JsonSchema {
            id: Some("base".to_string()),
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![SchemaProperty {
                name: "a".to_string(),
                schema_type: SchemaType::String,
                description: None,
                required: false,
                default_value: None,
            }],
            file_match: vec![],
        };
        let other = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "a".to_string(),
                    schema_type: SchemaType::Integer,
                    description: None,
                    required: false,
                    default_value: None,
                },
                SchemaProperty {
                    name: "b".to_string(),
                    schema_type: SchemaType::Boolean,
                    description: None,
                    required: false,
                    default_value: None,
                },
            ],
            file_match: vec![],
        };
        base.merge_with(&other);
        assert_eq!(base.property_count(), 2);
        // "a" kept original type
        assert_eq!(base.get_property("a").unwrap().schema_type, SchemaType::String);
        assert_eq!(base.get_property("b").unwrap().schema_type, SchemaType::Boolean);
    }

    #[test]
    fn json_schema_partial_eq_and_display() {
        let a = test_schema();
        let mut b = test_schema();
        b.title = Some("Different Title".to_string());
        assert_eq!(a, b); // equality is id + type only

        let display = format!("{}", a);
        assert!(display.contains("TypeScript Config"));
        assert!(display.contains("object"));
    }

    #[test]
    fn schema_registry_find_by_title() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        assert!(reg.find_by_title("TypeScript Config").is_some());
        assert!(reg.find_by_title("Nope").is_none());
    }

    #[test]
    fn schema_registry_get_schemas_by_type() {
        let mut reg = SchemaRegistry::new();
        reg.register(test_schema());
        reg.register(JsonSchema {
            id: Some("str_schema".to_string()),
            title: None,
            description: None,
            schema_type: SchemaType::String,
            properties: vec![],
            file_match: vec![],
        });
        assert_eq!(reg.get_schemas_by_type(SchemaType::Object).len(), 1);
        assert_eq!(reg.get_schemas_by_type(SchemaType::String).len(), 1);
        assert_eq!(reg.get_schemas_by_type(SchemaType::Array).len(), 0);
    }

    #[test]
    fn schema_registry_merge_registries() {
        let mut a = SchemaRegistry::new();
        a.register(test_schema());

        let mut b = SchemaRegistry::new();
        b.register(test_schema()); // duplicate id
        b.register(JsonSchema {
            id: Some("other".to_string()),
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![],
            file_match: vec![],
        });
        a.merge_registries(b);
        assert_eq!(a.schema_count(), 2); // duplicate skipped
    }

    #[test]
    fn format_schema_as_json_output() {
        let schema = test_schema();
        let json = format_schema_as_json(&schema);
        assert!(json.contains("\"$id\": \"tsconfig\""));
        assert!(json.contains("\"title\": \"TypeScript Config\""));
        assert!(json.contains("\"type\": \"object\""));
        assert!(json.contains("\"compilerOptions\""));
    }

    #[test]
    fn schema_property_builder_helpers() {
        let prop = SchemaProperty {
            name: "x".to_string(),
            schema_type: SchemaType::String,
            description: None,
            required: false,
            default_value: None,
        }
        .with_description("A description")
        .with_default("default_val");
        assert_eq!(prop.description.as_deref(), Some("A description"));
        assert_eq!(prop.default_value.as_deref(), Some("default_val"));
    }

    // ── SchemaValidator tests ──────────────────────────────────────────

    fn validator_schema() -> JsonSchema {
        JsonSchema {
            id: Some("test".to_string()),
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "name".to_string(),
                    schema_type: SchemaType::String,
                    description: None,
                    required: true,
                    default_value: None,
                },
                SchemaProperty {
                    name: "age".to_string(),
                    schema_type: SchemaType::Number,
                    description: None,
                    required: false,
                    default_value: None,
                },
            ],
            file_match: vec![],
        }
    }

    #[test]
    fn validate_type_string() {
        assert!(SchemaValidator::validate_type(
            &JsonValue::Str("hi".into()),
            SchemaType::String
        ));
        assert!(!SchemaValidator::validate_type(
            &JsonValue::Number(1.0),
            SchemaType::String
        ));
    }

    #[test]
    fn validate_type_number() {
        assert!(SchemaValidator::validate_type(
            &JsonValue::Number(3.14),
            SchemaType::Number
        ));
        assert!(!SchemaValidator::validate_type(
            &JsonValue::Bool(true),
            SchemaType::Number
        ));
    }

    #[test]
    fn validate_type_boolean() {
        assert!(SchemaValidator::validate_type(
            &JsonValue::Bool(false),
            SchemaType::Boolean
        ));
        assert!(!SchemaValidator::validate_type(
            &JsonValue::Null,
            SchemaType::Boolean
        ));
    }

    #[test]
    fn validate_type_null_array_object() {
        assert!(SchemaValidator::validate_type(&JsonValue::Null, SchemaType::Null));
        assert!(SchemaValidator::validate_type(
            &JsonValue::Array(vec![]),
            SchemaType::Array
        ));
        assert!(SchemaValidator::validate_type(
            &JsonValue::Object(vec![]),
            SchemaType::Object
        ));
    }

    #[test]
    fn validate_valid_object() {
        let schema = validator_schema();
        let value = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Alice".into())),
            ("age".into(), JsonValue::Number(30.0)),
        ]);
        let errors = SchemaValidator::validate(&schema, &value);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_missing_required_property() {
        let schema = validator_schema();
        let value = JsonValue::Object(vec![
            ("age".into(), JsonValue::Number(25.0)),
        ]);
        let errors = SchemaValidator::validate(&schema, &value);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("missing required"));
        assert_eq!(errors[0].path, "name");
    }

    #[test]
    fn validate_wrong_property_type() {
        let schema = validator_schema();
        let value = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Bob".into())),
            ("age".into(), JsonValue::Str("not a number".into())),
        ]);
        let errors = SchemaValidator::validate(&schema, &value);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].path, "age");
        assert_eq!(errors[0].expected_type, Some(SchemaType::Number));
    }

    #[test]
    fn validate_top_level_type_mismatch() {
        let schema = validator_schema();
        let value = JsonValue::Str("not an object".into());
        let errors = SchemaValidator::validate(&schema, &value);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("expected type object"));
    }

    #[test]
    fn validate_enum_allowed_value() {
        let allowed = vec!["red".to_string(), "green".to_string(), "blue".to_string()];
        let errors = SchemaValidator::validate_enum("color", "green", &allowed);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_enum_disallowed_value() {
        let allowed = vec!["red".to_string(), "green".to_string()];
        let errors = SchemaValidator::validate_enum("color", "purple", &allowed);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("purple"));
        assert!(errors[0].message.contains("not one of the allowed"));
    }

    #[test]
    fn validate_enum_empty_allows_any() {
        let errors = SchemaValidator::validate_enum("x", "anything", &[]);
        assert!(errors.is_empty());
    }

    #[test]
    fn validate_required_all_present() {
        let schema = validator_schema();
        let fields = vec![
            ("name".to_string(), JsonValue::Str("ok".into())),
            ("age".to_string(), JsonValue::Number(1.0)),
        ];
        let errors = SchemaValidator::validate_required(&schema, &fields);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_schema_type_all() {
        assert_eq!(SchemaType::all().len(), 7);
    }

    #[test]
    fn test_schema_type_is_numeric() {
        assert!(SchemaType::Number.is_numeric());
        assert!(SchemaType::Integer.is_numeric());
        assert!(!SchemaType::String.is_numeric());
    }

    #[test]
    fn test_schema_type_is_primitive() {
        assert!(SchemaType::String.is_primitive());
        assert!(!SchemaType::Array.is_primitive());
        assert!(!SchemaType::Object.is_primitive());
    }

    #[test]
    fn test_schema_type_from_json_name() {
        assert_eq!(SchemaType::from_json_name("string"), Some(SchemaType::String));
        assert_eq!(SchemaType::from_json_name("integer"), Some(SchemaType::Integer));
        assert_eq!(SchemaType::from_json_name("bogus"), None);
    }

    #[test]
    fn test_schema_property_builder() {
        let req = SchemaProperty::required_prop("name", SchemaType::String, "The name");
        assert!(req.required);
        assert_eq!(req.schema_type, SchemaType::String);
        let opt = SchemaProperty::optional_prop("count", SchemaType::Integer, "0");
        assert!(!opt.required);
        assert_eq!(opt.effective_default(), Some("0"));
    }

    #[test]
    fn test_schema_property_display() {
        let p = SchemaProperty::required_prop("id", SchemaType::String, "ID");
        let s = format!("{p}");
        assert!(s.contains("id"));
        assert!(s.contains("required"));
    }

    #[test]
    fn test_json_schema_helpers() {
        let schema = JsonSchema {
            id: Some("test".to_string()),
            title: Some("Test".to_string()),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
                SchemaProperty::optional_prop("age", SchemaType::Integer, "0"),
            ],
            file_match: vec![],
        };
        assert_eq!(schema.required_count(), 1);
        assert_eq!(schema.optional_count(), 1);
        assert!(schema.find_property("name").is_some());
        assert!(schema.find_property("missing").is_none());
        assert_eq!(schema.property_names(), vec!["age", "name"]);
    }

    #[test]
    fn test_compare_schemas() {
        let old = JsonSchema {
            id: Some("a".to_string()),
            title: Some("A".to_string()),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
                SchemaProperty::optional_prop("old_field", SchemaType::Boolean, "false"),
            ],
            file_match: vec![],
        };
        let new = JsonSchema {
            id: Some("a".to_string()),
            title: Some("A".to_string()),
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::Integer, "Name"),
                SchemaProperty::optional_prop("new_field", SchemaType::String, ""),
            ],
            file_match: vec![],
        };
        let diffs = compare_schemas(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, SchemaDiff::PropertyAdded(n) if n == "new_field")));
        assert!(diffs.iter().any(|d| matches!(d, SchemaDiff::PropertyRemoved(n) if n == "old_field")));
        assert!(diffs.iter().any(|d| matches!(d, SchemaDiff::TypeChanged { property, .. } if property == "name")));
    }

    #[test]
    fn test_schema_diff_display() {
        let d = SchemaDiff::PropertyAdded("foo".into());
        assert_eq!(format!("{d}"), "+ foo");
    }

    // ── Composition tests ──────────────────────────────────────────────

    #[test]
    fn composite_allof_merge_and_validate() {
        let s1 = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
            ],
            file_match: vec!["a.json".into()],
        };
        let s2 = JsonSchema {
            id: None,
            title: None,
            description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("age", SchemaType::Number, "Age"),
            ],
            file_match: vec!["b.json".into()],
        };
        let mut comp = CompositeSchema::new(CompositionKind::AllOf);
        comp.add(s1);
        comp.add(s2);

        // merge_all collects all properties
        let merged = comp.merge_all();
        assert_eq!(merged.property_count(), 2);
        assert!(merged.file_match.contains(&"a.json".to_string()));
        assert!(merged.file_match.contains(&"b.json".to_string()));

        // AllOf validation: both sub-schemas must pass
        let good = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Alice".into())),
            ("age".into(), JsonValue::Number(30.0)),
        ]);
        assert!(comp.validate(&good).is_empty());

        let bad = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Bob".into())),
        ]);
        assert!(!comp.validate(&bad).is_empty());
    }

    #[test]
    fn composite_anyof_validation() {
        let str_schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::String,
            properties: vec![], file_match: vec![],
        };
        let num_schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Number,
            properties: vec![], file_match: vec![],
        };
        let mut comp = CompositeSchema::new(CompositionKind::AnyOf);
        comp.add(str_schema);
        comp.add(num_schema);

        assert!(comp.validate(&JsonValue::Str("hi".into())).is_empty());
        assert!(comp.validate(&JsonValue::Number(42.0)).is_empty());
        assert!(!comp.validate(&JsonValue::Bool(true)).is_empty());
    }

    #[test]
    fn composite_oneof_validation() {
        let str_schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::String,
            properties: vec![], file_match: vec![],
        };
        let also_str = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::String,
            properties: vec![], file_match: vec![],
        };
        let mut comp = CompositeSchema::new(CompositionKind::OneOf);
        comp.add(str_schema);
        comp.add(also_str);

        // A string matches both -> oneOf fails (needs exactly 1)
        let errs = comp.validate(&JsonValue::Str("x".into()));
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("exactly one"));

        // A number matches none -> also fails
        assert!(!comp.validate(&JsonValue::Number(1.0)).is_empty());
    }

    // ── $ref resolution tests ──────────────────────────────────────────

    #[test]
    fn schema_definitions_resolve_internal_ref() {
        let mut defs = SchemaDefinitions::new();
        defs.insert("Address", JsonSchema {
            id: None, title: Some("Address".into()), description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("street", SchemaType::String, "Street"),
            ],
            file_match: vec![],
        });
        assert_eq!(defs.len(), 1);
        assert!(!defs.is_empty());
        assert_eq!(defs.names(), vec!["Address"]);

        let r = SchemaRef::parse("#/definitions/Address");
        let resolved = defs.resolve(&r);
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().title.as_deref(), Some("Address"));

        // $defs variant
        let r2 = SchemaRef::parse("#/$defs/Address");
        assert!(defs.resolve(&r2).is_some());

        // External ref returns None
        let ext = SchemaRef::parse("https://example.com/schema.json");
        assert!(defs.resolve(&ext).is_none());

        // Unknown definition returns None
        let unknown = SchemaRef::parse("#/definitions/Missing");
        assert!(defs.resolve(&unknown).is_none());
    }

    // ── Default extraction tests ───────────────────────────────────────

    #[test]
    fn extract_defaults_and_build_default_object() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
                SchemaProperty::optional_prop("count", SchemaType::Integer, "10"),
                SchemaProperty::optional_prop("verbose", SchemaType::Boolean, "true"),
            ],
            file_match: vec![],
        };

        let defaults = extract_defaults(&schema);
        // "name" has no default so only 2 entries
        assert_eq!(defaults.len(), 2);
        assert!(defaults.iter().any(|(k, v)| *k == "count" && *v == "10"));
        assert!(defaults.iter().any(|(k, v)| *k == "verbose" && *v == "true"));

        let obj = build_default_object(&schema);
        if let JsonValue::Object(fields) = &obj {
            assert_eq!(fields.len(), 2);
            assert!(fields.iter().any(|(k, v)| k == "count" && *v == JsonValue::Number(10.0)));
            assert!(fields.iter().any(|(k, v)| k == "verbose" && *v == JsonValue::Bool(true)));
        } else {
            panic!("expected Object");
        }
    }

    // ── Completion hint tests ──────────────────────────────────────────

    #[test]
    fn completion_hints_filters_existing_keys() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
                SchemaProperty::optional_prop("debug", SchemaType::Boolean, "false"),
                SchemaProperty::optional_prop("port", SchemaType::Integer, "8080"),
            ],
            file_match: vec![],
        };

        let hints = completion_hints(&schema, &["name"]);
        assert_eq!(hints.len(), 2);
        assert!(hints.iter().all(|h| h.label != "name"));

        let debug_hint = hints.iter().find(|h| h.label == "debug").unwrap();
        assert_eq!(debug_hint.value_type, SchemaType::Boolean);
        assert!(!debug_hint.required);
        assert!(debug_hint.insert_text.as_ref().unwrap().contains("false"));

        let port_hint = hints.iter().find(|h| h.label == "port").unwrap();
        assert!(port_hint.insert_text.as_ref().unwrap().contains("8080"));

        // With no existing keys all properties are returned
        let all = completion_hints(&schema, &[]);
        assert_eq!(all.len(), 3);
    }

    // ── JsonSchemaValidator tests ─────────────────────────────────────

    #[test]
    fn json_schema_validator_type_mismatch_at_root() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![],
            file_match: vec![],
        };
        let val = JsonValue::Str("not an object".into());
        let v = JsonSchemaValidator::new();
        let errs = v.validate(&schema, &val);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("expected type object"));
    }

    #[test]
    fn json_schema_validator_missing_required() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("host", SchemaType::String, "Host"),
                SchemaProperty::optional_prop("port", SchemaType::Integer, "80"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Object(vec![
            ("port".into(), JsonValue::Number(443.0)),
        ]);
        let v = JsonSchemaValidator::new();
        let errs = v.validate(&schema, &val);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].message.contains("host"));
    }

    #[test]
    fn json_schema_validator_valid_object() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Alice".into())),
        ]);
        let v = JsonSchemaValidator::new();
        assert!(v.validate(&schema, &val).is_empty());
    }

    #[test]
    fn json_schema_validator_nested_with_definitions() {
        let mut defs = SchemaDefinitions::new();
        defs.insert("address", JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("street", SchemaType::String, "Street"),
            ],
            file_match: vec![],
        });

        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty {
                    name: "address".into(),
                    schema_type: SchemaType::Object,
                    description: None,
                    required: true,
                    default_value: None,
                },
            ],
            file_match: vec![],
        };

        // Missing the required "street" inside nested address object.
        let val = JsonValue::Object(vec![
            ("address".into(), JsonValue::Object(vec![])),
        ]);
        let v = JsonSchemaValidator::with_definitions(&defs);
        let errs = v.validate(&schema, &val);
        assert!(errs.iter().any(|e| e.path == "address/street"));
    }

    #[test]
    fn json_schema_validator_array_items() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Array,
            properties: vec![
                SchemaProperty::required_prop("item", SchemaType::Number, "Item"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Array(vec![
            JsonValue::Number(1.0),
            JsonValue::Str("oops".into()),
            JsonValue::Number(3.0),
        ]);
        let v = JsonSchemaValidator::new();
        let errs = v.validate(&schema, &val);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].path.contains("[1]"));
    }

    // ── JsonSchemaDefaultValues tests ─────────────────────────────────

    #[test]
    fn default_values_applied_to_missing_fields() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("name", SchemaType::String, "Name"),
                SchemaProperty::optional_prop("level", SchemaType::Integer, "5"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("Bob".into())),
        ]);
        let result = JsonSchemaDefaultValues::apply(&schema, &val);
        if let JsonValue::Object(fields) = &result {
            assert_eq!(fields.len(), 2);
            assert!(fields.iter().any(|(k, v)| k == "level" && *v == JsonValue::Number(5.0)));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn default_values_does_not_overwrite_existing() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::optional_prop("color", SchemaType::String, "red"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Object(vec![
            ("color".into(), JsonValue::Str("blue".into())),
        ]);
        let result = JsonSchemaDefaultValues::apply(&schema, &val);
        if let JsonValue::Object(fields) = &result {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].1, JsonValue::Str("blue".into()));
        } else {
            panic!("expected Object");
        }
    }

    #[test]
    fn default_values_applied_keys_list() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::optional_prop("a", SchemaType::String, "x"),
                SchemaProperty::optional_prop("b", SchemaType::String, "y"),
            ],
            file_match: vec![],
        };
        let val = JsonValue::Object(vec![("a".into(), JsonValue::Str("z".into()))]);
        let keys = JsonSchemaDefaultValues::applied_keys(&schema, &val);
        assert_eq!(keys, vec!["b"]);
    }

    // ── JsonSchemaCompletion tests ────────────────────────────────────

    #[test]
    fn completion_required_sorted_first() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::optional_prop("z_opt", SchemaType::String, ""),
                SchemaProperty::required_prop("a_req", SchemaType::String, "Required"),
            ],
            file_match: vec![],
        };
        let items = JsonSchemaCompletion::from_schema(&schema, &[]);
        assert_eq!(items.len(), 2);
        assert!(items[0].is_required());
        assert_eq!(items[0].label, "a_req");
    }

    #[test]
    fn completion_excludes_existing() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![
                SchemaProperty::required_prop("a", SchemaType::String, "A"),
                SchemaProperty::required_prop("b", SchemaType::String, "B"),
            ],
            file_match: vec![],
        };
        let items = JsonSchemaCompletion::from_schema(&schema, &["a"]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "b");
    }

    // ── SchemaRefResolver tests ──────────────────────────────────────

    #[test]
    fn ref_resolver_resolves_internal() {
        let mut defs = SchemaDefinitions::new();
        defs.insert("Foo", JsonSchema {
            id: Some("foo".into()), title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![],
            file_match: vec![],
        });
        let resolver = SchemaRefResolver::new(&defs);
        assert!(resolver.resolve_ref("#/definitions/Foo").is_some());
        assert!(resolver.resolve_ref("#/definitions/Bar").is_none());
        assert!(resolver.resolve_ref("https://example.com/schema").is_none());
    }

    #[test]
    fn ref_resolver_all_resolvable() {
        let mut defs = SchemaDefinitions::new();
        defs.insert("A", JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::String,
            properties: vec![],
            file_match: vec![],
        });
        let resolver = SchemaRefResolver::new(&defs);
        assert!(resolver.all_resolvable(&["#/definitions/A"]));
        assert!(!resolver.all_resolvable(&["#/definitions/A", "#/definitions/B"]));
    }

    #[test]
    fn ref_resolver_unreferenced_definitions() {
        let mut defs = SchemaDefinitions::new();
        defs.insert("Used", JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::String,
            properties: vec![],
            file_match: vec![],
        });
        defs.insert("Unused", JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Number,
            properties: vec![],
            file_match: vec![],
        });
        let resolver = SchemaRefResolver::new(&defs);
        let unused = resolver.unreferenced_definitions(&["#/definitions/Used"]);
        assert_eq!(unused, vec!["Unused"]);
    }

    // -- SchemaConstraintChecker tests --

    #[test]
    fn constraint_checker_required_missing() {
        let schema = test_schema();
        let errors = SchemaConstraintChecker::check_required(&schema, &[]);
        assert!(errors.is_empty()); // compilerOptions is not required in test_schema
    }

    #[test]
    fn constraint_checker_required_with_required_field() {
        let schema = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![SchemaProperty {
                name: "name".to_string(),
                schema_type: SchemaType::String,
                description: None,
                required: true,
                default_value: None,
            }],
            file_match: vec![],
        };
        let errors = SchemaConstraintChecker::check_required(&schema, &[]);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("name"));
    }

    #[test]
    fn constraint_checker_check_type() {
        assert!(SchemaConstraintChecker::check_type(SchemaType::String, "string"));
        assert!(!SchemaConstraintChecker::check_type(SchemaType::Number, "string"));
    }

    #[test]
    fn constraint_checker_check_number_range() {
        assert!(SchemaConstraintChecker::check_number_range(5.0, Some(1.0), Some(10.0)).is_none());
        assert!(SchemaConstraintChecker::check_number_range(0.5, Some(1.0), None).is_some());
        assert!(SchemaConstraintChecker::check_number_range(15.0, None, Some(10.0)).is_some());
    }

    #[test]
    fn constraint_checker_check_enum() {
        assert!(SchemaConstraintChecker::check_enum("a", &["a", "b", "c"]));
        assert!(!SchemaConstraintChecker::check_enum("d", &["a", "b"]));
    }

    #[test]
    fn constraint_checker_check_pattern() {
        assert!(SchemaConstraintChecker::check_pattern("hello world", "world"));
        assert!(!SchemaConstraintChecker::check_pattern("hello", "xyz"));
    }

    // -- SchemaComposer tests --

    #[test]
    fn composer_merge_properties() {
        let base = test_schema();
        let other = JsonSchema {
            id: None, title: None, description: None,
            schema_type: SchemaType::Object,
            properties: vec![SchemaProperty {
                name: "extra".to_string(),
                schema_type: SchemaType::Boolean,
                description: None,
                required: false,
                default_value: None,
            }],
            file_match: vec![],
        };
        let merged = SchemaComposer::merge_properties(&base, &other);
        assert!(merged.iter().any(|p| p.name == "extra"));
        assert!(merged.iter().any(|p| p.name == "compilerOptions"));
    }

    #[test]
    fn composer_no_duplicate_on_conflict() {
        let base = test_schema();
        let merged = SchemaComposer::merge_properties(&base, &base);
        let count = merged.iter().filter(|p| p.name == "compilerOptions").count();
        assert_eq!(count, 1);
    }

    // -- SchemaDocGenerator tests --

    #[test]
    fn doc_property_table() {
        let schema = test_schema();
        let table = SchemaDocGenerator::property_table(&schema);
        assert!(!table.is_empty());
        assert_eq!(table[0].0, "compilerOptions");
    }

    #[test]
    fn doc_summary() {
        let schema = test_schema();
        let s = SchemaDocGenerator::summary(&schema);
        assert!(s.contains("TypeScript Config"));
    }

    #[test]
    fn doc_to_markdown() {
        let schema = test_schema();
        let md = SchemaDocGenerator::to_markdown(&schema);
        assert!(md.contains("# TypeScript Config"));
        assert!(md.contains("compilerOptions"));
    }

}
