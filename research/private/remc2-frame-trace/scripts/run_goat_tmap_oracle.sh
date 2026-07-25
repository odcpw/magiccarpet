#!/bin/sh
set -eu
umask 077

if [ "$#" -ne 1 ]; then
    echo "usage: run_goat_tmap_oracle.sh OUTPUT_DIRECTORY" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
trace_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
repo_root=$(CDPATH= cd -- "$trace_root/../../.." && pwd)
owner_root="$repo_root/research/private/remc2-native-runtime"
build_root=${MC2_FRAME_TRACE_BUILD_DIR:-/tmp/magiccarpet-remc2-frame-trace-build}
binary="$build_root/remc2-frame-trace"
build_manifest="$build_root/frame-trace-build.json"
config="$build_root/frame-trace-runtime-config.json"
output=$1

if [ -e "$output" ]; then
    echo "output already exists: $output" >&2
    exit 2
fi
if [ ! -x "$binary" ] || [ ! -f "$build_manifest" ]; then
    echo "trace target is not built; run scripts/build.sh first" >&2
    exit 2
fi

python3 "$trace_root/scripts/prepare_config.py" \
    --owner-root "$owner_root" \
    --output "$config"

export MC2_GOAT_TMAP_ORACLE_OUTPUT="$output"
export MC2_TRACE_BUILD_MANIFEST="$build_manifest"
export MC2_TRACE_CONFIG="$config"
export MC2_TRACE_BINARY="$binary"
export MC2_TRACE_OWNER_ROOT="$owner_root"
export MC2_TRACE_REPO_ROOT="$repo_root"
export SDL_AUDIODRIVER=dummy
export SDL_VIDEODRIVER=dummy

exec gdb -q -nx -batch -x "$trace_root/goat_tmap_oracle.py" --args "$binary" \
    --mode_test_regressions 1 \
    --text_output_to_console \
    --set_level 0 \
    --config_file_path "$config" \
    --memimages_path "$owner_root/memimages/regressions" \
    --set_max_regressions_steps 0
