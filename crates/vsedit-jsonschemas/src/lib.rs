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
        write!(f, "{}: {}", self.name, self.schema_type)
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
}
