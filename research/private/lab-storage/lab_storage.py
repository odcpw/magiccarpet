#!/usr/bin/env python3
"""Provision and operate the owner-only Magic Carpet analyzer workspace."""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Iterator


APP_ID = "magiccarpet-lab"
SCHEMA = "magiccarpet-lab-storage/1"
ID_PATTERN = re.compile(r"[A-Z][A-Z0-9]*(?:-[A-Z0-9]+){1,7}")
PRIVATE_LAB = Path("/home/oliver/Projects/odcpw/magiccarpet")
PUBLIC_CANDIDATE = Path("/home/oliver/Projects/odcpw/carpet-flight")


class LabError(RuntimeError):
    def __init__(self, code: str, message: str, exit_code: int = 2):
        super().__init__(message)
        self.code = code
        self.exit_code = exit_code


@dataclass(frozen=True)
class Roots:
    data: Path
    config: Path
    cache: Path
    state: Path
    synthetic: bool

    @classmethod
    def discover(cls, test_base: Path | None) -> "Roots":
        if test_base is not None:
            raw = test_base.absolute()
            if not raw.is_relative_to(Path("/tmp")):
                raise LabError(
                    "TEST_ROOT_OUTSIDE_TMP",
                    "synthetic test base must be an absolute path below /tmp",
                )
            reject_existing_symlink(raw)
            base = raw.resolve(strict=False)
            return cls(
                data=base / "data",
                config=base / "config",
                cache=base / "cache",
                state=base / "state",
                synthetic=True,
            )

        home = Path.home().resolve(strict=True)
        candidates = {
            "data": Path(
                os.environ.get("XDG_DATA_HOME", home / ".local" / "share")
            ),
            "config": Path(os.environ.get("XDG_CONFIG_HOME", home / ".config")),
            "cache": Path(os.environ.get("XDG_CACHE_HOME", home / ".cache")),
            "state": Path(
                os.environ.get("XDG_STATE_HOME", home / ".local" / "state")
            ),
        }
        resolved: dict[str, Path] = {}
        for name, candidate in candidates.items():
            candidate = candidate.expanduser()
            if not candidate.is_absolute():
                raise LabError(
                    "RELATIVE_XDG_ROOT", f"{name} XDG root is not absolute"
                )
            canonical = candidate.resolve(strict=False)
            if not canonical.is_relative_to(home):
                raise LabError(
                    "XDG_ROOT_OUTSIDE_HOME",
                    f"{name} XDG root is outside the owner home",
                )
            resolved[name] = canonical / APP_ID
        return cls(**resolved, synthetic=False)

    def named(self) -> tuple[tuple[str, Path], ...]:
        return (
            ("data", self.data),
            ("config", self.config),
            ("cache", self.cache),
            ("state", self.state),
        )


WRITABLE_LAYOUT = {
    "data": (
        "",
        "profiles",
        "profiles/ghidra",
        "staging",
        "baselines",
        "campaigns",
        "registry",
        "backups",
    ),
    "config": ("",),
    "cache": ("", "temp", "campaigns"),
    "state": ("", "locks", "receipts", "failures", "inventories"),
}


def now_utc() -> str:
    return datetime.now(UTC).isoformat()


def validate_id(value: str, label: str) -> str:
    if len(value) > 64 or ID_PATTERN.fullmatch(value) is None:
        raise LabError(
            "INVALID_ID",
            f"{label} must be 2-64 uppercase letters, digits, and hyphen groups",
        )
    return value


def reject_existing_symlink(path: Path) -> None:
    current = path
    checked: list[Path] = []
    while True:
        checked.append(current)
        if current.parent == current:
            break
        current = current.parent
    for candidate in reversed(checked):
        try:
            mode = candidate.lstat().st_mode
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(mode):
            raise LabError("SYMLINK_PATH", f"symlink path component: {candidate}")


