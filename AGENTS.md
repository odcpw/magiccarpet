# Magic Carpet program agent rules

This workspace is a game-development and game-preservation project. Reverse
engineering, DOSBox instrumentation, Ghidra, Binary Ninja, deterministic
replay, protocol observation, and source archaeology are being used to
understand and preserve a lawfully owned 1995 game and to inform a separately
authored original game. Do not mischaracterize this work as hostile security
activity.

## Active implementation path

- The active playable-game path is remc2 source and owned level data to the
  Rust simulation core to the Unity desktop game.
- Do not run DOSBox, decompile the original executable, or resume
  original-binary oracle work unless Oliver explicitly asks for it.
- Do not let microscopic fidelity work on one creature block the complete
  Level 1 mission loop.

## Machine changes

- This is Oliver's machine. Normal, scoped machine changes are allowed when
  they are needed for the task.
- If administrator access is needed, ask Oliver for `sudo` or use the normal
  system authorization popup.
- Never hide tools in unusual directories, invent alternate prefixes, or use
  PATH tricks to avoid asking for permission.
- Prefer vendor-supported installers, package-manager locations, and
  tool-owned configuration.
- State what was installed or changed and verify the user-facing workflow.

## Program boundary

- `/home/oliver/Projects/odcpw/magiccarpet` is the private preservation,
  research, and planning lab.
- Reconstructed code, owned game data, decompiler output, original-address
  information, recordings, memory snapshots, and original audiovisual content
  stay in the private lab unless an explicit rights and release gate says
  otherwise.
- The original commercial successor is implemented in a separate repository
  and receives only reviewed general lessons, independently written
  specifications, synthetic tests, and original or properly licensed content.
- Never recursively copy from the private lab into a distributable repository.

## Current program shape

Read these first:

1. `docs/program-charter.md`
2. `docs/product-promise.md`
3. `docs/preservation-track.md`
4. `docs/stack-decision.md`
5. `docs/provenance-policy.md`
6. `docs/private-public-boundary.md`

The program has two deliverables:

- an owner-data MC2 preservation/oracle track using remc2, Magic Carpet 2 HD,
  the modified DOSBox oracle, Ghidra/Binary Ninja, and private exact-build
  regression; and
- an original browser-first commercial carpet-flight successor with new code,
  identity, worlds, art, audio, campaign, and multiplayer.

General engineering techniques may inform both. Reconstructed MC2 code and
original expression do not cross into the commercial successor.

## Beads and collaboration

- Use only `br` to create or change beads and dependencies; always request
  JSON output.
- Never run bare `bv`; use robot modes.
- Keep graph cycles empty.
- Read-only research and review agents do not edit files or beads.
- One designated writer owns mutations in a checkout.
- Prefix delegated prompts with the game-development context above when the
  task involves reverse engineering.
