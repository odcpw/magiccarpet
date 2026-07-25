#!/usr/bin/env bash
set -euo pipefail

probe_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
lab_root=$(cd -- "${probe_dir}/../../../.." && pwd)
source_root="${lab_root}/upstream/magic-carpet-2-hd"
recording="${lab_root}/research/private/playtests/legion-remc2-native-6806c72-2026-07-24/playtest-level0-input.bin"
artifact_dir="${probe_dir}/artifacts"

unity_emscripten_root="/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Data/PlaybackEngines/WebGLSupport/BuildTools/Emscripten"
em_config="${unity_emscripten_root}/.emscripten"
emxx="${unity_emscripten_root}/emscripten/em++"
node="${unity_emscripten_root}/node/node"

test -f "${recording}"
test -f "${em_config}"
test -x "${emxx}"
test -x "${node}"
install -d -m 700 "${artifact_dir}"

common_sources=(
	"${probe_dir}/recording_probe.cpp"
	"${source_root}/remc2/portability/InputRecorder.cpp"
)
common_flags=(
	-std=c++17
	-O2
	-I"${source_root}/remc2"
)

g++ "${common_flags[@]}" "${common_sources[@]}" \
	-o "${artifact_dir}/recording-probe-native"

EM_CONFIG="${em_config}" "${emxx}" \
	"${common_flags[@]}" \
	"${common_sources[@]}" \
	--preload-file "${recording}@/input/playtest-level0-input.bin" \
	-sASSERTIONS=1 \
	-sENVIRONMENT=node \
	-sEXIT_RUNTIME=1 \
	-sFORCE_FILESYSTEM=1 \
	-o "${artifact_dir}/recording-probe.js"

"${artifact_dir}/recording-probe-native" "${recording}" \
	> "${artifact_dir}/native-result.json"
(
	cd -- "${artifact_dir}"
	"${node}" recording-probe.js /input/playtest-level0-input.bin \
		> wasm-result.json
)

diff -u \
	"${artifact_dir}/native-result.json" \
	"${artifact_dir}/wasm-result.json"

sha256sum \
	"${recording}" \
	"${artifact_dir}/recording-probe-native" \
	"${artifact_dir}/recording-probe.js" \
	"${artifact_dir}/recording-probe.wasm" \
	"${artifact_dir}/recording-probe.data" \
	"${artifact_dir}/native-result.json" \
	"${artifact_dir}/wasm-result.json"

echo "REMC2_INPUT_RECORDING_NATIVE_WASM_MATCH"
