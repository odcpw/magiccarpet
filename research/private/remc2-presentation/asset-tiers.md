# Classic HD asset tiers

The Classic HD route has one mandatory baseline and one optional capability:

- `original-indexed` uses the owner cache's original indexed terrain, sky, and
  sprite banks. It is selected as a complete family set before a session.
- `optional-high-resolution` may use owner-local 128×128 terrain, 1024×1024
  sky, and 4× sprite banks, but only when every selected family has validated
  dimensions, format, selector coverage, and SHA-256.

There is no replacement-art bundle here. A missing or invalid optional bank
falls back to the complete original family, never to a per-selector mixture.
The placeholder manifest deliberately contains no payload or hashes and is
therefore not a claim that those owner files are present.

Validate the contract with:

```bash
python3 research/private/remc2-presentation/validate_asset_tiers.py
python3 -m unittest research/private/remc2-presentation/tests/test_asset_tiers.py
```
