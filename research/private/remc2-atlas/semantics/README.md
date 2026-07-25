# remc2 semantic backbone

Status: active Track P private archaeology  
Tracking beads: `mc-dl0r.14`, `mc-dl0r.15`  
Classification: `PRIVATE_ANALYSIS` and `OWNED_BINARY` evidence

## Outcome

This is the first evidence-linked answer to “what is what” in remc2. It does
not rename every raw number. It records atomic claims across the identity
spaces the game actually uses:

- spell `(family, tier)`;
- runtime entity `(class, model, action)`;
- level entity `(type, subtype, parameters)`;
- asset `(pack, sprite index)`;
- localization `(language, text index)`;
- audio `(bank, sound index)`; and
- code `(function address, field offset)`.

Numeric equality between two spaces is not identity. A level subtype of `0`,
for example, is not a Fireball merely because Fireball is spell family `0`.

## What is now mapped

- All 26 runtime spell families.
- All 78 tiers and their actual owner-data parameter records.
- The displayed family and tier labels, including day/night/cave variants.
- Small family icon, large family icon, and tier icon indices.
- All 390 high-resolution icon bindings across day, night, and cave packs.
- The class-15 spell-holder family and the first complete runtime chain for
  Fireball.
- The compact flight spine, terrain/generation contracts, day/night/cave
  realms, authored world commands, outdoor/cave ambience selectors, all 16
  runtime classes, all 29 class-5 creature slots, and all ten objective
  predicates.
- Every one of the 845 declared action/constructor dispatch rows, including
  sentinels, disabled rows, no-op cases, and missing reconstruction cases.
- A 250-row semantic overlay over the 269 non-sentinel constructor rows;
  the remaining 19 are the deliberately addressless class-1/class-13 records.
- An individual physical-slot annotation for all 546 non-sentinel action rows,
  preserving both per-tick behavior or exact structural status and the proven
  class-5/class-15 model or family band.
- All 1,572 owned HSPR/MSPR sprite rows in palette-correct contact sheets.
- Eleven explicit unresolved questions and guardrails.

Class-15 action rows are entity roles, not tier ordinals: each family has an
active/equipped state, ordinary pickup, and persistent/respawning pickup,
while shared action 78 returns a detached spell to the world. The selected
owner-data tier is stored separately in the live entity.

The generated backbone currently contains 337 semantic claims. The companion
whole-program coverage builder places all 3,060 functions and 3,246 referenced
state targets while keeping raw and authoritative gaps explicit. This is not a
claim that mechanical placement equals semantic understanding.

`EXACT_STATIC` is reserved for exhaustive structural facts in the pinned
source, such as an empty or disabled table row. It does not imply observed
runtime behavior. `CONFIRMED` requires an independent causally consumed
asset/data corroborator, a runtime observation, or a causal test.

## Source precedence

For values, the live lineage is:

```text
compiled remc2 seed
  -> owned DATA/SPELLS.DAT overwrite
  -> SetDefaultSpells_5C0A0 patches
  -> day/night/cave LevelInit_56C00 patches
  -> spell holder and casting consumers
```

The owner-data table is not the same as the compiled seed. There are 269
atomic field differences affecting 71 of 78 tiers. Therefore the generated
`spell-tiers.csv` reports the owner-data values, while
`spell-seed-vs-owned.csv` preserves every seed-to-owner difference.

Identity and presentation are also separate. The code identity `Possession`
is displayed as `POSSESS`; `Metamorph` is displayed as `MORPH`. Tier zero of
Metamorph and Summon Army changes with map mode:

| Family | Day | Night and cave |
|---|---|---|
| Metamorph | Bee Morph, text 198, selector 2 | Firefly Morph, text 199, selector 19 |
| Summon Army | Bee Army, text 244, selector 2 | Firefly Army, text 245, selector 19 |

## Spell-family crosswalk

| ID | Code identity | Display label | Text | Large icon |
|---:|---|---|---:|---:|
| 0 | Fireball | FIREBALL | 160 | 123 |
| 1 | Possession | POSSESS | 161 | 124 |
| 2 | Castle | CASTLE | 162 | 125 |
| 3 | Speed Up | SPEED UP | 163 | 126 |
| 4 | Metamorph | MORPH | 164 | 127 |
| 5 | Heal | HEAL | 165 | 128 |
| 6 | Shield | SHIELD | 166 | 129 |
| 7 | Lightning | LIGHTNING | 167 | 130 |
| 8 | Rebound | REBOUND | 168 | 131 |
| 9 | Meteor | METEOR | 169 | 132 |
| 10 | Teleport | TELEPORT | 170 | 133 |
| 11 | Invisible | INVISIBLE | 171 | 134 |
| 12 | Beyond Sight | BEYOND SIGHT | 172 | 135 |
| 13 | Steal Mana | STEAL MANA | 173 | 136 |
| 14 | Duel | DUEL | 174 | 137 |
| 15 | Tremor | TREMOR | 175 | 138 |
| 16 | Crater | CRATER | 176 | 139 |
| 17 | Earthquake | EARTHQUAKE | 177 | 140 |
| 18 | Volcano | VOLCANO | 178 | 141 |
| 19 | Summon Army | SUMMON ARMY | 179 | 142 |
| 20 | Gravity Well | GRAVITY WELL | 180 | 143 |
| 21 | Whirlwind | WHIRLWIND | 181 | 144 |
| 22 | Fool's Mana | FOOL'S MANA | 182 | 145 |
| 23 | Magic Mine | MAGIC MINE | 183 | 146 |
| 24 | Alliance | ALLIANCE | 184 | 147 |
| 25 | Cave In | CAVE IN | 185 | 148 |

