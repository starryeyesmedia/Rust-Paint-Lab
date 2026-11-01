#!/usr/bin/env python3
"""Verify a portable BitchPixel/canvas-demo package without extracting it.

The verifier checks package structure, build metadata, included documentation,
third-party notices, and SHA-256 entries against the package's own manifest.
Those checks establish integrity relative to that manifest; they do not
authenticate the publisher or establish that a package is signed.

The report also lists payload files omitted from the checksum manifest. Such
gaps are warnings by default for compatibility with current alpha packages;
--strict-completeness rejects packages that omit any payload file. The manifest
itself is reported separately because it cannot verify its own digest.

Supported inputs are package directories, ZIP files, and tar archives (including
the .tar.gz format emitted by the current Linux/macOS workflow). Current
canvas-demo and future BitchPixel executable names are recognized.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys
import tarfile
from typing import Any, BinaryIO, Iterator
import zipfile
import zlib


EXECUTABLE_NAMES = {
    "canvas-demo.exe",
    "bitchpixel.exe",
    "canvas-demo",
    "bitchpixel",
}
SHA256_LINE = re.compile(r"^(?P<digest>[0-9a-fA-F]{64}) (?P<mode>[ *])(?P<name>.+)$")
FULL_COMMIT = re.compile(r"^(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})$")
MAX_TEXT_BYTES = 16 * 1024 * 1024
PACKAGE_IO_ERRORS = (
    OSError,
    EOFError,
    ValueError,
    RuntimeError,
    tarfile.TarError,
    zipfile.BadZipFile,
    zlib.error,
)


def _normalize_member(raw: str) -> str:
    """Return a safe archive-relative POSIX path, or raise ValueError."""
    name = raw.replace("\\", "/")
    while name.startswith("./"):
        name = name[2:]
    if re.match(r"^[a-zA-Z]:", name):
        raise ValueError(f"drive-qualified package path: {raw!r}")
    path = PurePosixPath(name)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise ValueError(f"unsafe package path: {raw!r}")
    normalized = path.as_posix()
    if normalized in {"", "."}:
        raise ValueError(f"empty package path: {raw!r}")
    return normalized


class Package:
    """Read regular files from a directory or archive without extraction."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.kind: str
        self.members: dict[str, Any] = {}
        self.discovery_errors: list[str] = []
        if path.is_dir():
            self.kind = "directory"
            self._index_directory()
        elif zipfile.is_zipfile(path):
            self.kind = "zip"
            self._index_zip()
        elif tarfile.is_tarfile(path):
            self.kind = "tar"
            self._index_tar()
        else:
            raise ValueError("input is neither a package directory, ZIP, nor tar archive")

        # GitHub artifact ZIPs may contain one enclosing package folder.
        if self.kind != "directory" and self.members:
            first_parts = {PurePosixPath(name).parts[0] for name in self.members}
            has_root_files = any("/" not in name for name in self.members)
            if len(first_parts) == 1 and not has_root_files:
                prefix = next(iter(first_parts)) + "/"
                stripped: dict[str, Any] = {}
                for name, source in self.members.items():
                    if not name.startswith(prefix) or not name[len(prefix):]:
                        continue
                    relative = name[len(prefix):]
                    if relative in stripped:
                        self.discovery_errors.append(f"duplicate package path after root normalization: {relative}")
                    else:
                        stripped[relative] = source
                self.members = stripped

    def _add_member(self, raw: str, source: Any) -> None:
        try:
            name = _normalize_member(raw)
        except ValueError as exc:
            self.discovery_errors.append(str(exc))
            return
        if name in self.members:
            self.discovery_errors.append(f"duplicate package path: {name}")
            return
        self.members[name] = source

    def _index_directory(self) -> None:
        for item in sorted(self.path.rglob("*")):
            if item.is_symlink():
                self.discovery_errors.append(
                    f"symbolic links are not accepted in portable packages: {item.relative_to(self.path)}"
                )
            elif item.is_file():
                self._add_member(item.relative_to(self.path).as_posix(), item)

    def _index_zip(self) -> None:
        with zipfile.ZipFile(self.path) as archive:
            for info in archive.infolist():
                if info.is_dir():
                    continue
                unix_mode = info.external_attr >> 16
                if unix_mode and (unix_mode & 0o170000) == 0o120000:
                    self.discovery_errors.append(f"symbolic link is not a regular package file: {info.filename}")
                    continue
                self._add_member(info.filename, info.filename)

    def _index_tar(self) -> None:
        with tarfile.open(self.path, mode="r:*") as archive:
            for info in archive.getmembers():
                if info.isdir():
                    continue
                if not info.isfile():
                    self.discovery_errors.append(f"non-regular package entry: {info.name}")
                    continue
                self._add_member(info.name, info.name)

    @contextmanager
    def open(self, name: str) -> Iterator[BinaryIO]:
        source = self.members[name]
        if self.kind == "directory":
            with source.open("rb") as stream:
                yield stream
            return
        if self.kind == "zip":
            with zipfile.ZipFile(self.path) as archive, archive.open(source) as stream:
                yield stream
            return
        with tarfile.open(self.path, mode="r:*") as archive:
            extracted = archive.extractfile(source)
            if extracted is None:
                raise OSError(f"cannot read package member: {name}")
            with extracted:
                yield extracted

    def read_text(self, name: str) -> str:
        with self.open(name) as stream:
            data = stream.read(MAX_TEXT_BYTES + 1)
        if len(data) > MAX_TEXT_BYTES:
            raise ValueError(f"text file is larger than {MAX_TEXT_BYTES} bytes: {name}")
        return data.decode("utf-8-sig", errors="strict")

    def sha256(self, name: str) -> tuple[str, int]:
        digest = hashlib.sha256()
        size = 0
        with self.open(name) as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
                size += len(block)
        return digest.hexdigest(), size