def ensure_disjoint_roots(roots: Roots) -> None:
    named = roots.named()
    for index, (left_name, left) in enumerate(named):
        reject_existing_symlink(left)
        left_resolved = left.resolve(strict=False)
        for right_name, right in named[index + 1 :]:
            right_resolved = right.resolve(strict=False)
            if (
                left_resolved == right_resolved
                or left_resolved.is_relative_to(right_resolved)
                or right_resolved.is_relative_to(left_resolved)
            ):
                raise LabError(
                    "OVERLAPPING_ROOTS",
                    f"{left_name} and {right_name} roots overlap",
                )
        if not roots.synthetic and (
            left_resolved.is_relative_to(PRIVATE_LAB)
            or left_resolved.is_relative_to(PUBLIC_CANDIDATE)
        ):
            raise LabError(
                "PROJECT_ROOT_STORAGE",
                f"{left_name} storage may not be inside a project checkout",
            )


def ensure_owner(path: Path) -> os.stat_result:
    info = path.lstat()
    if info.st_uid != os.getuid():
        raise LabError("WRONG_OWNER", f"path is not owned by uid {os.getuid()}: {path}")
    return info


def mkdir_private(path: Path) -> None:
    reject_existing_symlink(path)
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    ensure_owner(path)
    path.chmod(0o700)


def require_below(path: Path, root: Path) -> Path:
    reject_existing_symlink(path)
    canonical_root = root.resolve(strict=True)
    canonical = path.resolve(strict=False)
    if canonical == canonical_root or not canonical.is_relative_to(canonical_root):
        raise LabError("PATH_OUTSIDE_ROOT", f"path is outside its allowed root: {path}")
    return canonical


