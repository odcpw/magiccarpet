#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
trace_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$trace_root/../../.." && pwd)
upstream_root="$repo_root/upstream/magic-carpet-2-hd"
build_root=${MC2_FRAME_TRACE_BUILD_DIR:-/tmp/magiccarpet-remc2-frame-trace-build}

cmake -S "$upstream_root" -B "$build_root" \
    -DCMAKE_BUILD_TYPE=Debug \
    -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
    -DCMAKE_PROJECT_INCLUDE="$trace_root/cmake/inject-frame-trace.cmake" \
    -DMC2_FRAME_TRACE_ROOT="$trace_root" \
    -DBUILD_TESTING=OFF
cmake --build "$build_root" --target remc2-frame-trace --parallel "${MC2_BUILD_JOBS:-2}"
python3 "$trace_root/scripts/write_build_manifest.py" \
    --repo-root "$repo_root" \
    --upstream-root "$upstream_root" \
    --trace-root "$trace_root" \
    --build-root "$build_root" \
    --binary "$build_root/remc2-frame-trace"
