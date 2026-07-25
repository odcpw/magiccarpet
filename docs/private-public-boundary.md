# Private preservation lab and successor implementation boundary

Version: 1.1  
Effective: 2026-07-23  
Status: operational boundary for `mc-t8x`; public repository creation remains
blocked on `mc-ads` and the preproduction approval gate  
Owner: Oliver (`oliver:oliver`)  
Policy basis: [`docs/provenance-policy.md`](provenance-policy.md)

## Decision

The current workspace remains the private reference lab:

`/home/oliver/Projects/odcpw/magiccarpet`

The future publication-safe Track S successor root is frozen as:

`/home/oliver/Projects/odcpw/carpet-flight`

`carpet-flight` is a neutral local working name for the original commercial
successor, not an approved product name
or trademark decision. `mc-ads` may create that directory only after
`mc-w58.15` approves preproduction, Git identity is confirmed as the `odcpw`
project identity, and the checks in this document pass. A later rename is a
recorded migration; it is not permission to reuse either root.

The two roots must never share a Git object database, worktree, Unity
`Library/`, package cache, artifact cache, asset-library mount, or recursive
copy operation. This private root is never converted into the public root.

## Classified local surfaces

| Surface | Classification | Handling |
|---|---|---|
| `/home/oliver/Projects/odcpw/magiccarpet` | `PRIVATE_LAB` | Private policy, research, owned distributions, captures, Beads state, transfer staging |
| `/home/oliver/Projects/odcpw/magiccarpet/upstream/magic-carpet-2-hd` | `PRIVATE_RECONSTRUCTION_RESEARCH` | Pinned remc2/MC2HD evaluation clone; Track P only; never copied into successor |
| `/home/oliver/Games/gog/magic-carpet-2` | `PRIVATE_RUNTIME` | Working MC2 installation and saves; inventory input for `mc-ytu2` |
| `/home/oliver/Games/gog/magic-carpet` | `PRIVATE_RUNTIME` | Working MC1 installation retained for boundary control only; excluded from `mc-ytu2` |
| `/home/oliver/Downloads/Magic Carpet 2 GOG` | `PRIVATE_DUPLICATE` | Byte-identical same-volume duplicate of the four MC2 distribution files |
| `/home/oliver/Downloads/Magic Carpet [GOG]` | `PRIVATE_DUPLICATE` | Byte-identical same-volume duplicate of the two MC1 distribution files |
| `/home/oliver/Projects/odcpw/carpet-flight` | `SUCCESSOR_CANDIDATE` | Does not yet exist; independently authored or approved Track S material only |

As of Oliver’s 2026-07-23 scope decision, MC1 is not a research,
implementation, parity, or content target. Its already-present files remain
listed only so the boundary cannot accidentally leak them. They receive no
new analysis and may be handled by a separately authorized cleanup after a
verified backup; `mc-ytu2` inventories MC2 only.

The duplicate files are useful integrity copies, but they are on the same
encrypted volume and therefore are not a disaster-recovery backup.

## Ownership and permissions

The private lab and each private runtime/duplicate root are owned by
`oliver:oliver`. Their root mode is `0700`. `research/private`, `.beads`, and
future Ghidra/capture roots use directories no broader than `0700`; ordinary
private evidence uses `0600` unless a tool requires an executable bit.
Default ACLs on `research/private` and `.beads` remove group/other access from
new tool-created descendants even when a tool runs with the normal `0022`
umask. The owner-only root remains the outer traversal boundary, but permissive
descendant modes are still treated as a failed permission check.

The public candidate will initially be owned by `oliver:oliver`, with root mode
`0750`, source files normally `0644`, and reviewed tools `0755`. Publication
may deliberately broaden read access later, but it never broadens access to
the private surfaces.

Mode separation protects against other local users. It does not isolate
same-UID agents. Implementation workers therefore run through
`tools/provenance/run-public-sandbox`, which:

- makes the selected public checkout at `/workspace` the only host
  project/data mount;
- exposes a read-only `/usr` toolchain plus only the loader cache and timezone
  file from `/etc`; `/home`, `/run`, `/root`, `/var`, and host `/tmp` are not
  mounted, while sandbox `/tmp` is empty;
