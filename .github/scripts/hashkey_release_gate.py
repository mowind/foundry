#!/usr/bin/env python3
"""Dependency-contract checks for the HashKey B20 release gate."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


APPROVED_REPOSITORY = "https://github.com/TraceBundy/optimism"
APPROVED_REVISION = "149bcbfc8c5d5ba6d167e4d27e7d72f3b7ffa2d8"
B20_PACKAGES = ("hsk-b20-config", "hsk-b20-precompiles")
ALLOY_CORE_PACKAGES = (
    "alloy-primitives",
    "alloy-sol-types",
    "alloy-json-abi",
    "alloy-dyn-abi",
)
SINGLETON_PACKAGES = (*ALLOY_CORE_PACKAGES, "alloy-evm", "revm")


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as file:
        return tomllib.load(file)


def validate_dependency_files(root: Path) -> list[str]:
    errors: list[str] = []
    manifest = load_toml(root / "Cargo.toml")
    dependencies = manifest.get("workspace", {}).get("dependencies", {})

    for package in B20_PACKAGES:
        dependency = dependencies.get(package)
        if not isinstance(dependency, dict):
            errors.append(f"{package} must be a workspace Git dependency")
            continue
        if dependency.get("git") != APPROVED_REPOSITORY:
            errors.append(f"{package} must use {APPROVED_REPOSITORY}")
        if dependency.get("rev") != APPROVED_REVISION:
            errors.append(f"{package} must pin rev {APPROVED_REVISION}")
        if "branch" in dependency or "tag" in dependency:
            errors.append(f"{package} must not use a moving branch or tag")

    for manifest_path in root.rglob("Cargo.toml"):
        if any(part in {".git", "target"} for part in manifest_path.parts):
            continue
        text = manifest_path.read_text(encoding="utf-8")
        if "github.com/base/base" in text:
            errors.append(f"direct base/base dependency found in {manifest_path.relative_to(root)}")

    lock = load_toml(root / "Cargo.lock")
    packages = lock.get("package", [])
    expected_source = (
        f"git+{APPROVED_REPOSITORY}?rev={APPROVED_REVISION}#{APPROVED_REVISION}"
    )
    for package in B20_PACKAGES:
        matches = [entry for entry in packages if entry.get("name") == package]
        if len(matches) != 1:
            errors.append(f"Cargo.lock must contain exactly one {package} package")
        elif matches[0].get("source") != expected_source:
            errors.append(f"Cargo.lock {package} source must be {expected_source}")

    return errors


def validate_metadata(metadata: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    packages = metadata.get("packages", [])

    for name in SINGLETON_PACKAGES:
        identities = {
            (package.get("version"), package.get("source"))
            for package in packages
            if package.get("name") == name
        }
        if len(identities) != 1:
            rendered = ", ".join(
                f"{version} ({source})" for version, source in sorted(identities)
            ) or "missing"
            errors.append(f"{name} must resolve to one package identity; found {rendered}")

    alloy_core_identities = {
        (package.get("version"), package.get("source"))
        for package in packages
        if package.get("name") in ALLOY_CORE_PACKAGES
    }
    if len(alloy_core_identities) != 1:
        rendered = ", ".join(
            f"{version} ({source})" for version, source in sorted(alloy_core_identities)
        )
        errors.append(f"Alloy core packages must share one universe; found {rendered}")

    package_by_id = {package.get("id"): package for package in packages}
    for member_id in metadata.get("workspace_members", []):
        member = package_by_id.get(member_id)
        if member is None:
            continue
        for dependency in member.get("dependencies", []):
            source = dependency.get("source") or ""
            if "github.com/base/base" in source:
                errors.append(
                    f"workspace package {member.get('name')} directly depends on base/base"
                )

    return errors


def cargo_metadata(root: Path) -> dict[str, Any]:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--all-features",
            "--format-version",
            "1",
        ],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--metadata", type=Path)
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--print-approved-repository", action="store_true")
    output.add_argument("--print-approved-revision", action="store_true")
    args = parser.parse_args()

    if args.print_approved_repository:
        print(APPROVED_REPOSITORY)
        return 0
    if args.print_approved_revision:
        print(APPROVED_REVISION)
        return 0

    root = args.root.resolve()
    errors = validate_dependency_files(root)
    metadata = (
        json.loads(args.metadata.read_text(encoding="utf-8"))
        if args.metadata
        else cargo_metadata(root)
    )
    errors.extend(validate_metadata(metadata))

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    print(
        "HashKey dependency contract passed: "
        f"TraceBundy/optimism@{APPROVED_REVISION[:10]}, one Alloy/REVM universe, no base/base."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
