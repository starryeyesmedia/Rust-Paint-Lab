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
    ("dispatch2","0.3.1"): ["objc2-LICENSE.md"],
    ("dispatch","0.2.0"): ["dispatch-MIT.txt"],
    ("block2","0.5.1"): ["block2-MIT.txt"],
    ("block2","0.6.2"): ["block2-0.6.2-MIT.txt"],
    ("block","0.1.6"): ["block-MIT.txt"],
    ("clipboard-win","5.4.3"): ["clipboard.txt"],
    ("spirv","0.3.0+sdk-1.3.268.0"): ["spirv27.txt"],
    ("hexf-parse","0.2.1"): ["CC0-1.0.txt"],
    ("gpu-alloc","0.6.2"): ["alloc-APACHE.txt","alloc-MIT.txt"],
    ("gpu-alloc-types","0.3.1"): ["alloc-APACHE.txt","alloc-MIT.txt"],
    ("gpu-descriptor","0.3.2"): ["descriptor-APACHE.txt","descriptor-MIT.txt"],
    ("gpu-descriptor-types","0.2.0"): ["descriptor-APACHE.txt","descriptor-MIT.txt"],
})
for package in ["egui","emath","ecolor","epaint","epaint_default_fonts","egui-wgpu","egui-winit"]:
    SUPPLEMENTAL[(package,"0.33.3")]=["egui-APACHE.txt","egui-MIT.txt"]

SUPPLEMENTAL.update({('objc-sys', '0.3.5'): ['objc-4fc083f1.txt'], ('objc2', '0.5.3'): ['objc-5237cbf0.txt'], ('objc2', '0.6.5'): ['objc-d7d2fa23.txt'], ('objc2-app-kit', '0.2.2'): ['objc-e282618b.txt'], ('objc2-app-kit', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-cloud-kit', '0.2.2'): ['objc-e282618b.txt'], ('objc2-contacts', '0.2.2'): ['objc-e282618b.txt'], ('objc2-core-data', '0.2.2'): ['objc-e282618b.txt'], ('objc2-core-foundation', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-core-graphics', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-core-image', '0.2.2'): ['objc-e282618b.txt'], ('objc2-core-location', '0.2.2'): ['objc-e282618b.txt'], ('objc2-encode', '4.1.0'): ['objc-8d214f54.txt'], ('objc2-foundation', '0.2.2'): ['objc-e282618b.txt'], ('objc2-foundation', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-io-surface', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-link-presentation', '0.2.2'): ['objc-e282618b.txt'], ('objc2-metal', '0.2.2'): ['objc-e282618b.txt'], ('objc2-metal', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-quartz-core', '0.2.2'): ['objc-e282618b.txt'], ('objc2-quartz-core', '0.3.2'): ['objc-7b1abfd7.txt'], ('objc2-symbols', '0.2.2'): ['objc-e282618b.txt'], ('objc2-ui-kit', '0.2.2'): ['objc-e282618b.txt'], ('objc2-uniform-type-identifiers', '0.2.2'): ['objc-e282618b.txt'], ('objc2-user-notifications', '0.2.2'): ['objc-e282618b.txt'], ('malloc_buf', '0.0.6'): ['malloc-MIT.txt']})

# AccessKit and its AT-SPI bridge omit license texts from the published crate
# archives. The upstream manifests declare these SPDX expressions; include the
# exact standard texts with provenance recorded in canvas-supplemental/SOURCES.md.
for name, version in [
    ("accesskit", "0.21.1"),
    ("accesskit_atspi_common", "0.14.2"),
    ("accesskit_consumer", "0.31.0"),
    ("accesskit_macos", "0.22.2"),
    ("accesskit_unix", "0.17.2"),
    ("accesskit_windows", "0.29.2"),
    ("atspi", "0.25.0"),
    ("atspi-common", "0.9.0"),
    ("atspi-connection", "0.9.0"),
    ("atspi-proxies", "0.9.0"),
]:
    SUPPLEMENTAL[(name, version)] = ["egui-APACHE.txt", "egui-MIT.txt"]
SUPPLEMENTAL[("accesskit_winit", "0.29.2")] = ["egui-APACHE.txt"]
for name, version in [
    ("zbus", "5.19.0"),
    ("zbus-lockstep", "0.5.2"),
    ("zbus-lockstep-macros", "0.5.2"),
    ("zbus_macros", "5.19.0"),
    ("zbus_names", "4.3.5"),
    ("zbus_xml", "5.2.1"),
]:
    SUPPLEMENTAL[(name, version)] = ["egui-MIT.txt"]

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--inventory", action="store_true")
parser.add_argument("--target", default="x86_64-pc-windows-msvc")
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
command = ["cargo", "metadata", "--locked", "--format-version", "1"]
if not args.inventory:
    command += ["--filter-platform", args.target]
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
            f"Generated from the locked {args.target} canvas dependency graph.",
            "Original project source has no selected distribution license; these notices cover third-party code only."]
    for package in selected:
        name, version = package["name"], package["version"]
        folder = Path(package["manifest_path"]).parent
        files = sorted(p for p in folder.rglob("*") if p.is_file() and
            (p.name.lower().startswith(("license", "licence", "copying", "notice", "copyright"))
             or (p.suffix.lower()==".txt" and "fonts" in p.parts)))
        files += [ROOT / "docs/licenses/canvas-supplemental" / f for f in SUPPLEMENTAL.get((name, version), [])]
        if not files:
            raise SystemExit(f"Missing notice files for {name} {version}; investigate upstream before packaging")
        text += ["", "=" * 72, f"{name} {version}", f"Declared license: {package['license']}",
                 f"Repository: {package['repository']}"]
        for path in files:
            text += ["", f"--- {path.name} ---", path.read_text(encoding="utf-8", errors="strict")]
    symbols = ROOT / "apps/canvas-demo/assets/material-symbols-rounded"
    if symbols.is_dir():
        text += [
            "", "=" * 72, "Google Material Symbols Rounded",
            "Declared license: Apache-2.0 (locally bundled official SVG assets and derived PNG coverage masks)",
            "Repository: https://github.com/google/material-design-icons",
            "Source revision: 10ecc14790612ca8fd164e3f4ffe9c16fc50d5b7",
            "Assets and source attribution: apps/canvas-demo/assets/material-symbols-rounded/MATERIAL-SYMBOLS-SOURCE.md",
            "", "--- Apache-2.0-LICENSE.txt ---",
            (symbols / "Apache-2.0-LICENSE.txt").read_text(encoding="utf-8", errors="strict"),
        ]
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text("\n".join(text) + "\n", encoding="utf-8")
print(f"Generated {args.output}: {len(selected)} external packages")
