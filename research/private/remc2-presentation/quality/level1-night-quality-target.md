# Level 1 Night quality target

This is a renderer acceptance target, not concept art. The implementation must
make the existing measured world readable and attractive before any replacement
asset work is considered.

The target is deep blue night readability, warm but restrained settlement
emissives, clear player/actor/terrain silhouettes, localized spell light,
monotonic atmospheric depth, and a readable horizon. Water may reflect and
glint, but reflections must not erase the shoreline or terrain. The HUD remains
legible at the selected profile.

Required views are town approach and overflight, an active Fireball fight,
mana/castle activity, reflective water/terrain, fog depth, and high-speed
flight. These views are input-only routes: the route, tick/checkpoint, camera
state, display profile, build, GPU/driver/editor, and capture-tool version are
recorded in the manifest.

Exact indexed frames use byte hashes. Classic HD uses byte hashes when its
indexed rerender is deterministic and otherwise falls back to structural
regions plus review. Unity Enhanced GPU-lit frames use tolerant region metrics
and mandatory human review. Over-bloom, crushed blacks, fog washout, unreadable
HUD, unstable exposure, temporal ghosting, and concept-art-only detail are
blockers.

The exact corpus is protected reference evidence. Classic HD and Unity Enhanced
each receive their own corpus root; no manifest may cross those roots.
