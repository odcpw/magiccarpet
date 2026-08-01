#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exporter_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$exporter_root/../../.." && pwd)
upstream_root="$repo_root/upstream/magic-carpet-2-hd"
source_root=${MC2_LEVEL_EXPORTER_SOURCE_ROOT:-$upstream_root}
build_root=${MC2_LEVEL_EXPORTER_BUILD_DIR:-/tmp/magiccarpet-remc2-level-exporter-build}
pinned_commit=6806c7204d17220ecc0430759f784aea8ee2ddb2

if ! git -C "$source_root" diff --quiet HEAD -- ||
   ! git -C "$source_root" diff --quiet --cached HEAD -- ||
   [ -n "$(git -C "$source_root" status --porcelain --untracked-files=all)" ]; then
    cat >&2 <<EOF
remc2-level-exporter: refusing to compile a dirty source checkout:
  $source_root
Create a clean checkout/archive at the pinned remc2 commit and set
MC2_LEVEL_EXPORTER_SOURCE_ROOT to that path before running this script.
EOF
    exit 2
fi
upstream_commit=$(git -C "$source_root" rev-parse HEAD)
if [ "$upstream_commit" != "$pinned_commit" ]; then
    echo "remc2-level-exporter: source checkout must be pinned to $pinned_commit (got $upstream_commit)" >&2
    exit 2
fi

cmake -S "$source_root" -B "$build_root" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PROJECT_INCLUDE="$exporter_root/cmake/inject-exporter.cmake" \
    -DMC2_LEVEL_EXPORTER_ROOT="$exporter_root" \
    -DMC2_PRIVATE_ROOT="$repo_root/research/private" \
    -DMC2_UPSTREAM_COMMIT="$upstream_commit" \
    -DBUILD_TESTING=OFF
cmake --build "$build_root" --target remc2-level-exporter --parallel "${MC2_BUILD_JOBS:-2}"
