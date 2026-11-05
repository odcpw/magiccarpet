#!/usr/bin/env python3
"""Record the real modern player on a private X display, never the desktop.

The review is a presentation demo over the existing NightSpire replay. Video
capture adds CPU/readback cost and is not a frame-performance qualification.
Only processes created by this invocation are stopped. Output is retained on
failure; the directory must be new to avoid overwriting another capture.
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


ROOT = Path(__file__).resolve().parents[4]
DEFAULT_BASELINE = ROOT / "research/private/generated/nightspire-rework-20261105/baseline_shore1-command.json"
WIDTH, HEIGHT = 1600, 900


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("output_dir", type=Path, help="New directory for MP4, logs, command and receipt JSON")
    result.add_argument("--player", type=Path, required=True, help="Qualified MagicCarpetNative.x86_64 executable")
    result.add_argument("--baseline-command", type=Path, default=DEFAULT_BASELINE,
                        help="Existing owner-data/replay command JSON (array or object with player array)")
    result.add_argument("--duration", type=float, default=30, help="Video duration, 5–120 seconds")
    result.add_argument("--fps", type=int, default=30, choices=(24, 30, 60))
    result.add_argument("--ready-delay", type=float, default=5, help="Seconds after MC_WATER_READY before recording")
    result.add_argument("--review-start-delay", type=float, default=8,
                        help="Passed to the player; starts the demo after water initialization")
    result.add_argument("--startup-timeout", type=float, default=120, help="Bounded wait for MC_WATER_READY")
    result.add_argument("--contact-sheet", action="store_true", help="Extract 12 equally spaced video frames to one JPEG")
    return result


def remove_options(command, valued=(), flags=()):
    output = []
    index = 0
    while index < len(command):
        token = command[index]
        if token in valued:
            if index + 1 >= len(command):
                raise ValueError(f"Baseline command has no value for {token}")
            index += 2
        elif token in flags:
            index += 1
        else:
            output.append(token)
            index += 1
    return output


def player_command(args, output):
    baseline = json.loads(args.baseline_command.read_text())
    command = baseline.get("player") if isinstance(baseline, dict) else baseline
    if not isinstance(command, list) or not command or not all(isinstance(value, str) for value in command):
        raise ValueError("Baseline command must be a string array or contain a player string array")
    command = remove_options(command[1:], valued=(
        "-logFile", "-screen-width", "-screen-height", "-screen-fullscreen", "-mc-runtime-replay-save",
        "-mc-modern-capture", "-mc-modern-capture-delay", "-mc-modern-capture-count", "-mc-modern-capture-interval",
        "-mc-modern-review-camera", "-mc-modern-review-orbit", "-mc-modern-warmup", "-mc-water-review-start-delay",
    ), flags=("-force-wayland", "-force-glcore", "-force-vulkan", "-mc-modern-exit-after-capture",
              "-mc-modern-metrics", "-mc-modern-visual-debug", "-mc-water-review"))
    if "-mc-runtime-replay-load" not in command:
        raise ValueError("Baseline command must load the existing NightSpire replay")
    replay_index = command.index("-mc-runtime-replay-load") + 1
    if replay_index >= len(command) or not Path(command[replay_index]).is_file():
        raise ValueError("Baseline NightSpire replay is missing")
    return [str(args.player.resolve()), *command,
            "-force-vulkan", "-screen-fullscreen", "0", "-screen-width", str(WIDTH), "-screen-height", str(HEIGHT),
            "-logFile", str(output / "player.log"), "-mc-runtime-replay-save", str(output / "review-save.json"),
            "-mc-water-review", "-mc-water-review-start-delay", str(args.review_start_delay)]


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def stop_owned(process, graceful=signal.SIGTERM):
    if process is None:
        return
    # Every process below owns a new session. Never target a desktop process or
    # a process selected by name; only the process group created here is eligible.
    try:
        os.killpg(process.pid, graceful)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)


def wait_for_ready(player, log, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if player.poll() is not None:
            raise RuntimeError(f"Player exited before MC_WATER_READY: {player.returncode}")
        if log.exists() and "MC_WATER_READY" in log.read_text(errors="replace"):
            return time.monotonic()
        time.sleep(.1)
    raise TimeoutError(f"Player did not log MC_WATER_READY within {timeout:g} seconds")


def checked_run(command, log, timeout):
    with log.open("w") as stream:
        result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(f"{Path(command[0]).name} exited {result.returncode}; see {log}")


def capture(args):
    if not 5 <= args.duration <= 120 or not 0 <= args.ready_delay <= 30 or not 0 <= args.review_start_delay <= 30:
        raise ValueError("Duration must be 5–120 seconds and delays 0–30 seconds")
    if not 10 <= args.startup_timeout <= 300:
        raise ValueError("Startup timeout must be 10–300 seconds")
    if not args.player.is_file() or not os.access(args.player, os.X_OK):
        raise ValueError("--player must name an executable qualified player")
    for name in ("Xvfb", "ffmpeg", "ffprobe"):
        if shutil.which(name) is None:
            raise RuntimeError(f"Required tool is unavailable: {name}")
    output = args.output_dir.resolve()
    command = player_command(args, output)
    output.mkdir(parents=True, exist_ok=False)
    runtime = output / "private-runtime"
    runtime.mkdir(mode=0o700)
    env = dict(os.environ)
    for name in ("DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "SESSION_MANAGER", "XAUTHORITY"):
        env.pop(name, None)
    env.update(XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(runtime / "config"),
               XDG_CACHE_HOME=str(runtime / "cache"), XDG_SESSION_TYPE="x11", SDL_VIDEODRIVER="x11",
               MC_WIDTH=str(WIDTH), MC_HEIGHT=str(HEIGHT), MC_FULLSCREEN="0")
    receipt = {"schema_version": 1, "status": "starting", "actual_desktop": False,
               "performance_measurement": False, "presentation_review": True,
               "player": str(args.player.resolve()), "output_dir": str(output),
               "duration_requested_seconds": args.duration, "capture_fps": args.fps,
               "width": WIDTH, "height": HEIGHT, "review_start_delay_seconds": args.review_start_delay,
               "ready_delay_seconds": args.ready_delay}
    xvfb = player = encoder = None
    logs = []
    read_fd = write_fd = None
    try:
        read_fd, write_fd = os.pipe()
        display_command = ["Xvfb", "-displayfd", str(write_fd), "-screen", "0", f"{WIDTH}x{HEIGHT}x24", "-nolisten", "tcp"]
        display_log = (output / "xvfb.log").open("w"); logs.append(display_log)
        xvfb = subprocess.Popen(display_command, pass_fds=(write_fd,), stdout=display_log, stderr=subprocess.STDOUT,
                                start_new_session=True, env=env)
        os.close(write_fd); write_fd = None
        if not select.select([read_fd], [], [], 20)[0]:
            raise TimeoutError("Private Xvfb did not select a display within 20 seconds")
        selected = os.read(read_fd, 32).decode().strip()
        os.close(read_fd); read_fd = None
        if not selected.isdigit() or xvfb.poll() is not None:
            raise RuntimeError("Private Xvfb failed to allocate a display")
        display_name = ":" + selected
        env["DISPLAY"] = display_name
        receipt["private_display"] = display_name
        encoder_command = ["ffmpeg", "-nostdin", "-n", "-hide_banner", "-loglevel", "info",
                           "-f", "x11grab", "-framerate", str(args.fps), "-video_size", f"{WIDTH}x{HEIGHT}",
                           "-draw_mouse", "0", "-i", display_name + ".0+0,0", "-t", str(args.duration),
                           "-an", "-c:v", "libx264", "-threads", "4", "-preset", "fast", "-crf", "18",
                           "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(output / "water-review.mp4")]
        write_json(output / "commands.json", {"player": command, "display": display_command,
                   "encoder": encoder_command, "private_display": display_name, "private_runtime": str(runtime),
                   "baseline_command": str(args.baseline_command.resolve())})
        player_log = (output / "player-stdout.log").open("w"); logs.append(player_log)
        player = subprocess.Popen(command, cwd=ROOT, env=env, stdout=player_log, stderr=subprocess.STDOUT,
                                  start_new_session=True)
        ready_at = wait_for_ready(player, output / "player.log", args.startup_timeout)
        print("Water ready on private display; waiting for capture start.", file=sys.stderr, flush=True)
        deadline = ready_at + args.ready_delay
        while time.monotonic() < deadline:
            if player.poll() is not None:
                raise RuntimeError("Player exited during the capture warmup")
            time.sleep(.05)
        encoder_log = (output / "encoder.log").open("w"); logs.append(encoder_log)
        recording_at = time.monotonic()
        receipt["recording_started_after_ready_seconds"] = recording_at - ready_at
        encoder = subprocess.Popen(encoder_command, env=env, stdout=encoder_log, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        deadline = recording_at + args.duration + 30
        while encoder.poll() is None:
            if player.poll() is not None:
                raise RuntimeError(f"Player exited during video capture: {player.returncode}")
            if xvfb.poll() is not None:
                raise RuntimeError("Private display exited during video capture")
            if time.monotonic() > deadline:
                raise TimeoutError("Video encoder exceeded its bounded recording time")
            time.sleep(.1)
        if encoder.returncode:
            raise RuntimeError(f"Video encoder exited {encoder.returncode}; see encoder.log")
        probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
                                "stream=width,height,nb_frames,r_frame_rate:format=duration,size", "-of", "json",
                                str(output / "water-review.mp4")], text=True, capture_output=True, timeout=15, check=True)
        metadata = json.loads(probe.stdout)
        write_json(output / "video-metadata.json", metadata)
        stream = metadata["streams"][0]
        if stream["width"] != WIDTH or stream["height"] != HEIGHT:
            raise RuntimeError("Recorded video dimensions do not match the requested canvas")
        if float(metadata["format"]["duration"]) < args.duration - .5:
            raise RuntimeError("Recorded video ended before the requested duration")
        if args.contact_sheet:
            checked_run(["ffmpeg", "-nostdin", "-n", "-hide_banner", "-loglevel", "error", "-i",
                         str(output / "water-review.mp4"), "-vf",
                         f"fps=12/{args.duration},scale=400:225,tile=4x3", "-frames:v", "1", "-threads", "2",
                         str(output / "contact-sheet.jpg")], output / "contact-sheet.log", 30)
        receipt.update(status="complete", video=str(output / "water-review.mp4"),
                       video_duration_seconds=float(metadata["format"]["duration"]),
                       video_frames=stream.get("nb_frames"), encoder_exit_code=encoder.returncode)
    except BaseException as error:
        receipt.update(status="failed", error_type=type(error).__name__, error=str(error))
        raise
    finally:
        cleanup_errors = []
        for process, graceful in ((encoder, signal.SIGINT), (player, signal.SIGTERM), (xvfb, signal.SIGTERM)):
            try:
                stop_owned(process, graceful)
            except (OSError, subprocess.SubprocessError) as error:
                cleanup_errors.append(str(error))
        for fd in (read_fd, write_fd):
            if fd is not None:
                os.close(fd)
        for stream in logs:
            stream.close()
        receipt["owned_processes_stopped"] = all(process is None or process.poll() is not None for process in (encoder, player, xvfb))
        if cleanup_errors:
            receipt.update(status="failed", cleanup_errors=cleanup_errors)
        write_json(output / "receipt.json", receipt)
        if cleanup_errors:
            raise RuntimeError("Owned process cleanup failed; see receipt.json")
    return receipt


def main():
    args = parser().parse_args()
    try:
        receipt = capture(args)
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.SubprocessError) as error:
        print(json.dumps({"schema_version": 1, "status": "failed", "error_type": type(error).__name__, "error": str(error)}))
        return 1
    print(json.dumps(receipt))
    return 0


if __name__ == "__main__":
    sys.exit(main())
