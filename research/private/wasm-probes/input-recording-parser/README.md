# remc2 input-recording parser: native/WebAssembly parity probe

This is a private, deliberately narrow cross-architecture proof. It compiles
the pinned remc2 `InputRecorder.cpp` implementation twice:

1. native x86-64 with the host C++ compiler; and
2. WebAssembly with Unity 6.0's bundled Emscripten 3.1.39-git toolchain.

Both builds parse the exact deterministic Level 1 recording captured on the
Legion. The harness hashes spell metadata and every sorted turn payload using
a byte-defined FNV-1a 64-bit stream, so host endianness and integer layout
cannot silently change the comparison.

## Result

The native and WebAssembly JSON outputs are byte-identical:

```json
{"loaded":true,"level":0,"player":0,"declared_turns":272,"parsed_turns":272,"first_turn":1,"last_turn":272,"payload_bytes":2720,"fnv1a64":"2282f2c4ee21151f"}
```

The verification signal is:

```text
REMC2_INPUT_RECORDING_NATIVE_WASM_MATCH
```

Relevant hashes:

| Artifact | SHA-256 |
| --- | --- |
| native recording | `0d1dc0d60876d7280967390f7e187eff50507aecdc2362992011cd69f16b3ab4` |
| native probe | `3c52e46ab615ccdffd3dc9c9644cc5bcaa2999c70e382e5ddf6eef91deadbda9` |
| JavaScript loader | `3e960775efc4ac302c78b048a3cf063e619630e2113b8aa0df34ef9557b73790` |
| WebAssembly module | `2f7f1651de947994b883d498ab787efa8f6e7e09dc9d3b4c7ad2df5c1476e256` |
| native result | `7daf4e2573bd01dec77281b4e1dee88374b77fefa93aef803bb00f1787e97784` |
| WebAssembly result | `7daf4e2573bd01dec77281b4e1dee88374b77fefa93aef803bb00f1787e97784` |

Run from the lab root:

```bash
bash research/private/wasm-probes/input-recording-parser/build-and-verify.sh
```

## Scope and boundary

This proves that one real remc2 persistence component reads the native
recording identically after compilation to WebAssembly. It does not yet prove
that the MC2 simulation, renderer, audio, timer, or main loop runs in a browser.
The generated loader currently targets Node to make the first comparison
deterministic and automation-friendly.

The `.data` file contains the private input recording and must not be
distributed. The eventual browser build must instead ask the owner to select
their local files and mount them into the browser filesystem after validation.

The upstream parser emitted three useful warnings during the WebAssembly
build: a null character expressed as `NULL`, an abstract event base deleted
without a virtual destructor, and an incomplete `GameState` switch. None
changed this probe's result, but they should be reviewed before the parser is
promoted into a long-lived browser runtime.