The manual's 25-family list is not the runtime completeness authority. The
owned table, localization sequence, enum, icon range, and cave-only UI path
agree on a 26th family: Cave In.

## Fireball vertical proof

Fireball is the first fully cross-referenced slice:

```text
spell family 0 / text 160 / large icon 123
  -> tiers 186 FireBall, 187 Rapid Fire, 188 Fire Storm
  -> owned tier parameters in SPELLS.DAT record 0
  -> class 15, model 0 spell-holder entity
  -> affordability, charge and mana scheduling
  -> sub_6DCA0 projectile factory
  -> class 9, model 0, action 0 ordinary projectile
  -> shared fixed-point movement and collision
  -> class 10, model 5 splash when water is hit
  -> class 10, model 0 once-only area-damage writer
  -> target pending-damage accumulator and source link
```

The ordinary projectile constructor establishes speed `384`, render index
`340`, an initial internal value of `50`, and lifetime/range
`0x2000 / speed`. The factory clamps final speed to `384..8192`. The loaded
tier payload is submitted once through the shared spatial damage writer, but
per-target shields, immunities, death conversion, and actual life loss remain
class/action-specific. Fire Storm follows a separate chain that remains
explicitly unresolved.

The actual private owner-data icon is
[`family-00-fireball-big.png`](generated/icons/day/family-00-fireball-big.png).
The three day-mode tiers are
[`FireBall`](generated/icons/day/tier-00-0-fireball.png),
[`Rapid Fire`](generated/icons/day/tier-00-1-rapid-fire.png), and
[`Fire Storm`](generated/icons/day/tier-00-2-fire-storm.png).

## Generated artifacts

- `generated/claims.csv`: atomic semantic claims with confidence and evidence.
- `generated/core-behavior-claims.csv`: 158 flight, terrain, world, entity,
  creature, Fireball, ambience, and objective claims.
- `generated/spell-families.csv`: 26-family identity/display crosswalk.
- `generated/spell-tiers.csv`: 78 owner-data tier records.
- `generated/spell-seed-vs-owned.csv`: every compiled-seed difference.
- `generated/spell-icon-assets.csv`: 390 map-mode asset bindings.
- `generated/unresolved.csv`: questions that must not be papered over.
- `generated/runtime-class-families.csv`: all 16 runtime class families.
- `generated/creature-models.csv`: all 29 class-5 model slots, action bands,
  constructor addresses, initial actions, key stats, identity evidence, and
  confidence.
- `generated/creature-action-exceptions.csv`: action 232 and the Hydra-related
  disabled rows 233–234.
- `generated/runtime-model-annotations.csv`: 250 constructor-slot identity
  overlays with evidence, confidence, and explicit raw labels.
- `generated/runtime-action-annotations.csv`: all 546 non-sentinel
  physical-slot action reductions with behavior or structural status,
  constructor/transition linkage, confidence, and exact evidence anchors.
- `generated/runtime-dispatch-rows.csv`: all 845 action and constructor rows.
- `generated/runtime-constructor-registry.csv`: all 284 constructor rows,
  including 269 non-sentinel physical rows and their separate selector values.
- `generated/owned-sprite-atlas.csv` and `generated/sprite-atlas/`: all 1,572
  owned HSPR/MSPR bindings plus 30 palette-correct contact-sheet pages.
- `generated/visual-evidence.csv` and `generated/evidence/`: reproducible
  palette-correct cave-mushroom and terrain-material correlations with hashed
  source buffers.
- `generated/spell-icons-{day,night,cave}-contact.png`: visual atlas.
- `generated/manifest.json`: hashes, counts, and confidence policy.
- `generated/function-semantic-map.csv`: all 3,060 functions with maturity,
  claim links, graph counts, and ranked next action.
- `generated/state-semantic-map.csv`: all 3,246 referenced globals/fields.
- `generated/semantic-gap-queue.csv` and `generated/state-gap-queue.csv`:
  explicit ranked archaeology queues.
- `generated/system-surface-registry.csv`: 47 architectural landmarks.
- `generated/simulation-tick-pipeline.csv`: ordered 14-stage frame/tick spine.
- `whole-program-semantic-map.md`: the human-readable full-program map.

Run:

```bash
research/private/remc2-atlas/build_semantic_backbone.py
python3 research/private/remc2-atlas/build_dispatch_atlas.py
python3 research/private/remc2-atlas/build_semantic_coverage.py
```

The builder reads only the small hash-verified evidence set under
`semantics/owned-assets`. It does not mount or modify the GOG image.

## Next semantic slices

Before a flight prototype is treated as faithful:

1. finish target-specific Fireball damage/status handling and the separate
   Fire Storm chain;
2. add the player/flight field and constant rows, preserving signed integer
   interpretation and update order;
3. bind terrain ground, cave ceiling, water/material, contact, and mutation
   contracts;
4. finish the class/model/action dispatch matrices and identify creatures end
   to end using constructor, level record, icon/sprite, manual role, and
   runtime observation; and
5. use a short runtime trace only where static and asset evidence remain
   ambiguous.

That is enough semantic context for the first feel prototype. Naming every
menu helper or every creature action is not a prerequisite.
