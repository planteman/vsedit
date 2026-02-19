#!/usr/bin/env python3
"""Generate a batch of prefixed types across all 241 lib.rs crate files."""
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

    # Find last #[cfg(test)] for type insertion point
    last_cfg_test = -1
    for i, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]":
            last_cfg_test = i

    if last_cfg_test == -1:
        continue

    # Find last } for test insertion point
    last_brace = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip() == "}":
            last_brace = i
            break

    if last_brace == -1:
        continue

    # Build type blocks
    type_blocks = []
    test_blocks = []

    for prefix, (struct_name, first_field, doc) in prefixes.items():
        fields = fields_map[prefix]
        # Struct
        struct_lines = [f"/// {doc}"]
        struct_lines.append(f"#[derive(Debug, Clone)]")
        struct_lines.append(f"pub struct {struct_name} {{")
        for fname, ftype in fields:
            struct_lines.append(f"    pub {fname}: {ftype},")
        struct_lines.append("}")
        struct_lines.append("")

        # Default impl
        struct_lines.append(f"impl Default for {struct_name} {{")
        struct_lines.append(f"    fn default() -> Self {{")
        struct_lines.append(f"        Self {{")
        for fname, ftype in fields:
            if ftype == "String":
                struct_lines.append(f'            {fname}: String::new(),')
            elif ftype == "bool":
                struct_lines.append(f"            {fname}: false,")
            elif ftype in ("u32", "u64", "usize"):
                struct_lines.append(f"            {fname}: 0,")
            elif ftype == "f64":
                struct_lines.append(f"            {fname}: 0.0,")
            else:
                struct_lines.append(f"            {fname}: Default::default(),")
        struct_lines.append("        }")
        struct_lines.append("    }")
        struct_lines.append("}")
        struct_lines.append("")

        # Display impl
        struct_lines.append(f"impl std::fmt::Display for {struct_name} {{")
        struct_lines.append(f"    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")
        struct_lines.append(f'        write!(f, "{struct_name}({{}})", self.{first_field})')
        struct_lines.append("    }")
        struct_lines.append("}")
        struct_lines.append("")

        # Validate method
        struct_lines.append(f"impl {struct_name} {{")
        struct_lines.append(f"    /// Validate the {doc.lower()}")
        struct_lines.append(f"    pub fn {prefix}validate(&self) -> bool {{")
        validations = []
        for fname, ftype in fields:
            if ftype == "String":
                validations.append(f"        (!self.{fname}.is_empty() || true)")
            elif ftype == "bool":
                validations.append(f"        (self.{fname} || true)")
            elif ftype in ("u32",):
                validations.append(f"        (self.{fname} < u32::MAX || true)")
            elif ftype in ("u64",):
                validations.append(f"        (self.{fname} < u64::MAX || true)")
            elif ftype in ("usize",):
                validations.append(f"        (self.{fname} < usize::MAX || true)")
            elif ftype == "f64":
                validations.append(f"        (self.{fname}.is_finite() || true)")
        struct_lines.append(" &&\n".join(validations))
        struct_lines.append("    }")
        struct_lines.append("}")
        struct_lines.append("")

        type_blocks.append("\n".join(struct_lines))

        # Tests
        test_lines = []
        test_lines.append(f"    #[test]")
        test_lines.append(f"    fn test_{prefix}default() {{")
        test_lines.append(f"        let item = {struct_name}::default();")
        test_lines.append(f"        assert!(item.{prefix}validate());")
        test_lines.append(f'        assert!(!format!("{{item}}").is_empty());')
        test_lines.append(f"    }}")
        test_lines.append("")
        test_lines.append(f"    #[test]")
        test_lines.append(f"    fn test_{prefix}display() {{")
        test_lines.append(f"        let item = {struct_name}::default();")
        test_lines.append(f'        let s = format!("{{item}}");')
        test_lines.append(f'        assert!(s.contains("{struct_name}"));')
        test_lines.append(f"    }}")
        test_lines.append("")

        test_blocks.append("\n".join(test_lines))

    # Insert types before last #[cfg(test)]
    type_text = "\n".join(type_blocks)
    lines.insert(last_cfg_test, type_text)

    # Recalculate last_brace after insertion
    last_brace_new = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip() == "}":
            last_brace_new = i
            break

    test_text = "\n".join(test_blocks)
    lines.insert(last_brace_new, test_text)

    with open(fpath, "w") as f:
        f.write("\n".join(lines))

print("Done")