def apply_default_acl(path: Path) -> None:
    if shutil.which("setfacl") is None:
        raise LabError("SETFACL_MISSING", "setfacl is required for the private lab")
    completed = subprocess.run(
        [
            "setfacl",
            "-m",
            "u::rwx,g::---,o::---,m::---,"
            "d:u::rwx,d:g::---,d:o::---,d:m::---",
            str(path),
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise LabError(
            "ACL_FAILED",
            f"failed to apply owner-only default ACL to {path}: "
            f"{completed.stderr.strip()}",
        )
    path.chmod(0o700)


def verify_default_acl(path: Path) -> None:
    if shutil.which("getfacl") is None:
        raise LabError("GETFACL_MISSING", "getfacl is required for the private lab")
    completed = subprocess.run(
        ["getfacl", "-cp", str(path)],
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise LabError("ACL_READ_FAILED", f"cannot read ACL for {path}")
    required = {
        "default:user::rwx",
        "default:group::---",
        "default:mask::---",
        "default:other::---",
    }
    observed = set(completed.stdout.splitlines())
    missing = sorted(required - observed)
    if missing:
        raise LabError(
            "DEFAULT_ACL_MISSING",
            f"{path} lacks owner-only default ACL entries: {missing}",
        )


def write_json_atomic(path: Path, payload: object) -> None:
    parent = path.parent
    ensure_owner(parent)
    reject_existing_symlink(path)
    temporary = parent / f".{path.name}.{uuid.uuid4().hex}.tmp"
    descriptor = os.open(
        temporary,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            json.dump(payload, stream, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        path.chmod(0o600)
    finally:
        with contextlib.suppress(FileNotFoundError):
            temporary.unlink()


def walk_regular(root: Path) -> Iterator[tuple[Path, os.stat_result]]:
    stack = [root]
    while stack:
        directory = stack.pop()
        with os.scandir(directory) as entries:
            ordered = sorted(entries, key=lambda entry: entry.name, reverse=True)
        for entry in ordered:
            path = Path(entry.path)
            info = entry.stat(follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode):
                raise LabError("SYMLINK", f"symlink is forbidden: {path}")
            if stat.S_ISDIR(info.st_mode):
                yield path, info
                stack.append(path)
            elif stat.S_ISREG(info.st_mode):
                if info.st_nlink != 1:
                    raise LabError("HARDLINK", f"hard-linked file is forbidden: {path}")
                yield path, info
            else:
                raise LabError("SPECIAL_FILE", f"special file is forbidden: {path}")


def normalize_writable(root: Path) -> None:
    root.chmod(0o700)
    for path, info in walk_regular(root):
        path.chmod(0o700 if stat.S_ISDIR(info.st_mode) else 0o600)
        if stat.S_ISDIR(info.st_mode):
            apply_default_acl(path)
    apply_default_acl(root)


def seal_tree(root: Path) -> None:
    paths = list(walk_regular(root))
    for path, info in sorted(
        paths,
        key=lambda item: len(item[0].parts),
        reverse=True,
    ):
        if stat.S_ISDIR(info.st_mode):
            subprocess.run(
                ["setfacl", "-k", str(path)],
                text=True,
                capture_output=True,
                check=True,
            )
            path.chmod(0o500)
        else:
            path.chmod(0o400)
    subprocess.run(
        ["setfacl", "-k", str(root)],
        text=True,
        capture_output=True,
        check=True,
    )
    root.chmod(0o500)


def hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_inventory(root: Path, include_modes: bool = True) -> dict[str, object]:
    entries: list[dict[str, object]] = []
    digest = hashlib.sha256()
    for path, info in sorted(walk_regular(root), key=lambda item: item[0].as_posix()):
        relative = path.relative_to(root).as_posix()
        mode = stat.S_IMODE(info.st_mode)
        if stat.S_ISDIR(info.st_mode):
            record: dict[str, object] = {"path": relative, "type": "directory"}
        else:
            file_hash = hash_file(path)
            record = {
                "path": relative,
                "type": "file",
                "size": info.st_size,
                "sha256": file_hash,
            }
        if include_modes:
            record["mode"] = f"{mode:04o}"
        entries.append(record)
        digest.update(
            json.dumps(record, sort_keys=True, separators=(",", ":")).encode()
        )
        digest.update(b"\n")
    return {"entries": entries, "tree_sha256": digest.hexdigest()}


def layout_policy(roots: Roots) -> dict[str, object]:
    return {
        "schema": SCHEMA,
        "application": APP_ID,
        "classification": "PRIVATE_ANALYSIS",
        "roots": {name: str(path) for name, path in roots.named()},
        "private_lab": str(PRIVATE_LAB),
        "public_candidate": str(PUBLIC_CANDIDATE),
        "rules": {
            "directories": "0700 writable or 0500 sealed baseline",
            "files": "0600 writable or 0400 sealed baseline",
            "default_acl": "owner rwx; group/other none",
            "symlinks": "forbidden",
            "hardlinks": "forbidden",
            "shared_writable_projects": "forbidden",
            "campaign_writer_lock": "exclusive flock",
        },
    }


def init_layout(roots: Roots) -> dict[str, object]:
    ensure_disjoint_roots(roots)
    old_umask = os.umask(0o077)
    try:
        for name, root in roots.named():
            for relative in WRITABLE_LAYOUT[name]:
                directory = root / relative if relative else root
                mkdir_private(directory)
                apply_default_acl(directory)
        write_json_atomic(roots.config / "path-policy.json", layout_policy(roots))
    finally:
        os.umask(old_umask)
    audit_layout(roots)
    return {
        "schema": SCHEMA,
        "status": "INITIALIZED",
        "roots": {name: str(path) for name, path in roots.named()},
    }


def require_initialized(roots: Roots) -> None:
    policy = roots.config / "path-policy.json"
    if not policy.is_file():
        raise LabError("NOT_INITIALIZED", "run init before using the lab storage")
    observed = json.loads(policy.read_text(encoding="utf-8"))
    if observed != layout_policy(roots):
        raise LabError("POLICY_DRIFT", "stored path policy differs from live roots")


def open_lock(path: Path) -> int:
    mkdir_private(path.parent)
    descriptor = os.open(
        path,
        os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW,
        0o600,
    )
    os.fchmod(descriptor, 0o600)
    return descriptor


@contextlib.contextmanager
def exclusive_lock(path: Path, timeout: float) -> Iterator[None]:
    descriptor = open_lock(path)
    deadline = time.monotonic() + timeout
    try:
        while True:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise LabError(
                        "LOCK_BUSY",
                        f"exclusive writer lock is busy: {path.name}",
                        exit_code=73,
                    )
                time.sleep(0.05)
        yield
    finally:
        with contextlib.suppress(OSError):
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def record_event(
    roots: Roots,
    category: str,
    operation: str,
    details: dict[str, object],
) -> Path:
    directory = roots.state / category
    mkdir_private(directory)
    path = directory / (
        datetime.now(UTC).strftime("%Y%m%dT%H%M%S.%fZ")
        + f"-{uuid.uuid4().hex}.json"
    )
    write_json_atomic(
        path,
        {
            "schema": SCHEMA,
            "operation": operation,
            "recorded_at": now_utc(),
            **details,
        },
    )
    return path


def seal_baseline(roots: Roots, baseline_id: str) -> dict[str, object]:
    require_initialized(roots)
    baseline_id = validate_id(baseline_id, "baseline id")
    source = roots.data / "staging" / baseline_id
    destination = roots.data / "baselines" / baseline_id
    require_below(source, roots.data)
    require_below(destination, roots.data)
    if not (source / "project").is_dir():
        raise LabError(
            "STAGING_PROJECT_MISSING",
            f"staging baseline must contain project/: {source}",
        )
    if destination.exists():
        raise LabError("BASELINE_EXISTS", f"baseline already exists: {baseline_id}")
    source_inventory = tree_inventory(source / "project", include_modes=False)
    source_snapshot = tree_inventory(source, include_modes=False)
    lock = roots.state / "locks" / "layout.lock"
    with exclusive_lock(lock, 10):
        shutil.copytree(source, destination, symlinks=False)
        normalize_writable(destination)
        copied_inventory = tree_inventory(
            destination / "project", include_modes=False
        )
        copied_snapshot = tree_inventory(destination, include_modes=False)
        final_source_inventory = tree_inventory(
            source / "project", include_modes=False
        )
        final_source_snapshot = tree_inventory(source, include_modes=False)
        if (
            copied_inventory["tree_sha256"]
            != source_inventory["tree_sha256"]
            or final_source_inventory["tree_sha256"]
            != source_inventory["tree_sha256"]
            or copied_snapshot["tree_sha256"]
            != source_snapshot["tree_sha256"]
            or final_source_snapshot["tree_sha256"]
            != source_snapshot["tree_sha256"]
        ):
            shutil.rmtree(destination)
            raise LabError(
                "STAGING_CHANGED",
                "staging project changed while its baseline snapshot was copied",
            )
        write_json_atomic(
            destination / "baseline.json",
            {
                "schema": SCHEMA,
                "baseline_id": baseline_id,
                "classification": "PRIVATE_ANALYSIS",
                "project_tree_sha256": source_inventory["tree_sha256"],
                "project_entries": len(source_inventory["entries"]),
            },
        )
        seal_tree(destination)
    receipt = record_event(
        roots,
        "receipts",
        "seal-baseline",
        {
            "baseline_id": baseline_id,
            "project_tree_sha256": source_inventory["tree_sha256"],
        },
    )
    return {
        "schema": SCHEMA,
        "status": "BASELINE_SEALED",
        "baseline_id": baseline_id,
        "project_tree_sha256": source_inventory["tree_sha256"],
        "receipt": str(receipt),
    }


def clone_campaign(
    roots: Roots, baseline_id: str, campaign_id: str
) -> dict[str, object]:
    require_initialized(roots)
    baseline_id = validate_id(baseline_id, "baseline id")
    campaign_id = validate_id(campaign_id, "campaign id")
    baseline = roots.data / "baselines" / baseline_id
    campaign = roots.data / "campaigns" / campaign_id
    require_below(baseline, roots.data)
    require_below(campaign, roots.data)
    if not (baseline / "project").is_dir():
        raise LabError("BASELINE_MISSING", f"sealed baseline is missing: {baseline_id}")
    if campaign.exists():
        raise LabError("CAMPAIGN_EXISTS", f"campaign already exists: {campaign_id}")
    baseline_meta = json.loads(
        (baseline / "baseline.json").read_text(encoding="utf-8")
    )
    lock = roots.state / "locks" / f"{campaign_id}.lock"
    with exclusive_lock(lock, 10):
        mkdir_private(campaign)
        shutil.copytree(baseline / "project", campaign / "project", symlinks=False)
        for relative in (
            "observations",
            "exports",
            "home",
            "config",
            "data",
            "state",
            "runtime",
        ):
            mkdir_private(campaign / relative)
        normalize_writable(campaign)
        campaign_cache = roots.cache / "campaigns" / campaign_id
        mkdir_private(campaign_cache)
        mkdir_private(campaign_cache / "tmp")
        mkdir_private(campaign_cache / "gradle")
        apply_default_acl(campaign_cache)
        apply_default_acl(campaign_cache / "tmp")
        mkdir_private(roots.state / "receipts" / campaign_id)
        mkdir_private(roots.state / "failures" / campaign_id)
        write_json_atomic(
            campaign / "campaign.json",
            {
                "schema": SCHEMA,
                "campaign_id": campaign_id,
                "baseline_id": baseline_id,
                "baseline_project_tree_sha256": baseline_meta[
                    "project_tree_sha256"
                ],
                "classification": "PRIVATE_ANALYSIS",
                "single_writer_lock": str(lock),
            },
        )
    cloned = tree_inventory(campaign / "project", include_modes=False)
    if cloned["tree_sha256"] != baseline_meta["project_tree_sha256"]:
        raise LabError("CLONE_MISMATCH", "campaign clone differs from sealed baseline")
    receipt = record_event(
        roots,
        "receipts",
        "clone-campaign",
        {
            "campaign_id": campaign_id,
            "baseline_id": baseline_id,
            "project_tree_sha256": cloned["tree_sha256"],
        },
    )
    return {
        "schema": SCHEMA,
        "status": "CAMPAIGN_CLONED",
        "campaign_id": campaign_id,
        "baseline_id": baseline_id,
        "project_tree_sha256": cloned["tree_sha256"],
        "receipt": str(receipt),
    }


def run_locked(
    roots: Roots,
    campaign_id: str,
    timeout: float,
    command: list[str],
) -> int:
    require_initialized(roots)
    campaign_id = validate_id(campaign_id, "campaign id")
    if not command:
        raise LabError("COMMAND_MISSING", "run-locked requires a command")
    if command[0] == "--":
        command = command[1:]
    if not command:
        raise LabError("COMMAND_MISSING", "run-locked requires a command")
    campaign = roots.data / "campaigns" / campaign_id
    project = campaign / "project"
    require_below(project, roots.data)
    if not project.is_dir():
        raise LabError("CAMPAIGN_MISSING", f"campaign is missing: {campaign_id}")
    lock = roots.state / "locks" / f"{campaign_id}.lock"
    started = now_utc()
    with exclusive_lock(lock, timeout):
        inherited_keys = (
            "PATH",
            "USER",
            "LOGNAME",
            "LANG",
            "LC_ALL",
            "LC_CTYPE",
            "JAVA_HOME",
            "DISPLAY",
            "XAUTHORITY",
            "DBUS_SESSION_BUS_ADDRESS",
            "TERM",
        )
        environment = {
            key: os.environ[key] for key in inherited_keys if key in os.environ
        }
        campaign_cache = roots.cache / "campaigns" / campaign_id
        mkdir_private(campaign_cache)
        mkdir_private(campaign_cache / "tmp")
        mkdir_private(campaign_cache / "gradle")
        apply_default_acl(campaign_cache)
        apply_default_acl(campaign_cache / "tmp")
        apply_default_acl(campaign_cache / "gradle")
        settings = campaign / "config" / "ghidra"
        mkdir_private(settings)
        java_options = " ".join(
            (
                f"-Duser.home={campaign / 'home'}",
                f"-Djava.io.tmpdir={campaign_cache / 'tmp'}",
                f"-Dapplication.settingsdir={settings}",
                f"-Dapplication.cachedir={campaign_cache}",
                f"-Dapplication.tempdir={campaign_cache / 'tmp'}",
            )
        )
        environment.update(
            {
                "HOME": str(campaign / "home"),
                "XDG_CONFIG_HOME": str(campaign / "config"),
                "XDG_CACHE_HOME": str(campaign_cache),
                "XDG_DATA_HOME": str(campaign / "data"),
                "XDG_STATE_HOME": str(campaign / "state"),
                "XDG_RUNTIME_DIR": str(campaign / "runtime"),
                "TMPDIR": str(campaign_cache / "tmp"),
                "TMP": str(campaign_cache / "tmp"),
                "TEMP": str(campaign_cache / "tmp"),
                "GRADLE_USER_HOME": str(campaign_cache / "gradle"),
                "_JAVA_OPTIONS": java_options,
                "MC_LAB_CAMPAIGN_ID": campaign_id,
                "MC_LAB_CAMPAIGN_ROOT": str(campaign),
                "MC_LAB_PROJECT_ROOT": str(project),
                "MC_LAB_CACHE_ROOT": str(campaign_cache),
            }
        )
        completed = subprocess.run(command, cwd=project, env=environment, check=False)
    audit_layout(roots)
    receipt = record_event(
        roots,
        "receipts",
        "run-locked",
        {
            "campaign_id": campaign_id,
            "command": command,
            "started_at": started,
            "returncode": completed.returncode,
        },
    )
    print(
        json.dumps(
            {
                "schema": SCHEMA,
                "status": "COMMAND_FINISHED",
                "campaign_id": campaign_id,
                "returncode": completed.returncode,
                "receipt": str(receipt),
            },
            sort_keys=True,
        )
    )
    return completed.returncode


def cleanup_cache(roots: Roots, campaign_id: str) -> dict[str, object]:
    require_initialized(roots)
    campaign_id = validate_id(campaign_id, "campaign id")
    campaign = roots.data / "campaigns" / campaign_id
    if not campaign.is_dir():
        raise LabError("CAMPAIGN_MISSING", f"campaign is missing: {campaign_id}")
    cache = roots.cache / "campaigns" / campaign_id
    require_below(cache, roots.cache)
    if cache.exists():
        for path, _ in walk_regular(cache):
            require_below(path, roots.cache)
        shutil.rmtree(cache)
    mkdir_private(cache)
    mkdir_private(cache / "tmp")
    mkdir_private(cache / "gradle")
    apply_default_acl(cache)
    apply_default_acl(cache / "tmp")
    apply_default_acl(cache / "gradle")
    receipt = record_event(
        roots,
        "receipts",
        "cleanup-cache",
        {"campaign_id": campaign_id},
    )
    return {
        "schema": SCHEMA,
        "status": "CACHE_CLEAN",
        "campaign_id": campaign_id,
        "receipt": str(receipt),
    }


def audit_layout(roots: Roots) -> dict[str, object]:
    ensure_disjoint_roots(roots)
    violations: list[str] = []
    entry_count = 0
    for name, root in roots.named():
        if not root.is_dir():
            violations.append(f"missing root {name}")
            continue
        root_info = ensure_owner(root)
        if stat.S_IMODE(root_info.st_mode) & 0o077:
            violations.append(f"root exposes group/other access: {name}")
        try:
            verify_default_acl(root)
        except LabError as error:
            violations.append(str(error))
        for path, info in walk_regular(root):
            entry_count += 1
            if info.st_uid != os.getuid():
                violations.append(f"wrong owner: {path}")
            if stat.S_IMODE(info.st_mode) & 0o077:
                violations.append(f"group/other access: {path}")
            sealed_baseline = False
            if path.is_relative_to(roots.data / "baselines"):
                relative = path.relative_to(roots.data / "baselines")
                if len(relative.parts) >= 1:
                    sealed_baseline = True
                    expected = (
                        0o500 if stat.S_ISDIR(info.st_mode) else 0o400
                    )
                    if stat.S_IMODE(info.st_mode) != expected:
                        violations.append(f"unsealed baseline entry: {path}")
            if stat.S_ISDIR(info.st_mode) and not sealed_baseline:
                try:
                    verify_default_acl(path)
                except LabError as error:
                    violations.append(str(error))
    if violations:
        raise LabError(
            "LAYOUT_AUDIT_FAILED",
            "; ".join(violations[:20]),
        )
    return {
        "schema": SCHEMA,
        "status": "AUDIT_PASS",
        "entries": entry_count,
        "roots": {name: str(path) for name, path in roots.named()},
    }


def inventory_all(roots: Roots) -> dict[str, object]:
    require_initialized(roots)
    audit_layout(roots)
    root_records: dict[str, object] = {}
    digest = hashlib.sha256()
    for name, root in roots.named():
        record = tree_inventory(root)
        root_records[name] = record
        digest.update(name.encode())
        digest.update(b"\0")
        digest.update(str(record["tree_sha256"]).encode())
        digest.update(b"\n")
    return {
        "schema": SCHEMA,
        "status": "INVENTORY",
        "roots": root_records,
        "inventory_sha256": digest.hexdigest(),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--test-base",
        type=Path,
        help="map all four roots below an explicit /tmp synthetic test base",
    )
    subparsers = parser.add_subparsers(dest="operation", required=True)
    subparsers.add_parser("init")
    subparsers.add_parser("audit")
    subparsers.add_parser("inventory")

    seal = subparsers.add_parser("seal-baseline")
    seal.add_argument("--baseline", required=True)

    clone = subparsers.add_parser("clone-campaign")
    clone.add_argument("--baseline", required=True)
    clone.add_argument("--campaign", required=True)

    locked = subparsers.add_parser("run-locked")
    locked.add_argument("--campaign", required=True)
    locked.add_argument("--timeout", type=float, default=10.0)
    locked.add_argument("command", nargs=argparse.REMAINDER)

    cleanup = subparsers.add_parser("cleanup-cache")
    cleanup.add_argument("--campaign", required=True)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    roots: Roots | None = None
    old_umask = os.umask(0o077)
    try:
        roots = Roots.discover(args.test_base)
        if args.operation == "init":
            result = init_layout(roots)
        elif args.operation == "audit":
            require_initialized(roots)
            result = audit_layout(roots)
        elif args.operation == "inventory":
            result = inventory_all(roots)
        elif args.operation == "seal-baseline":
            result = seal_baseline(roots, args.baseline)
        elif args.operation == "clone-campaign":
            result = clone_campaign(roots, args.baseline, args.campaign)
        elif args.operation == "run-locked":
            if args.timeout < 0 or args.timeout > 3600:
                raise LabError("INVALID_TIMEOUT", "timeout must be 0-3600 seconds")
            return run_locked(roots, args.campaign, args.timeout, args.command)
        elif args.operation == "cleanup-cache":
            result = cleanup_cache(roots, args.campaign)
        else:
            raise AssertionError(args.operation)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except LabError as error:
        evidence: str | None = None
        if roots is not None and (roots.state / "failures").is_dir():
            with contextlib.suppress(Exception):
                evidence = str(
                    record_event(
                        roots,
                        "failures",
                        args.operation,
                        {"error_code": error.code, "message": str(error)},
                    )
                )
        print(
            json.dumps(
                {
                    "schema": SCHEMA,
                    "status": "ERROR",
                    "error_code": error.code,
                    "message": str(error),
                    "failure_evidence": evidence,
                },
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        return error.exit_code
    finally:
        os.umask(old_umask)


if __name__ == "__main__":
    raise SystemExit(main())
