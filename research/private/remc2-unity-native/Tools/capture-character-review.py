#!/usr/bin/env python3
"""Capture the actual character player on a private display, preserving all output.

Defaults to a portrait 1080x1920 full sequence. --still accepts
Archer,StandingShot,2.35,25 (character, clip, seconds, camera yaw).
Successful stdout is one mc-character-capture/v1 JSON object; errors are JSON
on stderr with exit 1. Rendering/readback cadence is not a performance result.
"""
import argparse
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import time


def stop(process):
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--player", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--still", help="character,clip,seconds,camera-yaw; capture one complete frame")
    parser.add_argument("--width", type=int, default=1080)
    parser.add_argument("--height", type=int, default=1920)
    parser.add_argument("--timeout", type=float, default=600)
    args = parser.parse_args()
    if not args.player.is_file() or not os.access(args.player, os.X_OK):
        raise ValueError("--player must be an executable character player")
    if not 320 <= args.width <= 3840 or not 320 <= args.height <= 3840 or not 10 <= args.timeout <= 1800:
        raise ValueError("Dimensions must be 320..3840 and timeout 10..1800 seconds")
    for command in ("Xvfb", "ffmpeg", "ffprobe"):
        if shutil.which(command) is None:
            raise ValueError("Missing required command: " + command)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    runtime = output / "private-runtime"; runtime.mkdir(mode=0o700)
    env = dict(os.environ)
    for name in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "SESSION_MANAGER", "XAUTHORITY"):
        env.pop(name, None)
    env.update(XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(runtime / "config"),
               XDG_CACHE_HOME=str(runtime / "cache"), XDG_SESSION_TYPE="x11", SDL_VIDEODRIVER="x11")
    receipt = {"schema": "mc-character-capture/v1", "status": "starting", "actual_desktop": False,
               "performance_measurement": False, "player": str(args.player.resolve())}
    display = player = None
    try:
        read_fd, write_fd = os.pipe()
        try:
            with (output / "display.log").open("w") as log:
                display = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0",
                    f"{args.width}x{args.height}x24", "-nolisten", "tcp"], pass_fds=(write_fd,),
                    stdout=log, stderr=subprocess.STDOUT, start_new_session=True, env=env)
            os.close(write_fd); write_fd = None
            if not select.select([read_fd], [], [], 20)[0]:
                raise TimeoutError("Private Xvfb failed to allocate a display")
            number = os.read(read_fd, 32).decode().strip()
            if not number.isdigit():
                raise RuntimeError("Private display number is invalid")
        finally:
            os.close(read_fd)
            if write_fd is not None: os.close(write_fd)
        env["DISPLAY"] = ":" + number
        command = [str(args.player.resolve()), "-force-vulkan", "-screen-fullscreen", "0",
                   "-screen-width", str(args.width), "-screen-height", str(args.height),
                   "-logFile", str(output / "player.log"), "-mc-character-capture", str(output / "frames")]
        if args.still:
            command += ["-mc-character-still", args.still]
        (output / "command.json").write_text(json.dumps(command, indent=2) + "\n")
        with (output / "player-stdout.log").open("w") as log:
            player = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=env, start_new_session=True)
        deadline = time.monotonic() + args.timeout
        print("Rendering actual Unity character frames on a private display.", file=sys.stderr, flush=True)
        while player.poll() is None:
            if time.monotonic() >= deadline:
                raise TimeoutError("Character player exceeded --timeout; retained output: " + str(output))
            time.sleep(.2)
        if player.returncode != 0:
            raise RuntimeError(f"Character player exited {player.returncode}; read {output / 'player.log'}")
        capture_path = output / "frames/capture.json"
        capture = json.loads(capture_path.read_text())
        if capture["framing_failures"] != 0:
            raise RuntimeError("Character or held prop left its viewing area; inspect retained frames")
        frames = sorted((output / "frames").glob("frame-*.png"))
        if len(frames) != capture["frames"] or not frames:
            raise RuntimeError("Captured frame count differs from the player's receipt")
        receipt.update(capture)
        if not args.still:
            video = output / "characters.mp4"
            with (output / "encoder.log").open("w") as log:
                subprocess.run(["ffmpeg", "-nostdin", "-n", "-v", "warning", "-framerate", str(capture["fps"]),
                    "-start_number", "0", "-i", str(output / "frames/frame-%05d.png"), "-c:v", "libx264",
                    "-threads", "4", "-crf", "17", "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(video)],
                    stdout=log, stderr=subprocess.STDOUT, check=True, timeout=300)
                subprocess.run(["ffmpeg", "-v", "error", "-i", str(video), "-f", "null", "-"],
                    stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
            receipt["video"] = str(video)
        else:
            receipt["image"] = str(frames[0])
        receipt["status"] = "passed"
    finally:
        stop(player); stop(display)
        receipt["owned_processes_stopped"] = True
        (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.SubprocessError, KeyError) as error:
        print(json.dumps({"schema": "mc-character-capture/v1", "status": "failed",
                          "error": type(error).__name__, "message": str(error)}), file=sys.stderr)
        sys.exit(1)