- runs the trusted private copy of the boundary scanner against the public
  tree and its `provenance/assets.json` before creating the namespace;
- starts a new user, mount, PID, IPC, UTS, cgroup, and network namespace;
- clears the inherited environment and provides only a minimal `HOME` and
  `PATH`;
- provides no network by default; and
- never mounts the private lab, downloads, installed games, Codex sessions,
  SSH material, or transfer staging.

The sandbox is a process boundary, not a legal conclusion or a defence against
a compromised kernel. Toolchain/package/license mounts and network access are
deny-by-default exceptions: they need a versioned wrapper change, provenance
review, and a focused isolation test. An implementation agent must itself be
started inside this boundary or a stronger CI/container boundary; merely
running one of its later shell commands inside the sandbox is insufficient.

Private analysts may use the private surfaces. Implementation workers receive
only accepted transfer packages and synthetic/original/licensed fixtures.
Reviewers record which side they accessed. No agent simultaneously writes both
roots.

## Current inventory and relocation decision

The current owned distributions comprise six files and 339,088,403 apparent
bytes. The private derived evidence comprises twenty files and 1,488,452
bytes. Exact hashes, counts, installed-root summaries, and duplicate checks
are recorded in
[`research/private/boundary/current-artifact-inventory.md`](../research/private/boundary/current-artifact-inventory.md).

The root is already the selected private lab, so moving the top-level GOG
directories would add path breakage without increasing separation. They remain
in place. The working GOG installations also remain in place until `mc-ytu2`
captures their exact build/config/save relationships. Relocation is prohibited
until a verified backup exists and launch/recovery tests are preregistered.

No owned file, capture, analyzer project, Beads record, or raw note is copied
to `carpet-flight`. `research/transfer/` is staging inside the private lab, not
an allowlisted source tree. Transfers name individual reviewed files and never
copy a directory recursively.

## Frozen pre-initialization successor layout

`mc-ads` creates the following minimum tree from new files; it does not copy
this private lab:

```text
carpet-flight/
  AGENTS.md
  README.md
  .gitattributes
  .gitignore
  docs/
    architecture/
    decisions/
    provenance/
  provenance/
    public-boundary-policy.json
    assets.json
    fixtures.json
    dependencies.json
  tools/
    provenance/
      audit_public_tree.py
      test_public_boundary.py
  tests/
    fixtures/
      generated/
  Packages/             # created if the Unity baseline passes its gate
  ProjectSettings/      # created if the Unity baseline passes its gate
  Assets/               # original/licensed content only after manifest entry
```

Before engine-project creation the tree contains text and generated synthetic fixtures
only. There is no `.beads/`, owned-data mount, `research/private`, analyzer
directory, copied manual, screenshot, save, level, media placeholder, or
binary package. The source/license decision may initially keep the repository
private, but the entire history must remain publication-safe.

## Beads decision

The complete `.beads/` store is `PRIVATE_PROJECT_CONTROL`.
`issues.jsonl`, SQLite/WAL state, history, recovery files, and viewer caches
must not enter the public repository. They contain private paths, research
campaign descriptions, and history that can retain text removed from the live
graph.

If public issue tracking is later useful, create a new tracker or a
field-allowlisted, human-reviewed export containing only selected public
implementation work. Never sanitize by copying `.beads/` and deleting obvious
fields.

## Public allowlist

The public repository is deny-by-default for non-text payloads:

- source, configuration, independently written documentation, and generated
  text fixtures are normally eligible;
- executables, object code, archives, disk images, saves, dumps, Ghidra
  projects, traces, and analyzer exports are always rejected from source
  history;
- images, audio, video, fonts, PDFs, models, and other binary assets are
  rejected unless their exact path and content hash appear in a reviewed asset
  manifest with author, source, class, license, modification, attribution, and
  redistribution fields;
- a known owned/private hash is always rejected, including when it appears as
  a Git LFS object ID;
- symlinks are rejected so a checkout cannot escape the reviewed tree; and
- ignored files are still scanned locally. `.gitignore` is never treated as a
  security control.

The machine-readable rules are
[`docs/provenance/public-boundary-policy.json`](provenance/public-boundary-policy.json).
The scanner is
[`tools/provenance/audit_public_tree.py`](../tools/provenance/audit_public_tree.py).
`mc-ads` copies reviewed versions into the new repository, adds Git-history
and CI integration, and proves a clean clone without any private mount.

