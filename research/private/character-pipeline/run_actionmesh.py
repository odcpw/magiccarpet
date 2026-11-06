"""Verify pinned model bytes, then reconstruct a sampled motion reference.

Run with the documented ActionMesh Python overlay. Upstream diagnostics go to
stderr; stdout contains one versioned JSON result. Output directories must be new.
"""
import argparse
import contextlib
import hashlib
import json
import os
import runpy
import sys
import time
from pathlib import Path


def verify_checkpoints(repo, manifest, phase):
    verified = []
    for line in manifest.read_text().splitlines():
        expected, relative = line.split(maxsplit=1)
        path = repo / "pretrained_weights" / relative
        with path.open("rb") as file:
            actual = hashlib.file_digest(file, "sha256").hexdigest()
        if actual != expected:
            raise ValueError(f"Checkpoint checksum mismatch ({phase}): {relative}; expected {expected}, read {actual}")
        verified.append(relative)
        print(json.dumps({"version": 1, "status": "checkpoint_verified", "phase": phase,
                          "file": relative}), file=sys.stderr, flush=True)
    return verified


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--frames", type=Path, required=True)
    parser.add_argument("--fps", type=float, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=20261106)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--mesh", type=Path)
    source.add_argument("--native", action="store_true")
    args = parser.parse_args()
    repo, frames, output = (p.resolve() for p in (args.repo, args.frames, args.output))
    if not 0 < args.fps <= 120:
        raise ValueError("--fps must be positive and at most 120")
    count = len(list(frames.glob("*.png")))
    if not 2 <= count <= 31:
        raise ValueError("Supply 2–31 sampled PNG frames; the upstream loader truncates longer inputs")
    if output.exists():
        raise FileExistsError(f"Preserving existing output: {output}")
    script = repo / "inference" / ("video_to_animated_mesh.py" if args.native
                                   else "video_and_3d_to_animated_mesh.py")
    if not script.is_file():
        raise FileNotFoundError(script)
    mesh = args.mesh.resolve() if args.mesh else None
    if mesh and not mesh.is_file():
        raise FileNotFoundError(mesh)
    manifest = Path(__file__).with_name("actionmesh.sha256")
    verified = verify_checkpoints(repo, manifest, "before_inference")
    sys.path[:0] = [str(repo), str(repo / "third_party" / "TripoSG")]
    import torch

    output.mkdir(parents=True)
    report = {"version": 1, "status": "running", "frames": count, "fps": args.fps,
              "input": str(frames), "mesh": str(mesh) if mesh else None,
              "seed": args.seed, "verified_checkpoints": verified,
              "representation": "fixed-topology mesh sequence; not a skeletal rig"}
    receipt = output / "run.json"
    receipt.write_text(json.dumps(report, indent=2) + "\n")
    previous_cwd, previous_argv = Path.cwd(), sys.argv
    sys.argv = [str(script), "--input", str(frames), "--output_dir", str(output),
                "--low_ram", "--seed", str(args.seed)]
    if mesh:
        sys.argv.extend(["--mesh_input", str(mesh)])
    started = time.monotonic()
    try:
        os.chdir(repo)
        with contextlib.redirect_stdout(sys.stderr):
            runpy.run_path(str(script), run_name="__main__")
        report["verified_after_inference"] = verify_checkpoints(repo, manifest, "after_inference")
        report["status"] = "complete"
    except BaseException:
        report["status"] = "failed"
        raise
    finally:
        os.chdir(previous_cwd)
        sys.argv = previous_argv
        report.update(elapsed_seconds=time.monotonic() - started,
                      peak_allocated_GiB=torch.cuda.max_memory_allocated() / 2**30,
                      peak_reserved_GiB=torch.cuda.max_memory_reserved() / 2**30)
        receipt.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"version": 1, "status": "complete", "output": str(output)}))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(json.dumps({"version": 1, "status": "error", "type": type(error).__name__,
                          "message": str(error)}), file=sys.stderr)
        raise SystemExit(1) from None
