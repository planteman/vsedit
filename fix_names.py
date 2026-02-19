#!/usr/bin/env python3
"""Fix struct name conflicts across all lib.rs files."""
import json, sys, glob

renames = json.load(sys.stdin)  # [[old_name, new_name, first_field, prefix], ...]

files = sorted(glob.glob("crates/*/src/lib.rs"))
print(f"Fixing names in {len(files)} files")

for fpath in files:
    with open(fpath, "r") as f:
        content = f.read()
    
    modified = False
    for old_name, new_name, first_field, prefix in renames:
        if old_name in content:
            content = content.replace(f"pub struct {old_name} ", f"pub struct {new_name} ")
            content = content.replace(f"impl Default for {old_name} ", f"impl Default for {new_name} ")
            content = content.replace(f"impl std::fmt::Display for {old_name} ", f"impl std::fmt::Display for {new_name} ")
            content = content.replace(f"impl {old_name} ", f"impl {new_name} ")
            content = content.replace(f'write!(f, "{old_name}({{}})", self.{first_field})', f'write!(f, "{new_name}({{}})", self.{first_field})')
            content = content.replace(f"let item = {old_name}::default();", f"let item = {new_name}::default();")
            content = content.replace(f'assert!(s.contains("{old_name}"));', f'assert!(s.contains("{new_name}"));')
            modified = True
    
    if modified:
        with open(fpath, "w") as f:
            f.write(content)

print("Done")
