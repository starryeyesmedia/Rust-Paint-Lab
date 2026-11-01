"""Deterministic tests for the portable package verifier."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bp_package_verify as verifier


ROOT = Path(__file__).resolve().parent.parent
SCRIPT = Path(__file__).with_name("bp_package_verify.py")
SOURCE_SHA = "a" * 40
MERGE_SHA = "b" * 40


def package_fixture(
    root: Path,
    *,
    platform: str = "windows",
    executable: str | None = None,
    commit: str = MERGE_SHA,
    source_head: str | None = None,
    merge_commit: str | None = None,
    target: str | None = None,
) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    if platform == "windows":
        executable = executable or "canvas-demo.exe"
        target = target or "x86_64-pc-windows-msvc"
        files = {
            "README.txt": "Portable BitchPixel Windows package.\n",
            "USER-GUIDE.md": "How to start the app.\n",
            "docs/windows-startup-smoke.md": "Startup check details.\n",
        }
        toolchain = "Toolchain: rustc 1.90.0 (test build)"
    elif platform in {"linux", "macos"}:
        executable = executable or "canvas-demo"
        target = target or (
            "x86_64-unknown-linux-gnu" if platform == "linux" else "aarch64-apple-darwin"
        )
        files = {
            "README.md": "BitchPixel user guide.\n",
            "README.txt": "Desktop package instructions.\n",
            "SYSTEM-LIBRARIES.txt": "system libraries for this target\n",
        }
        # The current desktop package writes the rustc version as a bare line.
        toolchain = "rustc 1.90.0 (test build)"
    else:
        raise ValueError(platform)

    exe_bytes = f"fixture executable for {platform}".encode("utf-8")
    (root / executable).write_bytes(exe_bytes)
    for relative, content in files.items():
        target_path = root / relative
        target_path.parent.mkdir(parents=True, exist_ok=True)
        target_path.write_text(content, encoding="utf-8")

    metadata = [f"Commit: {commit}", toolchain, f"Target: {target}"]
    if source_head:
        metadata.append(f"Source branch HEAD: {source_head}")
    if merge_commit:
        metadata.append(f"Synthetic PR merge commit: {merge_commit}")
    (root / "BUILD.txt").write_text("\n".join(metadata) + "\n", encoding="utf-8")
    notices = (
        "Third-party notices\n"
        "Manrope — SIL Open Font License 1.1\n"
        "Outfit — SIL Open Font License 1.1\n"
        "Dependency notices include MIT and Apache-2.0 texts.\n"
    )
    (root / "THIRD-PARTY-NOTICES.txt").write_text(notices, encoding="utf-8")
    digest = hashlib.sha256(exe_bytes).hexdigest()
    (root / "SHA256SUMS.txt").write_text(f"{digest}  {executable}\n", encoding="ascii")
    return root


def write_manifest_for_all_payload_files(root: Path) -> None:
    entries = []
    for file in sorted(path for path in root.rglob("*") if path.is_file() and path.name != "SHA256SUMS.txt"):
        digest = hashlib.sha256(file.read_bytes()).hexdigest()
        entries.append(f"{digest}  {file.relative_to(root).as_posix()}")
    (root / "SHA256SUMS.txt").write_text("\n".join(entries) + "\n", encoding="ascii")


def checks_by_id(result: dict[str, object], check_id: str) -> list[dict[str, object]]:
    return [check for check in result["checks"] if check["id"] == check_id]  # type: ignore[index]


class PackageVerifierTests(unittest.TestCase):
    def test_legacy_windows_canvas_demo_directory_passes_with_provenance_warning(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            result = verifier.verify_package(root, platform="windows")

        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["executable"], "canvas-demo.exe")
        self.assertEqual(result["format"], "directory")
        provenance = result["commit_provenance"]
        self.assertEqual(provenance["build_commit_role"], "unclassified")
        self.assertEqual(checks_by_id(result, "commit_provenance")[0]["status"], "warning")
        coverage = result["integrity_coverage"]
        self.assertEqual(coverage["listed_and_successfully_hashed"][0]["path"], "canvas-demo.exe")
        self.assertIn("README.txt", coverage["packaged_files_absent_from_manifest"])
        self.assertEqual(coverage["checksum_manifest"]["verification"], "self_excluded")
        self.assertEqual(len(coverage["checksum_manifest"]["sha256_observed"]), 64)
        self.assertEqual(checks_by_id(result, "integrity_coverage")[0]["status"], "warning")

    def test_future_bitchpixel_windows_zip_classifies_synthetic_merge_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            package = package_fixture(
                base / "pkg",
                executable="BitchPixel.exe",
                commit=MERGE_SHA,
                source_head=SOURCE_SHA,
                merge_commit=MERGE_SHA,
            )
            archive_path = base / "bitchpixel-windows.zip"
            with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
                for file in sorted(package.rglob("*")):
                    if file.is_file():
                        archive.write(file, Path("BitchPixel") / file.relative_to(package))

            result = verifier.verify_package(
                archive_path,
                platform="windows",
                source_head=SOURCE_SHA,
                merge_commit=MERGE_SHA,
            )

        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["format"], "zip")
        self.assertEqual(result["executable"], "BitchPixel.exe")
        self.assertEqual(
            result["commit_provenance"]["build_commit_role"], "synthetic_pr_merge_commit"
        )

    def test_linux_tar_archive_accepts_legacy_desktop_layout(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            package = package_fixture(base / "desktop", platform="linux")
            archive_path = base / "painting-alpha.tar.gz"
            with tarfile.open(archive_path, "w:gz") as archive:
                for file in sorted(package.rglob("*")):
                    if file.is_file():
                        info = archive.gettarinfo(str(file), arcname="./" + file.relative_to(package).as_posix())
                        with file.open("rb") as stream:
                            archive.addfile(info, stream)

            result = verifier.verify_package(archive_path, platform="linux")

        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["format"], "tar")
        self.assertEqual(result["executable"], "canvas-demo")
        self.assertEqual(checks_by_id(result, "system_library_metadata")[0]["status"], "pass")

    def test_macos_target_and_bitchpixel_name_pass(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(
                Path(temp) / "macos",
                platform="macos",
                executable="bitchpixel",
                commit=SOURCE_SHA,
                source_head=SOURCE_SHA,
            )
            result = verifier.verify_package(root)

        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["executable"], "bitchpixel")
        self.assertEqual(result["commit_provenance"]["build_commit_role"], "source_branch_head")

    def test_tampered_executable_fails_checksum(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            (root / "canvas-demo.exe").write_bytes(b"changed after checksum generation")
            result = verifier.verify_package(root, platform="windows")

        self.assertEqual(result["status"], "failed")
        self.assertTrue(any(c["id"] == "checksum_file" and c["status"] == "fail" for c in result["checks"]))
        coverage = result["integrity_coverage"]
        self.assertEqual(coverage["listed_with_failed_hashes"][0]["path"], "canvas-demo.exe")
        self.assertEqual(coverage["listed_and_successfully_hashed"], [])

    def test_unlisted_document_and_notice_changes_are_not_seen_by_manifest_comparison(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            before = verifier.verify_package(root, platform="windows")
            original_manifest_digest = before["integrity_coverage"]["checksum_manifest"]["sha256_observed"]

            (root / "README.txt").write_text("Changed package instructions.\n", encoding="utf-8")
            notices = (root / "THIRD-PARTY-NOTICES.txt").read_text(encoding="utf-8")
            (root / "THIRD-PARTY-NOTICES.txt").write_text(
                notices.replace("Dependency notices include", "Updated dependency notices include"),
                encoding="utf-8",
            )
            after = verifier.verify_package(root, platform="windows")

        self.assertEqual(before["status"], "passed")
        self.assertEqual(after["status"], "passed")
        self.assertEqual(
            [check["actual"] for check in checks_by_id(before, "checksum_file")],
            [check["actual"] for check in checks_by_id(after, "checksum_file")],
        )
        self.assertEqual(
            after["integrity_coverage"]["checksum_manifest"]["sha256_observed"],
            original_manifest_digest,
        )
        self.assertIn("README.txt", after["integrity_coverage"]["packaged_files_absent_from_manifest"])
        self.assertIn("THIRD-PARTY-NOTICES.txt", after["integrity_coverage"]["packaged_files_absent_from_manifest"])

    def test_strict_completeness_rejects_current_executable_only_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            default_result = verifier.verify_package(root, platform="windows")
            strict_result = verifier.verify_package(root, platform="windows", strict_completeness=True)

        self.assertEqual(default_result["status"], "passed")
        self.assertEqual(checks_by_id(default_result, "integrity_coverage")[0]["status"], "warning")
        self.assertEqual(strict_result["status"], "failed")
        self.assertEqual(checks_by_id(strict_result, "integrity_coverage")[0]["status"], "fail")

    def test_strict_completeness_passes_when_all_payload_files_are_listed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            write_manifest_for_all_payload_files(root)
            result = verifier.verify_package(root, platform="windows", strict_completeness=True)

        self.assertEqual(result["status"], "passed")
        coverage = result["integrity_coverage"]
        self.assertEqual(coverage["packaged_files_absent_from_manifest"], [])
        self.assertEqual(coverage["checksum_manifest"]["verification"], "self_excluded")
        self.assertEqual(coverage["payload_coverage_percent"], 100.0)

    def test_checksum_manifest_is_reported_separately_when_self_listed(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            with (root / "SHA256SUMS.txt").open("a", encoding="ascii") as manifest:
                manifest.write(f"{'0' * 64}  SHA256SUMS.txt\n")
            result = verifier.verify_package(root, platform="windows")

        coverage = result["integrity_coverage"]
        self.assertEqual(coverage["checksum_manifest"]["verification"], "self_reference")
        self.assertEqual(coverage["checksum_manifest"]["self_entry_expected_sha256"], "0" * 64)
        self.assertNotIn("SHA256SUMS.txt", [
            entry["path"] for entry in coverage["listed_with_failed_hashes"]
        ])
        self.assertEqual(checks_by_id(result, "checksum_manifest_self_reference")[0]["status"], "fail")

    def test_missing_required_notice_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            (root / "THIRD-PARTY-NOTICES.txt").unlink()
            result = verifier.verify_package(root, platform="windows")

        self.assertEqual(result["status"], "failed")
        self.assertTrue(any(c["id"] == "third_party_notices" and c["status"] == "fail" for c in result["checks"]))

    def test_malformed_build_commit_and_expected_commit_mismatch_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            build = (root / "BUILD.txt").read_text(encoding="utf-8")
            (root / "BUILD.txt").write_text(build.replace(MERGE_SHA, "not-a-commit"), encoding="utf-8")
            result = verifier.verify_package(root, platform="windows", expected_commit=SOURCE_SHA)

        self.assertEqual(result["status"], "failed")
        self.assertTrue(any(c["id"] == "build_commit" and c["status"] == "fail" for c in result["checks"]))
        self.assertTrue(any(c["id"] == "expected_commit" and c["status"] == "fail" for c in result["checks"]))

    def test_malformed_manifest_and_platform_conflict_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows", target="x86_64-unknown-linux-gnu")
            (root / "SHA256SUMS.txt").write_text("bad checksum record\n", encoding="utf-8")
            result = verifier.verify_package(root, platform="windows")

        self.assertEqual(result["status"], "failed")
        self.assertTrue(any(c["id"] == "checksum_manifest" and c["status"] == "fail" for c in result["checks"]))
        self.assertTrue(any(c["id"] == "platform_target" and c["status"] == "fail" for c in result["checks"]))

    def test_cli_json_output_is_machine_readable(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), str(root), "--platform", "windows", "--format", "json"],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        parsed = json.loads(completed.stdout)
        self.assertEqual(parsed["status"], "passed")
        self.assertEqual(parsed["schema_version"], 1)
        self.assertIn("limitations", parsed)
        self.assertIn("publisher", " ".join(parsed["limitations"]))
        self.assertIn("integrity_coverage", parsed)
        self.assertIn("packaged_files_absent_from_manifest", parsed["integrity_coverage"])

    def test_cli_strict_completeness_returns_failure_for_unlisted_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    str(root),
                    "--platform",
                    "windows",
                    "--strict-completeness",
                    "--format",
                    "json",
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

        parsed = json.loads(completed.stdout)
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(parsed["integrity_coverage"]["strict_completeness"], True)
        self.assertEqual(checks_by_id(parsed, "integrity_coverage")[0]["status"], "fail")

    def test_human_output_enumerates_integrity_coverage(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = package_fixture(Path(temp) / "windows")
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), str(root), "--platform", "windows"],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Listed and successfully hashed: canvas-demo.exe", completed.stdout)
        self.assertIn("Packaged files absent from manifest:", completed.stdout)
        self.assertIn("Checksum manifest itself: SHA256SUMS.txt (self_excluded; observed SHA-256:", completed.stdout)

    def test_archive_path_traversal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            archive_path = root / "unsafe.zip"
            with zipfile.ZipFile(archive_path, "w") as archive:
                archive.writestr("../outside.txt", "unsafe")
                archive.writestr("package/README.txt", "readme")
            result = verifier.verify_package(archive_path)

        self.assertEqual(result["status"], "failed")
        self.assertTrue(any(c["id"] == "package_entry" and c["status"] == "fail" for c in result["checks"]))


if __name__ == "__main__":
    unittest.main()
