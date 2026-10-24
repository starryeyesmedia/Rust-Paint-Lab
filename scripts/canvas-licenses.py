#!/usr/bin/env python3
"""Generate a lockfile inventory or Windows canvas notices using Cargo metadata.

No network fetching beyond Cargo's normal verified registry downloads. Four
upstream notices omitted by their crate archives are checked into docs/licenses.
"""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SUPPLEMENTAL = {
    ("gl_generator", "0.14.0"): ["gl_generator-LICENSE.txt"],
    ("khronos_api", "3.1.0"): ["khronos_api-LICENSE.txt"],
    ("profiling", "1.0.18"): ["profiling-LICENSE-APACHE.txt", "profiling-LICENSE-MIT.txt"],
    ("spirv", "0.4.0+sdk-1.4.341.0"): ["spirv-LICENSE.txt"],
}

SUPPLEMENTAL.update({
    ("clipboard-win","5.4.3"): ["clipboard.txt"],
    ("spirv","0.3.0+sdk-1.3.268.0"): ["spirv27.txt"],
    ("hexf-parse","0.2.1"): ["CC0-1.0.txt"],
    ("gpu-alloc","0.6.2"): ["alloc-APACHE.txt","alloc-MIT.txt"],
    ("gpu-alloc-types","0.3.1"): ["alloc-APACHE.txt","alloc-MIT.txt"],
    ("gpu-descriptor","0.3.2"): ["descriptor-APACHE.txt","descriptor-MIT.txt"],
    ("gpu-descriptor-types","0.2.0"): ["descriptor-APACHE.txt","descriptor-MIT.txt"],
})
for package in ["egui","emath","ecolor","epaint","egui-wgpu","egui-winit"]:
    SUPPLEMENTAL[(package,"0.33.3")]=["egui-APACHE.txt","egui-MIT.txt"]

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--inventory", action="store_true")
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
command = ["cargo", "metadata", "--locked", "--format-version", "1"]
if not args.inventory:
    command += ["--filter-platform", "x86_64-pc-windows-msvc"]
metadata = json.loads(subprocess.check_output(command, cwd=ROOT))
packages = {p["id"]: p for p in metadata["packages"]}
if args.inventory:
    selected = list(packages.values())
else:
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [p["id"] for p in packages.values() if p["name"] == "canvas-demo"]
    seen = set()
    while pending:
        package_id = pending.pop()
        if package_id in seen:
            continue
        seen.add(package_id)
        pending.extend(dep["pkg"] for dep in nodes[package_id]["deps"])
    selected = [packages[package_id] for package_id in seen]
selected = sorted((p for p in selected if p["source"]), key=lambda p: (p["name"], p["version"]))
if args.inventory:
    text = ["# Locked third-party dependency inventory", "",
            "Generated with `python scripts/canvas-licenses.py --inventory --output docs/dependency-licenses.md`.",
            "Includes all target-specific packages in Cargo.lock; not every package is compiled or shipped on every OS.",
            "Declared SPDX expressions come from verified crate manifests. See dependencies.md for direct choices and notices.",
            "", "| Package | Version | Declared license |", "| --- | --- | --- |"]
    text += [f"| {p['name']} | {p['version']} | {p['license'] or 'SEE LICENSE FILE'} |" for p in selected]
else:
    text = ["Rust Paint Lab canvas demo: third-party license notices",
            "Generated from the locked Windows canvas dependency graph.",
            "Original project source has no selected distribution license; these notices cover third-party code only."]
    for package in selected:
        name, version = package["name"], package["version"]
        folder = Path(package["manifest_path"]).parent
        files = sorted(p for p in folder.rglob("*") if p.is_file() and
            (p.name.lower().startswith(("license", "licence", "copying", "notice", "copyright"))
             or (p.suffix.lower()==".txt" and "fonts" in p.parts)))
        if not files:
            files = [ROOT / "docs/licenses/canvas-supplemental" / f for f in SUPPLEMENTAL.get((name, version), [])]
        if not files:
            raise SystemExit(f"Missing notice files for {name} {version}; investigate upstream before packaging")
        text += ["", "=" * 72, f"{name} {version}", f"Declared license: {package['license']}",
                 f"Repository: {package['repository']}"]
        for path in files:
            text += ["", f"--- {path.name} ---", path.read_text(encoding="utf-8", errors="strict")]
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text("\n".join(text) + "\n", encoding="utf-8")
print(f"Generated {args.output}: {len(selected)} external packages")
