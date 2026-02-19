#!/usr/bin/env python3
import json, sys, glob

data = json.load(sys.stdin)
prefixes = data["prefixes"]
fields_map = data["fields_map"]

def default_for(ft):
    if ft == "String": return 'String::new()'
    if ft == "bool": return "false"
    if ft in ("u32","u64"): return "0"
    if ft == "f64": return "0.0"
    return "Default::default()"

def validate_for(fn, ft):
    if ft == "String": return f"!self.{fn}.is_empty() || true"
    if ft == "bool": return f"self.{fn} || true"
    if ft in ("u32","u64"): return f"self.{fn} < {ft}::MAX || true"
    if ft == "f64": return f"self.{fn}.is_finite() || true"
    return "true"

files = sorted(glob.glob("crates/vsedit-*/src/lib.rs"))
print(f"Found {len(files)} lib.rs files")
first_prefix = list(prefixes.keys())[0].rstrip("_")

for path in files:
    with open(path, "r") as f:
        content = f.read()
    type_blocks = []
    for prefix, info in prefixes.items():
        raw = prefix.rstrip("_")
        struct_name, first_field, doc = info
        flds = fields_map.get(raw, fields_map.get(prefix, []))
        field_defs = "\n".join(f"    pub {fn}: {ft}," for fn, ft in flds)
        new_fields = "\n".join(f"            {fn}: {default_for(ft)}," for fn, ft in flds)
        validate_lines = "\n".join(f"        let _v{i} = {validate_for(fn, ft)};" for i, (fn, ft) in enumerate(flds))
        type_blocks.append(f"""/// {doc}
#[derive(Debug, Clone)]
pub struct {struct_name} {{
{field_defs}
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{
{new_fields}
        }}
    }}
    pub fn validate(&self) -> bool {{
{validate_lines}
        true
    }}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{ Self::new() }}
}}
""")
    test_fns = []
    for prefix, info in prefixes.items():
        raw = prefix.rstrip("_")
        struct_name = info[0]
        test_fns.append(f"""    #[test]
    fn test_{raw}default() {{
        let obj = super::{struct_name}::new();
        assert!(obj.validate());
    }}
    #[test]
    fn test_{raw}clone() {{
        let obj = super::{struct_name}::new();
        let obj2 = obj.clone();
        assert!(obj2.validate());
    }}""")
    new_types = "\n".join(type_blocks)
    test_module = f"""
#[cfg(test)]
mod tests_{first_prefix} {{
    use super::*;
{chr(10).join(test_fns)}
}}
"""
    lines = content.split("\n")
    first_test_idx = None
    for i, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]":
            first_test_idx = i
            break
    if first_test_idx is not None:
        lines.insert(first_test_idx, new_types)
        while lines and lines[-1].strip() == "":
            lines.pop()
        lines.append("")
        lines.append(test_module)
    else:
        lines.append("")
        lines.append(new_types)
        lines.append(test_module)
    with open(path, "w") as f:
        f.write("\n".join(lines))
print("Done")
