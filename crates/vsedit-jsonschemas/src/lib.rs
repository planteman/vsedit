//! JSON schema validation support – types, properties, and a schema registry.

use std::collections::HashMap;
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


// ---------------------------------------------------------------------------
// jsonschemas – Data validation and analysis helpers
// ---------------------------------------------------------------------------

/// Result of validating a value against a schema-like rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XJsonschemasValidationResult {
    Ok,
    Error(String),
    Warning(String),
}

impl XJsonschemasValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(m) | Self::Warning(m) => Some(m),
        }
    }
}

/// A key-value pair with optional metadata tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XJsonschemasTaggedEntry {
    pub key: String,
    pub value: String,
    pub tag: Option<String>,
}

impl XJsonschemasTaggedEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn matches_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

/// Validate that a string is non-empty and within a max length.
pub fn x_jsonschemas_validate_string(value: &str, max_len: usize) -> XJsonschemasValidationResult {
    if value.is_empty() {
        return XJsonschemasValidationResult::Error("value must not be empty".into());
    }
    if value.len() > max_len {
        return XJsonschemasValidationResult::Error(
            format!("value exceeds max length of {max_len}"),
        );
    }
    XJsonschemasValidationResult::Ok
}

/// Validate that a number falls within an inclusive range.
pub fn x_jsonschemas_validate_range(value: i64, min: i64, max: i64) -> XJsonschemasValidationResult {
    if value < min || value > max {
        XJsonschemasValidationResult::Error(
            format!("{value} is outside range [{min}, {max}]"),
        )
    } else {
        XJsonschemasValidationResult::Ok
    }
}

/// Filter entries by tag, returning only matching ones.
pub fn x_jsonschemas_filter_by_tag<'a>(
    entries: &'a [XJsonschemasTaggedEntry],
    tag: &str,
) -> Vec<&'a XJsonschemasTaggedEntry> {
    entries.iter().filter(|e| e.matches_tag(tag)).collect()
}

/// Group entries by their tag (entries without a tag go under `"_untagged"`).
pub fn x_jsonschemas_group_by_tag(
    entries: &[XJsonschemasTaggedEntry],
) -> std::collections::HashMap<String, Vec<&XJsonschemasTaggedEntry>> {
    let mut map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for e in entries {
        let key = e.tag.clone().unwrap_or_else(|| "_untagged".into());
        map.entry(key).or_default().push(e);
    }
    map
}

/// Compute a simple digest of a string (DJB2 hash).
pub fn x_jsonschemas_djb2_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// Deduplicate entries by key, keeping the first occurrence.
pub fn x_jsonschemas_dedup_entries(entries: Vec<XJsonschemasTaggedEntry>) -> Vec<XJsonschemasTaggedEntry> {
    let mut seen = std::collections::HashSet::new();
    entries.into_iter().filter(|e| seen.insert(e.key.clone())).collect()
}


/// Configuration manager for jsonschemas functionality.
pub struct JsonschemasConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl JsonschemasConfig {
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

