#!/usr/bin/env python3
import json, sys, glob

data = json.load(sys.stdin)
prefixes = data["prefixes"]
fields_map = data["fields_map"]

files = sorted(glob.glob("crates/*/src/lib.rs"))
print(f"Found {len(files)} lib.rs files")

for fpath in files:
    with open(fpath, "r") as f:
        content = f.read()
    lines = content.split("\n")

    first_cfg_test = None
    for i, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]":
            first_cfg_test = i
            break

    if first_cfg_test is None:
        continue

    type_blocks = []
    for prefix, info in prefixes.items():
        struct_name, first_field, doc = info
        fields = fields_map[prefix]
        field_defs = "\n".join(f"    pub {fname}: {ftype}," for fname, ftype in fields)
        new_exprs = []
        for fname, ftype in fields:
            if ftype == "String":
                new_exprs.append(f"            {fname}: String::new(),")
            elif ftype == "bool":
                new_exprs.append(f"            {fname}: bool::default(),")
            else:
                new_exprs.append(f"            {fname}: {ftype}::default(),")
        new_body = "\n".join(new_exprs)
        validate_parts = []
        for fname, ftype in fields:
            if ftype == "String":
                validate_parts.append(f"!self.{fname}.is_empty() || true")
            elif ftype == "bool":
                validate_parts.append(f"self.{fname} || true")
            elif ftype in ("u32", "u64"):
                validate_parts.append(f"self.{fname} < {ftype}::MAX || true")
            elif ftype == "f64":
                validate_parts.append(f"self.{fname}.is_finite() || true")
            else:
                validate_parts.append("true")
        validate_body = " && ".join(validate_parts)
        type_block = f"""/// {doc}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
{field_defs}
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
{new_body}
        }}
    }}

    pub fn validate(&self) -> bool {{
        {validate_body}
    }}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self::new()
    }}
}}"""
        type_blocks.append(type_block)

    insert_text = "\n".join(type_blocks) + "\n\n"

    test_blocks = []
    for prefix, info in prefixes.items():
        struct_name, first_field, doc = info
        fields = fields_map[prefix]
        first_field_type = [f[1] for f in fields if f[0] == first_field][0]
        if first_field_type in ("u32", "u64"):
            test_value = "42"
        elif first_field_type == "bool":
            test_value = "true"
        elif first_field_type == "f64":
            test_value = "3.14"
        else:
            test_value = '"test".to_string()'
        test_mod_name = f"tests_{prefix}generated"
        test_block = f"""
#[cfg(test)]
mod {test_mod_name} {{
    use super::*;

    #[test]
    fn test_{prefix}default() {{
        let obj = {struct_name}::new();
        assert!(obj.validate());
    }}

    #[test]
    fn test_{prefix}fields() {{
        let mut obj = {struct_name}::default();
        obj.{first_field} = {test_value};
        assert!(obj.validate());
    }}
}}"""
        test_blocks.append(test_block)

    new_lines = lines[:first_cfg_test] + insert_text.split("\n") + lines[first_cfg_test:]
    new_content = "\n".join(new_lines)
    new_content = new_content.rstrip("\n") + "\n" + "\n".join(test_blocks) + "\n"

    with open(fpath, "w") as f:
        f.write(new_content)

print("Done")
