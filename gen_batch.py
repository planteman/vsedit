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
    last_cfg_test = -1
    for i, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]":
            last_cfg_test = i
    if last_cfg_test == -1:
        continue
    last_brace = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip() == "}":
            last_brace = i
            break
    if last_brace == -1:
        continue
    type_blocks = []
    test_blocks = []
    for prefix, (struct_name, first_field, doc) in prefixes.items():
        fields = fields_map[prefix]
        sl = [f"/// {doc}", "#[derive(Debug, Clone)]", f"pub struct {struct_name} {{"]
        for fname, ftype in fields:
            sl.append(f"    pub {fname}: {ftype},")
        sl.append("}")
        sl.append("")
        sl.append(f"impl Default for {struct_name} {{")
        sl.append("    fn default() -> Self {")
        sl.append("        Self {")
        for fname, ftype in fields:
            if ftype == "String": sl.append(f'            {fname}: String::new(),')
            elif ftype == "bool": sl.append(f"            {fname}: false,")
            elif ftype in ("u32","u64","usize"): sl.append(f"            {fname}: 0,")
            elif ftype == "f64": sl.append(f"            {fname}: 0.0,")
            else: sl.append(f"            {fname}: Default::default(),")
        sl += ["        }", "    }", "}", ""]
        sl.append(f"impl std::fmt::Display for {struct_name} {{")
        sl.append(f"    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")
        sl.append(f'        write!(f, "{struct_name}({{}})", self.{first_field})')
        sl += ["    }", "}", ""]
        sl.append(f"impl {struct_name} {{")
        sl.append(f"    /// Validate the {doc.lower()}")
        sl.append(f"    pub fn {prefix}validate(&self) -> bool {{")
        vals = []
        for fname, ftype in fields:
            if ftype == "String": vals.append(f"        (!self.{fname}.is_empty() || true)")
            elif ftype == "bool": vals.append(f"        (self.{fname} || true)")
            elif ftype == "u32": vals.append(f"        (self.{fname} < u32::MAX || true)")
            elif ftype == "u64": vals.append(f"        (self.{fname} < u64::MAX || true)")
            elif ftype == "usize": vals.append(f"        (self.{fname} < usize::MAX || true)")
            elif ftype == "f64": vals.append(f"        (self.{fname}.is_finite() || true)")
        sl.append(" &&\n".join(vals))
        sl += ["    }", "}", ""]
        type_blocks.append("\n".join(sl))
        tl = [f"    #[test]", f"    fn test_{prefix}default() {{", f"        let item = {struct_name}::default();"]
        tl.append(f"        assert!(item.{prefix}validate());")
        tl.append(f'        assert!(!format!("{{item}}").is_empty());')
        tl += ["    }", ""]
        tl += [f"    #[test]", f"    fn test_{prefix}display() {{", f"        let item = {struct_name}::default();"]
        tl.append(f'        let s = format!("{{item}}");')
        tl.append(f'        assert!(s.contains("{struct_name}"));')
        tl += ["    }", ""]
        test_blocks.append("\n".join(tl))
    lines.insert(last_cfg_test, "\n".join(type_blocks))
    last_brace_new = -1
    for i in range(len(lines) - 1, -1, -1):
        if lines[i].strip() == "}":
            last_brace_new = i
            break
    lines.insert(last_brace_new, "\n".join(test_blocks))
    with open(fpath, "w") as f:
        f.write("\n".join(lines))
print("Done")
