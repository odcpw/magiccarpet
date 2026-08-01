# Rejected cleanup candidates

- `L1-REJ-01`: equal house-table values are independently source-auditable rows, not accidental duplication.
- `L1-REJ-02`: equal exit-phase audio tuples correspond to distinct pinned source phases and must remain visible.
- `L1-REJ-03`: extracting tens of thousands of inline test lines would improve navigation, but it is a bulk file move with no LOC reduction and a disproportionate closure diff. Defer to a dedicated structural pass.
- `L1-REJ-04`: the historical pedantic Clippy set mixes useful findings with exact-port representation choices. Blanket fixes or suppressions would be noise, not elegance.
- `L1-REJ-05`: deleting the obsolete resident-gap method would break the public
  Rust API. Keep a deprecated forwarding alias while moving active callers to
  the canonical name.
- `L1-ISO-02`: the compact distance-loop form preserved runtime behavior but
  hid literal `audio_site` fields from the compiler-bound source census. The
  failing proof test made this a reject, and the source-shaped arms were
  restored.
- `L1-ISO-03`: prototype reduced one match arm but rustfmt produced no net physical-line reduction, so the change was reverted.
