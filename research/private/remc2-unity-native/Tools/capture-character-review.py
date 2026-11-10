#!/usr/bin/env python3
"""Capture the actual character player on a private display, preserving all output.

Defaults to a portrait 1080x1920 full sequence. --still accepts
Archer,StandingShot,2.35,25 (character, clip, seconds, camera yaw).
Successful stdout is one mc-character-capture/v1 JSON object; errors are JSON
on stderr with exit 1. Rendering/readback cadence is not a performance result.
"""
import argparse
import hashlib
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
    parser.add_argument("--review-character", choices=("Villager", "VillagerBrown", "VillagerStriped", "VillagerCarrier"),
                        help="Record this villager's complete five-action sequence in the isolated review player")
    parser.add_argument("--gameplay-command", type=Path, help="JSON with actual campaign launch arguments and provenance")
    parser.add_argument("--follow", default="Goat", help="Goat, Archer or native source slot; Player keeps the normal gameplay camera")
    parser.add_argument("--frames", type=int, default=360, help="Native gameplay capture frame count")
    parser.add_argument("--fps", type=int, choices=(30, 60, 120), default=30, help="Gameplay recording cadence; native simulation stays at 30Hz")
    parser.add_argument("--width", type=int, default=1080)
    parser.add_argument("--height", type=int, default=1920)
    parser.add_argument("--timeout", type=float, default=600)
    args = parser.parse_args()
    if args.gameplay_command and args.still:
        raise ValueError("Gameplay and isolated still capture are separate modes")
    if args.review_character and (args.gameplay_command or args.still):
        raise ValueError("Full character review cannot be combined with gameplay or a still")
    if not 30 <= args.frames <= 3600:
        raise ValueError("--frames must be 30..3600")
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
    # A private X display has no desktop PipeWire session. FMOD/ALSA can hang
    # while shutting down that failed connection; this visual capture uses a
    # process-local null PCM instead of touching the user's audio settings.
    alsa = runtime / "alsa-null.conf"
    alsa.write_text("pcm.!default { type null }\n", encoding="utf-8")
    env["ALSA_CONFIG_PATH"] = str(alsa)
    receipt = {"schema": "mc-character-capture/v1", "status": "starting", "actual_desktop": False,
               "performance_measurement": False, "player": str(args.player.resolve())}
    player_path = args.player.resolve()
    data_path = player_path.with_name(player_path.stem + "_Data")
    artifacts = [player_path, player_path.parent / "build-install.json",
                 data_path / "Plugins/x86_64/libsim_capi.so",
                 data_path / "Managed/MagicCarpet.Native.Runtime.dll"]
    def artifact_hashes():
        result = {}
        for path in artifacts:
            if path.is_file():
                with path.open("rb") as stream:
                    result[str(path)] = hashlib.file_digest(stream, "sha256").hexdigest()
        return result
    receipt["artifact_hashes"] = artifact_hashes()
    display = player = None
    try:
        read_fd, write_fd = os.pipe()
        try:
            with (output / "display.log").open("w") as log:
                display = subprocess.Popen(["Xvfb", "-displayfd", str(write_fd), "-screen", "0",
                    f"{args.width}x{args.height}x24", "-nolisten", "tcp"], pass_fds=(write_fd,),
                    stdout=log, stderr=subprocess.STDOUT, start_new_session=True, env=env)
            os.close(write_fd); write_fd = None
            # Xvfb can write the digits and newline separately. Closing after
            # a partial read can break its final write and kill the display.
            deadline = time.monotonic() + 20
            selected = b""
            while b"\n" not in selected:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not select.select([read_fd], [], [], remaining)[0]:
                    raise TimeoutError("Private Xvfb failed to allocate a display")
                chunk = os.read(read_fd, 32)
                if not chunk:
                    raise RuntimeError("Private Xvfb closed its readiness pipe")
                selected += chunk
                if len(selected) > 32:
                    raise RuntimeError("Private display readiness message is invalid")
            number = selected.decode().strip()
            if not number.isdigit():
                raise RuntimeError("Private display number is invalid")
        finally:
            os.close(read_fd)
            if write_fd is not None: os.close(write_fd)
        env["DISPLAY"] = ":" + number
        launch = []
        if args.gameplay_command:
            launch = json.loads(args.gameplay_command.read_text())["arguments"]
            if not isinstance(launch, list) or not all(isinstance(value, str) for value in launch) or "-mc-game-runtime" not in launch or "-mc-modern" not in launch:
                raise ValueError("Gameplay arguments must launch the actual Modern game runtime")
        command = [str(args.player.resolve()), *launch, "-force-vulkan", "-screen-fullscreen", "0",
                   "-screen-width", str(args.width), "-screen-height", str(args.height),
                   "-logFile", str(output / "player.log")]
        if args.gameplay_command:
            command += ["-mc-character-gameplay-capture", str(output / "frames"),
                        "-mc-character-gameplay-frames", str(args.frames),
                        "-mc-character-gameplay-fps", str(args.fps)]
            if args.follow != "Player":
                command += ["-mc-character-follow", args.follow]
        else:
            command += ["-mc-character-capture", str(output / "frames")]
        if args.still:
            command += ["-mc-character-still", args.still]
        if args.review_character:
            command += ["-mc-character-review-appearance", args.review_character]
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
        if not args.gameplay_command and capture["framing_failures"] != 0:
            raise RuntimeError("Character or held prop left its viewing area; inspect retained frames")
        if args.gameplay_command:
            records = [json.loads(line) for line in (output / "frames/native-frames.jsonl").read_text().splitlines()]
            expected_ticks = (len(records) - 1) * 30 / capture["fps"]
            if capture.get("native_gameplay") is not True or len(records) != capture["frames"] or max(r["tick"] for r in records) - min(r["tick"] for r in records) < expected_ticks * .9:
                raise RuntimeError("Native gameplay did not advance through the captured sequence")
            if args.follow != "Player" and any(r.get("framingChecked") is not True or r.get("framingValid") is not True for r in records):
                raise RuntimeError("Selected live character or held prop left the viewing area; inspect retained frames")
            receipt["scenario"] = str(args.gameplay_command.resolve())
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
        if receipt["artifact_hashes"] != artifact_hashes():
            raise RuntimeError("Player artifacts changed during recording")
        receipt["status"] = "passed"
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.SubprocessError, KeyError) as error:
        receipt.update(status="failed", error=type(error).__name__, message=str(error))
        raise
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
