#!/usr/bin/env python3
"""Rebuild labeled synthetic compatibility files with their historical encoders.

These are not captured artist documents. Each file is generated from a small,
deterministic state by the serializer at the source commit recorded below.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[5]
HERE = Path(__file__).resolve().parent
OUTPUT = HERE / "historical-serializer-generated"

GENERATORS = (
    (
        "v1",
        "d322c55619b50278e3a2ced50cd9fc27cbe1ff49",
        "generators/v1.rs",
        "project-v1.rpl",
        b"RPLALP01",
    ),
    (
        "v2",
        "4b4e6c4bacc813148d1b35cccb64ae888f7cfe88",
        "generators/v2.rs",
        "project-v2.rpl",
        b"RPLALP02",
    ),
    (
        "v3",
        "4a5a808bc1f20bd7432a22d3f2cac17885888d89",
        "generators/v3.rs",
        "project-v3.rpl",
        b"RPLALP03",
    ),
    (
        "v4",
        "0d26bd60b14fd42ac11a6d77dadcbcb97b2474e0",
        "generators/v4.rs",
        "project-v4.rpl",
        b"RPLALP04",
    ),
    (
        "brush-v1",
        "173bf1c92113f6ecc90f3246dfc24bb2cc8b7223",
        "generators/brush_v1.rs",
        "brush-preset-v1.rplbrush",
        None,
    ),
)


def run(command: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> None:
    print("$", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, env=env, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=OUTPUT,
        help="destination for the generated synthetic fixture files",
    )
    args = parser.parse_args()
    out_dir = args.out_dir.resolve()
    cargo = shutil.which("cargo")
    if cargo is None:
        raise SystemExit("cargo is not on PATH; source the pinned Rust 1.90 toolchain first")

    with tempfile.TemporaryDirectory(prefix="bp-compat-history-") as temporary:
        temp = Path(temporary)
        staged = temp / "outputs"
        staged.mkdir()
        for label, commit, generator_rel, filename, expected_magic in GENERATORS:
            run(["git", "cat-file", "-e", f"{commit}^{{commit}}"], cwd=ROOT)
            source = temp / f"source-{label}"
            source.mkdir()
            archive = subprocess.run(
                ["git", "archive", "--format=tar", commit],
                cwd=ROOT,
                check=True,
                stdout=subprocess.PIPE,
            ).stdout
            subprocess.run(["tar", "-xf", "-", "-C", str(source)], input=archive, check=True)

            example_dir = source / "crates/document/examples"
            example_dir.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(HERE / generator_rel, example_dir / "bp_compat_fixture.rs")
            generated = staged / filename
            env = os.environ.copy()
            env["CARGO_TARGET_DIR"] = str(temp / f"target-{label}")
            run(
                [
                    cargo,
                    "run",
                    "--locked",
                    "--offline",
                    "-p",
                    "paint-document",
                    "--example",
                    "bp_compat_fixture",
                    "--",
                    str(generated),
                ],
                cwd=source,
                env=env,
            )
            data = generated.read_bytes()
            if expected_magic is not None and not data.startswith(expected_magic):
                raise SystemExit(
                    f"{filename}: expected {expected_magic!r}, got {data[:8]!r}"
                )
            print(
                f"{filename}: {len(data)} bytes sha256="
                f"{hashlib.sha256(data).hexdigest()} source={commit}",
                flush=True,
            )

        out_dir.mkdir(parents=True, exist_ok=True)
        for _, _, _, filename, _ in GENERATORS:
            shutil.copyfile(staged / filename, out_dir / filename)
    print(f"Wrote synthetic historical-serializer fixtures to {out_dir}")


if __name__ == "__main__":
    main()
