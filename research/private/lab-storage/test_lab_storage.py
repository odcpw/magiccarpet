#!/usr/bin/env python3
"""Synthetic acceptance test for the owner-only analyzer storage."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path


HERE = Path(__file__).resolve().parent
CLI = HERE / "lab_storage.py"
BASELINE = "BLD-SYNTH-0001"
CAMPAIGN = "CMP-SYNTH-0001"


def run(
    base: Path,
    *arguments: str,
    expected: int = 0,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        [str(CLI), "--test-base", str(base), *arguments],
        text=True,
        capture_output=True,
        check=False,
    )
    assert completed.returncode == expected, (
        completed.stdout + completed.stderr
    )
    return completed


def mode(path: Path) -> int:
    return stat.S_IMODE(path.lstat().st_mode)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="mc-lab-storage.", dir="/tmp") as raw:
        base = Path(raw)
        init = json.loads(run(base, "init").stdout)
        assert init["status"] == "INITIALIZED"
        for root_name in ("data", "config", "cache", "state"):
            assert mode(base / root_name) == 0o700

        acl = subprocess.run(
            ["getfacl", "-cp", str(base / "data")],
            text=True,
            capture_output=True,
            check=True,
        ).stdout
        assert "default:group::---" in acl
        assert "default:other::---" in acl

        staging = base / "data/staging" / BASELINE / "project"
        staging.mkdir(parents=True)
        (staging / "fixture.gpr").write_bytes(b"synthetic project pointer\n")
        repository = staging / "fixture.rep"
        repository.mkdir()
        (repository / "project.prp").write_bytes(b"synthetic project\n")
        (repository / "idata").mkdir()
        (repository / "idata/index").write_bytes(b"synthetic index\n")

        sealed = json.loads(
            run(base, "seal-baseline", "--baseline", BASELINE).stdout
        )
        assert sealed["status"] == "BASELINE_SEALED"
        baseline = base / "data/baselines" / BASELINE
        assert mode(baseline) == 0o500
        assert mode(baseline / "project") == 0o500
        assert mode(baseline / "project/fixture.gpr") == 0o400
        try:
            (baseline / "project/fixture.gpr").write_bytes(b"changed")
        except PermissionError:
            pass
        else:
            raise AssertionError("sealed baseline remained writable")

        cloned = json.loads(
            run(
                base,
                "clone-campaign",
                "--baseline",
                BASELINE,
                "--campaign",
                CAMPAIGN,
            ).stdout
        )
        assert cloned["status"] == "CAMPAIGN_CLONED"
        campaign_project = base / "data/campaigns" / CAMPAIGN / "project"
        assert mode(campaign_project) == 0o700
        assert mode(campaign_project / "fixture.gpr") == 0o600
        assert (
            campaign_project / "fixture.gpr"
        ).read_bytes() == b"synthetic project pointer\n"
        assert os.stat(
            campaign_project / "fixture.gpr"
        ).st_ino != os.stat(
            baseline / "project/fixture.gpr"
        ).st_ino

        bad_id = "BLD-SYNTH-BAD"
        bad_root = base / "data/staging" / bad_id
        bad = bad_root / "project"
        bad.mkdir(parents=True)
        (bad / "fixture.gpr").write_text("synthetic")
        (bad_root / "outside").symlink_to("/etc/passwd")
        rejected = run(
            base,
            "seal-baseline",
            "--baseline",
            bad_id,
            expected=2,
        )
        assert json.loads(rejected.stderr)["error_code"] == "SYMLINK"
        assert not (base / "data/baselines" / bad_id).exists()
        assert any((base / "state/failures").glob("*.json"))
        (bad_root / "outside").unlink()
        (bad / "fixture.gpr").unlink()
        bad.rmdir()
        bad.parent.rmdir()

        hard_id = "BLD-SYNTH-HARDLINK"
        hard_project = base / "data/staging" / hard_id / "project"
        hard_project.mkdir(parents=True)
        original = hard_project / "original"
        original.write_text("synthetic")
        os.link(original, hard_project.parent / "linked")
        hard_rejected = run(
            base,
            "seal-baseline",
            "--baseline",
            hard_id,
            expected=2,
        )
        assert json.loads(hard_rejected.stderr)["error_code"] == "HARDLINK"
        (hard_project.parent / "linked").unlink()
        original.unlink()
        hard_project.rmdir()
        hard_project.parent.rmdir()

        fifo_id = "BLD-SYNTH-FIFO"
        fifo_project = base / "data/staging" / fifo_id / "project"
        fifo_project.mkdir(parents=True)
        fifo = fifo_project.parent / "special"
        os.mkfifo(fifo)
        fifo_rejected = run(
            base,
            "seal-baseline",
            "--baseline",
            fifo_id,
            expected=2,
        )
        assert json.loads(fifo_rejected.stderr)["error_code"] == "SPECIAL_FILE"
        fifo.unlink()
        fifo_project.rmdir()
        fifo_project.parent.rmdir()

        marker = campaign_project / "lock-held"
        holder_code = (
            "from pathlib import Path; import time; "
            "Path('lock-held').write_text('held'); time.sleep(1.2)"
        )
        holder = subprocess.Popen(
            [
                str(CLI),
                "--test-base",
                str(base),
                "run-locked",
                "--campaign",
                CAMPAIGN,
                "--timeout",
                "2",
                "--",
                sys.executable,
                "-c",
                holder_code,
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        deadline = time.monotonic() + 5
        while not marker.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        assert marker.exists(), "first writer never acquired the campaign lock"
        contention = run(
            base,
            "run-locked",
            "--campaign",
            CAMPAIGN,
            "--timeout",
            "0.1",
            "--",
            sys.executable,
            "-c",
            "raise SystemExit(0)",
            expected=73,
        )
        assert json.loads(contention.stderr)["error_code"] == "LOCK_BUSY"
        holder_stdout, holder_stderr = holder.communicate(timeout=5)
        assert holder.returncode == 0, holder_stdout + holder_stderr

        run(
            base,
            "run-locked",
            "--campaign",
            CAMPAIGN,
            "--timeout",
            "1",
            "--",
            sys.executable,
            "-c",
            "from pathlib import Path; Path('after-lock').write_text('ok')",
        )
        assert (campaign_project / "after-lock").read_text() == "ok"
        assert mode(campaign_project / "after-lock") == 0o600

        environment_probe = (
            "import json, os; from pathlib import Path; "
            "Path('environment.json').write_text(json.dumps({"
            "k: os.environ[k] for k in "
            "('HOME','XDG_CONFIG_HOME','XDG_CACHE_HOME','XDG_DATA_HOME',"
            "'XDG_STATE_HOME','XDG_RUNTIME_DIR','TMPDIR','TMP','TEMP',"
            "'GRADLE_USER_HOME','_JAVA_OPTIONS','MC_LAB_PROJECT_ROOT')"
            "} | {'ambient_leak': os.environ.get('MC_SYNTHETIC_LEAK')}))"
        )
        os.environ["MC_SYNTHETIC_LEAK"] = "must-not-pass"
        run(
            base,
            "run-locked",
            "--campaign",
            CAMPAIGN,
            "--timeout",
            "1",
            "--",
            sys.executable,
            "-c",
            environment_probe,
        )
        environment = json.loads(
            (campaign_project / "environment.json").read_text()
        )
        campaign_root = base / "data/campaigns" / CAMPAIGN
        cache_root = base / "cache/campaigns" / CAMPAIGN
        expected_paths = {
            "HOME": campaign_root / "home",
            "XDG_CONFIG_HOME": campaign_root / "config",
            "XDG_CACHE_HOME": cache_root,
            "XDG_DATA_HOME": campaign_root / "data",
            "XDG_STATE_HOME": campaign_root / "state",
            "XDG_RUNTIME_DIR": campaign_root / "runtime",
            "TMPDIR": cache_root / "tmp",
            "TMP": cache_root / "tmp",
            "TEMP": cache_root / "tmp",
            "GRADLE_USER_HOME": cache_root / "gradle",
            "MC_LAB_PROJECT_ROOT": campaign_project,
        }
        for name, expected_path in expected_paths.items():
            assert environment[name] == str(expected_path)
        expected_java_options = " ".join(
            (
                f"-Duser.home={campaign_root / 'home'}",
                f"-Djava.io.tmpdir={cache_root / 'tmp'}",
                f"-Dapplication.settingsdir={campaign_root / 'config/ghidra'}",
                f"-Dapplication.cachedir={cache_root}",
                f"-Dapplication.tempdir={cache_root / 'tmp'}",
            )
        )
        assert environment["_JAVA_OPTIONS"] == expected_java_options
        assert environment["ambient_leak"] is None

        cache = base / "cache/campaigns" / CAMPAIGN
        (cache / "discard.me").write_text("synthetic cache")
        cleaned = json.loads(
            run(base, "cleanup-cache", "--campaign", CAMPAIGN).stdout
        )
        assert cleaned["status"] == "CACHE_CLEAN"
        assert {path.name for path in cache.iterdir()} == {"tmp", "gradle"}
        assert mode(cache) == 0o700
        run(
            base,
            "run-locked",
            "--campaign",
            CAMPAIGN,
            "--timeout",
            "1",
            "--",
            sys.executable,
            "-c",
            "from pathlib import Path; Path('after-cleanup').write_text('ok')",
        )
        assert (campaign_project / "after-cleanup").read_text() == "ok"

        audit = json.loads(run(base, "audit").stdout)
        assert audit["status"] == "AUDIT_PASS"
        first = json.loads(run(base, "inventory").stdout)
        second = json.loads(run(base, "inventory").stdout)
        assert first == second
        assert len(first["inventory_sha256"]) == 64

    print(
        "LAB_STORAGE_ACCEPTANCE_PASS "
        "baseline=sealed campaign=isolated lock=exclusive acl=owner-only"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
