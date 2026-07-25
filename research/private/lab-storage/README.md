# Owner-only analyzer storage

Status: qualified; empty live XDG layout provisioned 2026-07-23  
Tracking bead: `mc-51m.3`  
Classification: `PRIVATE_ANALYSIS`

This tool provisions the private Ghidra/campaign workspace in normal per-user
XDG locations:

```text
~/.local/share/magiccarpet-lab
~/.config/magiccarpet-lab
~/.cache/magiccarpet-lab
~/.local/state/magiccarpet-lab
```

It never places analyzer state in the future public candidate
`/home/oliver/Projects/odcpw/carpet-flight`. The code and synthetic tests live
under `research/private` because this checkout is the private reference lab.

The layout separates:

- Ghidra profiles and import staging;
- sealed read-only baseline projects;
- one writable project clone per campaign;
- observations and exports;
- registry and backup destinations;
- bounded cache/temp state;
- exclusive campaign writer locks; and
- receipts and failure evidence.

Writable directories/files are `0700/0600` with owner-only default ACLs.
Sealed baseline directories/files are `0500/0400`. Symlinks, hard links,
special files, overlapping roots, relative XDG roots, and storage inside
either project checkout are rejected.

## Synthetic gate

```bash
./research/private/lab-storage/test_lab_storage.py
./tools/provenance/test_public_boundary.py
```

The storage test uses only a temporary `/tmp` root and synthetic Ghidra-shaped
files. It proves baseline sealing, byte-identical non-hardlinked campaign
cloning, exclusive lock contention, cache cleanup, structured failure evidence,
owner-only ACLs, and deterministic inventory.

The provisioned empty live layout passed its audit with 14 entries and
inventory SHA-256
`067867ee1cdbdeaf2da473bf37a92f9e0e70cddcc4cd591e94405d4a661b3d86`.
No baseline, campaign, extension, or owned game file was created by
provisioning.

## Live provisioning

```bash
./research/private/lab-storage/lab_storage.py init
./research/private/lab-storage/lab_storage.py audit
./research/private/lab-storage/lab_storage.py inventory
```

No owned executable is imported by `init`. A later baseline bead creates an
import below:

```text
~/.local/share/magiccarpet-lab/staging/BLD-MC2-0001/project/
```

It is then sealed and cloned with:

```bash
./research/private/lab-storage/lab_storage.py \
  seal-baseline --baseline BLD-MC2-0001

./research/private/lab-storage/lab_storage.py \
  clone-campaign --baseline BLD-MC2-0001 --campaign CAM-MC2-0001
```

Every Ghidra writer must run through the campaign lock:

```bash
./research/private/lab-storage/lab_storage.py \
  run-locked --campaign CAM-MC2-0001 --timeout 10 -- \
  /opt/ghidra/support/analyzeHeadless ...
```

The wrapper starts from an allowlisted host environment and assigns per-campaign
`HOME`, all XDG roots, all common temporary-directory variables, Gradle home,
and Java/Ghidra settings, cache, and temporary directories. The initial gate is
for headless analyzer writers; a later GUI-specific gate must separately prove
display/session integration without relaxing these storage roots.

`cleanup-cache` removes only the validated cache directory for an existing
campaign and recreates it owner-only with the required `tmp/` and `gradle/`
subdirectories. It never deletes a campaign, baseline, observation, receipt,
failure record, or backup.
