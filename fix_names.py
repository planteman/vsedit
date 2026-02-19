import glob, json, sys
renames = json.loads(sys.stdin.read())
lib_files = sorted(glob.glob("crates/vsedit-*/src/lib.rs"))
for lib_path in lib_files:
    with open(lib_path, 'r') as f:
        content = f.read()
    changed = False
    for old_name, new_name, first_field, prefix in renames:
        marker = f"pub struct {old_name} {{\n    pub {first_field}"
        if marker not in content: continue
        changed = True
        content = content.replace(marker, f"pub struct {new_name} {{\n    pub {first_field}")
        content = content.replace(f"impl {old_name} {{\n    pub fn {prefix}", f"impl {new_name} {{\n    pub fn {prefix}")
        content = content.replace(f"impl Default for {old_name} {{\n    fn default() -> Self {{\n        Self {{\n            {first_field}", f"impl Default for {new_name} {{\n    fn default() -> Self {{\n        Self {{\n            {first_field}")
        content = content.replace(f'"{old_name}[{prefix}]:', f'"{new_name}[{prefix}]:')
        lines = content.split("\n")
        new_lines = []
        for i, line in enumerate(lines):
            if f"impl std::fmt::Display for {old_name}" in line:
                lookahead = "".join(lines[i:i+5])
                if first_field in lookahead: line = line.replace(old_name, new_name)
            if f'write!(f, "{old_name}(' in line and i > 0:
                lookback = "".join(lines[max(0,i-3):i])
                if new_name in lookback: line = line.replace(f"{old_name}(", f"{new_name}(")
            new_lines.append(line)
        content = "\n".join(new_lines)
        lines = content.split("\n")
        new_lines = []
        in_test = False
        for line in lines:
            if f"fn test_{prefix}" in line: in_test = True
            elif in_test and line.strip().startswith("fn test_") and f"test_{prefix}" not in line: in_test = False
            elif in_test and line == "}": in_test = False
            if in_test: line = line.replace(f"{old_name}::", f"{new_name}::")
            new_lines.append(line)
        content = "\n".join(new_lines)
    if changed:
        with open(lib_path, "w") as f:
            f.write(content)
print("Fixed")
