#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exporter_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$exporter_root/../../.." && pwd)
upstream_root="$repo_root/upstream/magic-carpet-2-hd"
build_root=${MC2_LEVEL_EXPORTER_BUILD_DIR:-/tmp/magiccarpet-remc2-level-exporter-build}

upstream_commit=$(git -C "$upstream_root" rev-parse HEAD)

cmake -S "$upstream_root" -B "$build_root" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PROJECT_INCLUDE="$exporter_root/cmake/inject-exporter.cmake" \
    -DMC2_LEVEL_EXPORTER_ROOT="$exporter_root" \
    -DMC2_PRIVATE_ROOT="$repo_root/research/private" \
    -DMC2_UPSTREAM_COMMIT="$upstream_commit" \
    -DBUILD_TESTING=OFF
cmake --build "$build_root" --target remc2-level-exporter --parallel "${MC2_BUILD_JOBS:-2}"

