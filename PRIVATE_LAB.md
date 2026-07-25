# Private reference lab — do not publish

This workspace contains owned game installers, private research evidence,
reconstructed-source checkouts, planning, and preservation experiments. It is
the Magic Carpet program's private preservation and oracle lab. It is not the
original commercial successor repository and must never be converted into one.

Allowed here:

- owned binaries, manuals, reference cards, saves, and controlled captures;
- Ghidra/debugger projects and raw reverse-engineering notes;
- remc2, Magic Carpet 2 HD, the modified DOSBox oracle, MagicBalls, and related
  pinned public-code research checkouts;
- private longplay/manual research;
- publication-safe specifications being prepared for reviewed transfer.

Never push this root to a public remote. Never initialize the commercial
successor or a preservation-release repository inside it. The original
successor will use a separate sibling repository created only after the
planning and identity gates approve its path, stack, licenses, fixtures, and
CI controls. Any future preservation release also receives its own cleared
repository and history.

The operational boundary, exact roots, access controls, backup state,
contamination scanner, and recovery procedure are defined in
[the private/public boundary specification](docs/private-public-boundary.md).
In particular, `.gitignore` is defense in depth and not a publication or
access-control boundary.

Read [the complete provenance policy](docs/provenance-policy.md) before
analysis, implementation, asset work, agent delegation, or distribution.
Read [the program charter](docs/program-charter.md) first to understand the
two-track boundary.