    pub fn merge(&mut self, other: &JsonschemasConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for jsonschemas operations.
pub struct JsonschemasRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl JsonschemasRateTracker {
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

/// Validation result collector for jsonschemas.
pub struct JsonschemasValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl JsonschemasValidator {
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

    pub fn merge(&mut self, other: &JsonschemasValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for jsonschemas
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaJsonschemasRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaJsonschemasRingBuf {
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
pub struct XaJsonschemasCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaJsonschemasCounter {
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

impl Default for XaJsonschemasCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 101
// ---------------------------------------------------------------------------

/// Generic object pool `Xc101Pool<T>`.
pub struct Xc101Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc101Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc101PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc101Pool<T> {
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
    pub fn stats(&self) -> Xc101PoolStats {
        Xc101PoolStats {
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

impl<T> Default for Xc101Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc101Scheduler`.
pub struct Xc101Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc101Scheduler {
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

impl Default for Xc101Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_101 hash for the given byte slice.
pub fn xc_101_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_101 convention.
pub fn xc_101_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe19 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe19Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe19PipelineError {
    pub stage: Xe19Stage,
    pub message: String,
}

impl std::fmt::Display for Xe19PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe19Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe19Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError>>>,
    stage_names: Vec<Xe19Stage>,
}

impl Xe19Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe19Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe19Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe19Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe19Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
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

    pub fn compose(mut self, other: Xe19Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe19CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe19CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe19Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe19CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe19CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe19Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe19CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_19_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe19CacheEntry {
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

    fn xe_19_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe19CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_19_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
    Ok(data)
}

pub fn xe_19_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_19_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_19_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_19_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe19PipelineError> {
    Err(Xe19PipelineError {
        stage: Xe19Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #89
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf89Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf89TrieNode {
    children: std::collections::HashMap<char, Xf89TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf89Trie {
    root: Xf89TrieNode,
    count: usize,
}

impl Xf89Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf89TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf89TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf89TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf89BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf89BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 100).
pub struct Xh100SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh100SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 142 as u64,
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

/// A compact bit set supporting boolean operations (variant 100).
pub struct Xh100BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh100BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 100).
pub struct Xi100Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi100Deque<T> {
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
pub struct Xi100Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi100Interval {
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

/// A simple interval tree (variant 100).
pub struct Xi100IntervalTree {
    xi_intervals: Vec<Xi100Interval>,
}

impl Xi100IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi100Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi100Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi100Interval) -> Vec<&Xi100Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi100Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi100Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi100Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi100Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi100Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi100Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 99) ---

/// Disjoint set / union-find for crate 99.
pub struct Xj99UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj99UnionFind {
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

const XJ99_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 99.
pub struct Xj99BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj99BTreeNode<K, V>>>,
    len: usize,
}

struct Xj99BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj99BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj99BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ99_BTREE_ORDER - 1
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
        let mid = XJ99_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj99BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj99BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj99BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj99BTreeNode::xj_new_leaf();
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


// --- xk_100 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk100SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk100SegmentTree {
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
pub struct Xk100DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk100DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_99).
#[derive(Debug, Clone)]
pub struct Xl99Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl99Rope {
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

/// Suffix array for efficient string searching (xl_99).
#[derive(Debug, Clone)]
pub struct Xl99SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl99SuffixArray {
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


    // -- jsonschemas additional tests -------------------------------------------

    #[test]
    fn x_jsonschemas_validation_ok() {
        let r = x_jsonschemas_validate_string("hello", 100);
        assert!(r.is_ok());
        assert!(r.message().is_none());
    }

    #[test]
    fn x_jsonschemas_validation_empty() {
        let r = x_jsonschemas_validate_string("", 100);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("empty"));
    }

    #[test]
    fn x_jsonschemas_validation_too_long() {
        let r = x_jsonschemas_validate_string("abcdef", 3);
        assert!(!r.is_ok());
        assert!(r.message().unwrap().contains("max length"));
    }

    #[test]
    fn x_jsonschemas_validate_range_ok() {
        assert!(x_jsonschemas_validate_range(5, 1, 10).is_ok());
        assert!(x_jsonschemas_validate_range(1, 1, 10).is_ok());
        assert!(x_jsonschemas_validate_range(10, 1, 10).is_ok());
    }

    #[test]
    fn x_jsonschemas_validate_range_out() {
        assert!(!x_jsonschemas_validate_range(0, 1, 10).is_ok());
        assert!(!x_jsonschemas_validate_range(11, 1, 10).is_ok());
    }

    #[test]
    fn x_jsonschemas_tagged_entry_basic() {
        let e = XJsonschemasTaggedEntry::new("k", "v");
        assert_eq!(e.key, "k");
        assert_eq!(e.value, "v");
        assert!(e.tag.is_none());
    }

    #[test]
    fn x_jsonschemas_tagged_entry_with_tag() {
        let e = XJsonschemasTaggedEntry::new("k", "v").with_tag("important");
        assert!(e.matches_tag("important"));
        assert!(!e.matches_tag("other"));
    }

    #[test]
    fn x_jsonschemas_filter_by_tag_basic() {
        let entries = vec![
            XJsonschemasTaggedEntry::new("a", "1").with_tag("x"),
            XJsonschemasTaggedEntry::new("b", "2").with_tag("y"),
            XJsonschemasTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let filtered = x_jsonschemas_filter_by_tag(&entries, "x");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_jsonschemas_group_by_tag_basic() {
        let entries = vec![
            XJsonschemasTaggedEntry::new("a", "1").with_tag("x"),
            XJsonschemasTaggedEntry::new("b", "2"),
            XJsonschemasTaggedEntry::new("c", "3").with_tag("x"),
        ];
        let groups = x_jsonschemas_group_by_tag(&entries);
        assert_eq!(groups["x"].len(), 2);
        assert_eq!(groups["_untagged"].len(), 1);
    }

    #[test]
    fn x_jsonschemas_djb2_hash_deterministic() {
        let h1 = x_jsonschemas_djb2_hash("hello");
        let h2 = x_jsonschemas_djb2_hash("hello");
        assert_eq!(h1, h2);
        assert_ne!(x_jsonschemas_djb2_hash("hello"), x_jsonschemas_djb2_hash("world"));
    }

    #[test]
    fn x_jsonschemas_dedup_entries_basic() {
        let entries = vec![
            XJsonschemasTaggedEntry::new("a", "1"),
            XJsonschemasTaggedEntry::new("a", "2"),
            XJsonschemasTaggedEntry::new("b", "3"),
        ];
        let deduped = x_jsonschemas_dedup_entries(entries);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].value, "1");
    }

    #[test]
    fn x_jsonschemas_validation_result_warning() {
        let w = XJsonschemasValidationResult::Warning("low disk".into());
        assert!(!w.is_ok());
        assert_eq!(w.message(), Some("low disk"));
    }

    #[test]
    fn x_jsonschemas_filter_by_tag_empty() {
        let entries: Vec<XJsonschemasTaggedEntry> = vec![];
        assert!(x_jsonschemas_filter_by_tag(&entries, "x").is_empty());
    }

    #[test]
    fn x_jsonschemas_tagged_entry_no_tag_match() {
        let e = XJsonschemasTaggedEntry::new("k", "v");
        assert!(!e.matches_tag("any"));
    }


    #[test]
    fn jsonschemas_config_new() {
        let cfg = JsonschemasConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn jsonschemas_config_set_get() {
        let mut cfg = JsonschemasConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn jsonschemas_config_remove() {
        let mut cfg = JsonschemasConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn jsonschemas_config_keys_sorted() {
        let mut cfg = JsonschemasConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn jsonschemas_config_bump_version() {
        let mut cfg = JsonschemasConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn jsonschemas_config_clear() {
        let mut cfg = JsonschemasConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn jsonschemas_config_merge() {
        let mut cfg1 = JsonschemasConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = JsonschemasConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn jsonschemas_config_disable() {
        let mut cfg = JsonschemasConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn jsonschemas_rate_tracker_empty() {
        let rt = JsonschemasRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn jsonschemas_rate_tracker_record() {
        let mut rt = JsonschemasRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn jsonschemas_rate_tracker_prune() {
        let mut rt = JsonschemasRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn jsonschemas_validator_valid() {
        let v = JsonschemasValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn jsonschemas_validator_errors() {
        let mut v = JsonschemasValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn jsonschemas_validator_clear() {
        let mut v = JsonschemasValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn jsonschemas_validator_merge() {
        let mut v1 = JsonschemasValidator::new();
        v1.add_error("e1");
        let mut v2 = JsonschemasValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn jsonschemas_rate_tracker_clear() {
        let mut rt = JsonschemasRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for jsonschemas
    #[test]
    fn xa_jsonschemas_ring_new() {
        let rb = super::XaJsonschemasRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_jsonschemas_ring_push_len() {
        let mut rb = super::XaJsonschemasRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_jsonschemas_ring_wrap() {
        let mut rb = super::XaJsonschemasRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_jsonschemas_ring_mean_empty() {
        let rb = super::XaJsonschemasRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_jsonschemas_ring_mean_values() {
        let mut rb = super::XaJsonschemasRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_jsonschemas_ring_min_max() {
        let mut rb = super::XaJsonschemasRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_jsonschemas_ring_iter() {
        let mut rb = super::XaJsonschemasRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_jsonschemas_counter_new() {
        let c = super::XaJsonschemasCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_jsonschemas_counter_inc() {
        let mut c = super::XaJsonschemasCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_jsonschemas_counter_inc_by() {
        let mut c = super::XaJsonschemasCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_jsonschemas_counter_reset() {
        let mut c = super::XaJsonschemasCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_jsonschemas_counter_clear() {
        let mut c = super::XaJsonschemasCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_jsonschemas_counter_default() {
        let c = super::XaJsonschemasCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 101 ----

    #[test]
    fn xc_101_pool_new_empty() {
        let pool: super::Xc101Pool<i32> = super::Xc101Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_101_pool_release_acquire() {
        let mut pool = super::Xc101Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_101_pool_acquire_empty() {
        let mut pool: super::Xc101Pool<i32> = super::Xc101Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_101_pool_full() {
        let mut pool = super::Xc101Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_101_pool_drain() {
        let mut pool = super::Xc101Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_101_pool_stats() {
        let mut pool = super::Xc101Pool::new(8);
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
    fn xc_101_pool_clear() {
        let mut pool = super::Xc101Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_101_pool_shrink() {
        let mut pool = super::Xc101Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_101_pool_default() {
        let pool: super::Xc101Pool<String> = super::Xc101Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_101_pool_extend() {
        let mut pool = super::Xc101Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_101_pool_retain() {
        let mut pool = super::Xc101Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_101_scheduler_round_robin() {
        let mut sched = super::Xc101Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_101_scheduler_empty() {
        let mut sched = super::Xc101Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_101_scheduler_reset() {
        let mut sched = super::Xc101Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_101_scheduler_add_remove() {
        let mut sched = super::Xc101Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_101_scheduler_targets() {
        let sched = super::Xc101Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_101_hash_empty() {
        assert_eq!(super::xc_101_hash(b""), 5381);
    }

    #[test]
    fn xc_101_hash_data() {
        let h = super::xc_101_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_101_hash(b"hello"), h);
    }

    #[test]
    fn xc_101_reverse_str() {
        assert_eq!(super::xc_101_reverse("abc"), "cba");
        assert_eq!(super::xc_101_reverse(""), "");
    }


    #[test]
    fn xe_19_pipeline_empty() {
        let p = super::Xe19Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_19_pipeline_parse_stage() {
        let p = super::Xe19Pipeline::new()
            .add_parse(super::xe_19_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_19_pipeline_transform_double() {
        let p = super::Xe19Pipeline::new()
            .add_transform(super::xe_19_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_19_pipeline_validate_reverse() {
        let p = super::Xe19Pipeline::new()
            .add_validate(super::xe_19_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_19_pipeline_emit_filter() {
        let p = super::Xe19Pipeline::new()
            .add_emit(super::xe_19_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_19_pipeline_multi_stage() {
        let p = super::Xe19Pipeline::new()
            .add_parse(super::xe_19_pipeline_identity)
            .add_transform(super::xe_19_pipeline_double)
            .add_validate(super::xe_19_pipeline_reverse)
            .add_emit(super::xe_19_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_19_pipeline_error_propagation() {
        let p = super::Xe19Pipeline::new()
            .add_parse(super::xe_19_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe19Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_19_pipeline_compose() {
        let p1 = super::Xe19Pipeline::new()
            .add_parse(super::xe_19_pipeline_identity);
        let p2 = super::Xe19Pipeline::new()
            .add_transform(super::xe_19_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_19_pipeline_error_display() {
        let e = super::Xe19PipelineError {
            stage: super::Xe19Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_19_cache_put_get() {
        let mut c = super::Xe19Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_19_cache_miss() {
        let mut c: super::Xe19Cache<&str, i32> = super::Xe19Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_19_cache_ttl_expiry() {
        let mut c = super::Xe19Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_19_cache_evict() {
        let mut c = super::Xe19Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_19_cache_capacity() {
        let mut c = super::Xe19Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_19_cache_stats() {
        let mut c = super::Xe19Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_19_cache_clear() {
        let mut c = super::Xe19Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #89 --

    #[test]
    fn xf89_trie_insert_search() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf89_trie_starts_with() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf89_trie_remove() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf89_trie_word_count() {
        let mut t = Xf89Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf89_trie_longest_prefix() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf89_trie_all_words() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf89_trie_autocomplete() {
        let mut t = Xf89Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf89_trie_empty_search() {
        let t = Xf89Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf89_bloom_add_contains() {
        let mut bf = Xf89BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf89_bloom_probably_absent() {
        let bf = Xf89BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf89_bloom_false_positive_rate() {
        let mut bf = Xf89BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf89_bloom_clear() {
        let mut bf = Xf89BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf89_bloom_union() {
        let mut a = Xf89BloomFilter::xf_new(512, 2);
        let mut b = Xf89BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf89_bloom_intersection_estimate() {
        let mut a = Xf89BloomFilter::xf_new(512, 2);
        let mut b = Xf89BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf89_bloom_union_size_mismatch() {
        let a = Xf89BloomFilter::xf_new(256, 2);
        let b = Xf89BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh100_skip_insert_contains() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh100_skip_remove() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh100_skip_len() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh100_skip_range_query() {
        let mut sl = super::Xh100SkipList::xh_new(4);
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
    fn xh100_skip_floor_ceiling() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh100_skip_rank() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh100_skip_empty() {
        let sl = super::Xh100SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh100_skip_duplicates() {
        let mut sl = super::Xh100SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh100_bitset_set_test() {
        let mut bs = super::Xh100BitSet::xh_new(256);
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
    fn xh100_bitset_clear_count() {
        let mut bs = super::Xh100BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh100_bitset_and_or_xor() {
        let mut a = super::Xh100BitSet::xh_new(128);
        let mut b = super::Xh100BitSet::xh_new(128);
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
    fn xh100_bitset_iter_ones() {
        let mut bs = super::Xh100BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh100_bitset_first_last() {
        let mut bs = super::Xh100BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh100_bitset_empty() {
        let bs = super::Xh100BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi100_deque_push_pop_back() {
        let mut dq = super::Xi100Deque::xi_new(4);
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
    fn xi100_deque_push_pop_front() {
        let mut dq = super::Xi100Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi100_deque_mixed_ops() {
        let mut dq = super::Xi100Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi100_deque_get_and_split() {
        let mut dq = super::Xi100Deque::xi_new(8);
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
    fn xi100_deque_rotate_left() {
        let mut dq = super::Xi100Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi100_deque_rotate_right() {
        let mut dq = super::Xi100Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi100_deque_grow() {
        let mut dq = super::Xi100Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi100_deque_empty() {
        let dq = super::Xi100Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi100_interval_tree_insert_query() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi100Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi100Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi100_interval_tree_overlap() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi100Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi100Interval::xi_new(12, 20));
        let q = super::Xi100Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi100_interval_tree_remove() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi100Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi100_interval_tree_gaps() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi100Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi100Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi100Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi100Interval::xi_new(8, 10));
    }

    #[test]
    fn xi100_interval_tree_merge() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi100Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi100Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi100Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi100Interval::xi_new(10, 15));
    }

    #[test]
    fn xi100_interval_tree_all() {
        let mut tree = super::Xi100IntervalTree::xi_new();
        tree.xi_insert(super::Xi100Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi100Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi100_interval_tree_empty() {
        let tree = super::Xi100IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi100_interval_tree_contains_point() {
        let iv = super::Xi100Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 99) ---

    #[test]
    fn xj_99_uf_make_and_find() {
        let mut uf = super::Xj99UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_99_uf_union_connected() {
        let mut uf = super::Xj99UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_99_uf_component_count() {
        let mut uf = super::Xj99UnionFind::xj_new();
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
    fn xj_99_uf_component_size() {
        let mut uf = super::Xj99UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_99_uf_largest_component() {
        let mut uf = super::Xj99UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_99_uf_many_elements() {
        let mut uf = super::Xj99UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_99_uf_separate_components() {
        let mut uf = super::Xj99UnionFind::xj_new();
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
    fn xj_99_uf_path_compression() {
        let mut uf = super::Xj99UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_99_bt_insert_get() {
        let mut bt = super::Xj99BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_99_bt_contains_len() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_99_bt_replace() {
        let mut bt = super::Xj99BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_99_bt_remove() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_99_bt_keys_values() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_99_bt_range() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_99_bt_min_max() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_99_bt_many_inserts() {
        let mut bt = super::Xj99BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_100 segment tree tests ---

    #[test]
    fn xk_100_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_100_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk100SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_100_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_100_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_100_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_100_st_single_element() {
        let data = vec![42];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_100_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk100SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_100_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk100SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_100 disjoint intervals tests ---

    #[test]
    fn xk_100_di_add_and_count() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_100_di_merge_overlap() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_100_di_contains() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_100_di_remove() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_100_di_covered_length() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_100_di_gaps() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_100_di_merge_adjacent() {
        let mut di = super::Xk100DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_100_di_empty() {
        let di = super::Xk100DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_99_rope_new_empty() {
        let rope = super::Xl99Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_99_rope_from_str() {
        let rope = super::Xl99Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_99_rope_insert_at() {
        let mut rope = super::Xl99Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_99_rope_delete_range() {
        let mut rope = super::Xl99Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_99_rope_char_at() {
        let rope = super::Xl99Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_99_rope_split_concat() {
        let rope = super::Xl99Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_99_rope_line_count() {
        let rope = super::Xl99Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_99_rope_line_at() {
        let rope = super::Xl99Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_99_sa_build_and_search() {
        let sa = super::Xl99SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_99_sa_count() {
        let sa = super::Xl99SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_99_sa_longest_repeated() {
        let sa = super::Xl99SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_99_sa_all_positions() {
        let sa = super::Xl99SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_99_sa_len() {
        let sa = super::Xl99SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_99_sa_empty() {
        let sa = super::Xl99SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_99_rope_slice() {
        let rope = super::Xl99Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_99_sa_search_start() {
        let sa = super::Xl99SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