## Backup and retention

Current storage is an encrypted LUKS volume. Encryption protects the device at
rest; it is not a backup. No Borg, Restic, or user-level backup timer was
detected on 2026-07-23. Backup status is therefore `UNVERIFIED`.

Until backup status becomes `VERIFIED`:

- retain all owned distributions, installed builds, private evidence, source
  registers, campaign records, and transfer/revocation records indefinitely;
- do not delete, deduplicate, move, rewrite, or compact any sole copy;
- same-volume duplicates may be integrity-checked but do not satisfy the
  backup gate; and
- temporary public downloads may be deleted only under the existing
  provenance policy after their source, checksum, purpose, and retained notes
  are recorded.

A valid backup requires one encrypted offline or off-host target named in a
private backup record, per-file or manifest hashes, a completed copy, a
read-back hash verification, and a restore drill to a temporary directory.
The record must include target identity, encryption owner, date, tool/version,
manifest hash, restore sample, and next verification date. Only then may a
relocation task remove a prior copy. The actual backup target is intentionally
unset rather than pretending that a same-disk copy is safe.

## Approved services and egress

The approved-services record is
[`research/private/boundary/approved-services.md`](../research/private/boundary/approved-services.md).
Its initial rule is simple: no remote service receives owned binaries,
manuals, captures, Ghidra projects, traces, private notes, or raw transfer
staging. Public web research may fetch public URLs without uploading private
material. Local DOSBox, hashing, Ghidra, debugger, and capture tools may access
private files only when their campaign records local execution and verifies
the required no-egress posture.

## Contamination audit

Before the first commit, before every transfer receipt, on every public CI
change, and before every distributable build:

1. run the boundary scanner over the entire candidate worktree, including
   ignored/untracked files;
2. validate every conditional binary against the reviewed asset manifest;
3. reject symlinks, LFS pointers with forbidden or unmanifested object hashes,
   forbidden suffixes/signatures, private path markers, and known hashes;
4. run the scanner’s adversarial self-test;
5. once Git exists, scan every reachable Git blob and LFS object, not only the
   checkout;
6. build and test in a clean clone with no private path or mount;
7. inspect build logs, source maps, symbols, test artifacts, screenshots,
   replays, SBOM, and release bundles; and
8. record scanner/policy/manifest hashes and reviewer identity.

Passing automation is necessary but not sufficient. A human reviewer checks
for analyzer-shaped identifiers and structure, copied prose or tables,
expressive similarity, licensing, and overbroad transfer context.

## Incident and recovery

If a private or unknown artifact reaches the public candidate:

1. stop all implementation, CI publication, hosting, and friend distribution;
2. preserve a private incident record with file hash, path, source, detection
   time, agents, commits, builds, caches, transfers, and destinations;
3. quarantine the affected checkout and build artifacts without deleting the
   evidence;
4. revoke affected transfer receipts and trace every dependent task/artifact;
5. determine whether any remote, package registry, CI cache, browser host, LFS
   store, or friend received it; rotate secrets if relevant;
6. create a new clean repository from independently reviewed source files,
   rather than trusting deletion or history rewriting as the primary repair;
7. independently recreate contaminated implementation from an accepted
   behavioral specification where possible;
8. run full tree, history, LFS, build, and clean-clone audits; and
9. obtain legal review before resuming if distribution occurred or provenance
   cannot be reconstructed.

The contaminated repository and its artifacts remain private until retention
and legal needs are resolved. No “force push and forget” recovery is accepted.

## Gate evidence

`mc-t8x` is satisfied only when:

- the private modes are applied and verified;
- the current inventory and same-volume duplicate hashes are recorded;
- the scanner rejects all adversarial test cases and accepts a clean synthetic
  tree;
- the native `bwrap` isolation probe can read its selected public sentinel but
  cannot see `/home`, private evidence, agent state, or the network;
- `.beads/` is explicitly private;
- no owned file has moved or been deleted; and
- `mc-ads` remains responsible for public repo creation, Git-history/LFS CI,
  clean-clone proof, and the first real transfer receipt.
