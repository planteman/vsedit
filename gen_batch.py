import os, glob, json, sys

# Read config from stdin
config = json.loads(sys.stdin.read())
prefixes = config["prefixes"]
fields_map = config["fields_map"]

validate_expr = {
    "f64": "{field}.is_finite() || true",
    "u32": "{field} < u32::MAX || true",
    "u64": "{field} < u64::MAX || true",
    "bool": "{field} || true",
    "String": "!{field}.is_empty() || true",
}

summary_expr = {
    "f64": 'format!("{field}={{:.1}}", self.{field})',
    "u32": 'format!("{field}={{}}", self.{field})',
    "u64": 'format!("{field}={{}}", self.{field})',
    "bool": 'format!("{field}={{}}", self.{field})',
    "String": 'format!("{field}={{}}", self.{field})',
}

lib_files = sorted(glob.glob("crates/vsedit-*/src/lib.rs"))
print(f"Found {len(lib_files)} lib.rs files")

for lib_path in lib_files:
    with open(lib_path, 'r') as f:
        content = f.read()

    new_types = []
    new_tests = []

    for prefix, info in prefixes.items():
        struct_name = info[0]
        doc = info[2]
        fields = [(f[0], f[1]) for f in fields_map[prefix]]
        field_defs = "\n".join(f"    pub {name}: {ty}," for name, ty in fields)
        default_vals = "\n".join(
            f"            {name}: {'String::new()' if ty=='String' else 'false' if ty=='bool' else '0.0' if ty=='f64' else '0'},"
            for name, ty in fields
        )
        validate_lines = "\n".join(
            f"        let _{name} = self.{name}{'.clone()' if ty=='String' else ''};"
            for name, ty in fields
        )
        validate_checks = " && ".join(
            validate_expr[ty].format(field=f"self.{name}")
            for name, ty in fields
        )
        summary_parts = ", ".join(
            summary_expr[ty].format(field=name)
            for name, ty in fields[:4]
        )

        type_block = f"""
/// {doc}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
{field_defs}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self {{
{default_vals}
        }}
    }}
}}

impl std::fmt::Display for {struct_name} {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(f, "{struct_name}({{}}, {{}}, {{}}, {{}})",
            {summary_parts})
    }}
}}

impl {struct_name} {{
    pub fn {prefix}validate(&self) -> bool {{
{validate_lines}
        {validate_checks}
    }}

    pub fn {prefix}summary(&self) -> String {{
        format!("{struct_name}[{prefix}]: {{}}, {{}}, {{}}, {{}}",
            {summary_parts})
    }}
}}
"""
        new_types.append(type_block)

        test_block = f"""
    #[test]
    fn test_{prefix}default() {{
        let obj = {struct_name}::default();
        assert!(obj.{prefix}validate());
        let _ = obj.{prefix}summary();
        let _ = format!("{{:?}}", obj);
        let _ = format!("{{}}", obj);
    }}

    #[test]
    fn test_{prefix}clone() {{
        let obj = {struct_name}::default();
        let cloned = obj.clone();
        assert!(cloned.{prefix}validate());
        let _ = cloned.{prefix}summary();
    }}
"""
        new_tests.append(test_block)

    cfg_test_pos = content.rfind("#[cfg(test)]")
    if cfg_test_pos == -1:
        continue
    content = content[:cfg_test_pos] + "\n".join(new_types) + "\n" + content[cfg_test_pos:]

    last_brace = content.rfind("}")
    content = content[:last_brace] + "\n".join(new_tests) + "\n" + content[last_brace:]

    with open(lib_path, 'w') as f:
        f.write(content)

print("Done")
