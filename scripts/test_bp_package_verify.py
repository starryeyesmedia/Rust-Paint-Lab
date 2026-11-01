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
