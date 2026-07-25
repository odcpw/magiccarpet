# Pinned upstream research checkout

`magic-carpet-2-hd` is tracked as a Git submodule at the exact revision used
by the private qualification work.

The checkout currently carries a small owner-local SDL quit-handling patch.
It is preserved separately at
`upstream/patches/magic-carpet-2-hd-owner-local.patch` so a clean clone can
reproduce the working tree without publishing a fork.

```bash
git submodule update --init --recursive
git -C upstream/magic-carpet-2-hd apply \
  ../patches/magic-carpet-2-hd-owner-local.patch
```

