#!/usr/bin/env python3
"""Retain owner-local PCM for the first four MC2 campaign speech tracks."""
import argparse
import hashlib
import json
from pathlib import Path
import wave


def build(source: Path, destination: Path):
    tracks = []
    payloads = []
    for track in (1, 2, 3, 4):
        name = f"TRACK{track:02d}.wav"
        path = source / name
        data = path.read_bytes()
        with wave.open(str(path), "rb") as audio:
            if (audio.getnchannels(), audio.getsampwidth(), audio.getframerate(),
                    audio.getcomptype()) != (2, 2, 44100, "NONE"):
                raise ValueError(f"{path}: expected source stereo PCM16 at 44100 Hz")
            frames = audio.getnframes()
            if len(audio.readframes(frames)) != frames * 4:
                raise ValueError(f"{path}: truncated speech PCM")
        tracks.append(dict(track=track, path=name, bytes=len(data),
                           sha256=hashlib.sha256(data).hexdigest(), frames=frames))
        payloads.append((name, data))
    # Validate every input before publishing; existing owner outputs are preserved.
    destination.mkdir(parents=True, exist_ok=False)
    for name, data in payloads:
        with (destination / name).open("xb") as output:
            output.write(data)
    manifest = dict(schema="mc2-campaign-speech/v1", sample_rate=44100,
                    channels=2, sample_width=2, tracks=tracks)
    with (destination / "speech.json").open("x") as output:
        json.dump(manifest, output, indent=2, sort_keys=True)
        output.write("\n")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.source, args.output), sort_keys=True))
