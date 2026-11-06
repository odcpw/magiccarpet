"""Submit once, persist the receipt, and resume a fal generation job.

Credentials stay in the environment; request payloads and results stay local.
Run with the project's .env loaded and a JSON payload containing API inputs.
"""

import argparse
import base64
import json
import os
import shutil
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path


def save_outputs(result, output):
    """Resume interrupted downloads without submitting another paid request."""
    files = [("video", "video.mp4"), ("model_mesh", "model.glb"),
             ("model_glb", "textured.glb"), ("rendered_image", "preview.png"),
             ("rigged_character_glb", "rigged.glb"),
             ("rigged_character_fbx", "rigged.fbx"),
             ("animation_glb", "animation.glb"), ("animation_fbx", "animation.fbx"),
             ("basic_animations.walking_glb", "walking.glb"),
             ("basic_animations.running_glb", "running.glb")]
    for field, filename in files:
        value = result
        for component in field.split("."):
            value = (value or {}).get(component)
        url = (value or {}).get("url")
        destination = output / filename
        if url and not destination.exists():
            if urllib.parse.urlparse(url).scheme != "https":
                raise ValueError("Output download requires HTTPS")
            partial = destination.with_suffix(destination.suffix + ".part")
            with urllib.request.urlopen(url, timeout=120) as response, partial.open("wb") as file:
                shutil.copyfileobj(response, file)
            partial.replace(destination)


def request(url, key, payload=None):
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" or parsed.hostname != "queue.fal.run":
        raise ValueError("Authenticated requests must use the FAL queue host")
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(
        url, data=data,
        headers={"Authorization": f"Key {key}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=120) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--payload", type=Path, required=True)
    parser.add_argument("--image", type=Path)
    parser.add_argument("--image-field", default="image_url")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout-seconds", type=float, default=1800)
    args = parser.parse_args()
    if args.timeout_seconds <= 0:
        parser.error("--timeout-seconds must be positive")
    key = os.environ["FAL_KEY"]
    args.output.mkdir(parents=True, exist_ok=True)
    receipt = args.output / "receipt.json"
    result_path = args.output / "result.json"
    if result_path.exists():
        save_outputs(json.loads(result_path.read_text()), args.output)
        print(json.dumps({"version": 1, "status": "already_complete", "path": str(result_path)}))
        return
    if receipt.exists():
        job = json.loads(receipt.read_text())
    else:
        payload = json.loads(args.payload.read_text())
        if args.image:
            payload[args.image_field] = "data:image/png;base64," + base64.b64encode(args.image.read_bytes()).decode()
        job = request("https://queue.fal.run/" + args.endpoint, key, payload)
        receipt.write_text(json.dumps(job, indent=2) + "\n")
    previous = None
    deadline = time.monotonic() + args.timeout_seconds
    while True:
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Polling timed out; resume the saved receipt at {receipt}")
        state = request(job["status_url"], key)
        status = state["status"]
        if status != previous:
            print(json.dumps({"version": 1, "status": status, "request_id": job["request_id"]}), flush=True)
            previous = status
        if status == "COMPLETED":
            result = request(job["response_url"], key)
            result_path.write_text(json.dumps(result, indent=2) + "\n")
            save_outputs(result, args.output)
            print(json.dumps({"version": 1, "status": "saved", "path": str(result_path)}), flush=True)
            return
        if status not in {"IN_QUEUE", "IN_PROGRESS"}:
            raise RuntimeError(f"Unexpected queue state: {status}")
        time.sleep(min(10, max(0, deadline - time.monotonic())))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print(json.dumps({"version": 1, "status": "error", "type": type(error).__name__,
                          "message": str(error)}), file=sys.stderr)
        raise SystemExit(75 if isinstance(error, TimeoutError) else 1) from None
