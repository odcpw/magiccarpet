# Analyzer storage qualification

Date: 2026-07-23  
Tracking bead: `mc-51m.3`  
Schema: `magiccarpet-lab-storage/1`  
Status: `PASS`  
Input: synthetic files only

## Live empty layout

```text
data:   /home/oliver/.local/share/magiccarpet-lab
config: /home/oliver/.config/magiccarpet-lab
cache:  /home/oliver/.cache/magiccarpet-lab
state:  /home/oliver/.local/state/magiccarpet-lab
```

The live audit reported 14 expected entries. All directories are owner
`oliver:oliver` mode `0700`; the path policy is mode `0600`. Each live root has
this access/default ACL:

```text
user::rwx
group::---
mask::---
other::---
default:user::rwx
default:group::---
default:mask::---
default:other::---
```

The deterministic empty-layout inventory is:

```text
inventory_sha256:
  067867ee1cdbdeaf2da473bf37a92f9e0e70cddcc4cd591e94405d4a661b3d86
data_tree_sha256:
  b1f7dff0badb708755f69d1f44ca5f34a80dc30892fd49517f74483349b17e4d
config_tree_sha256:
  250220deb587faa367ece0dfbfbdd540c6695c1b3430bfa16f12090d3a8baec8
cache_tree_sha256:
  9c15c497f393f5bbfbadc23430dada4eaa8aa576c7c022f9f58dffbd071c0137
state_tree_sha256:
  6d81f7885533da35f6d0de2cd546b9b591ceea30bead87fee3a5c42aa8966bac
```

Baseline, campaign, and staging directories are empty. The future public
candidate `/home/oliver/Projects/odcpw/carpet-flight` remains absent. No
extension or owned MC2 file was installed, imported, copied, or inspected.

## Synthetic acceptance

The focused test passed:

```text
LAB_STORAGE_ACCEPTANCE_PASS
baseline=sealed campaign=isolated lock=exclusive acl=owner-only
```

It proves:

- normal XDG path selection and checkout-root exclusion;
- exact owner-only modes and default ACL inheritance;
- whole-staging rejection of symlinks, hard links, and special files;
- stable full-staging and project hashes during baseline copy;
- sealed baseline `0500/0400` modes;
- byte-identical, non-hardlinked, writable per-campaign clones;
- one exclusive writer with contention rejection and later recovery;
- an allowlisted writer environment with exact per-campaign `HOME`, XDG,
  runtime, temporary, Gradle, Java, and Ghidra paths;
- rejection of ambient environment leakage;
- validated cache-only cleanup and a successful writer after cleanup;
- persisted structured failure evidence; and
- repeatable inventory output for unchanged state.

Existing boundary gates also passed:

```text
PUBLIC_BOUNDARY_SELF_TEST_PASS cases=22
PUBLIC_SANDBOX_SELF_TEST_PASS
```

## Independent review

Three NTM/tmux review passes were tended through timeouts and one transient
pane-state error. The first two returned `REVISE`; their whole-staging,
environment-proof, and post-cleanup findings were fixed and retested. The
final narrow review returned `APPROVE` with file/line evidence for every
corrected requirement.

## Qualified artifact identities

```text
lab_storage.py
  e91a237c9605405ab21f3259d8f25a8dafcf9b414b053ada0208648beb56dfd0
test_lab_storage.py
  e7acb2fee0689520305c7dd1629c5bf1a7ee0febbfb435d2662a3e7cedc0096a
README.md
  b988a1905fe9681927eb6cab98536a0bf01b01918c7148f67c5418ae98229daa
```

The two Python tools are owner-only mode `0700`; documentation is mode `0600`
inside an owner-only directory.