class Verification:
    def __init__(self, package_path: str) -> None:
        self.result: dict[str, Any] = {
            "schema_version": 1,
            "package": package_path,
            "format": None,
            "status": "passed",
            "checks": [],
            "executable": None,
            "build": {},
            "commit_provenance": {},
            "limitations": [
                "SHA-256 checks compare package bytes with the supplied manifest; they do not authenticate a publisher.",
                "This utility does not verify digital signatures or establish that a package is trusted.",
                "Payload files absent from the manifest are not protected by manifest comparison; --strict-completeness can reject that coverage gap.",
            "Notice presence and bundled-font markers are checked; complete dependency-license coverage requires a separate dependency-graph review.",
            ],
        }

    def add(
        self,
        check_id: str,
        status: str,
        message: str,
        *,
        expected: Any = None,
        actual: Any = None,
    ) -> None:
        item: dict[str, Any] = {"id": check_id, "status": status, "message": message}
        if expected is not None:
            item["expected"] = expected
        if actual is not None:
            item["actual"] = actual
        self.result["checks"].append(item)
        if status == "fail":
            self.result["status"] = "failed"

    def finish(self) -> dict[str, Any]:
        return self.result


def _field_values(text: str) -> dict[str, list[str]]:
    fields: dict[str, list[str]] = {}
    for line in text.splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            normalized = re.sub(r"[^a-z0-9]+", " ", key.casefold()).strip()
            value = value.strip()
            if normalized and value:
                fields.setdefault(normalized, []).append(value)
        elif re.match(r"^rustc\s+\S+", line.strip()):
            fields.setdefault("toolchain", []).append(line.strip())
    return fields


def _field(fields: dict[str, list[str]], aliases: tuple[str, ...]) -> tuple[str | None, bool]:
    found = [value for alias in aliases for value in fields.get(alias, [])]
    if not found:
        return None, False
    unique = list(dict.fromkeys(found))
    return unique[0], len(unique) > 1


def _platform_from_target(target: str | None) -> str | None:
    if not target:
        return None
    value = target.casefold()
    if "windows" in value:
        return "windows"
    if "darwin" in value or "macos" in value:
        return "macos"
    if "linux" in value:
        return "linux"
    return None


