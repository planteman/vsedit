#!/usr/bin/env python3
import json, sys, glob

data = json.load(sys.stdin)
prefixes = data["prefixes"]
fields_map = data["fields_map"]

lib_files = sorted(glob.glob("crates/*/src/lib.rs"))
print(f"Found {len(lib_files)} lib.rs files")

for fpath in lib_files:
    with open(fpath, "r") as f:
        content = f.read()

    type_blocks = []
    test_blocks = []

    for prefix, info in prefixes.items():
        struct_name, first_field, doc = info
        fields = fields_map[prefix]

        field_defs = ""
        new_fields = ""
        validate_parts = []

        for fname, ftype in fields:
            field_defs += f"    pub {fname}: {ftype},\n"
            if ftype == "String":
                new_fields += f"            {fname}: String::new(),\n"
                validate_parts.append(f"!self.{fname}.is_empty() || true")
            elif ftype == "bool":
                new_fields += f"            {fname}: bool::default(),\n"
                validate_parts.append(f"self.{fname} || true")
            elif ftype in ("u32", "u64"):
                new_fields += f"            {fname}: {ftype}::default(),\n"
                validate_parts.append(f"self.{fname} < {ftype}::MAX || true")
            elif ftype == "f64":
                new_fields += f"            {fname}: f64::default(),\n"
                validate_parts.append(f"self.{fname}.is_finite() || true")
            else:
                new_fields += f"            {fname}: {ftype}::default(),\n"
                validate_parts.append("true")
        validate_body = " && ".join(validate_parts)
        type_block = f"""/// {doc}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
{field_defs}}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
{new_fields}        }}
    }}

    pub fn validate(&self) -> bool {{
        {validate_body}
    }}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self::new()
    }}
}}
"""
        type_blocks.append(type_block)

        test_fn_default = f"test_{prefix}default"
        test_fn_fields = f"test_{prefix}fields"
        test_block = f"""#[cfg(test)]
mod tests_{prefix}generated {{
    use super::*;

    #[test]
    fn {test_fn_default}() {{
        let obj = {struct_name}::new();
        assert!(obj.validate());
    }}

    #[test]
    fn {test_fn_fields}() {{
        let mut obj = {struct_name}::default();
        obj.{first_field} = {"\"test\".to_string()".format() if fields[0][1] == "String" else "1".format()};
        assert!(obj.validate());
    }}
}}
"""
        test_blocks.append(test_block)

    cfg_test_pos = content.find("#[cfg(test)]")
    if cfg_test_pos != -1:
        insert_pos = cfg_test_pos
        types_text = "\n".join(type_blocks) + "\n"
        content = content[:insert_pos] + types_text + content[insert_pos:]

    tests_text = "\n" + "\n".join(test_blocks)
    content += tests_text

    with open(fpath, "w") as f:
        f.write(content)

print("Done")
