# Level 2 authored/reset census

`level2-authored-reset-census.json` is the machine-readable Level 2 denominator
for the pinned remc2 specification (`6806c7204d17220ecc0430759f784aea8ee2ddb2`).
It is derived from two qualified external-2/internal-1 Day exports:

* `research/private/generated/l2-debug3.fJ3PaM/level2`
* `research/private/generated/mc2-l2-native2.Ibpz9b/out`

The builder compares the stable census members byte-for-byte, retains hashes
and source provenance for every package member, and emits complete authored,
reset, wizard/player, stage/objective, relation, constructor, parameter, and
presentation records.  The authored partition explicitly distinguishes the
755 null rows, 158 group-zero-consumed rows, 246 still-nonzero dormant rows, 19
terrain auxiliaries, and 22 sentinel rows.  This is a denominator/reset
contract; it makes no simulation-completeness claim.

Rebuild and validate:

```sh
python3 research/private/remc2-sim-rs/level2-conformance/census/build_level2_census.py \
  research/private/generated/l2-debug3.fJ3PaM/level2 \
  research/private/generated/mc2-l2-native2.Ibpz9b/out \
  --output research/private/remc2-sim-rs/level2-conformance/census/level2-authored-reset-census.json
python3 research/private/remc2-sim-rs/level2-conformance/census/build_level2_census.py \
  --validate research/private/remc2-sim-rs/level2-conformance/census/level2-authored-reset-census.json
python3 -m unittest discover \
  -s research/private/remc2-sim-rs/level2-conformance/census -p 'test_*.py'
```