def _parse_sha256_manifest(text: str) -> tuple[list[tuple[str, str]], list[str]]:
    entries: list[tuple[str, str]] = []
    errors: list[str] = []
    seen: set[str] = set()
    for number, raw_line in enumerate(text.splitlines(), 1):
        line = raw_line.rstrip("\r")
        if not line.strip():
            continue
        match = SHA256_LINE.fullmatch(line)
        if not match:
            errors.append(f"line {number} is not a sha256sum record")
            continue
        try:
            name = _normalize_member(match.group("name"))
        except ValueError as exc:
            errors.append(f"line {number}: {exc}")
            continue
        if name in seen:
            errors.append(f"line {number} repeats manifest path {name}")
            continue
        seen.add(name)
        entries.append((match.group("digest").lower(), name))
    if not entries and not errors:
        errors.append("manifest has no checksum entries")
    return entries, errors


def verify_package(
    path: str | Path,
    *,
    platform: str | None = None,
    expected_commit: str | None = None,
    source_head: str | None = None,
    merge_commit: str | None = None,
    strict_completeness: bool = False,
) -> dict[str, Any]:
    """Return a JSON-serializable verification report."""
    package_path = Path(path)
    report = Verification(str(path))
    try:
        package = Package(package_path)
    except PACKAGE_IO_ERRORS as exc:
        report.add("package_format", "fail", str(exc))
        return report.finish()

    report.result["format"] = package.kind
    report.add("package_format", "pass", f"Read package as {package.kind}")
    for error in package.discovery_errors:
        report.add("package_entry", "fail", error)

    build_name = "BUILD.txt"
    if build_name not in package.members:
        report.add("build_metadata", "fail", "Required BUILD.txt is missing")
        build_text = ""
    else:
        try:
            build_text = package.read_text(build_name)
            report.add("build_metadata", "pass", "Read BUILD.txt")
        except PACKAGE_IO_ERRORS as exc:
            report.add("build_metadata", "fail", f"Cannot read BUILD.txt: {exc}")
            build_text = ""

    fields = _field_values(build_text)
    build_revision, duplicate_revision = _field(
        fields, ("commit", "build commit", "build sha", "commit sha")
    )
    target, duplicate_target = _field(fields, ("target", "platform target", "rust target"))
    toolchain, duplicate_toolchain = _field(fields, ("toolchain", "rust toolchain"))
    source_metadata, duplicate_source = _field(
        fields,
        ("source branch head", "source head", "pr head commit", "pull request head commit", "head commit"),
    )
    merge_metadata, duplicate_merge = _field(
        fields,
        ("synthetic pr merge commit", "pr merge commit", "pull request merge commit", "github merge commit", "merge commit"),
    )
    report.result["build"] = {
        "commit": build_revision,
        "target": target,
        "toolchain": toolchain,
        "source_head": source_metadata,
        "merge_commit": merge_metadata,
    }

    if duplicate_revision:
        report.add("build_commit", "fail", "BUILD.txt contains conflicting commit values")
    elif build_revision is None:
        report.add("build_commit", "fail", "BUILD.txt has no Commit field")
    elif not FULL_COMMIT.fullmatch(build_revision):
        report.add("build_commit", "fail", "Build commit must be a full 40- or 64-character hexadecimal SHA", actual=build_revision)
    else:
        report.add("build_commit", "pass", "Build commit is a full hexadecimal SHA", actual=build_revision)

    if duplicate_target:
        report.add("build_target", "fail", "BUILD.txt contains conflicting target values")
    elif not target:
        report.add("build_target", "fail", "BUILD.txt has no Target field")
    else:
        report.add("build_target", "pass", "Build target is recorded", actual=target)

    if duplicate_toolchain:
        report.add("build_toolchain", "fail", "BUILD.txt contains conflicting toolchain values")
    elif not toolchain:
        report.add("build_toolchain", "fail", "BUILD.txt has no Rust toolchain information")
    else:
        report.add("build_toolchain", "pass", "Rust toolchain is recorded", actual=toolchain)

    if expected_commit:
        if not FULL_COMMIT.fullmatch(expected_commit):
            report.add("expected_commit", "fail", "--expected-commit must be a full hexadecimal SHA", actual=expected_commit)
        elif build_revision and build_revision.casefold() == expected_commit.casefold():
            report.add("expected_commit", "pass", "Build commit matches --expected-commit", actual=build_revision)
        else:
            report.add("expected_commit", "fail", "Build commit does not match --expected-commit", expected=expected_commit, actual=build_revision)

    if duplicate_source:
        report.add("source_head_metadata", "fail", "BUILD.txt contains conflicting source-head values")
    if duplicate_merge:
        report.add("merge_metadata", "fail", "BUILD.txt contains conflicting merge-commit values")
    for label, value in (("source head", source_head), ("merge commit", merge_commit)):
        if value and not FULL_COMMIT.fullmatch(value):
            report.add("commit_reference", "fail", f"Expected {label} must be a full hexadecimal SHA", actual=value)
    for label, value in (("source branch HEAD", source_metadata), ("synthetic PR merge commit", merge_metadata)):
        if value and not FULL_COMMIT.fullmatch(value):
            report.add("commit_reference_metadata", "fail", f"BUILD.txt {label} must be a full hexadecimal SHA", actual=value)

    recorded_source = source_metadata or source_head
    recorded_merge = merge_metadata or merge_commit
    for check_id, recorded, expected, label in (
        ("source_head_match", source_metadata, source_head, "source branch HEAD"),
        ("merge_commit_match", merge_metadata, merge_commit, "synthetic PR merge commit"),
    ):
        if recorded and expected:
            if recorded.casefold() == expected.casefold():
                report.add(check_id, "pass", f"Recorded {label} matches the supplied value", actual=recorded)
            else:
                report.add(check_id, "fail", f"Recorded {label} differs from the supplied value", expected=expected, actual=recorded)

    role = "unclassified"
    provenance_status = "warning"
    provenance_message = "Package records a build commit, but source HEAD and PR merge commit were not both available for classification"
    if build_revision and FULL_COMMIT.fullmatch(build_revision):
        build_lower = build_revision.casefold()
        source_lower = recorded_source.casefold() if recorded_source else None
        merge_lower = recorded_merge.casefold() if recorded_merge else None
        if source_lower and merge_lower:
            if source_lower == merge_lower == build_lower:
                role = "source_head_and_merge_same_commit"
                provenance_status = "pass"
                provenance_message = "Source HEAD and merge commit are the same recorded commit"
            elif build_lower == merge_lower:
                role = "synthetic_pr_merge_commit"
                provenance_status = "pass"
                provenance_message = "Build commit is GitHub's synthetic pull-request merge commit; source branch HEAD is recorded separately"
            elif build_lower == source_lower:
                role = "source_branch_head"
                provenance_status = "pass"
                provenance_message = "Build commit is the source branch HEAD; synthetic PR merge commit is recorded separately"
            else:
                role = "other_commit"
                provenance_status = "fail"
                provenance_message = "Build commit matches neither the source branch HEAD nor the synthetic PR merge commit"
        elif source_lower:
            if build_lower == source_lower:
                role = "source_branch_head"
                provenance_status = "pass"
                provenance_message = "Build commit matches the recorded source branch HEAD"
            else:
                role = "other_commit"
                provenance_status = "fail"
                provenance_message = "Build commit differs from the recorded source branch HEAD"
        elif merge_lower:
            if build_lower == merge_lower:
                role = "synthetic_pr_merge_commit"
                provenance_status = "pass"
                provenance_message = "Build commit matches the recorded synthetic PR merge commit"
            else:
                role = "other_commit"
                provenance_status = "fail"
                provenance_message = "Build commit differs from the recorded synthetic PR merge commit"

    report.result["commit_provenance"] = {
        "build_commit_role": role,
        "source_head": recorded_source,
        "source_head_source": "BUILD.txt" if source_metadata else ("command_line" if source_head else None),
        "merge_commit": recorded_merge,
        "merge_commit_source": "BUILD.txt" if merge_metadata else ("command_line" if merge_commit else None),
    }
    report.add("commit_provenance", provenance_status, provenance_message, actual=report.result["commit_provenance"])

    inferred_platform = _platform_from_target(target)
    selected_platform = platform or inferred_platform
    if platform and platform not in {"windows", "linux", "macos"}:
        report.add("platform_selection", "fail", "--platform must be windows, linux, or macos", actual=platform)
    elif selected_platform is None:
        report.add("platform_selection", "warning", "Platform could not be inferred from BUILD.txt; use --platform to check platform-specific files", actual=target)
    else:
        if inferred_platform and inferred_platform != selected_platform:
            report.add("platform_target", "fail", "Selected platform conflicts with BUILD.txt target", expected=selected_platform, actual=target)
        else:
            report.add("platform_target", "pass", "Package platform agrees with build target", actual=selected_platform)

    executable_candidates = [
        name for name in package.members
        if "/" not in name and PurePosixPath(name).name.casefold() in EXECUTABLE_NAMES
    ]
    if len(executable_candidates) == 1:
        executable = executable_candidates[0]
        report.result["executable"] = executable
        report.add("executable", "pass", "Found a recognized portable app executable", actual=executable)
        if selected_platform == "windows" and not executable.casefold().endswith(".exe"):
            report.add("executable_platform", "fail", "Windows package executable must use the .exe name", actual=executable)
        elif selected_platform in {"linux", "macos"} and executable.casefold().endswith(".exe"):
            report.add("executable_platform", "fail", "Linux/macOS package must use a Unix executable name", actual=executable)
        else:
            report.add("executable_platform", "pass", "Executable filename matches the selected platform", actual=executable)
    elif not executable_candidates:
        executable = None
        report.add("executable", "fail", "No recognized app executable was found at the package root", expected=sorted(EXECUTABLE_NAMES))
    else:
        executable = None
        report.add("executable", "fail", "Multiple recognized app executables make package identity ambiguous", actual=sorted(executable_candidates))

    manifest_name = "SHA256SUMS.txt"
    manifest_entries: list[tuple[str, str]] = []
    manifest_readable = False
    manifest_sha256: str | None = None
    if manifest_name not in package.members:
        report.add("checksum_manifest", "fail", "Required SHA256SUMS.txt is missing")
    else:
        try:
            manifest_text = package.read_text(manifest_name)
            manifest_readable = True
            manifest_entries, manifest_errors = _parse_sha256_manifest(manifest_text)
            for error in manifest_errors:
                report.add("checksum_manifest", "fail", error)
            if not manifest_errors:
                report.add("checksum_manifest", "pass", "Parsed SHA256SUMS.txt")
        except PACKAGE_IO_ERRORS as exc:
            report.add("checksum_manifest", "fail", f"Cannot read SHA256SUMS.txt: {exc}")
        try:
            manifest_sha256, _ = package.sha256(manifest_name)
        except PACKAGE_IO_ERRORS as exc:
            report.add("checksum_manifest_hash", "fail", f"Cannot calculate the manifest's observed SHA-256: {exc}")

    manifest_names = {name for _, name in manifest_entries}
    if executable and executable not in manifest_names:
        report.add("executable_checksum_entry", "fail", "SHA256SUMS.txt does not list the package executable", expected=executable)
    elif executable:
        report.add("executable_checksum_entry", "pass", "SHA256SUMS.txt lists the package executable", actual=executable)

    listed_and_hashed: list[dict[str, str]] = []
    listed_with_failed_hashes: list[dict[str, str | None]] = []
    manifest_self_entry: str | None = None
    for expected_hash, name in manifest_entries:
        if name == manifest_name:
            manifest_self_entry = expected_hash
            report.add("checksum_manifest_self_reference", "fail", "Checksum manifest cannot independently verify its own digest", expected=expected_hash, actual=manifest_sha256)
            continue
        if name not in package.members:
            listed_with_failed_hashes.append({"path": name, "expected_sha256": expected_hash, "actual_sha256": None, "reason": "listed file is absent from package"})
            report.add("checksum_file", "fail", "Checksum manifest references a missing file", actual=name)
            continue
        try:
            actual_hash, size = package.sha256(name)
        except PACKAGE_IO_ERRORS as exc:
            listed_with_failed_hashes.append({"path": name, "expected_sha256": expected_hash, "actual_sha256": None, "reason": f"cannot hash file: {exc}"})
            report.add("checksum_file", "fail", f"Cannot hash manifest file: {exc}", actual=name)
            continue
        if name == executable and size == 0:
            report.add("executable_size", "fail", "Package executable is empty", actual=name)
        if actual_hash == expected_hash:
            listed_and_hashed.append({"path": name, "sha256": actual_hash})
            report.add("checksum_file", "pass", "SHA-256 matches the supplied manifest", actual=name)
        else:
            listed_with_failed_hashes.append({"path": name, "expected_sha256": expected_hash, "actual_sha256": actual_hash, "reason": "SHA-256 mismatch"})
            report.add("checksum_file", "fail", "SHA-256 does not match the supplied manifest", expected=expected_hash, actual={"path": name, "sha256": actual_hash})

    listed_names = {name for name in manifest_names if name != manifest_name}
    payload_files = sorted(name for name in package.members if name != manifest_name)
    successfully_hashed_names = {entry["path"] for entry in listed_and_hashed}
    unlisted_files = sorted(set(payload_files) - listed_names)
    coverage_count = len(successfully_hashed_names & set(payload_files))
    coverage_total = len(payload_files)
    coverage_percent = round(100.0 * coverage_count / coverage_total, 2) if coverage_total else None
    manifest_is_self_listed = manifest_name in manifest_names
    manifest_report = {
        "path": manifest_name,
        "present": manifest_name in package.members,
        "listed_in_manifest": manifest_is_self_listed,
        "verification": "self_reference" if manifest_is_self_listed else ("self_excluded" if manifest_readable else "missing_or_unreadable"),
        "sha256_observed": manifest_sha256,
        "self_entry_expected_sha256": manifest_self_entry,
        "note": "The manifest's observed digest is reported for external comparison; a manifest cannot authenticate itself.",
    }
    coverage = {
        "strict_completeness": strict_completeness,
        "listed_and_successfully_hashed": sorted(listed_and_hashed, key=lambda entry: entry["path"]),
        "listed_with_failed_hashes": sorted(listed_with_failed_hashes, key=lambda entry: (entry["path"], entry["reason"] or "")),
        "packaged_files_absent_from_manifest": unlisted_files,
        "checksum_manifest": manifest_report,
        "payload_files_total": coverage_total,
        "payload_files_successfully_covered": coverage_count,
        "payload_coverage_percent": coverage_percent,
    }
    report.result["integrity_coverage"] = coverage
    has_coverage_gap = bool(unlisted_files or listed_with_failed_hashes or not manifest_readable or manifest_is_self_listed)
    if strict_completeness and has_coverage_gap:
        report.add("integrity_coverage", "fail", "Strict completeness requires every packaged payload file to be listed and successfully hashed", actual=coverage)
    elif unlisted_files:
        report.add("integrity_coverage", "warning", "Some packaged files are absent from SHA256SUMS.txt; current packages may checksum only the executable", actual=coverage)
    elif listed_with_failed_hashes:
        report.add("integrity_coverage", "fail", "One or more manifest entries did not verify", actual=coverage)
    elif manifest_is_self_listed:
        report.add("integrity_coverage", "fail", "SHA256SUMS.txt is self-listed and cannot establish its own digest", actual=coverage)
    elif not manifest_readable:
        report.add("integrity_coverage", "fail", "Integrity coverage cannot be determined without a readable checksum manifest", actual=coverage)
    else:
        report.add("integrity_coverage", "pass", "Every packaged payload file is listed and successfully hashed; SHA256SUMS.txt is reported separately", actual=coverage)

    def require_file(name: str, check_id: str, *, nonempty: bool = True) -> str | None:
        if name not in package.members:
            report.add(check_id, "fail", f"Required package file is missing: {name}")
            return None
        if nonempty:
            try:
                content = package.read_text(name)
            except PACKAGE_IO_ERRORS as exc:
                report.add(check_id, "fail", f"Cannot read required package file {name}: {exc}")
                return None
            if not content.strip():
                report.add(check_id, "fail", f"Required package file is empty: {name}")
                return None
        report.add(check_id, "pass", f"Required package file is present: {name}")
        return content if nonempty else None

    readme_candidates = [name for name in ("README.txt", "README.md") if name in package.members]
    if not readme_candidates:
        report.add("package_readme", "fail", "Package must include README.txt or README.md")
    else:
        readme_ok = False
        for name in readme_candidates:
            if require_file(name, "package_readme") is not None:
                readme_ok = True
        if not readme_ok:
            report.add("package_readme_content", "fail", "No included README has readable non-empty text")

    notice_name = "THIRD-PARTY-NOTICES.txt"
    notices = require_file(notice_name, "third_party_notices")
    if notices is not None:
        folded = notices.casefold()
        missing_font_names = [name for name in ("manrope", "outfit") if name not in folded]
        if missing_font_names:
            report.add("bundled_font_notices", "fail", "Third-party notices omit bundled font names", expected=["Manrope", "Outfit"], actual=missing_font_names)
        elif "ofl" not in folded and "open font license" not in folded:
            report.add("bundled_font_notices", "fail", "Third-party notices do not identify the bundled fonts' OFL license")
        else:
            report.add("bundled_font_notices", "pass", "Third-party notices identify Manrope, Outfit, and the OFL")

    if selected_platform == "windows":
        require_file("USER-GUIDE.md", "package_user_guide")
        require_file("docs/windows-startup-smoke.md", "windows_startup_documentation")
    elif selected_platform in {"linux", "macos"}:
        require_file("README.md", "desktop_package_readme")
        require_file("README.txt", "desktop_runtime_documentation")
        require_file("SYSTEM-LIBRARIES.txt", "system_library_metadata")

    return report.finish()


def _render_text(result: dict[str, Any]) -> str:
    lines = [f"Package: {result['package']}", f"Format: {result['format'] or 'unknown'}"]
    for check in result["checks"]:
        status = check["status"].upper()
        suffix = f" (actual: {check['actual']})" if "actual" in check else ""
        lines.append(f"{status} {check['id']}: {check['message']}{suffix}")
    coverage = result.get("integrity_coverage")
    if coverage:
        passed = coverage["listed_and_successfully_hashed"]
        failed = coverage["listed_with_failed_hashes"]
        unlisted = coverage["packaged_files_absent_from_manifest"]
        lines.append("Integrity coverage:")
        lines.append("  Listed and successfully hashed: " + (", ".join(item["path"] for item in passed) or "none"))
        lines.append("  Listed with failed hashes: " + (", ".join(item["path"] for item in failed) or "none"))
        lines.append("  Packaged files absent from manifest: " + (", ".join(unlisted) or "none"))
        manifest = coverage["checksum_manifest"]
        digest = manifest["sha256_observed"] or "unavailable"
        lines.append(f"  Checksum manifest itself: {manifest['path']} ({manifest['verification']}; observed SHA-256: {digest})")
    failures = sum(item["status"] == "fail" for item in result["checks"])
    warnings = sum(item["status"] == "warning" for item in result["checks"])
    lines.append(f"Result: {result['status'].upper()} ({failures} failure(s), {warnings} warning(s))")
    lines.extend(f"Note: {limitation}" for limitation in result["limitations"])
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path, help="package directory, ZIP, or tar archive")
    parser.add_argument("--platform", choices=("windows", "linux", "macos"), help="expected package platform")
    parser.add_argument("--expected-commit", help="expected full build commit SHA")
    parser.add_argument("--source-head", help="source branch HEAD SHA for provenance classification")
    parser.add_argument("--merge-commit", help="GitHub synthetic PR merge commit SHA for provenance classification")
    parser.add_argument("--strict-completeness", action="store_true", help="fail unless every packaged file other than SHA256SUMS.txt is listed and hashes successfully")
    parser.add_argument("--format", choices=("text", "json"), default="text", help="output format (default: text)")
    args = parser.parse_args(argv)
    result = verify_package(
        args.package,
        platform=args.platform,
        expected_commit=args.expected_commit,
        source_head=args.source_head,
        merge_commit=args.merge_commit,
        strict_completeness=args.strict_completeness,
    )
    if args.format == "json":
        print(json.dumps(result, indent=2, sort_keys=True))
    else:
        print(_render_text(result))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
