# Shared-engine delivery plan

### Castle collector cleanup in the private Original — November 24

Runtime `d7d89563` is pushed and installed with native `e9a394b13f4a`.
On the same paid NightSpire Castle checkpoint, the old collector kept flying
after demolition. The fixed runtime retired it on the update that removed
the Castle. Both rival Castle-loss checks passed, including the eligible
rival's replacement and paid upgrade. The full-pool regression confirmed
that collectors retire even when releasing the Castle's mana reclaims its
slot first. The browser opened NightSpire on the matching native.

The same build passed focused checks for the Bee absence switch, the
Payahandra vault objective opening its exit, and the experience-scroll
award through tier publication and reset. This covers those routes.
The broader objective and reward acceptance stays open. Evidence is under
`research/private/generated/earned-campaign-20261124/castle-defender/`.

### Installed browser brightness and outdoor reflections — November 24

Private Three.js Original `37739e43` is installed on runtime `2d753b7e`.
In Payahandra, F7 cycled Day brightness from 4 to 0 and changed both HUD
and world pixels. After a cold launch the next press advanced the saved
Day value from 1 to 2. Actual Day and Night actors now reflect in water.
The GPU outlines still differ from the native reference. The combined
build opened the map and returned to flight on the RTX 3090. On the installed
service NightSpire opened with 10,000 life, 1,000 mana, Fireball and Possession.

Evidence is under `research/private/generated/` in
`geometry-reflection-20261124` and `f7-brightness-20261124`.

### Payahandra experience scroll sound in the private Original — November 24

The browser now looks up missing gameplay sounds in the selected mission's
owner cache. A matching presentation sample keeps priority. Across all 30
installed packages, all 1,617 previously available mission/sample pairs kept
their descriptors. Payahandra now resolves original sound 63, `SPELLUP.WAV`.

In the whole Payahandra package, five controlled player positions activated
the authored switches. On contact, row 248 awarded four Fireball experience
points and emitted sound 63 on the same turn. These four points did not cross
a spell tier. In a separate Chrome check the captured event fetched the
25,824-byte owner WAV and started its 1.16-second Web Audio source. The
single forward-flight attempt ended before reaching the scroll. A live browser
pickup and physical audio output remain unobserved.

Browser `01ae67c2` is pushed and installed on native `cd6fe94208a6`.
The installed NightSpire opened with 10,000 life, 1,000 mana, Fireball and
Possession. Evidence is in `research/private/generated/scroll-audio-20261124/`.

### Wizard loss, restart and F9 in the private Original — November 24

In the installed NightSpire, Shift+K set the wizard's life to −1. The death
notice and turning camera appeared. Space reopened NightSpire at tick 0 with
10,000 life. Forward input then moved the carpet from y=56960 to 55040.
An isolated run checked that bare K and Alt+Shift+K left the wizard alive,
and that holding Shift+K issued one surrender. That first run timed out
waiting for Chrome to show the restart. The new native owner had reopened
the mission. A second isolated run and the installed run both displayed
the restarted flight. The cause of the first timeout is unresolved.

Rust checks covered the recorded surrender, cold replay and no-Castle retry,
and an older paid Castle save through death, revival and the next fight.
The F9 bridge now excludes synchronous save loading from its inactivity
timer. In Chrome, the 26.9-second restore kept Speed active. Enter opened
the map at tick 8 without a resume, and leaving the browser paused play.

Runtime `2d753b7e` and browser `fb499f55` are pushed. The private Original
loaded native `cd6fe94208a6`. Evidence is in `flight-controls/surrender-public/`,
`flight-controls/retry-diagnostic/` and `f9-map-diagnostic/` under
`research/private/generated/earned-campaign-20261124/`. The broader
player-loss and campaign acceptance remains open.

### Flight speed, speech and braking in the private Original — November 24

F3 switches between normal and fast simulation speed. The installed browser
showed the two source notices in NightSpire at ticks 49 and 65. Fast flight
also passed on the RTX 3090 and on software WebGL after the bridge began
yielding between batches of elapsed turns. F4 toggles objective speech.
In an isolated Chrome profile, restarting NightSpire with speech off produced
no objective audio request. With speech on, Chrome fetched the original
segment and started its decoded audio. A missing speech cache kept F4 inactive.

Backspace stops the carpet and cancels an active Speed spell in flight and
on the full map. A controlled campaign carry supplied Speed in NightSpire.
The cast cost 1,000 mana. One physical keypress cleared the same spell holder
and took target and actual speed from 160 to zero in both views. The map
stayed open. The combined reload-to-map check timed out; the separate map
check passed. In the normal installed launcher, Backspace took an unboosted
carpet from speed 32 to zero. Browser `318e6704` is pushed and installed
with native `637bfe6b41d9`. See `source-speed/`, `full-stop-browser/` and
`flight-controls/` under `research/private/generated/earned-campaign-20261124/`
and `research/private/generated/f4-speech-20261124/receipt-final.json`.

### NightSpire Goat animation in the private Original browser — November 24

Rust now advances an animated sprite's shared texture after the browser draws
that selector. In NightSpire, Chrome acknowledged 90 of 90 draws with a camera
positioned to see the authored Goats. Selector 152 produced four different
native images. On the installed cadence revision, two real WebGL uploads to
the Goats' 22 × 22 atlas rectangle matched the native pixel bytes. Its pixels
stayed fixed for six updates after the Goats left
view and during pause. The first candidate missed all 115 draws because the
server advanced before Chrome could acknowledge them. A cold Rust replay
reconstructed the current image. The existing indexed-renderer check passed
for NightSpire Goats and a Day Villager.

The bridge lets the simulation take elapsed turns between browser frames.
A queued map open and close each took one turn, flight resumed, and one
Fireball spent 100 mana. On the RTX 3090, eight seconds of ordinary NightSpire
flight measured 30.10 native ticks and 18.86 new scene publications per
second. Before the animation change the same check measured 29.98 and 29.23.
On SwiftShader the new bridge measured 25.80 native ticks per second, up
from 1.62 when every turn waited for a draw. These are publication and
simulation measurements. The Goat camera check leaves exact terrain
occlusion and human play open.

Browser `cfa9a417` and runtime `73a36309` are pushed; the private Original
service loaded NightSpire on native `637bfe6b41d9`, with Fireball, Possession,
10,000 life and 1,000 mana. Unity still runs `2f477f43c882`. Evidence is in
`research/private/generated/earned-campaign-20261124/goat-tmap/`,
`tmap-goat-browser/` alongside it, and `research/private/generated/tmap-bridge-20261124/`.

### Castle revival, secret entry and spell marks on abandonment — November 24

The private Original now serves browser `b3e6a320` on native `fb6315dc5d4f`.
F5 toggles sound and F6 toggles music. In Payahandra, two paid casts spent mana.
The muted cast started no browser audio source. The next cast, with sound on,
started two. F6 stopped and restarted the music; the save slot stayed
byte-identical. The GPU now draws the Day minimap and HUD against the live
world with the source palette lookup. Chrome showed the translucent panels
in Payahandra at tick 96 and opaque panels in Spider Cave at tick 6. The CPU
check compared all 640 × 480 output indices in each mode. The installed build
opened NightSpire. See `audio-panel-joined` and `mission-retry/f5-audio-b3e6-proof`
under `research/private/generated/earned-campaign-20261124/`.

Runtime `a1797bb5` retains the player's hand marks when abandoning a mission.
On the same saves that lost the marks in the previous native, the public API
returned from Payahandra and reselected Castle on the right. Abandoning Pav
Durivium, saving the Jaleen map, loading it cold and cycling the right hand
selected Possession. Entry spell experience was preserved on both routes.

A secret destination now applies its own LevelInit panel setting. T’Klom's
actual portal into Karakir produced a Cave save that cold-loaded and returned
to its Day parent. The check controlled the starting frontier and combat
contacts. In a separate paid NightSpire Castle save, controlled damage,
death and revival switched the music back to normal. A later hit restored
combat music. The music check observed the native events.

Browser `05b8fa7b` includes the Castle and balloon status help and loose-mana
guidance. Chrome showed the Castle's mana, life and size on Spider Cave and
“Possess This Mana” pointing to live sphere 259 on The Revealed Way. Claiming
that sphere in the same browser run remains unobserved. The installed private
Original opened NightSpire on native `fb6315dc5d4f` with Fireball, Possession,
10,000 life and 1,000 mana. The map-save fixture correction `79912f5d`
preserved the byte-roundtrip and tamper checks. The Unity player still uses
`2f477f43c882`. Evidence is under `research/private/generated/earned-campaign-20261124/`
in `mission-retry`, `castle-status-guidance` and `loose-mana-guidance`.

### Marker stones in the private Original browser — November 24

Browser `42df6493` is pushed and installed with runtime `97e1a224`.
On The Revealed Way's full map, Chrome drew “Marker stone” pointing to
the same stone (slot 233) on a dark frame at tick 2944 and a lit frame
at tick 2951. Creature and spell-pickup hints appeared first. The
ordinary launch opened NightSpire with Fireball and Possession.
Focused presenter checks cover both map sizes and clearing the hint
outside the map. Jaleen supplied real blinking markers, but its
browser observation reached player death before the stone hint was seen.
See `research/private/generated/earned-campaign-20261124/marker-stone-private/`.

### Replay victories, campaign spell marks and map guidance — November 24

Runtime `97e1a224` keeps hand marks through ordinary victories, Results,
save/load and entry into the next mission. The connected check loaded
NightSpire's earned five-objective save and replayed the recorded flight
into row 137. Castle was marked on an existing neutral input. The player
reached the recorded exit position at tick 10695, carried the left mark
through Results into Payahandra and selected Castle by cycling that hand.

Completing an ordinary mission again now shows Results, refreshes its five
statistics and retains the player's newly learned spells and marks. The
repeat check reused the real pre-exit state, activated the authored Speed
pickup group, and positioned the player for the pickup and portal. Payahandra
opened with Speed and the right Castle mark. Banked XP survived without a
second award. The Uluth check used a paid Meteor victory and replayed its
completed ending state. The repeat showed credits and Results without
replaying the already-consumed CUT6. Older pending-retry saves also bank an
earned objective award on restart. Continuous campaign play remains open.

Browser `bac0b9d3` presents the original rival Castle, revealed balloon and
healing Dolmen map hints. Chrome displayed “This is an enemy castle” with the
leader reaching the actual Castle dot on Perilium. The map selected the live
Castle at slot 176 at tick 517. The paid Beyond Sight save also published the
rival balloon at slot 370 and three Dolmens. When the reveal expired, the
balloon marker cleared. The balloon and Dolmen labels have focused presenter
checks. The Perilium screenshot demonstrates the Castle label. Evidence is
in `research/private/generated/earned-campaign-20261124/mission-retry/`
and `research/private/generated/earned-campaign-20261124/world-map-guidance/`.

Both revisions are pushed. The installed private browser loaded NightSpire
with native `8a6fa4c6faf5`, 10,000 life, 1,000 mana, Fireball in the left
hand and Possession in the right. The service was paused after the launch.
The native also loaded an older earned Results save, saved and loaded it
again, and opened Payahandra with the same three spells. The Unity player
still uses native `2f477f43c882` pending its Editor build.

### Original flight help, Castle demolition and saved spell controls — November 24

In NightSpire, F1 now turns map help off and on, writes the browser player's
preference and displays remc2's “Help Mode Off” and “Help Mode On” messages.
Chrome showed both original messages while flight continued. Holding F1
produced one change. Shift+F1 and Alt+F1 left the help preference intact.
Browser `f935dce6` first delivered the F1 control in the private Three.js Original.

Browser `115c4bc7` adds F2 flight assistance and the campaign map's Save
and Load actions. In Chrome, F2 displayed the original status messages and
wrote the preference. After a cold launch, enabling assistance resumed the
native carpet's idle recenter commands. Turning it off stopped those commands.
From an earned NightSpire Results save, Chrome saved the map with NightSpire
selected, entered Payahandra and loaded that map back from flight. Both
unlocked missions and the player's Fireball, Possession and Castle survived.
This browser uses its existing single save slot.

Browser `9b85f182` restores the physical Space key to remc2's death-camera
recovery. In NightSpire, pressing Space during flight kept the wizard at
1,000 mana. Casting Fireball with the left mouse button then spent mana.
In The Revealed Way, a saved flight reached lethal damage at tick 2111 and
the death camera at 2112. One physical Space press restored 10,000 life in
the same mission. The installed browser bundle and native `faff2bf937ab`
matched the qualified copies. Evidence is in
`research/private/generated/earned-campaign-20261124/physical-space/`.

In remc2, Shift+L removes one stage of the player's Castle. In the private
Three.js Original, one Shift+L on a paid NightSpire Castle sent command 42
once. The stage-1 root entered its dying state and disappeared from the
economy and HUD on the following tick. Bare L and Alt+Shift+L left it intact.
The bundle also reads saved spell-menu, map, information, mark and
hand-cycle keys. In Payahandra, two native preference profiles exercised
marking Fireball and Possession, cycling between them and viewing the map's
player information. Browser `764f1562` first delivered these controls. Unity source
`47aa66ec` includes the corrected chord and compiled with the project's
runtime references. Its player still requires an Editor build.

### Death without a Castle and same-mission restart — November 24

On a saved approach in The Revealed Way, ordinary damage killed the player
at tick 2111. The death camera took over on the next tick. One Space press
restarted the same mission and restored 10,000 HP. The Rust runtime admits
recovery in the death camera, rebuilds the current mission, and preserves
learned spells, banked experience and hand marks. A Payahandra package check
followed a marked Fireball across retry, cold load and the next hand cycle.
The older input rules still apply to steps recorded in older saves.

Runtime `a19bd7d9` and browser `2476effd` are pushed. The private Three.js
Original serves the matching native `4c5ef577fdc7`. After one Space press,
the restarted Day 3 played Day music, effects and the opening objective
speech in the same Chrome audio context. Entering Payahandra from the earned
campaign map also started Day music, effects and speech. Night and Cave
music, the slow Spider Cave load and saved movement controls passed in Chrome.
When Payahandra's player dies after completing the objectives and before
leaving, the earned spell XP now survives the restart. The real scroll in
row 248 awarded four points. Paid Fireballs opened the required vaults.
With player health lowered for the death check, a Skeleton killed the player.
The source result was 0x0e and the same mission restarted with the four points
banked once. Reload preserved them. Runtime `0092fe27` was pushed and
first delivered in the private Three.js Original with native `faff2bf937ab`.
The Unity runtime assembly compiled from current
sources. Its installed player still uses `2f477f43c882`. `mc-paj.6` stays
open. Evidence is in `research/private/generated/earned-campaign-20261124/`.

### NightSpire player information and spoken objectives — November 24

NightSpire now draws its one actual player on the full map. On the same start
save, holding Alt showed “PLAYER INFORMATION UNAVAILABLE” in the previous
installed Original. The updated player showed PLAYER with maximum mana 1000;
releasing Alt restored the map. The mission count limits visible rows, while
the source keeps eight kill columns. A real-package check covers NightSpire,
Payahandra’s two participants and an invalid in-range entity link. Chrome
also displayed the repaired NightSpire row. Runtime `bf2a84b8` is pushed.
The installed Original native is `2f477f43c882` and the browser native is
`ddf6dffbd1c6`. The Unity build uses the matching existing managed ABI and
the browser uses its campaign palette export.

In the private browser, Payahandra’s opening objective selected original CD
track 2, segment 4. A Chrome recording of both channels matched the owner
recording. Turning Speech off survived a service restart and suppressed the cue
through tick 35. Turning it on restored the cue on mission restart. The
browser also presents the spell-ability improvement message when a pickup
crosses a tier. The four-point Payahandra pickup is covered above. An earned
tier crossing and its on-screen notice together remain unobserved.
Browser revisions `8ae4e10d` and `629d9b84` are pushed; `629d9b84` is
installed. Evidence is under `research/private/generated/earned-campaign-20261124`
and `research/private/generated/night-scoreboard-unity-20261124`. Physical
audio output and the continuous earned campaign remain open.

### Flight sound and map controls in the private browser — November 24

On an earned Payahandra save, casting Fireball spent 100 mana. The native
voice journal produced the launch on tick 1 and the quieter impact on tick 22.
Both Day-bank waveforms were present in a recording of Chrome's stereo output.
Physical speaker output remains unverified.

The map uses the saved key (Return by default) and the original two-button
mouse gesture. Chrome opened and closed the NightSpire map with both buttons
after taking pointer lock. The native input trace recorded no further cast
after the map opened. A separate profile bound the map to B and Enter ceased
to toggle it. Browser source `ab074243` is pushed and installed with native
`678f88daff11`. The live service served the matching bundle and Night sound
bank. Evidence is in `research/private/generated/earned-campaign-20261124/`.
NightSpire's player-information view exposed a separate scoreboard roster
error; the source game draws only the current mission's player slots.

### Player information and the objective flash in the private browser — November 24

On Payahandra's full map, holding Alt displays the player and Nyphur, their live
mana and the kill matrix. Releasing Alt returns to the map. Chrome showed both
rows from an earned save. Alt-click and Shift-click still cycle marked spells.
The scoreboard uses the source font, panels and draw order.

The Three.js world and HUD now use the current native palette. At Payahandra's
opening objective, all 768 colour bytes matched the indexed renderer in each
of 53 observed frames through the flash and recovery. Chrome pixels from both
the world and HUD followed the fade. The private service runs browser
`2d0ec9ce` with native `678f88daff11` from `5d1a5546`. The installed Unity
Original retains native `52c453bf5cbd`; its 478 Rust inputs and five header
hashes match the published source used for that build. It already consumes the
indexed palette. Unity Editor login remains outstanding for a new Unity build.
Evidence is in `research/private/generated/earned-campaign-20261124/`.

### Payahandra vault Meteor, Wind switches and marked spells — November 24

Payahandra vault group 27 produces a third-tier Meteor. Once the player's
spawn protection expired through 100 ordinary updates, its scheduled pulse
took 1,600 health. The check controlled the approach and earlier vault damage.
Both Wind switches (109 and 121) displaced the player. A real House placed in
the wind lost 100 health on its next update. The Wind lasted 40 updates, and
its head and eleven segments expired and were reclaimed. Old saves retain
the earlier behavior for already-recorded operations. The previous native's
Payahandra save replayed, advanced, saved and reopened on the installed native
`52c453bf5cbd`. Original also loaded that save. Runtime source is published
at `ca40bb5e` on `codex/original-ending`.

The private Three.js client now marks and cycles spells independently for each
hand. In Chrome on the earned Payahandra save, holding Shift and the left mouse
button advanced from Fireball to Possession once. Alt and a left click restored
Fireball. The right hand retained its selection. The check opened the spell menu
through the native control endpoint and performed the marking and cycling in
Chrome. Browser source is published at `b7d5591a` on `codex/browser-exit-hint`.
The running service serves that bundle with the same native as Original.
Evidence is in `research/private/generated/earned-campaign-20261124/`.

### Campaign, credits and live map guidance in the private browser — November 24

The normal Three.js launch now resolves the same 25 ordinary and five secret
packages as Original. A fresh launch opens NightSpire and keeps later missions
locked until earned. In Chrome, cancelling Abandon returned to flight. Confirming
from Payahandra returned to Title and reopening the map retained NightSpire
completion and all three carried spells. In a separate retained secret save,
abandoning Pav Durivium returned to its Jaleen parent with 12 completions.

With final victory staged, the browser consumed all 259 CUT6 frames and displayed
all 21 credits pages. In a separate post-credits Uluth Results save, Continue
reopened the completed campaign map. An earned final victory remains open.
Chrome also drew the original creature guidance on the corner and full Day 3
maps and the spell-pickup guidance on the full Payahandra map. The subject and
its position came from the live native simulation. A focused check covers
clearing a removed marker. Collection and hint removal together in Chrome, and
priority against other help rows, remain unverified. The private service runs
the published browser code at `95a6ae6d` on `codex/browser-exit-hint`.

### Payahandra spell pickups, Fireball accuracy and campaign intermissions — November 24

Payahandra’s authored Speed pickup now remains a held spell after the same
simulation pass. Both player records and the world slot retain its owner.
The Results spell count uses the authored rows remaining after initial spawns
and refreshes on revival, as in remc2. In the actual Payahandra package,
controlled pickup approaches gave 0% after Speed, 50% after the dormant
Shield reward (authored row 155, spell family 6) and 0% after revival. A separate acquired-Speed check exercised
the public cast. A paid Fireball that splashes into water leaves hit accuracy
unchanged. Its ordinary solid impact on Day 3 still counts.

The rig-built native `743ac8febd62` and all 478 Rust inputs match the
installed Original. The launcher loaded Payahandra from a save written by
the preceding native. Cold replay also preserved the old cast and the next
cast recorded with the new behavior. Runtime source is pushed at `7d2dfd9d`
on `codex/original-ending`.

The Three.js client carries an earned NightSpire exit through Results and the
map into Payahandra with Fireball, Possession and Castle. Holding O there
displays Payahandra's actual objective text. It also presents the first
campaign intermission using the native media owner. With the campaign return
explicitly staged, Chrome consumed all 529 CUT1 frames, including Vissuluth’s
subtitle and audio events, through input release. An earned T’Klom victory
and speaker output remain unverified. Browser source is pushed at
`df33e843` on `codex/browser-exit-hint`. The private service loaded the
installed native and its published client bundle. Evidence for these routes
is in `research/private/generated/earned-campaign-20261124/`.

### Live exit controllers on the Original maps — November 24

The map now reads the current exit controller when an authored switch creates
it. The existing entity snapshot publishes that same slot and position. In
T’Klom’s retained authored workspace, real flight into switch 136 activated
group 4. At tick 301, exit controller 298 (class 11, model 31) was active
before mission completion. It projected to (67,4) inside the full map and
outside the corner map. Previously both map readers omitted it. The
corrected native supplies sprite 84 and help selector 83. The public-input
check continued through the secret exit and verified reclamation. The
workspace changes the activation group of the portal and its controller.

The installed Original now uses native `0fc9d27e2b46`. All 478 Rust inputs
match the rig and install receipt. A separate first-visit campaign save
cold-loaded through the public native API and reproduced the same controller,
position and map coordinates. This save starts from a controlled campaign map
with the first four missions marked complete. The older replay of an already
unlocked mission did not produce that controller.

The graphical launch of the first-visit save stopped while validating the
Level 2 owner cache. Its in-process SHA-256 for `audio/SOUND.DAT` differed
from the manifest. The on-disk file measured afterward matches the manifest.
The cause remains open. The installed T’Klom display has not been verified.
The prior Perilium launch on native `54a21c6f11d6` passed. The English
exit-hint text, pointer and lifetime are pending a qualified Unity build.
Unity Editor returned 198 before compilation because its license entitlement
was unavailable. The candidate runtime and EditMode assemblies compiled with
Roslyn. The live menu cache and installed managed player are still the
previous versions. Evidence is in
`research/private/generated/original-exit-hint-20261124/`.

### Mission-triggered Tremor, Night map colours and Skeleton combat — November 24

Baraghan's repeating switch 628 now supplies the authored tier-two Tremor
payload of 800 to row 629. The first damage pulse is 24 and the lifetime
remains 120. The port previously used the constructor default of 20,000,
producing a pulse of 664. The shared group publisher applies the tier when
it creates the effect. Stored level definitions retain their existing hashes.
On the real Baraghan package, a controlled approach triggered the Tremor,
deformed terrain and reached the damage dispatcher. Separate switch visits
verified the repeating and one-shot Gravity Wells downstream of switch 633.

On Perilium's Night map, a rival-owned effect takes its wizard's colour on
both map sizes. The local and unowned sphere colours are preserved. The
Magic Mine uses the source blink phase in all three environments.

A Day 3 combat check closed to firing distance and followed two paid
Fireballs through their own impacts, each dealing 250 damage to a Skeleton.
Separate controlled fatal hits in Levels 2 and 4 verified the death burst,
its natural hide and following-pass reclamation. Level 2 reused the original
Skeleton's slot in the same turn. Live handles distinguished that new occupant.
The full Day 3 horde route and continuous campaign play remain open.

The initial candidate broke two retained saves by changing their stored
definition. The installed correction leaves the definition validator intact.
The Baraghan entry save opened and advanced a turn. Perilium's paid Beyond
Sight save reached tick 180, where both native maps changed the rival
effect from palette 164 to 88 while the two sphere controls stayed 170
and 232. The canonical Original launcher loaded that save and displayed the
full map with native `54a21c6f11d6`. All 478 Rust inputs in the install
receipt matched the rig build. The launcher check confirms load and display;
the native map reader verified the individual colours. Evidence is in
`research/private/generated/original-routes-20261124/` and
`research/private/generated/original-night-owned-20261124/`.

### Perilium: paid Beyond Sight and the rival balloon — November 23

The player entered Perilium from a controlled campaign carry with Possession
and Beyond Sight. One ordinary Possession cast claimed three nearby mana
spheres and raised capacity from 1,000 to 2,536. Beyond Sight spent 1,800.
Both native maps published the existing rival balloon at slot 370 with help
row 26. The row was present at tick 180 and gone by tick 184; the same
collector still belonged to player 357. Its Castle marker remained throughout.

The installed Original loaded the tick-36 paid save. Opening the full map
showed the balloons and rival names, then the Castle markers as Sight expired.
Other hints won the displayed popup during this run. The earlier controlled
Unity check covers the English balloon text and pointer. The source checks
and installed captures are in `research/private/generated/beyond-sight-balloon-20261123/`
and `research/private/generated/perilium-paid-sight-20261123/`.

### Night Castle colours on the Original maps — November 23

The Night map gave every Castle sprite 58. In Perilium the rival Castle at
slot 366 belongs to player 357, whose colour is 3. remc2 selects sprite 61.
Both map sizes now use the existing single-player owner-colour lookup for
Castles as well as balloons. The Perilium check reads the live Castle owner
and checks both map sizes. The NightSpire minimap and full-map camera checks
across four missions also pass.

The paid Perilium save reproduced the change from 58 to 61 on both maps.
At tick 180 three rival Castles had distinct sprites (59, 62, 61). After
Beyond Sight expired, all three Castles remained and all three balloon
markers disappeared. Reopening the real Spider Cave save preserved the
player's Castle sprite 58. Original native `d5932a518476` is installed with
the Castle HUD player. Evidence is in
`research/private/generated/original-night-castles-20261123/`.

### Mission-triggered Gravity Wells — November 23

Baraghan's area switches and Galiphur's repeating switch can now create a
Gravity Well through the ordinary mission group publisher. Previously the
group rejected class 10, model 67 before creating anything. The shared
constructor takes the authored tier's power and lifetime and links the well
into the terrain and scheduler. Casting the spell still uses that constructor.

On the installed Baraghan package, placing the player in switch 633 activated
group 17 in order (634, 653, 655). Its tier-one well changed the terrain,
pulled the player compared with the same turn without the field, and was
reclaimed. The check controls player position at the switch and in the pull
band. Galiphur's real repeating switch 412 created the tier-two well at row
413 and retained the source row for the next activation. The existing
spell-cast Gravity and Wind check also passed. All three ran with the final
source. The separate branches beyond Baraghan switches 653 and 655 were
traced in source only. Full switched-group acceptance remains open.

The Original player uses native `83d27f3b7e93` with the Castle HUD build.
The normal launcher loaded Baraghan from a previously unlocked campaign map
and mapped the matching library. This launch check ended at mission entry.
Evidence is in `research/private/generated/baraghan-gravity-20261123/`.

### Castle and balloon status guidance — November 23

Original build `build-3cqlrcw2` uses 61 English help rows and native
`bd6d2711cd66`. Six labels point to the Castle's mana, life and size and
the balloons' mana, number and life. The seventh, “Castle mana required”,
uses the live objective percentage and the selected FONT1 glyph width.
Eligibility follows the two stage lookups in remc2's
`GetHelpPopupTextAndCoords_87CF0`. A displayed percentage by itself is
insufficient. Only the help table changed among the 678 cache outputs.

A normal Castle cast in Spider Cave spent 1,000 mana. The saved game
reloaded with a stage-one Castle and a 40% goal. With that real tick-5
observation held fixed, the Original help loop selected and drew all seven
labels. The goal pointer reached (89,38) and expired after 50 draws.
The live NightSpire Castle at tick 1740 showed a 15% goal but did not qualify
for this label. An earlier 4,000-turn Spider Cave run selected higher-priority
help; that run did not display the goal label. Evidence is in
`research/private/generated/original-castle-status-20261123/`.

### Marker stones on the Original maps — November 23

Original build `build-8nx1869r` and native `bd6d2711cd66` keep the stone
available for help while its dot blinks off. The corner map changes phase
every three turns and the full map every two. The 54th English help row is
“Marker stone”. Its arrow stays with the live stone inside the map aperture.

Both native map readers retained the same stone IDs across all twelve phases
on The Revealed Way, Cymmeria, Jaleen, and the ordinary entries to Levels 17
and 24. The previous native lost the IDs during the dark phase and used the
corner cadence on the full map. Four focused Rust checks passed. The final
Unity cases used a real Day 3 full-map stone and a Jaleen corner-map stone.
They verified the pointer through visible and hidden frames, off-map exclusion,
and the source lifetime. Jaleen started from a previously unlocked mission on
a retained controlled campaign map. The installed Perilium screenshot from the
preceding build also shows “This is an enemy castle” over the actual Castle.
Evidence is in `research/private/generated/original-marker-stone-20261122/`
and `original-world-hints-20261122/return-map-20.png`.

### Rival Castle, balloon and Dolmen map hints — November 22

Original player `build-fgp79ian` reads 53 English help rows. The HUD follows
the map subject for a rival Castle (25), a rival balloon revealed by Beyond
Sight (26), or a healing Dolmen (22), using the original text, timing and
dismissal behavior. All 678 menu-cache outputs were checked. Only the help
table changed. The native library is the existing `3b77e98571bd`.

On Perilium the real full-map Castle at slot 366 produced the pointer and
text “This is an enemy castle” in the Unity presenter. Switching to the corner
map cleared it. Seven Unity cases passed, including the established creature,
spell-pickup and friendly Castle/balloon paths. Payahandra published three real
Dolmens. The controlled balloon and Dolmen hints completed their 50-draw
lifetimes. Perilium produced a real rival balloon at slot 370 after 32 neutral
turns. A retained campaign gave the player Beyond Sight, but only 1,000 of
the 1,800 mana required for the cast. The positive live-balloon hint remains
unverified. Evidence is in `research/private/generated/original-world-hints-20261122/`.
The installed Original loaded Perilium from a previously unlocked, controlled
campaign map and opened its full map with Return. The broader HUD bead
`mc-oi6.6` remains open.

## Castle mana objective in the installed Original client — November 22

Original native `3b77e98571bd` publishes the live Castle mana percentage from
remc2's ordered objective scan. Stage initialization clears the marker and the
successful mana check clears it in the same scan. Four focused release checks
passed, including NightSpire at 15% and The Revealed Way at 30%, with the
prior stages, Castle and funding controlled in the threshold check.

The recorded NightSpire cast produced a Castle at tick 1740 with a 15% goal.
The completed NightSpire save had all five objectives satisfied and a zero
goal. Cave 4 published 40% after its first turn and restored that value from
its saved game. The old native reported 10% in all three situations. The
Original player loaded the retained Day 3 Castle during the Skeleton objective;
both erroneous 10% dots disappeared from the HUD. This was a native refresh
of the existing Unity player. Evidence is in
`research/private/generated/original-castle-mana-20261122/`.

Separate historical checks remain open. The scripted Day 3 Skeleton aim and
burst-cleanup checks and the NightSpire Archer route stopped before the mana
objective. The older editable-threshold setup settled at 26% against a 30%
requirement. These results establish neither a campaign progression defect
nor completion of those routes.

The next confirmed gap is rival map guidance. The installed native publishes Perilium’s actual rival Castle at slot 366 inside the full map at (94, 155) on the opening frame. The Original help exporter and Unity row filter omit its row 25 and the foreign collector’s Beyond Sight row 26. The collector condition still needs a live subject.

## Installed Original HUD and controls — November 22

Original build `build-i1a2zazh` ships the 50-row English help cache and native
`da15d844f920`. Creature and spell-pickup hints use remc2 rows 19 and 20
(text records 307 and 308). Focused Unity checks on The Revealed Way selected
the nearest of multiple real markers, checked the drawn pointer and English
text, followed the creature, and cleared the pickup hint when the marker left
the corner map. That check did not collect the pickup. The binding dialog and
native preferences reserve O. An otherwise valid older profile that already
assigned O keeps its other settings. In the installed player, holding O showed
the skeleton-horde objective and releasing O dismissed it. The installation
check and the running process matched the new native. Evidence is in
`research/private/generated/original-minimap-objective-20261122/`.
The broader HUD and input beads (`mc-oi6.6`, `mc-5r5.2`) stay open.

## Active Modern/Voxel Level 1 asset completion — 2026-11-09

Oliver confirms Rust Level 1 is playable and requests completing its Modern
and Voxel presentation, beginning with a source-verified remaining asset list.
This work preserves that behavior baseline and the accepted goat/archer/arrow
masters. Its current inventory and per-family delivery acceptance are in
research/private/remc2-unity-native/LEVEL1_ASSETS.md; character-pipeline/README.md
links the same scope. Source and installed-binding audit confirms Villager
(four looks), Builder, Trader, Worm, Firefly and Castle collector/auxiliary
conversions, remaining building selectors/stages, authored scenery, pickups,
portal, effects and playable HUD. Whole-map Voxel coverage remains integration
work; the current selected house/local patch does not finish the level.
Keep the separate Original/shared-gameplay work below independent. Complete
each converted family through real native binding, checked important frames,
Voxel death breakup and qualified installation before calling it delivered.
Then run the complete level and gameplay/performance checks in both styles.

### All remaining Level1 assets authorized — 2026-11-10

Oliver now requests all remaining Level1 assets. Preserve the completed
characters and full acceptance in LEVEL1_ASSETS.md. Trader mc-oi5 stays open;
its native checks proceed alongside independently owned Firefly and Worm art
preparation. This newer scope replaces the prior batch-only preparation stop,
without waiving any unfinished Trader check. Terra6.1xhigh owns one asset per
worker; GentleBadger owns shared native/render joins, building/scenery/pickup/
effect coverage, final gameplay checks and installation. Serialize shared-file
edits and GPU/Unity/Cargo work. No paid3D generation. Finish each asset through
its real consumer, source lifecycle, checked frames and installed result; do
not close an art-only handoff or call whole-map Voxel/fullLevel1 complete early.
Continue through the complete existing inventory and final mission checks.
Oliver's current execution preference is a small team: two disjoint asset
owners plus root integration. Keep one compact capture/video and receipt for a
decisive behavior, inspect its important frames, then discard redundant raw
frame sequences; do not accumulate large review-player copies.

Private integrated6 contains source-backed Dolmen, Tree, Stone, exit target,
Collector, Experience Scroll and shared Castle/Speed jar presenters plus final
native `ed08ffe1`; it is not installed. The pinned integrated5/c58f player remains
the exact Stone replay/capture pair. The read-only Castle-child ABI passes an earned tick2304
replay and a focused producer/reset test. The corrected Dolmen passes a checked
150-frame normal-camera Level1 capture: both feet meet the slope with no visible
pop; charge and Voxel behavior remain open. The Tree169 teal/cutout trial passes
focused Unity tests and a checked 150-frame native-gameplay view with fuller,
cooler crown and stable grounding; Voxel graphics, frame times and installation
remain open. Stone candidate07 passes five Unity material/lighting checks,
an exact current-native t10688 replay plus 120 neutral ticks, and a checked
120-frame Modern night view with a stable source-like silhouette. Two Voxel
captures showed the same scenery but no statue. The built Voxel FBX root carries
an import rotation and 100x scale that the presenter overwrote. A test measured
0.0183 instead of about 7.8 height on the old Voxel path; the wrapper fix in
1bd39f0b preserves the imported transform and passes 7/7 focused Unity tests.
An integrated8 player using the pinned c58 native plugin passes a 120-frame
earned Voxel capture: frames 0/60/110 show the statue present at matching scale
and stationary on its base, unlike the previous empty scene. Dark Voxel seams
reduce face/arm legibility and precise terrain contact is unresolved from this
angle. Installation and a normal full-level pass remain open. A Fireball passed
through its volume; the original stone lacks the direct-hit flag, so that
observation is not a demonstrated defect.
Collector candidate03 passes actual native-gameplay flight; candidate05's
bright circles read as windows. Candidate06's flatter patches passed a checked
150-frame night flight without motion defects but still appeared too bright.
The darker owner-tinted material passes a second 150-frame integrated6 night
flight with subdued cloth highlights and stable native placement. A checked
150-frame Voxel-world flight keeps this smooth balloon stable as a hybrid asset;
voxelized balloon geometry, loss/reclaim/reset/load qualification and
installation remain open.
The Castle jar passes 2/2 Unity import and public native pickup/reset/cold-load
checks; checked Modern approach and Voxel orbit frames show its appearance.
An earned-terrain Unity check found both jars buried in reconstructed ground
by 0.1044 Castle and 0.4119 Speed world units in both styles. Jar fix 4df898c7
and presenter join c03a7c90 sample rendered terrain while preserving a higher
native position; all four post-fix contact deltas are below 0.0000003, and
six portable focused checks pass. Actual player F5/R, Castle on-screen pickup
removal and installation remain open.
Speed uses the same source TMAP59 vessel, with a distinct class15/model3
presentation matcher. Its fresh public route spawns slot49 at t1107 and collects
it through 76 ordinary inputs at t1183; native cold/reset and 4/4 focused Unity
tests pass. Integrated6 90-frame Modern and Voxel scenery-follow views have
checked stable lid, body and two handles. A fixed-camera earned Modern capture
shows the jar through frame74 and absent from frame75 at the native tick1183
pickup; a matching fixed-camera Voxel capture shows the same earned removal.
Source sound18 was played by the integrated player. The elevated view does
not prove precise ground contact; player F5/R and installation remain open.
Physical sound audibility was not checked.
Experience Scroll has a free editable master, Modern/Voxel import 2/2, and a
focused live class14 DTO/hide projection test. The current-native earned group4
cut matches all five live rows; 255 ordinary inputs collect all five with one
effect code4 and sound63 each, hidden state and slot reclaim. The focused
sim-core test passes exact +4 experience per enabled family, sound63, hidden
same-pass state and following-pass reclaim. Public snapshots expose no raw XP,
and no tier notice was observed. The first integrated6 live Modern capture was
rejected: its dark smooth cylinder did not read as parchment. Candidate07's
lighter layers pass 2/2 Unity imports but its integrated7 Modern side view still
reads as a tan tube and is rejected. Candidate08b has a checked isolated angled
right-end and exposed lip preview; its 2/2 real Unity import/orientation checks
pass and it is committed as 2b79eeb4. Integrated8 is built privately with this
scroll and the Stone wrapper fix. Its checked 225-frame earned Modern view is
still rejected: the foreground side view reads as a dark tan cylinder even
though the rolled end is clearer in the background. A stronger parchment
shape/material pass, live Modern/Voxel review and installation remain open.
Candidate08c7 committed as 424763bb removes Voxel preview stripes with
outward normals, retaining sampled block geometry; its red/green Unity gate
passes. An integrated9 earned 100-frame Modern view still reads as a gray
roll with the right rim obscured by vegetation and was rejected. Source TMAP259
is a horizontal tied roll, so the next pass targets clearer source-faithful
paper and right-rim visibility in the actual night camera.
Scoped cleanup of old generated caches, failed villager trials,
derived character history, obsolete review players and duplicate PNG frames
recovered about 22 GiB across SSD and HDD. Videos, receipts and native frame
observations were retained for qualified captures; the active Unity and
TRELLIS toolchains, accepted masters and current/rollback players remain.
Portal appearance, dynamic effects, whole-map Voxel and gameplay performance
also remain open. Public Level1 controls reach Firefly group6, its real lethal
combat, Worm group4 and a genuine House10 demolition-born Trader172/gen27.
The duplicate shared StageVar prepass is removed; positive-life Villager entry
is qualified separately from death. The Castle holder's previously stale
source-cursor phase is fixed: the original dead-player failure now advances
3497→3498 and continues120ticks. WormV2 uses stable appearance-based segment
indices, not the scheduler's advancing cadence byte; all16parts pass121real
native observations after natural birth. These are native/source results,
not installed-asset or full-mission completion claims.

Firefly and Worm art/imports pass actual Unity surface/axis checks. Correcting
the imported Worm's swapped height/depth bounds restores the intended native
visual height; actual baked Idle geometry determines nominal size and sampled
clip unions determine conservative culling. The integrated character suites
passed159 cases without skips. CPU albedo sampling repairs grey new Voxel
skins while preserving all24skins' geometry/weights and accepted Goat/Archer
palettes. Actual native2f combat confirms Trader's one lethal breakup and
positive-life house entry without death; its actual F5/R/coldloads pass both
styles. Firefly's two270frame combat captures and F5/R/coldloads pass; its
Modern post-removal fade lasts0.167s. Worm's real full-family kill emits one
8192-cell breakup. Both Worm styles pass actual F5/R/coldload, with all17native
lifetimes/links checked separately from the head-only frame receipt.
These are candidate-client checks, not
installed delivery. Original explosion sprites still obscure some breakup
frames; a review-camera projection reset also widens the Voxel death view.
The camera correction is being qualified before new checked media.

Rendering source49895483 removes only coincident equal-weight interior faces
from Firefly64/WormHead64/WormSegment64, retaining all source cells and exact
posed breakup goldens. Three new cases reject the old unculled skins;140
focused cases pass after culling, with no skips. Matched private3090KWin Vulkan
1280x960 no-readback Voxel combat runs improve29.25/34.22→36.26/39.95FPS;
GPU median falls~8.5→~5.3ms and submitted triangles~44m→~22m. This is not
smooth yet: native throughput remains~19ticks/s and frame p95~85–90ms.
Repeated tiny high-speed impact-body physics is the remaining blocker, not
the bounded Worm snapshot. A current controlled captured-terrain comparison
reproduces speculative CCD being~5xslower; do not adopt it. Sphere shapes
give only a small free-flight gain without any contacts, so are not adopted
from that evidence. Oracle is cross-examining the code/measurements. One
candidate Modern sample overlapped an unexpected stopped background index and
is explicitly excluded. UBS is partial owing to taint-work budgets; doctor
passes and changed hunks/actual tests are checked. Keep installed players.

Independent collector asset mc-chx now has inspected eight Day/Night owner
variants, editable candidate06 and Unity-format export. Unchanged earned
native2f inputs produce Castle115 at1739 and collector372 at1761, collect four
500-mana spheres and deposit2000 at2745. The shared read-only Castle-child
consumer now reports slot372/gen11, Castle115/gen2, mana2000 and player colour0
at public tick2304 on native33ce; source8e rebuild retains the same ABI and
passes the focused native test. Unity import and matching private player build
pass. Candidate03 and candidate06 in-game Collector flight captures each pass
150 frames; candidate05's circular detailing read as portholes, and candidate06's
flat patches were too bright. The darker material passes another checked
150-frame integrated6 night flight. Voxel/hybrid form,
loss/reclaim/reset/load visual qualification and installation remain open.
Generic owner_color is a palette index and is not a safe cloth tint.
Trader/Firefly/Worm/collector beads retain unfinished acceptance. Castle,
auxiliary, scenery, pickups/effects/HUD and whole-mapVoxel remain open.

### Active Builder/Trader delivery — 2026-11-10

Oliver authorizes the next asset batch. `mc-jcy` owns Builder appearance/model12
(TMAP141–143,parameter221); `mc-oi5` owns Trader/model14
(TMAP138–140,parameter219). Both use source logical height250. One Terra6.1
xhigh owner per asset; GentleBadger owns shared rendering/native consumer joins,
actual Unity/gameplay checks and qualified installation. Preserve all accepted
villager/goat/archer assets and current player. Use the established free
human/garment pipeline; inspect Day/Night source frames before choosing clothes
or props. No paid3D generation. Distinguish real damage/death from positive-life
house entry/build completion, which may share removal callbacks. Deliver genuine
house production and admitted behavior, five basic actions plus any source-
required work, Modern/Voxel binding, exactly-once true-death breakup, public
reset/save/coldload, important-frame review and performance evidence. Do not start
another asset family before this batch's complete installed cycle works.

Builder/Trader art is committed in c36150ee/6a0c53dc; shared native/presentation
joins are65724d7b and the production Unity reader admission is7f554577. Both
have five same-rig actions, separate editable human/costume/props and64/96Voxel
derivatives. Both25-pose actual Unity surfaces pass0.1mm; integrated tests pass
106 with one existing optional profiling skip; final human/reader suite60pass.
An actual gameplay run found the old reader's model13-only whitelist despite
standalone art passing; its strict class/model/appearance join is corrected.
No paid3D generation. The accepted goat/archer/villager masters stay intact.

Actual public Level1 inputs produce Builder454/gen28 from demolished house16.
One branch kills it at life−1 and emits one retained death; another constructs
house16/gen1/BUILD39 and enters house38 with life999, without a death. Two
120-tick fresh native comparisons pass. All four Modern/Voxel death/work
captures pass180frames. F5/R plus coldloads in both styles preserve the live
Builder, then clear it and restore28 villagers on reset with no false bursts.
Nativead803e03 is an isolated committed-source build: base6eb88722 plus owned
read-only character/death leaves, recorded in65724d7b; concurrent Original WIP
and its installed player/plugin are excluded. Presentation now selects
generated/settlement-people-20261110/player-reader; prior pointer is preserved
at Builds/Presentation-before-settlement-people-20261110.

Trader mc-oi5 remains OPEN. Art/import/animation/Voxel bindings are ready. A
real direct Level7 session exposes four Traders219/base138/height250 and living
116→hidden→reclaim house entry; a fresh instance matches1200input steps. This
does not prove natural Level1 production, lethal combat or a live Voxel death.
All47 Level1 opening houses have2occupants; bounded public routes did not reach
the Trader producer population gate. The direct Level7 handle rejects GameRuntime
save/voxel enable. A real public Whirlwind pickup succeeds at tick23, but its
2000mana cost exceeds that fresh session's1000maximum; cast attempts match a
no-cast counterfactual and do no damage. Continuing that public route hits
native status8 at tick1139: target slot53 lacks its shared spell record. A fresh
instance reproduces all1140commands and the error. No grants, actor injection
or replay edits were used. The runtime defect is handed to the shared engine
owner; do not close Trader or start another family.

Checked16.6s portrait media and evidence are under generated/
settlement-people-20261110. Actual3090 privateKWin/Vulkan1280×960 no-readback
two-run medians are218/113FPS Modern/Voxel, versus contemporaneous old-player
229/120. Native throughput is~27.99/27.24ticks/s. This is no speedup claim;
tails exceed100ms and Voxel is slower in these samples. Performance comparison
and limits stay explicit; no unlocked-desktop, VR or full-Level1 claim. Source
recipe is character-pipeline/VILLAGERS.md. UBS C# remains partial owing to
callable/taint budgets; owned changes were reviewed, compiled and run.
Both actual default launchers independently passed180frames of the Builder's
tick1554 kill from the qualified installation, without a player-path override.
`mc-jcy` is closed; `mc-oi5` retains all unfinished acceptance. Final media is
builder-trader-installed-checked.mp4 and uses those installed launcher captures.

### Installed villager roster — 2026-11-10

Oliver's accepted human/robe pilot now covers all28 reset villagers, through
all four source appearances:239×3,241×6,242×9,271×10. The three new asset
projects `mc-tfo`/`mc-ijv`/`mc-rio` complete their Modern/Voxel consumers,
five actions and real native lifetimes. No paid 3D generation. Goat/archer
masters and the previous Presentation player remain intact. Source is through
`9cf9159f`; normal run-modern.sh/run-voxels.sh now select generated/
villager-roster-20261110/player-view. Native is qualified `e8a6e707` from
`eaf0f1bc`; this asset work does not replace concurrent Original engine work.

All new exports pass25 actual Unity poses within0.1mm. Integrated character,
ground/material, crown-contact/recoil/fracture, terrain and debris suites pass
105 cases with one existing optional profiling skip; after view culling,
53 relevant cases pass again. Real public inputs kill239/slot53/tick352,
242/slot54/tick415 and271/slot51/tick350; cold replay reproduces every sampled
native target state and reclamation. Each has one actual Voxel breakup.
Modern corpse playback completes after native projection removal. Current
player R/F5 and cold loads retain28 unique villagers with the four exact look
counts in both styles. Both actual default launchers independently record the
carrier's native kill without a player override. Prior pilot pointer is
Builds/Presentation-before-villager-roster-20261110.

The shared768-body debris pool now prioritizes deaths and refreshes old impact
pieces instead of starving real breakups after repeated Fireballs. The carrier
uses actual cached rigid crown geometry to support its level jar on terrain
and progressing recoil; released Death remains authored. Modern culls unseen
villager animation with generous view bounds. Matching private KWin/3090
Vulkan1280×960 runs improve Modern~160→222FPS, Voxel~120FPS, two runs/style.
Native~28.05/27.17ticks/s and>100ms frame outliers remain. Capture cadence is
separate from performance. No unlocked-desktop, VR or full-Level1 claim.
Recipe: character-pipeline/VILLAGERS.md. All live/source/install/performance
proofs and checked portrait media are under generated/villager-roster-20261110/.
Builders/Traders are the next humanoid conversion family; no new assets were
started before closing this cycle. The older pilot notes below are history.

### Installed pilot — 2026-11-10

Oliver accepted this pilot and requests the remaining Level1 villagers with
the same free separate-human/garment pipeline. Active asset owners are
Villager239 (`mc-tfo`, brown robe/cap, three residents), Villager242 (`mc-ijv`,
pale vertically striped robe, nine), and Villager271 (`mc-rio`, dark robe,
pale headcloth and head-carried jar, ten). Day and Night original sheets have
both been inspected: Night palette colours must not be mistaken for neutral
fabric colours. GentleBadger owns shared Modern/Voxel admission, import,
native lifetime verification and qualified installation. Preserve the accepted
241/goat/archer and currently installed player until the complete roster is
qualified. No paid 3D generation or Rust gameplay changes. Source logical
heights are 200/250/250/300 for 239/241/242/271 respectively, as declared in
sim-definition/adapter.rs; the carrier's total height includes its jar.
Live roster integration exposed a shared Voxel consumer defect: four Fireball
impacts exhaust the 768-body rubble pool, so `EmitCharacter` repeatedly refuses
the real death and its hidden actor never breaks up. Fix bounded priority/
admission in the shared rubble owner, retain the existing body budget, and
rerun all three public-input native kills before installing or closing them.
The bounded pool priority/recycling fix is committed in `8863af56`. The
integrated roster/import, held-jar contact/recoil/fracture, existing characters,
terrain/water and rubble suites now pass 105 cases (one existing optional
terrain/CCD profiling case is skipped). Brown/striped/final carrier exports
each pass 25 actual Unity surface poses within 0.1 mm. The carrier's actual
rigid crown supports its level jar on slopes and during hits without per-frame
mesh baking; released Death remains authored. Native cold replay reproduces
all three public-input deaths and reclaim. Final player, repeated live Voxel
breakups, reset/save/load, performance and installation remain pending.

`mc-fu7` now delivers one Villager appearance 241 (six residents, TMAP180–182),
using a separate editable MPFB human and robe. The lower garment is simulated
offline and fitted to 48 shared cloth bones; the final 224-bone skin has all
five actions and 64/96 voxel derivatives. No paid 3D generation was used.
Actual Unity surface conformance passes at 25 poses within 0.1 mm; its rendered-
point negative control rejects. Focused retained-character, terrain, voxel and
receipt suites pass. Real native inputs produce the tick529 death: Modern falls
and settles with measured ~2 mm costume clearance, Voxel emits one breakup.
Actual R/F5 controls and fresh save loads retain six unique residents in both
styles. Both installed launchers independently captured the real native kill.

Source includes `59f8a42e` and `69ed7623`. `Builds/Presentation` selects the
qualified owner-local player in generated/villager-modern-20261109/runtime-character/
player-receipt, with native SHA256 `e8a6e707` built from `eaf0f1bc`. The previous
pointer/player remains at `Builds/Presentation-before-villager-20261110`.
The unchanged real terrain-impact validator also passes: 2,007 current-base
native removals equal the visual removals; all 6,751 historical intents remain.
Waterbed updates now use bounded source/voxel invalidation and cached vertices.
Accelerated KWin/3090 Vulkan1280×960 measurements are separate from capture;
Xvfb totals are not comparable with the earlier accelerated benchmarks. The
installed player measured medians 231.54/123.00 FPS Modern/Voxel in two runs per
style, with >100 ms outliers and native throughput ~28.10/27.23 ticks/s still
present. No unlocked-desktop, universal
smoothness, whole-villager-family or complete-Level1 claim. Checked media,
source hashes, XMLs and install receipt are under that runtime-character folder.

### Pilot authoring history (superseded by the installed result above)

The pilot `mc-fu7` began with one Villager appearance 241 (original TMAP
180–182; six Level 1 residents), preserving accepted goat/archer and installed
players. Oliver requires **zero paid 3D generation** and explicitly requests
setting up local TRELLIS and comparing it with the Blender attempt. Imagegen
front/back references are owner-local in generated/villager-modern-20261109/.
WhiteBay's original Blender experiments are retained read-only; GentleBadger
owns integration. The isolated local TRELLIS comparison is complete.
Native observation/journal admission is committed in `eaf0f1bc`; Root reran the
549-input earned Fireball kill and warm/cold replay on optimized native
`e8a6e707`, with exactly one death record. Five Unity admission/input/material
loader checks pass. The isolated 8189 local TRELLIS runtime is working; Root
verified successful 512 and 1024 jobs and output hashes. Matched clay/Eevee
comparison favors 1024 as a starting mesh, with better face and real cloth
folds. It is not a finished asset: the two-view gown opens/distorts in motion.
Paired 120 Hz FBX/GLB reimports retain loop/contact, but these metrics do not
approve the visible character. The four-view job also completed (507.83 s),
and Root checked its history/hash and matched Eevee/clay frames. Explicit
profiles improve the deep side slit at rest; the common-rig diagnostic Run
still produces stretched lower-garment triangles, and the four-view eyes
regress. This comparison is complete; further generation/weight-field variants
are stopped. TRELLIS1024 is useful as a sculpting base, not an automatic rigged
asset. Deliberate garment/foot separation, retopology and skinning are the next
concrete repair. Current five Unity checks cover admission/input/material
loading only; whole-asset animation checks have not passed. Complete actions,
Unity import, live important frames and installation remain pending. No new
villager player is installed. Keep the pilot open. Runtime/setup/comparison
instructions and the three measured jobs are in character-pipeline/README.md;
owner-local meshes, prompts and matched frames are retained in generated/.

Oliver resumes with a separate human figure wearing a draped robe. TopazReef
owns `dressed_villager.py` and actual body/garment authoring and moving-frame
repair. WhiteBay's prior mesh experiments are retained read-only. Fitting over
the reused archer failed actual-mesh checks: its bulky existing clothing is
not an anatomical human, despite simplified proxy clearance. That branch is
stopped. `human_figure.py` now builds from the local CC0 Vitruvian human and
preserves its own Rigify rig, all 146 drivers and A-rest skin binding. Its
procedural actions use the original rig; no foreign A-to-T mesh warp. Candidate08
has corrected arm carriage and same-rig Idle/Walk/Run/Hit/Death, normalized top-four
weights, one LBS modifier and single-segment weighted bones. Reopened gait loops
match; actual sole/body contact is sampled at quarter frames. Root's compact
86-bone bake matches 25 body poses within 2.35 micrometres before FBX. FBX
roundtrip exposed Blender's automatic connected-joint translation lock; the
verifier now restores ordinary transform semantics and matches within 0.0235 mm.
Root's actual Unity6000.3.20f1 import matches all 25 sampled surfaces in both
directions within 0.1 mm after one fixed (x,z,y) axis conversion. This is a body
consumer check, not dressed gameplay, Voxel, performance or installation.
The direct sewn robe remained flat after bounded static draping. A free CC0
Donitz monk robe has useful authored folds, but its hm08 fitting data cannot
directly address Vitruvian. TopazReef's supported MPFB matched-human/garment proof
is therefore the next worn-figure foundation, with motion owner reusing the
procedural Rigify actions. Keep all prior body/fit trials.
MPFB motion02 now has five same-rig actions and measured Walk/Run stance speeds
1.4300195/3.7283345 m/s in measurements-audited.json. Root exported its visible
body with 188 deform bones and independently checked all 25 Unity poses within
0.1 mm; an exposed-unweighted-helper negative check also rejects correctly.
The robe still wedges/crosses the leg; TopazReef owns its sector/vent and costume
repairs. No dressed motion acceptance or new player installation yet. The full anatomical
body stays editable; game export may use the garment's authored covered-body
mask, as documented by MakeHuman. Root corrected the overstrict hidden-skin
intersection gate: actual visible clipping, exposed stumps/holes, bad garment
crossings and incomplete moving silhouettes still fail. No arbitrary height
cutoffs. Clean skinned bodice, human and costume remain the foundation. Eight-
bone sectors, translation cages, local ease and direct same-leg split panels
all failed actual moving silhouette/contact checks; stop those variants.
Test one bounded offline lower-garment cloth cache on the new clean human with
an initially clear surface, pelvis support and real leg collisions, then bake
only acceptable motion to cheap game bones. Earlier whole-gown simulation used
the invalid clothed donor; do not repeat that setup. No runtime cloth solver.
Oracle's independent actual-image/code review was attempted in
generated/villager-lower-cloth-oracle-20261110/; browser submission/recovery
failed, so no new Oracle answer has been used. Actual Blender freefall controls
proved default air drag incompatible with the trial's tiny areal vertex mass.
Native-reference material trial16 produces visibly smoother drape; Root reviewed
large front/side frames and the contact sheet and admits it as raw input for
explicit loop authoring/skeletal fitting, not final character acceptance. Keep
its ~2.3 mm sampled fingertip contact caveat visible. Authored loop09 becomes
polish04's shared 48-bone/fixed-weight Walk, which Root reviewed at large front/
side worst-contact frames. Actual 120 Hz playback has zero nonneighbor cloth
self-crossings or shoe pairs; small covered-thigh/fingertip residuals remain
(worst sampled trouser vertex ~0.34 mm inward, not an exact depth bound).
Root accepts this as the Walk basis and stops intersection-count polishing.
Its complete 18-component joined export now passes five actual Unity poses
within 0.1 mm in both directions (252 deform bones; 29,405 split vertices).
Small skin weights must be configured during model preprocessing; a post-import
setter did not preserve them. This remains a partial Walk conformance check.
TopazReef's upper-only game preparation is committed in `7bbbd645`; Root reviewed
the collar correction and large moving upper frames. Root's actual Level1
calibration measured six residents over 300 unmodified coast ticks: ordinary
source speed18/action111 yields median 8.30627 world units/s, so Run is the
principal live gait at the measured costume scale. Walk-only cloth weights lose
visible folds/contact in Run; AmberCave is testing one jointly trained weight
set and source-rate transform keys. Keep accepted Walk immutable for comparison.
The corrected shared-weight Walk/Run master is now frozen at SHA256
0a24b6ddc552cd078960e3db76b9df95f43dc2e0b72262c764fab8e7281cbf56.
Root reviewed stride/flight/worst-contact frames and independently reran upper
preparation, joined FBX and actual Unity conformance: all ten sampled poses
match within 0.1 mm in both directions. Evidence is under
generated/villager-modern-20261109/gait-unity-conformance/. Small covered-body/
natural-fold contact residuals remain disclosed. Twelve retained goat/archer
Unity animation checks pass after the pending import/review-source changes.
AmberCave owns cloth_skinning.py and the remaining action integration; TopazReef
now owns villager_cloth.py and finite Idle/Hit/Death targets. Full-costume Death
auditing found upper robe/trousers/headwear/shoe floor penetrations despite
the bare-human proof. Root authorizes correcting only the authored Death pose
for dressed support before simulating it; other four body actions stay exact.
Idle/Hit/Death garment behavior, final Unity frames, Voxel and gameplay/install
still remain open. No new Villager player is installed.
Oracle GPT-6 Pro source review is retained under generated/villager-robe-oracle-20261109/;
Root verified the unusual rig layout (Spine02 is lower, Spine is upper), and
TopazReef corrected garment weights, the collar discontinuity, a 36 mm imported
origin shift, missing GroundControl and the defective per-proxy clearance gate.
Use final combined actual-body checks and actual complete-phase frames. Old
trial27 quarter-frame sole checks did not establish acceptable anatomy or a
wearable robe. The final dressed appearance remains unapproved; AmberCave now
owns the fixed-weight garment bake with released py-dem-bones0.13.1. Its Walk
result is admitted above; the remaining actions and consumers are still open.
GentleBadger owns integration and acceptance. Keep editable body and robe
separate; a final game export may join disconnected components on one rig
without fusing garment to skin or shoes. First establish a believable worn
robe in motion; a simulated cache, solver import or training error alone does
not finish the character or qualify runtime performance. No paid 3D services.

The final dressed master now supersedes the partial checks above. Cloth and
costume source is committed in `0f89d761`, `2aa530fa` and `205ce5a2`; Root reran
the three-step cloth recipe and independently reproduced all 264 Death target
poses exactly. Final joined game skin: 224 bones, 26,204 face-referenced points,
26,931 Unity split vertices. All five actions pass the actual Unity surface
gate at 25 poses within the unchanged 0.1 mm tolerance. Four masked non-face
anchors are explicitly counted/excluded; a 1 mm rendered-point negative check
still fails. Thirteen villager tests and 48 Goat/Archer/Voxel tests pass. The
actual 3090 Vulkan importer produced 64/96 villager skins (8,393/20,045 cells).
Root owns pending shared presentation/input wiring and is building the private
player. Actual Level 1 moving/death frames, Voxel breakup, performance and
qualified installation remain open. No new Villager player is installed.

## Shared binary presentation delivery — 2026-11-08

Oliver requested finishing the JSON hot-state fix as shared engine infrastructure
before building out and gameplay-benchmarking Level 1 in Modern, Voxel and
Three.js/WASM. Implemented MCPO v1 in sim-session, a guarded native copy endpoint,
an instance-owned reusable Unity buffer and an independent TypeScript reader.
One committed publication binds runtime/session identity, tick, terrain revision
and the complete render view. Ordinary state reads avoid diagnostic hashing,
JSON decoding and redundant full identity polling. Pause/load/reset/preferences
and mission transitions retain explicit lifecycle validation; event journals
retain their ordered cursor and lifetime rules. JSON control, save and diagnostic
interfaces remain. See research/private/remc2-unity-native/PRESENTATION.md.

Installed: normal run-modern.sh and run-voxels.sh now select the same qualified
Builds/Presentation player, native SHA dc5ea14833386b7831c90d6148b06eb6dd949c0df683dd3c0a37e7b01bd91216.
Previous Modern/sandbox/voxel artifacts and explicit legacy selectors remain.
The new shared default opens the actual Level 1 start. Goat/archer presets were
regenerated by executing their recorded public inputs against this native;
old replay files and validators were preserved. This is not save migration.

Verified: 14 focused Unity checks; native workspace/all-target check, focused
binary and diagnostic parity checks, C11 header; four TypeScript checks including
field-by-field comparison at two real Level 1 boundaries; shared sim-session
codec compiles for wasm32-unknown-unknown. Root independently reran the native
observation tests. Real B/F2/P/Space/F5/F10 checks removed 626 house cells, saved,
and restored all 626 in a fresh installed launch without historical impact
emission. Installed Modern advances real Rust ticks, draws all 32 source
buildings, and makes no full identity JSON reads in its measured window.

Three paired same-build warm voxel opening runs at 1280x960 on the actual 3090
measured median average FPS 47.37 with the former full diagnostic/polling pattern
versus 223.16 with binary publication and metadata reuse; p95 31.61→16.46 ms.
This measures the complete read-path correction, not binary encoding alone.
Full identity reads fell from roughly 12,900 per control run to zero.
Quiet opening is not combat/destruction, full-game or desktop/VR qualification.
Initial loading and long frames remain; the full campaign WASM adapter,
Three.js visual/gameplay parity and matched gameplay benchmarks are next work.
The existing intermittent music integrity failure remains unresolved; diagnostics
now preserve actual/repeated/fresh-read digests on failure, with no guard bypass.
Changed-source C# UBS coverage is partial; compiler/tests and focused review
remain necessary evidence. All private runs and source/artifact receipts are
under research/private/generated/mc-engine-benchmark-20261108/.

## Voxel rendering optimization pass — 2026-11-08

Oliver requested one Astra high review, measured FPS improvements and a hybrid
rendering assessment. The review and local implementation are complete: larger
render groups preserve exact voxel faces and 16-cell collider/edit chunks; local
lake-bed invalidation preserves full rebuild output and shoreline changes.
Twenty A/B runs measured ground throughput 32.64→39.20 fps and draw calls
6,847→831; quiet 29.32→32.29 fps. Three paired water comparisons reduced median
early impact-window worst frame 873.34→194.35 ms. Remaining roughly 20 ms campaign
CPU turns, cold loading and residual hitches are open; this is not 60 fps qualification.

Installed separately through `run-voxels.sh --optimized`; accepted `--connected`
player-06 and previous Modern/classic selectors remain. 92 integrated checks pass
with one opt-in profiler skipped. Installed cast/save/controls and cold reload
pass; final impact/crater/shore frames inspected. Intermittent music integrity
failures and one native Unity Editor crash are preserved and unresolved; no
validation weakened. Native remains frozen `ea9276a3`. See
`research/private/remc2-unity-native/VOXEL_SANDBOX.md` and ignored evidence under
`research/private/generated/mc-voxel-perf-20261108/`. Fireball visual lifetime,
smooth mana and full 3D actor admission remain separate future presentation work.

## Installed native voxel combat and mana loop — `mc-mwt` (2026-11-08)

The Terra 6.1 xhigh team delivered the first native-connected Level 1 voxel
interaction. Rust owns real Fireball hits, sparse terrain/house edits, exact
lifetime death records, native mana ownership/collection and replay persistence.
Unity presents the approved goat/archer rigs as fine voxels and explodes deaths
into independent physical pieces. Proper source-colored 3D mana spheres remain
after rubble fades. Accepted Modern player-15 and classic player-7 are preserved.
This closes this connected sandbox milestone, not every Level 1 asset, whole-map
voxel streaming, complete campaign assembly or the separate native-family work.

Implemented/committed/pushed: native geometry and journals `b0db8ae4`, guarded
world observer query `d6665e38`, client and earned E2E checker `73408438`.
Installed: separate player-06 at `Builds/VoxelsConnected`, launched with
`research/private/remc2-unity-native/run-voxels.sh --connected`. Native SHA starts
`ea9276a3`; managed assembly starts `a1ea55bf`. All 170 payload files are pinned.
Exact source closure: 424 copied inputs and 195 read-only resources. See
`research/private/remc2-unity-native/VOXEL_SANDBOX.md` for controls and receipts.

Verified by Root: 71 relevant Unity checks plus one revised collision fixture,
9 Python checks, the focused native/ABI checks and earned public-input mana E2E.
Native death at 535 → sphere 364/500 birth at 539 → player 152 claim at 543 →
defender 372 collection at 1574 → reclaim at 1575 → Castle 113 arrival/500
receipt at 1969. Cold hashes
match; numeric Castle balance is not exposed, so credit evidence is the actual
arrival event and its native consumer call. Earlier failed helper-reuse evidence
is preserved without weakening shared assertions.

Installed B/F2/P/Space/F5/F10 checks pass. Fresh loads restore house 626 cells at
hash 15883481164168150708 and ground 6,816 at 5432479835024191935, with matching
native/visual counts and no historical impact emission. Ground escape failures
are fixed: both final unthrottled runs have zero below-terrain fragments, versus
68 impact pieces before physical-surface clearance. No native gameplay positions
were changed to hide presentation penetration.

Root checked important raw and encoded burst, fragment, settling, crater and
mana-after-fade frames. The 26-second full-frame phone preview is at
http://3090.bumblebee-hue.ts.net:8095/voxel-connected/ . House/ground slow motion
is visibly labelled. The full capture tree, including failed runs, is independently
byte-verified on private Rig; manifest 38b39babc0a83368a0bfc845d3dabe4a45bd12ea0ff8df7212def3a6c866d537.

Performance remains qualified: quiet p50 27.51/27.62 ms; ground-settling p50
32.42/32.64 ms, p95 43.47/54.62 ms at 1280×960 on 3090 without readback. Frame
percentiles exclude the first Unity game second and the initial hit. Whole-run
PhysX maxima 16.60/8.31 ms and edit maxima 5.91/8.92 ms are retained; this is not
a hitch-free/60 fps claim. Original flames obscure the victim early. Close-ground
KWin captures failed their separate F5 checks, while the earlier Xvfb save and
installed cold-load passed; the cause is unestablished. Desktop/VR and interactive
phone-browser playback remain unqualified (private DNS unavailable in the remote
browser). HTTP download/hash/range/fast-start and complete H.264 decode pass.

## Installed Modern voxel refinement (2026-11-07)

Oliver accepted the first voxel experiment and requests smaller independent
marble-like rubble, removal of destruction hitches, and the beautiful Modern
environment with voxelized approved goats and archers retaining native game
behaviour. Use brick-sized house cells, finer characters and varied terrain
resolution. Preserve existing installed versions. Root owns Modern composition,
native-state joins, installation and checked video; Terra physics owns pooled
small debris, Terra characters owns fine skinned conversion/snapshot, and Terra
volume owns bounded edits and local terrain contacts. Verify the real integrated
player, important frames and comparable impact/steady frame timings before
handoff. Do not substitute authored character motion for native behaviour or
claim local Unity terrain edits are Rust campaign persistence.

Implemented and installed as separate `Builds/VoxelsModern` player-15; the launcher
defaults to this iteration, with `--classic` preserving player-7. The approved
Modern player is unchanged. House cells are 0.25 units, editable ground 0.125,
and near goat/archer skins 96 cells high, using original rigs and native decisions.
Pooled small rubble, bounded edits, sparse terrain contact tiles and the physical
patch seam fix the measured allocation/CCD spike and escaped-fragment defects.
Exact terrain-height caching removes the repeated hoof-sampling computation.

Self-verified: 93 relevant tests, 29 subsequent affected terrain tests and 18
final physics/contact/seam tests passed (overlapping sets). Final repeated world
runs, native death, all four captures and installed F2/B/W/Space/F10 checks pass.
Inspected gait, release, impact, crater and settling frames in the 36.5-second
video. Evidence: `/mnt/pinokio-shared/magiccarpet-voxel-modern-20261107/`;
phone video and exact scope are in `research/private/remc2-unity-native/VOXEL_SANDBOX.md`.
Full-scene 3090 median is 37–40 fps at 1280×960, p95 33.55–39.25 ms, worst
88.48–95.25 ms: whole-frame hitch/performance work remains open. Local destruction
does not alter native campaign terrain/saves/AI, and the native combat flame is
still a legacy sprite. Actual desktop/VR remain unqualified; runs used a private
GPU compositor, with installed controls on a private X display.

## Delivered voxel sandbox experiment (2026-11-07)

Oliver accepted the Modern goat/archer delivery and requested a separate small
Level 1 voxel sandbox. Preserve the accepted Modern and Original installations.
Compare coarse/medium/fine voxel sizes on the same scene; reuse the approved
image-derived goat model and its animation. Implement destructible terrain and
building volumes, goat death as fragmentation with bouncing/settling rubble and
fade, and a cohesive voxel fireball whose impact scatters fragments with incoming
momentum and physical contacts. Deliver an interactive sandbox plus a short
actual-player video with important frames inspected, and separate unthrottled
performance measurements covering quiet/impact/settling. This is an isolated
physics experiment over Level 1 source geometry, not full campaign destruction
or proof that the existing campaign CPU bottleneck is fixed.

Implemented and installed separately through `run-voxels.sh` / `Builds/Voxels`:
three terrain sizes, three animated goat resolutions, destructible source house
and terrain, impact-driven fireball fragmentation, bounded physical rubble and
rest/fade. Root integrated the three Terra-owned components. The 24 focused Unity
tests pass, and actual keyboard checks pass against the installed launcher on a
private display. The 34-second actual-player recording was inspected at gait,
impact, fragmentation, roof damage, crater, settling and fading frames.

Evidence: `/mnt/pinokio-shared/magiccarpet-voxel-sandbox-20261107/`; qualified
`player-7`, `tests-compact.xml`, `capture-compact`, `installed-controls` and
`final-benchmark-*`. Fine preset median 4.19 ms / p95 5.82 ms at 1280×960 on the
3090; normal destruction worst frame 37.71 ms, 64-cell goat stress 68.04 ms.
Synchronous fracture work remains a hitch limitation. Measurements use an
isolated GPU compositor; the locked desktop and full campaign are not qualified
by them. See `research/private/remc2-unity-native/VOXEL_SANDBOX.md` for scope,
commands, physics approximations and the phone video. Accepted Modern remains.

## Active Modern characters — finish the live Level 1 cycle (2026-11-07)

Oliver rejected treating isolated animation previews as finished assets. Finish
goat and archer in actual Level 1 gameplay before starting another asset. Preserve
original gameplay speed/behaviour; author natural modern motion. A same-height
archer target requires a horizontal arrow at release; elevated targets require
coherent pitched aim. The checked preview milestone below remains prior evidence.

- Goat Terra owner: measure source/game travel speeds and timing, repair gait
  and stride/playback coupling, reactions/death and terrain contact. Own the
  goat exporter and focused goat runtime presenter/checks through live delivery.
- Archer Terra owner: repair nock/draw/release/flight alignment, movement,
  reactions/death and complete clip transitions. Own archer export and focused
  archer runtime presenter/checks through live delivery.
- Root integration owner: typed native-state joins, shared playback/import,
  real Level 1 model bindings, serialized Unity/native builds, installed player,
  gameplay recording and independent important-frame inspection. Fix defects
  with the asset owner until live movement, attacks, hit/death, cleanup and reset
  are coherent. Keep all other asset remakes inactive. Record reusable pipeline
  fixes in canonical code and the character README, with verifiable checks.

Acceptance requires matching current Rust gameplay in the installed Modern
Level 1 player, with real source-driven actors and attacks; a staged camera or
declared scenario setup is acceptable evidence only when disclosed. An isolated
review sequence or mocked entity row does not finish this capability.

Current implementation: source-backed goat/archer/arrow presenters, an 80-byte
read-only native observation, allocation/world identities, actual target aim,
distance-driven gallop/flee and moving-hit head overlay, bounded terrain IK,
standing/kneeling shots, continuous recovery/hit/rise and death cleanup. The
asset/task inventory and reusable repairs are in the character-pipeline README.
All 48 combined character checks pass, including moving-hit interpolation stalls
and both standing-aim exits. Native public-input checkpoint checks cover eight
movement/damage/stance scenarios and a real allocated archer arrow/target.

The qualified player is installed through the existing build/install checker at
`/home/oliver/Projects/odcpw/magiccarpet-character-level1-20261107/remc2-unity-native`,
currently `build-txchc9z2`, native SHA256
`dc5a115321b1780f5f328ea6c6e2edad1cf4818885e655ff5e32365d58b2288d`.
Normal `run-modern.sh` and `run-characters.sh` select it; the frozen environment
sandbox and Original installation retain their previous selectors. Real X11
keyboard restart/save/F10 exit passes on this installed build (saved tick1,
clean exit0). Nine scenario captures plus two final standing-exit views pass
all 1,410 full portrait frames. Root and Terra inspected important action and
transition frames. Terrain obscures some shoes; precise sole clearance is
measured separately on flat imported geometry, with live goat terrain telemetry.
The 24.5-second phone review is installed at
`http://3090.bumblebee-hue.ts.net:8095/`, with checked H264 decode, HTTP range
seeking and browser playback. Its manifest names each checked build/angle;
recording cadence is not FPS. The visual asset cycle is self-verified and
installed; user review and the gameplay performance issue below remain open.
Evidence: `/mnt/pinokio-shared/magiccarpet-character-level1-evidence-20261107/`.

Gameplay performance remains open. The private 1000x700, Vulkan, unthrottled
launcher smoke measured median37.77ms/p9569.40ms; measured campaign processing
is the largest CPU component (step/snapshot mean14.77ms, campaign-frame mean
24.66ms in the first run; Modern rendering mean1.97ms). These cold/private
measurements are not a comparable baseline or an actual-desktop qualification.
Do not call this a smooth-play milestone or start another asset family while
the operator's current hold remains in force.

## Modern goat and bow archer — checked asset delivery (2026-11-07)

Oliver requested repaired, complete character actions and results checked in
Unity. Implementation `41fe2868` delivers the grounded goat idle/walk and archer
idle/walk, standing reach/nock/draw/release/recovery, descent, kneeling shot and
rise. Selected owner-local exports are `goat-neutral2` and `archer-candidate12`
under `research/private/generated/characters-complete-20261106/`. Canonical
authoring/export scripts and Unity importer/playback/checks are recorded in
`research/private/character-pipeline/README.md`; generated assets stay ignored.

Fresh Unity checks pass 12/12. Actual-player capture contains 874 full portrait
frames, with zero framing failures; root and the Terra motion owner inspected
key action/contact frames. Real selection, pause, Shoot, scrub and normal exit
pass on a private display. The installed `run-characters.sh` launcher was also
checked. Phone review: `http://3090.bumblebee-hue.ts.net:8095/` within the tailnet;
raw evidence: `/mnt/pinokio-shared/magiccarpet-character-checked-unity/` and
`generated/characters-complete-20261106/delivery.json` under `research/private/`.
The archer retains modeled finger grips. This milestone covers character assets
and the dedicated inspection player; ordinary campaign binding, terrain foot
placement, deaths, crowd LODs and actual-desktop FPS remain unverified. Do not
treat the character-specific repairs as universal automatic video-to-animation.
Keep the separate Original gameplay work below untouched.

## Modern buildings — source-shaped sandbox delivery (2026-11-06)

Oliver accepted the environment and chose original-inspired, richly detailed
architecture. His subsequent correction rejects the invented domes and castle
façades: retain the original Level 1 BUILD silhouettes, proportions and dark
slate identity. Generate the four source heightfield profiles from owner-local
data, place all 32 buildings at their verified Level 1 locations, and add restrained
surface detail and rectangular lit windows. Preserve shared meshes, three LODs,
day/night views and individual frame review against source presentation.
The building set must be installed and checked in the existing environment.
Original BUILD bodies are terrain data, not house torch sprites. This frozen
visual delivery does not claim live construction/destruction; that needs the
runtime source BUILD selector and terrain-footprint feed. Rejected dome candidates
are preserved under ignored generated/buildings-20261106/rejected-domes-source.
Implemented and self-verified: all 32 placements, four source mesh profiles,
shared material/LODs, filtered masonry and roof detail, small rectangular night
windows and facade light spill. The canonical extractor verifies all 1,220
positive source samples; Unity Modern checks pass 94/94. Installed player
`build-32a44fhy` retains the frozen `a98a8317` native. The actual launcher,
building viewpoint keys, lighting changes and normal exit pass on a private
display. Individual day/coastal/stepped and moving night frames were inspected;
Oliver has not yet reviewed this corrected candidate. The physical desktop,
live construction/destruction and collision are not qualified here.
Evidence and limits: `research/private/remc2-unity-native/MODERN_RENDERER.md`.
Keep the separate Original gameplay work below untouched.

## Modern environment sandbox — verified milestone (2026-11-06)

Oliver requested environment first, then replacement 3D assets. The Modern
owner has delivered a freely explorable NightSpire sandbox using the existing terrain,
vegetation, wind, water and steam. Creature/menu replacement and VR follow this
environment milestone. The separate Original gameplay delivery below continues
under its existing ownership; do not mix its native changes into visual proofs.

- [x] Free flight, useful viewpoints and a dedicated launcher; sandbox controls
      must not cast spells, advance missions or overwrite campaign saves.
- [x] Coherent daylight, dusk and moonlight; adjustable cloud cover and variable
      wind shared by vegetation, water, clouds and steam.
- [x] Interactive ripples and hot-water quenches, persistent steam wakes and
      clear controls for repeatable experiments.
- [x] Reconstructed terrain, continuous submerged banks and grounded vegetation
      inspected from close, high and moving views. The original exaggerated
      mountain layout remains; square sea shelves and floating rocks are fixed.
- [x] Tapered moving torch flames, restrained local glow, shared day/night sky
      and local volumetric firelight. Steam and flame extinction share optical
      detail; the former dark wedge and solid plume front were corrected.
- [x] Shoreline crest breakup, foam, spray and lapping presentation; four
      independent steam fields with bounded fade/handoff and persistent wakes.
      These are real-time approximations, not an overturning-wave fluid solver.
- [x] Installed build `build-p87tf4pa`, 84/84 Modern checks, real launcher controls
      and normal exit verified. All 33 changed/new Unity inputs match the build.
      Individually inspected important day/night/shore/steam frames, including
      adjacent low-bank and complete-frame cloud-crossing samples. The reflection
      rectangle, hard green contact and coarse foam lattice are fixed.
- [x] Repeated RTX 3090 / Vulkan / 1600×900 private-Wayland measurements:
      quiet shore/four clouds 8.45–8.63 ms mean; crossing 12.84–13.25 ms,
      crossing p95 16.86–17.08 ms. Campaign paused, recording costs excluded.
      Actual desktop remains locked; gameplay, Windows, VR and audio are not
      qualified by these runs.

The environment sandbox is ready for replacement 3D asset work. Visual acceptance
is the inspected NightSpire views, not a claim that every possible camera is
artifact-free. The final 28.56-second clip uses 360 complete Unity screenshots
(plus an encoded endpoint repeat), with capture intervals retained. Its capture
cadence is not engine FPS. A torn external-screen-grab frame is retained as a
rejected capture, not passed off as clean evidence. Annotated frames and clip:
`/mnt/pinokio-shared/magiccarpet-build-proofs-20261106/environment-final-frames/`.

Implementation and evidence live in
`research/private/remc2-unity-native/MODERN_RENDERER.md`. Keep incomplete visual
acceptance open; a working control panel alone does not finish the environment.

## Current Original mission delivery — 2026-11-14

### Extended creature observations and the NightSpire exit — 2026-11-18

Canonical `127397fd`, published as `814c738d`, adds guarded actor pages
for Bee, Dragon, Archer, Skeleton, Villager and Trader alongside Goat.
Dragon parts keep distinct opaque identities, pose and lifetime. Health
belongs to the root, and an authored parent must own the current body chain.
Nine exact lifecycle checks pass, including real paid Bee and Villager
damage, authored Dragon retirement, and converted Dragon slot reuse.
Same-record authored reuse has no separate gameplay fixture.

Native `87863f39` is installed and all 627 checked inputs match canonical,
Rig and published source. A direct query of that installed library returns
176 real Level5 actors in 16-entry pages, including 80 moving Dragon parts
with stable parent identities. Repeated reads preserve the state snapshot, and
the guard rejects stale ticks. The normal Original launcher loaded NightSpire with
30 roles, one portal click and zero retries. The launch covers packaging;
the guarded query and exact lifecycle checks cover the observation behavior.
The existing `sim-cli game session` stream now discovers and reads the same
guarded actor pages. The Rig-matched release CLI paged 61 live Payahandra
actors against all 30 owner roles at tick 354. Reads preserved the snapshot.
Close and restore reissued opaque lifetimes and rejected the old generation;
invalid limit, offset and tick were also refused. The installed native adds
direct owner-package witnesses for Day2 Villager and Level22 Trader alongside
the Level5 families above. See `generated/actor-pages-20261118/`.
This completes the named actor observation scope of `mc-b9s.4`. Broader
`mc-b9s` stays open.

English idle movement help is implemented and published as `cdbf29f7`.
The exact GLCore check cold-loads Cave4, resets the idle counter through
normal movement, checks eligibility at 100 stopped draws, and displays both
owner prompts for 50 draws with the source successor priority. Repaints
preserve the counter and replay. Original `build-8ojdq7uu` installs the managed
prompts and the rehashed active menu cache. See
`generated/idle-help-20261118.BrSDUEaV/`. Full `mc-oi6.6` stays open.

Original `build-qfa6a9rf` adds the English top-bar life and spell-mana
prompts at source rows 36/37, with the same verified native `87863f39`.
The exact Cave4 GLCore check exercises the large-map and hidden-HUD masks,
original selection order, both indexed two-stroke leaders, and both 50-draw
lifetimes. The first repaint legitimately advances the separate Shift+F5
notice once; the checked subsequent reads and repaints leave replay bytes
unchanged. The normal Original launcher cold-loaded the real Cave4
checkpoint with four exact roles, no input and no retry. Both prompts are
visible in the installed capture. Only the help table and its manifest
digest changed in the active owner cache. See
`generated/top-bar-vitals-help-20261118.Kb2v1CJL/`.

Canonical `0e279bb2`, published as `0daa1605`, adds the assigned-hand
cast prompts at source rows 38/39. The exact Cave6 GLCore check passed in
30.859 s using an ordinarily reacquired Fireball. It covered source priority,
the empty right hand, ordinary right-hand assignment, both indexed leaders,
both 50-draw lifetimes and replay purity. The active cache changed only the
help table and manifest. Original `build-idpw2fbd` cold-loaded the retained
Cave6 checkpoint through the normal launcher with the matching native, six
owner roles and zero retries. The exact GLCore check supplies the prompt
evidence. Assigned-but-unready live behavior, further help and alternate
languages remain open under `mc-oi6.6`. See
`generated/top-bar-cast-help-20261118.D8i2Cf6r/`.

Canonical `e7abbbcb`, published as `ad48e28f`, adds English spell-search
rows 6 and 8 from the current held-spell and Castle acquisition state.
Five focused GLCore checks passed in 141.280 s. Cave4 displayed row 6
at tick 284; Cave6 displayed row 8 at 323. Acquired but unassigned Castle
suppressed both in Day3 through the real tick-2854 empty poll, after the
recurring objective reminder and minimap prompt. Cast and vitals regressions
also passed. Only the help table and manifest changed in the active cache.
Original `build-fmy6_bc4` cold-loaded Cave6 via the normal launcher with
six roles and matching native `3987657a`, without gameplay input. A prior
explicit override passed the cache alias itself and was refused by the
regular-directory guard; using its resolved path passed. The focused
GLCore runs supply the prompt evidence. Full `mc-oi6.6` remains open.
See `generated/spell-search-help-20261118.2GyLq1eO/`.

Canonical `5b6cf275`, published as `faa2fcbb`, adds English Castle help
rows 12 and 13. The three focused GLCore cases passed in 115.194 s on the
retained Revealed Way checkpoint. They cover the full-map spell-menu cue,
centered build cue and right-hand build pointer, with indexed pixels,
50-draw expiry and unchanged replay bytes. An ordinary 1,000-mana paid
Castle followed by ordinary reassignment suppresses both reminders through
the next lower-priority poll. The three spell-search regressions also
passed. Original `build-0q5zw5gs` cold-loaded Revealed Way with three owner
roles, the rehashed menu cache and verified native `3987657a`, with no
input or retry. Full `mc-oi6.6` remains open. See
`generated/castle-help-20261118.eQq6W9MF/`.

Canonical `f06063fa`, published as `b526b94f`, binds Fireball help rows
7 and 10 to the source popup-close request and current spell/hand state.
Six focused GLCore checks passed in 211.006 s. On the retained Revealed Way
checkpoint, the real row82 close arms the cue. Row10 paints the left-hand
pointer at tick 2504 and the centered cue at 2522 after ordinary Speed
assignment; both retain indexed pixels, 50-draw expiry and replay purity.
The full-map check consumes the masked request at the scheduled poll and
later displays row12 without reviving Fireball. Three Castle regressions
also pass. Original `build-ba6mvgi0` cold-loaded Revealed Way with three
owner roles, matching native `4dbbe5fd`, no input and no retry. Row7 still
needs a live positive witness; full `mc-oi6.6` remains open. See
`generated/fireball-help-20261118.pHL9vpFC/`.

Canonical `0bd5b24c`, published as `867241a2`, connects original building
help row21 to the independent Possession cue. The source-distance winner
in the Revealed Way full-map aperture is current subject50. Its indexed
marker follows the same subject for 50 draws and closes at tick 2833.
The actual assigned right-hand row11 paints at tick 2854 and expires
normally. A full-map poll consumes the masked request; after closing the map
row12 appears at tick 2924 without restoring it. All 11 scoped checks passed
across the first run and the three timing-expectation reruns, with identical
production code. Original `build-4vjz0mq3` cold-loaded Revealed Way with three
roles, native `4dbbe5fd`, no input and no retry. The live unacquired variant
and broader locale/other help acceptance remain open under `mc-oi6.6`.
See `generated/possession-help-20261118.clbzbSVe/`.

Canonical `be41e10b`, published as `fe05606e`, connects neutral mana to
Original minimap help row 18. The Day3 checkpoint shows live sphere 259
with its indexed leader for 50 draws, then paints the assigned right-hand
Possession cue after dismissal. An ordinary paid Possession cast claims
the same 500-mana sphere and drops row 18 while retaining the map dot.
Thirteen distinct GLCore checks passed across two runs on identical
production code, including Cave6 spell search and the Castle reminder
refusals at their actual source-priority polls. Full `mc-oi6.6` stays open.
Original `build-k3rzmq6w` installs native `1d235e04`. All 628 recorded
native inputs match Rig and both Git revisions. The normal launcher
cold-loaded Revealed Way with three roles, the verified menu cache and
the matching mapped plugin, without input or retry. Only the help table
and its manifest changed in the active cache. UBS C# coverage was partial;
none of the reported critical findings fell on added lines. See
`generated/loose-mana-help-20261118.gaDBxSF0/`.

The installed Original `build-k3rzmq6w` Possession search branch is now
verified with the genuine Cave6 checkpoint and its six owner roles. The
recurring neutral mana marker draws from tick 603, closes after 50 draws,
then the first eligible poll consumes its request and shows row 9 at
tick 673. Fireball remains assigned and Possession is unacquired. The
original English text, indexed pixels, draw lifetime and replay purity
passed in the exact 32.76 s GLCore check. Only the existing test changed.
See `generated/possession-search-help-20261118.0viovs0s/`.

Canonical `744e27d2`, published as `85860242`, binds Castle return reminder
row 14 to the original one-shot Fireball/Possession requests and the linked
Castle mana predicate. Three exact GLCore checks passed in 105.05 s on
Revealed Way. The real objective-marker close after normal hand reassignment
shows row 14 at tick 2592. The real sphere 259 close after reassignment shows
it at tick 2522. Both verified the owner text, centered indexed pixels,
50 draws and replay purity. An actual paid stage-1 Castle below the current
holder cost refused row 14 at a poll it would otherwise win. In the strict
NightSpire continuation the affordable t1739 seed has no current owner
link; the link appears at t1740 when the cost is already 10,000. An
affordable linked Castle positive remains open under `mc-oi6.6`.

Original `build-nyblk8x9` reuses the verified native `1d235e04`; since that
native build the only Rust source change is the test-only Level2 regression.
The normal launcher cold-loaded Revealed Way with three owner roles and
the matching mapped native, without input or retry. Only the help table
and its manifest differ in the verified active menu cache. UBS C# coverage
was partial with no critical finding on a changed line. See
`generated/spell-menu-reminder-20261118.YM4AfQhP/` and
`generated/castle-recast-help-20261118.a2t5zV/native-affordability-proof.json`.

The installed `build-nyblk8x9` Fireball search branch is verified in
Ammyridia using a strict cold checkpoint reached through the campaign portal.
The first four Wyvern marker closures lose to earlier or higher-priority
prompts. Opening the real full map and making one short flight input resets
the idle prompt and admits Wyvern 299. Its marker closes at tick 1242, the
next eligible poll consumes the request, and row 7 draws at tick 1263.
Fireball remains unacquired, Possession is assigned, and the original indexed
text, 50 draws and replay purity pass in the exact 129.81 s GLCore check.
Only the existing test changed. See
`generated/fireball-search-ammyridia-20261118.dx3k373s/`.

Canonical `496b8905`, published as `87d0f043`, connects the local Castle's
remaining post-draw hit frames to help row 17. The existing Day HUD and
compact render layouts are unchanged. The generation/tick-guarded read
is observational and returns zero without a current linked Castle. In the
retained Revealed Way route, the hostile hit at tick 2050 drops the real
Castle from 20,000 to 17,400 life. At tick 2059 the panel still flashes while
the remaining hit counter is already zero. The native check verifies that
boundary, stale reads and replay purity. The exact GLCore test shows the
original attack text from tick 2051 for 50 draws, with indexed pixels and
source top-center placement. Source type 1 uses (320,80) rather than the
authored row's stored axes. The focused pass took 29.50 s.

Original `build-gvrp0e5f` installs the Rig release native `a8609040`.
The normal launcher cold-loaded the strict pre-hit checkpoint with three
owner roles and the matching mapped library, without input or retry.
Only the help table and its manifest changed in the active owner cache;
all 678 payloads were verified. UBS C# coverage was partial with no
critical finding on a changed line. Full `mc-oi6.6` stays open. See
`generated/castle-attack-help-20261118.YU2vndGe/` and
`generated/castle-attack-help-native-20261118.zMzMpe/`.

Canonical `87950d17`, published as `8ddc1426`, connects the live owned
Castle and Collector minimap markers to English rows 24 and 23. The strict
Day3 checkpoint admits both through the ordinary full-map command and
selects Collector 258 at tick 2051 and Castle 261 at tick 2121. Each hint
tracks its own indexed leader for 50 draws and closes without arming a
spell cue. Repaints preserve the replay bytes. The exact GLCore check took
31.69 s. The paid linked Castle rejection check also passes in 52.37 s;
its decisive actual poll is the third recurring Collector hint, where
all three refused rows 12–14 outrank the selected row in the source order.

Original `build-0fgqnutb` reuses native `a8609040`. The normal launcher
cold-loaded Revealed Way with three owner roles and the matching mapped
library, without input or retry. The active owner cache differs only in
the verified help table and manifest. UBS C# coverage was partial and
reported no critical finding on changed lines. Full `mc-oi6.6` stays open.
See `generated/owned-castle-map-help-20261118.lhliYyq3/`.

Canonical `b04052f8`, published as `0150f3f7`, exposes the bound objective
condition, authored ID and live stage in one guarded native/CLI read. Both
consumers matched the retained ordinary NightSpire tick 168→169 boundary:
My Beacon completed, Foot of Spire became current, and the count rose from
zero to one. Reads preserved the prior installed state hashes, event journal
and terrain hash, and stale guards were refused. The Day2 read returned the
bound Nyphur target ID. All 627 input hashes match canonical, Rig and published
source. Original `build-d6_t4mf7` installs native `a0551e64`; the ordinary
launcher entered NightSpire with 30 roles, one portal click and no retry.
The launch covers packaging. The focused native/CLI proof covers the
transition. See `generated/objective-observations-20261118/`.
Canonical `6c8d8813`, published as `7a619281`, adds guarded native and CLI
reads of live Castles, Collectors and mana spheres with current generation-safe
relations. The retained NightSpire input reached a paid Castle at tick 1739,
its Collector at 1761, four ordinary 500-mana pickups and a 2,000-mana
deposit at 2745. Native reads follow the spheres through hide and reclaim;
the optimized CLI cold-load reaches the same deposit boundary. Paired runs
preserve game hashes and journal, and stale, invalid and reset cases are
checked. All 628 recorded inputs match canonical, Rig and published source.
Original `build-idpw2fbd` installs the matching native `534882ac`. See
`generated/economy-observations-20261118/`.

Canonical `e54ac147`, published as `fc7617c8`, exposes guarded, bounded,
nonwrapping terrain windows in Native and CLI. At the ordinary NightSpire
Castle stamp, ticks 1743→1744, both read the same 8×8 window and report
ground height 768→800, the collision material change and protected-height
flag at tile (116,230). The full window records 46 height changes and 16
material/protection changes. Read purity and stale/bounds refusals passed.
All 628 inputs match canonical, Rig and published source, and the installed
Original `build-fmy6_bc4` maps verified native `3987657a`. See
`generated/semantic-terrain-20261118/`.

Canonical `5675391d`, published as `4eb95f99`, adds generation-safe
economy payloads to the existing bounded event journal. Native and CLI
report the same real Castle construction start at tick 1740, four
500-mana transfers and the 2,000-mana deposit at 2745. The paid root is
visible at 1739; construction is published on the subsequent frame with
stage 1. All four sphere identities survive reclaim in retained history,
and a cold CLI replay reconstructs the final pickup. The 136.437 s proof
also covers stale guards, expiration, unchanged reads and seven exact
prior state/journal receipts. The current Cave6 sample at tick 40 returns
ground 224 and ceiling 5568 without changing the state. All 628 inputs
match Rig and the published revision. Original native refresh
`build-s8mo2uez` cold-loaded NightSpire via the normal launcher with
native `4dbbe5fd`, no input and no retry. `mc-b9s.5` is closed for its
existing acceptance; broader session and campaign acceptance remains open.
See `generated/semantic-economy-history-20261118/`.

The earned NightSpire exit checkpoint matches 13 historical pose/exit
boundaries and reaches the real authored exit at tick 10695. The native
runtime completes the 187-frame exit and emits ordinary Results. The prior
installed Original `build-lfyj29b2` fatals on the first cinematic step:
its managed validator demands ordinary speed passes while the source exit
intentionally retains the gameplay tick. The correction admits the bound
cinematic clock and requires changing authoritative state. Ordinary stall and
speed-pass guards remain active. The exact earned exit and ordinary-stall
checks passed in the qualified JSON and canonical binary render routes.

Original `build-8ojdq7uu` with native `87863f39` completed the earned exit,
displayed Jahwl Results and opened Payahandra via the normal map in the same
process. Night track 1 at session generation 3 and Day track 2 at generation
4 each started once. The private software PCM matched their owner heads at
0.999979 and 0.999994 correlation. Results and the map used their own track 4.
Loading had 17.29 seconds of exact digital silence and the new Day output
had no detected old-track carryover. Root viewed Results and the Payahandra
selection. All six owned capture processes stopped. See
`generated/night-day-music-replacement-20261118/`. Full campaign and
remaining gameplay sound acceptance stay open under `mc-paj.1`,
`mc-oi6.9` and `mc-oi6.8`.

The retained earned NightSpire exit now has an intermediate cold-save witness
on installed native `327b91e6`. The public runtime finished the 187-frame exit
tail, saved its actual Results state, dismissed Results, and saved the map.
Two fresh CLI processes loaded those saves and opened Payahandra Ruins at tick
zero with byte-identical entry payloads. The frontier and three carried spells
match, with persistent experience 49/9/1 for Fireball/Possession/Castle and
Fireball still selected. A stale duplicate open leaves the live owner intact.
The installed Original independently cold-loaded Results with all 30 roles,
displayed its statistics, and opened the same successor through the map portal.
The save producer here is the public runtime API. The installed Results sheet
consumes a key as dismissal, so this runs no Results-screen quick-save check.
Broader `mc-zdo.6` acceptance stays open. Evidence is under
`generated/earned-results-persistence-20261118/`.

### Goat lifetime pages and Cave loop timing — 2026-11-18

Original `build-4zmyev0o` now installs the audio rewind and capture-tap
fixes, the collector area-damage fix, and English large-map help. Canonical
`64748bb7`, `f2d714d0`, and `6c67bf95` are published through `075c7f04`.
All 626 checked native inputs agree across local, Rig, and published source.
The current native is
`3f3744cf7b4531c0dbcc68cb47357017338d56b332e7982d643650146c01859f`;
`--check-installed` passes. The default menu cache now resolves to the
validated 678-output row 4 cache. The canonical Original launcher entered
NightSpire with all 30 roles, one portal click, zero retries and the matching
mapped native. The launch covers the installed build and default resources.
The installed Cave track continuity check passed below. Collector retirement
in the installed minimap remains open. See
`generated/collector-hostile-area-20261118/`.

Canonical `63b8b0f2`, published as `ff7e2bea`, adds bounded campaign actor
pages for `creature.goat`. The public read binds session, generation and tick;
opaque lifetime IDs use the authoritative arena incarnation. The campaign
profile advertises its Goat scope separately from the existing exact Unity
capability arrays. Live Day2 paging and guard/read-purity checks passed in
3.19s. The NightSpire paid Fireball test follows a Goat with a declared,
published 200-life property through damage, dying, hide and reclaim in 5.51s.
The original 600/1200-life cases remain in the same test. The extended check
passed in 6.84s: ordinary allocation reused slot 50 at tick 9 for class 10
model 14, changed the incarnation from 0 to 1, and kept the retired Goat out
of the public page. This completes `mc-b9s.3`. The broader `mc-b9s` remains
open. The older full snapshot's scoreboard contains `source_slot`;
the untouched optimized registry test fails its existing guard too. The
actor-page change preserves the original scoreboard consumers and guard.
Original `build-41je7c5x` is installed with native
`fb751cf5501717c68956502c1b8e740e95a0265f3a6509f991640edac9ebf546`.
All 626 native inputs match canonical, Rig and published source. The optimized
public read paged 36 distinct live Day2 Goats and repeated identically.
The canonical launcher loaded NightSpire with all 30 resource roles, one portal
click and zero retries. The mapped native matched. This smoke covers launch and
load; the focused runtime checks cover Goat observation and lifetime.
See `generated/goat-actor-pages-20261118.Lk77Lx/`.

Cave track 3 repeated in both installed captures. Reanalysis of both audible
startup anchors on `build-41je7c5x` finds the full 240.001451s transport
period in Pulse and 240.001474s in direct PipeWire. Both saw two matching owner
heads 35,280 source frames apart at startup and lost phase in the final 0.8s.
The earlier 239.201451s measurement used the duplicate startup head.
One managed start and continuous packet timing rule out a second game start
or a gap in the capture. The qualified Unity A/B reproduced the same 35,280
frames at the mixer before the device. Setting AudioSource.timeSamples lets
Unity seek and prefill, then a redundant managed seek replayed the buffered
head and displaced the tail. Removing that seek preserves the full
176,464-frame diagnostic clip and its tail on both fresh start and restart.
The game capture tap now keeps enable-state reads off the mixer thread.
The short test uses controlled PCM with the production stream/source and real
capture tap. The exact installed `build-4zmyev0o` Cave4 checkpoint then
opened normally with four package/cache roles and one source track-3 start.
Both private captures measured all 10,584,064 owner frames (240.001451s).
The opening, final tail at 239.95s and repeating opening matched owner
PCM with at least 0.986 stereo correlation in both readers. There was no
matching duplicate at the 0.8s prefill offset, and the same gate rejected
the older installed recording. All owned capture processes stopped.
This verifies the first natural Cave loop. The later installed Revealed Way
Day capture on the same build passed at 16,695,296 owner frames in Pulse
(378.578141s), within one resampled frame in direct PipeWire. Head, final
tail, and next head matched the owner in both readers, with no 0.8s
duplicate. The same neutral interval produced the ordinary native
idle/battle/idle transition and matching PCM gains. The installed Night
checkpoint selected track 1 once and measured 19,472,384 owner frames
(441.550658s) in both capture paths. Its final tail and next opening matched
the owner without the duplicate prefill. All three environments now have
installed natural-loop evidence. The later same-client mission replacement is
recorded above. The gameplay-sound dependency remains open under `mc-oi6.9`. See
`generated/adaptive-cave-loop-timed-20261118/installed-cave4-build-4zmyev0o/`
`generated/adaptive-stream-loop-discriminator-20261118/` and
`generated/adaptive-day-night-loop-20261118/`.

The authored Karakir Cave switch at row 29, tile (201,204), can start Wind
row 175 through ordinary overlap and group 4. Its owner sound 49 is Cave
bank 2 TORNADO.WAV. No retained strict Original save reaches that encounter.
The retained map save cold-loads with all 30 roles but rejects Karakir as
locked. Its available Spider Cave opens normally, with the carried Whirlwind
insufficiently funded at 1000 mana and no Castle.
The direct role-26 package exposes the native producer,
while installed sound 49 playback and natural release still need that route
or an ordinary acquired and funded Cave cast. Existing Cave door and drip
captures are separate evidence. Keep `mc-oi6.8` open.

Ordinary English large-map help now binds source row 4 to record 289,
the current indexed minimap subject state and the normal closed-map
poll. Unavailable subject data cannot select it. Actual Cave4 cold replay
and normal map actions verified selection, rendered text, dismissal
cadence and replay purity. The existing Night row 2 check passed on the
same files; the unchanged Day3 blink-hidden subject check passed in the
first run. The default Night start itself has a source subject and cannot
serve as the positive row 4 case. Previous verified cache receipts remain
admitted. Evidence: `generated/large-map-help-20261118.zeY5ft6H/`.
Other source help rows, tutorial and alternate-language acceptance remain
open under `mc-oi6.6`.

The English low-health prompt binds source row 5 to owner record 290 and
uses the published live quarter-life threshold at ordinary help polling.
Two exact GLCore cases passed in 84.87s. The real Day3 retreat checkpoint
follows combat damage at low life through indexed text and the 50-draw
lifetime. A continuation crosses 2500 to 2505 with normal regeneration
and stays ineligible at the next open poll. Repaints leave timer and replay
unchanged. The older four table receipts remain admitted, and only the help
output differs in the independently rehashed 678-output cache. Canonical
`b05676f9` is published as `c8f0e2dd` and installed in Original
`build-lfyj29b2` with verified native `3f3744cf`. The default launcher
loaded NightSpire with 30 roles, one portal click and zero retries. That
smoke qualifies the installed launch and load; the injured encounter evidence
comes from the focused GLCore tests. See
`generated/low-health-help-20261118.bc502sO6/`.

### Objective popup closure, Night music and collector retirement — 2026-11-18

Canonical `c3e5cd77` is installed in Original `build-nstm28wl` with native
`1ba74ee6`. A real NightSpire objective-text expiry emits help row 79.
Its popup draws the owner text, closes after its 50-draw lifetime and promotes
minimap rows 2 and 81 for subsequent polling. Reads and repaints leave the
timer alone. Both this check and the Day3 objective-marker regression passed.
See `generated/minimap-help-20261118.ovAjIr/objective-row79-gl.xml`.

The installed NightSpire checkpoint starts track 1 at tick 10363 and reaches
combat and idle at the same ticks as native. Owner C2GAME1 matched the captured
stereo PCM at 0.99956 correlation. Combat was 3.256 dB above idle, and pause
silenced the capture. Day, Night and Cave now have installed adaptive PCM
evidence. The later exact Cave full loop is recorded above. See
`generated/adaptive-night-music-20261118.ZsshtN/installed-night1/`.

The focused paid Castle/collector check follows the actual typed minimap input
through the collector's own lethal hit, hidden state, reclaim and same-slot
stock refill with a new incarnation. It passed against the authored package
in 4.20s. The damage is a declared test control. Rendering the actual death and
reuse in the installed player remains unverified. The retained Payahandra
checkpoint admits all 30 roles and has live rival Nyphur at tick 354, with
zero Castle or Collector markers. Its recorded input ends at pause. The
installed witness still needs a current checkpoint with an exposed hostile
Collector and ordinary inputs that retire it in view. See
`generated/collector-map-retirement-20261118/collector-focused.log`.

The shared scheduled area-effect reader now includes the current typed
class 3/model 3 collector. It uses the canonical particle/map collision
shape and source damage mask before the stale raw-row fallback. The exact
Level5 regression passed in 5.40s: the paid Castle produced its real
collector, a staged friendly Fireball impact left the mailbox unchanged,
and 41 staged hostile terminal impacts at source tier-zero power caused
ordinary mailbox consumption, hide, and reclaim with the Castle still
allocated. The friendly comparison advances a matched no-impact frame
because the collector naturally regenerates one point there. Existing
refill/slot-incarnation and Cave collector controls pass; the affected
ordinary all-tier Meteor path passes in 8.32s. These are controlled impact
births, with no paid rival launch or installed minimap pixel claim. Keep
`mc-bhq.10` and `mc-oi6.5` open. See
`generated/collector-hostile-area-20261118/`.

### Objective-marker help, speech volume and Cave music — 2026-11-18

Canonical `29b5ecea` and `813d8d00` shipped in qualified Original
`build-i9hrj52v`. When the real Pav speech is active, changing one volume in
the paused owner now updates that channel immediately and keeps the other
channel's fade. The focused test reproduced a 5.25 mixer gain before the fix,
then passed the real preference, pause/resume and speech/cue guards. See
`generated/speech-slider-20261118/{baseline-paused,candidate-paused}.xml`.

The objective-marker help consumer uses the current minimap aperture before
blink filtering, source-ordered nearest subject selection and the existing
indexed leader. The new digest-verified owner menu cache preserves admission
of the previous two variants. The Day3 checkpoint reached the real row82
text and anchor. Repeated reads and repaints preserved the lifetime and
replay. Evidence: `generated/minimap-help-20261118.ovAjIr/objective-help-anchor-gl.xml`.
Row79's close-time priority promotion and broader recursive help stay open.

On the same installed player, strict cold load of the Cave4 ordinary retreat
checkpoint and neutral input reached track3 combat then idle. Captured PCM
matched owner C2GAME3 at 0.99543 stereo correlation and measured a 3.26 dB
change. Pause silenced the private capture. Evidence:
`generated/adaptive-cave-music-20261118.JvQJNs/installed-cave4/`. Night adaptive
PCM and full-track looping remain open. No full campaign or hardware proof.

Canonical `6940934c`, published as `d012ff1d`, joins Whirlwind sound49 to
the current scheduled field actor. The ordinary source package check now
prepares audio after field motion using the typed actor and actual updated
listener, emits the terminal End49 before hiding the field, and follows
natural reclaim. The first run caught the blank raw arena position; the
corrected check passed in 14.48s. The earlier native replay test verified
the independent optional Wind audio boundary alongside the existing Goat
boundary. The finite-voice consumer check verifies reuse, stop, completion
and reset. The installed refresh `build-fbtna7ng` on the same Unity player
uses native `1ba74ee68b2cefc75e64237e9d03593f0da4d5a12a8501c17dd569dfd7b47c8a`.
All 626 recorded inputs match the local, Rig and published source. The
matching client opened NightSpire with all 30 roles, one portal click and
zero retries. See `generated/wind-audio-20261118/`. The Wind chain exercised a bounded
source scenario; actual Cave49 PCM at an admitted Wind encounter remains open.
The retained save and input census found no earned Karakir route. The
shortest admissible witness is a Karakir save before the row29 overlap
at tile (201,204). The retained Phyrydia Whirlwind state has neither the
2,000 player mana nor the 60,000 Castle mana required for the first tier.

### Original audio, minimap and campaign continuity — 2026-11-18

Canonical `936b3bd8`, `255f6dfc`, `000a391e`, and `3adf0176` are published on
`codex/original-ending` through `fa5e8a56`. Current Original build `build-wnhko4t3`
uses the same native `0c58356c8df10fffb23dac5f0627edc5531a479666505b7d32a4c59a774e62cc`
as the initial audio/help build `build-b2f6evq_`.
All 626 recorded Rig inputs match the published source. The normal `--menu`
launch reached the three startup clips, Title, New Game, and NightSpire with
one portal click and zero retries. All 30 resource roles and the mapped plugin
were verified. Evidence is in `generated/gameplay-audio-20261118/`.

Night Goat sound 46 now goes through the scheduled positional mixer. The
unchanged released Night v9 replay cold-loads with all original receipts,
then adds new Goat audio. Its continued save preserves the verified operation
boundary and cold-resaves byte-for-byte. The previous reader rejects the new
optional marker. The installed private stereo capture matched the owner
GOAT.WAV at the actual logged playback rates and gains. The Day3 minimap now
uses the scheduler's current creature chain. The same input that previously
failed published all ten remaining Skeletons in source order after the dying
member was unlinked. The Castle marker retired after combat loss and reuse.
Its collector stayed alive, so direct collector-marker removal remains open.
Repeating that bounded Day3 route against installed native `1d235e04`
verified that a real hostile impact at tick 2110 damaged Collector 258
(incarnation 21) for 500 life on the next tick, with its selector 66
marker retained. The Castle marker retired at tick 2252 and the Collector
was still alive at tick 2257. Its ordinary death and reuse remain open.
See `generated/collector-map-retirement-20261118/ordinary-day3-1d235e04/`.

Optimized music callbacks run during accepted gameplay steps. The selected
owner XMIDI track changes gain from 0.55 at idle to 0.80 in combat. The first
installed Day3 flight reached combat but its mouse steering did not reach idle.
An exact ordinary-input checkpoint allowed a neutral-only repeat of the real
combat-to-idle transition. Installed trace ticks 2168 and 2247 match native
callbacks 128 and 256. Phase-matched owner PCM measured 0.7990 combat and
0.5496 idle (+3.25 dB), with silent paused output. Evidence is in
`generated/adaptive-music-playback-20261118.OPkPs3/`. The installed minimap help uses
owner row 287 and the existing indexed HUD. Its real selection, anchor pixels,
repeated-read purity, and 50-draw lifetime passed with GLCore. NightSpire's
objective-advance and completion lines both matched the actual owner PCM in
the preceding installed player from a retained fixed public input trace.
Those captures do not certify hardware output or the rest of the campaign.

The Keys dialog rejects new mappings of fixed gameplay controls. The first
physical input check then found an IMGUI text-only event that incorrectly
rejected the next capture after a valid physical key. The dialog now consumes
those text events without changing its draft or clearing a real rejection.
Focused checks verified direct Apply, rejected-key atomicity and cold reload.
The installed Title check rejects L, cancels without changing profile bytes,
and persists Q directly without the earlier Escape workaround. A fresh
Original process loaded the same profile and genuine NightSpire save. Physical
Q opened and closed the visible world map, while Return did not toggle it.
The saved history contains exactly two new map-toggle operations. Evidence:
`generated/settings-key-capture-20261118/installed-keys-no-escape/`.

Pav abandon/cancel and cold parent return reopened Jaleen once with retained
Possession, on a checkpoint with declared prior state. The final Uluth Results
save and map restored byte-for-byte on the current native with all 30 roles
and the real owner catalog. Dismissal preserved all 26 progression rows and
did not replay CUT6. This starts after CUT6 and credits. Earned NightSpire
Results and consecutive opening transitions, earned secret round trips, new
full ending playback, and broader help/locale acceptance remain open. No bead
acceptance or dependency is changed by these bounded results.

Current capability (2026-11-16): Castle/balloon, flat shading, brightness, Speed,
the source in-game viewport, Player Names, F4/F5/F6 speech/sound/music, F2
flight assistance, F1 Help Mode, Shift+F1/F2/F3/F4 graphics and original Title
Exit/cursor/idle-help/F1 are implemented, verified, committed, pushed and installed.
Finite mc-5r5.3 remains CLOSED; do not restart its checks.

Finite mc-paj.5 is now CLOSED on its original bounded Night/Day loading,
recovery and resource-lifecycle acceptance. Canonical1fb4e497 includes
ac574add/020a9d41; Original302965a9 includes cb53b978. Original build-5a4ygxbx
was installed with native8c862188. The current native is now f3bf7893 after the
semantic spell batch below; the same player is retained. All627 inputs match
local/Rig/published source at each recorded native receipt. Spider/backend
qualification additions were cfg(test)-only and reused their release native.
Unrelated Modern/browser work is preserved; main was not pushed.

Implemented fixes: retain the real campaign frontier on Title/NewGameMap;
separate held IMGUI Escape from fresh source confirmation; publish every map
scroll coordinate to drawing/hit tests; read current native MenuState through a
pure guarded ABI query in ordinary/cold/reset/paused observations; materialize
only the actual admitted action29 write into the existing optional banishment
carrier; end gameplay menus/pending actions with their borrowed world before
Title draws. Generic AI/projection setters still require qualified backing.
MCPO/save schemas, source order and canonical refusal/admission gates stay intact.

Verified: actual L1 flight/map confirm, read purity/guards and post-confirm cold
replay/return1.075s; legacy/qualified carrier bytes0.152s; immutable released
lighting-OFF save and signed incoming Day2 map load/exact byte resave; current
managed menu consumer30.832s and actual confirm-to-Title GUI/borrower release
41.289s; canonical116-source typecheck. Required native loading0.22s and managed
visible-frame1.312s remain covered. UBS is partial/legacy; no added-line teardown
finding. Failed installed run7 is preserved as the discovered teardown defect,
not successful return/reopen evidence.

Installed run8 passes missing-art ReturnMenu/Retry, visible Day2 source loading,
exactly one bound/frame/open per request, confirmed Title return and same-process
second Day2 open. Root viewed both worlds/map/Title/art; immutable incoming map
unchanged. Actual unsteered native death clears menu13 to0 at tick805 before any
confirm/cancel input, and settled pixels show no stale modal. A separate fresh
canonical Night1 portal/loading/world run passes with zero retries. Owned smoke
players/displays are stopped. Evidence: generated/loading-20261116/installed-day2-8,
installed-night-final, abandon-death-1, source-menu-observation and menu-teardown.
Declared incoming Day2 progress is not earned campaign proof. Human combat/FPS,
hardware audio, full missions, general old saves and assembled Cave/secret routes
are outside these bounded claims; full campaign parents remain OPEN.

Finite mc-oi6.7 is CLOSED after fresh bounded original-acceptance review at
canonical16f3a83d and root independent source/evidence inspection. Its current
installed aggregate already owns the web writer;
the old presentation_bridge refusal alone is not a current-client defect.
Actual controlled Level4 Spider contact/damage/natural expiry/reclaim passes
0.54s. Its emitted authoritative observations feed the matching MWEBC Cave
aggregate GPU; source pixel, repeated-read purity, expiry/reset pass2.828s.
Declared pose/cadence/protection and separate producer/renderer owners remain
explicit; no unstaged live encounter or same-owner replay claim. PearlBay's
actual admitted Day exit through native GTD2/blur/phase12 and reset history
passes10.38s on the real Day2 owner table after the focused Rig compile.
Canonical16f3a83d is pushed as Originala22863b5; only cfg(test) code changed,
so that checkpoint reused native8c862188/player5a4ygxbx. No fabricated exit phase, new ABI,
automated player or second receipt bridge. UBS's whole backend totals are legacy
scope; none of the130 added test lines has a critical finding. No concrete
missing producer/current-client consumer remains in that finite inspected scope.
Prior acquired supported-status timing evidence is reused; native alive/full-map
gates and world-history -> WEB -> minimap/HUD order were inspected. Native
controlled-component evidence does not establish live C ABI/Unity Day exit or
next-mission teardown by itself; the exact GPU LUT and actual installed Day2
teardown/reopen remain separately identified evidence. These limits and the
assembled campaign acceptance are preserved rather than claimed from a fixture.
Finite mc-b9s.2 is CLOSED after fresh bounded independent consumer assessment and
root inspection. Canonical1ce609de is pushed as Original08852538; nativef3bf7893
is installed by native-only refresh build-l818_b3q into player5a4ygxbx. All627
local/Rig/published inputs match and --check-installed passes; no Unity rebuild.
One shared26-family mapping now uses the identical current-holder/incarnation
predicate for observation, selection, assignment, marking and cycling. The old
cached-slot semantic selection bypass reproduced before repair. Remembered marks,
ordinary movement, source scan order and low-mana selection/cycling are preserved;
no cost or activation prerequisite changed. Existing per-tier source affordability
fields remain the actual menu's consumer, without a new generic readiness overlay.

Actual controlled BeyondSight pickup -> ID/tier/cost/hand/read purity -> public
select/cast/expiry plus stale generation guards passes0.198s; required semantic
API0.20s; mark/cycle and2 cold replays/full snapshots0.50s; learned release/reclaim
and next entry4.09s; progression/two-entry save1.88s; matching managed inventory/
selection/cast/reset0.234s. The replay fixture initially omitted actual HUD-policy
history; it now copies the real sidecar and all5 strict receipts remain. The old
released lighting-OFF save loads/exactly resaves. Root viewed normal canonical
Night map/loading/world with mapped f3bf native, one portal, zero retries, then
stopped owned processes. Source scans remain legacy-scope; no added production
finding. Evidence: generated/semantic-spells-20261116. Native pickup pose/Castle/
mana and stale reuse are declared controls; the managed UI uses a separate owner.
This is not one continuous unstaged physical pickup/UI route or earned campaign.
The historical Nov10 Castle save was rejected for incompatible runtime_flow,
remains untouched and is not positive evidence. Full parents remain OPEN.
Finite mc-oi6.2 is CLOSED. Canonical 10aa9b48 and Original 7c49d918 contain
the cached native frame owner check and Unity read-time output integrity check.
Original build-wuukj1s5 is installed with native d8a7a7b6, reusing the verified
Rig artifact because no additional Rust changes were needed. `--check-installed`
passed. Normal map/loading/NightSpire terrain and HUD were viewed using the
matching mapped native, one portal and zero retries. All 30 resource roles
were present and the owned smoke processes stopped.

Exact cache validation and source byte comparisons cover Night1, Day2, Cave12
and fire-Night25, including the original palette, tables, block atlas and
dormant16 bank. Three focused importer tests pass. Matching Unity passes the
four named wrong-bank, changed-output, Day/Night/Cave and fire-Night material
checks. Actual Level25 takes a source turn and renders the real boss with
declared group/position controls; the Level25 material frame and Cave12
wrong-owner rejection/rebind pass separately. Evidence under
`generated/materials-20261116/` and `generated/ci-baseline-20261116/materials-batch-serial.xml`.
The standalone smoke covers NightSpire entry; controlled editor/native checks
cover the other material profiles. Hardware audio, Cave occlusion and full
missions retain their separate acceptance.

Shared Original animation source is committed as c8175c10 and pushed as
a7323502. Its installation used native 22809f18 on build-wuukj1s5. The canonical
launcher displayed the map, loaded NightSpire through one portal with zero
retries, and rendered the terrain and HUD with the matching mapped plugin.
The smoke did not perform creature combat or play a mission.

The current campaign renderer advances one retained indexed image per
selector after an admitted draw. A real Night1 two-Goat selector152 and
Day2 directional Villager check reached actual composition pixels and
verified paused frames, repeat reads and generation reset. Cold replay
restored the same content state and re-serialized byte for byte. The real
Level25 boss death reached the source animation finish. The creature28
reset hook has clock-level coverage and the real Level14 transformation
and hit test passes. Matching Unity passed the
semantic texture identity test and the native scene/no-duplicate world
test for missions1–4. The broader selected legacy presenter tests failed,
including 20 failures caused by owner audio descriptor size mismatches
before a frame was rendered. Keep mc-oi6.1 open for that verification gap.
Evidence: generated/animation-20261117.

Outdoor composition verification is pushed as 3a533792 and aaa48724.
Real Night1 and Day2 views retain visible actor pixels and overpaint others
with later terrain. The real Day3 water view verifies reflection, shadows and
mode5/7/26. A real Night1 Fireball contributes indexed pixels in flight and
disappears through impact and reclaim. The source camera and ordering ABI
checks pass. The installed Original renderer contains the verified path.

Cave source 67eca929 and verification f7e0abcf are pushed as 29cf777a and
0a6ef850. Actual Cave12 flat mode changes floor shading while texture1 ceiling
commands and pixels retain mode5. The real Cave4 corridor scheduler and
reset/flight checks pass. The authored Spider600 contributes terrain-covered
pixels at pinned normal and rolled views, and its visibility changes with
the actual open and closed corridor206 plane data at a controlled cut.
Original build-wuukj1s5 now maps native ddca481a from f7e0abcf. All 628
checked source inputs match Rig. The standalone smoke displayed the map,
loading frame, NightSpire terrain and HUD, using one portal and zero retries.
The Spider and Cave shading checks use controlled native frames. Keep
mc-oi6.3 and mc-oi6.4 open with their original acceptance and dependency chain.
Tutorial/help mc-oi6.6 and speech mc-oi6.10 retain their open prerequisites;
do not bypass them. Full mc-5r5.2/mc-oi6.5/.6 and assembled campaign acceptance
remain OPEN. Hardware resolution switching remains the explicitly separate
renderer option, not the source in-game viewport. Root keeps sole Cargo/RCH/
push/install dispatch; owners remain through consumer failures.

Animation follow-up: canonical bb716821 is pushed as Original 2e495072.
Installed player build-wuukj1s5 now uses native 842fd4c1 (native refresh
receipt build-58kn9shu). The real Level7 group5 segmented creature at a
naturally scheduled oscillation frame6 selected TMAP64 instead of source
base58. Direct source modes0/1/21 now ignore that entity frame; framed11/36
and the downstream mode21 pivot/masks are unchanged. The real authored
spawn/carried Possession regression fails before the fix and passes after it.
Affected shared Goat/Villager pixels, ordinary Fireball/reclaim and Cave4
Spider/corridor checks pass. The final test-only whitespace cleanup does not
change the tested production code. A bounded independent changed-path review
found no introduced defect. A possible signed-frame difference in11/36 has
no demonstrated visible ordinary producer and is not counted as a new blocker.

All628 checked source inputs match Rig and the returned native hash. Installed
Unity passes actual mission1–4 native/GPU pixel equality/no duplicate sprites,
runtime read/pause/resume/reset lifetime and semantic asset texture identity
(three selected cases,16.56s). The lifetime fixture now uses the real installed
resource roles and normal selected-locale/FONT1 binding; strict missing-font
refusal is retained. The semantic qualification helper uses the current fixture
owner and typed bind_asset operation; direct CLI/native/TypeScript/Unity checks
pass. Its synthetic asset identity proof is separate from actual MC2 actors.

The attempted legacy Level2 overlay test-cache update was reverted: matching
resource inputs still reach a retired material-schema/fixed-slot presenter,
rather than the current native-composed world. Those failures are preserved;
no legacy assertion or validator was weakened. UBS remains partial/whole-file,
with no critical added-line finding; baseline Rust formatting debt is intact.
Root viewed normal installed map/loading/NightSpire scene/HUD with mapped
842fd4c1, one portal and zero retries, then stopped owned processes. Evidence:
generated/presenter-20261117. This is not a native Level7 encounter, human
flight/combat, hardware audio, FPS or full campaign proof. Full .1/.3/.4
acceptance remains OPEN, including the assembled Cave flight/corridor path.

Renderer qualification follow-up — 2026-11-17: the existing Level14 two-form
creature check now follows the natural action-driven group311 clear, actual
draw admission, next source-turn SS2 transition and a later natural reset.
Its original transformation/splash/contact assertions remain. The final exact
check passes157.64s, including direct retained-clock equality across repeated
reads (the gameplay hash alone does not cover those clocks). A same-state
group-image ablation changes native pixels; that proves the group's visible
contribution, not exclusive attribution of every changed pixel to that actor.
The expected transition uses the production decoder and is sequencing evidence,
not an independent decoder oracle.

The existing strict incoming replay check now optionally covers actual Cave4,
using its complete source owner before construction and the real prefix1–4
resolver. It passes33.68s with exact restored snapshot/flow and empty completed
history. This is a declared incoming Loading control, not earned prior missions.
The resulting unchanged save also passes the public native loader. Root launched
that save through the normal installed Original launcher, viewed Cave terrain,
bounded forward/turn input and the source pause menu, and verified mapped native
842fd4c1. The owned player/display are stopped. The first smoke checker waited
for an obsolete marker and timed out; the corrected checker uses the existing
SpiderCave startup event. No client/source bypass or runtime repair was needed.
F5 while paused produced no save; no new save-key claim is made.

The six old fixed-slot managed presenter checks now use the actual native-composed
world, retaining their original semantic obligations. All six pass28.737s in the
qualified Original cohort: exact Goat/Villager and five class5 mappings,
same-pass hidden/current generation followed by reclaim, malformed/stale guards,
runtime resource binding and public Fireball/Bee interaction. Actual source34
group23 produces Fireball739; public pickup acquires it, held left input services
its real activation delay, and declared player position/pitch tracks the moving
Bee. The check requires payment, a live player-owned projectile, attributed
damage and action17→18, with the same Unity surface/texture and no duplicate
overlay. The compact public probe records projectile tick18 and damage250 at
tick20. This is not organic secret arrival or uniquely attributed actor pixels.
Earlier failures released the cast before activation and aimed above the moving
Bee; source readiness or input acceptance alone was never a hit. A final C#
conditional needed an explicit short cast in the older Original cohort.

No production source changed in this follow-up; current installed native/player
are reused and the qualified Original --check-installed passes. The canonical
Modern checkout's separate installation is not the Original qualification target.
UBS reports no C# warning/critical. Its two Rust whole-file scan yields three
unchanged false-positive criticals: semantic secret-outcome equality and a
diagnostic file-name nonce, not credentials. No added critical was identified;
legacy formatting/whole-file warnings remain. Evidence:
generated/render-completion-20261117, final presenter-current-6.xml.
Bounded independent review found no new demonstrated production defect. Full
.1/.3/.4 acceptance stays OPEN: broad admitted-family coverage, separately
attributed final reflection pixels/Night water, and the joined installed Cave
flight/corridor/creature route must not be inferred from representative checks.

Sequential renderer batch1 — animated-sprite acceptance is now CLOSED as
mc-oi6.1. Fresh bounded selector/lifetime review at41d28f1b found no demonstrated
reachable mismatch. All30 currently installed owner roles match launcher roles;
their15,120 decoded payloads match manifest size/hash, with source-bound504-entry
group tables. A current-source Rust decoder build handles all359 distinct
animated payloads/2,541 declared transitions, including zero-child and multi-child
F1FA records, and rejects359 final-truncation controls without changing retained
pixels. The all30 public reset-world DTO queries pass with6,045 rows. This covers
actual installed stream shapes and reset admissions, not every conditional spawn,
original pixel parity or complete missions. Existing real Goat/Villager shared
draw/pause/replay, two-form Creature28 natural reset, boss finish and current Unity
consumer/lifetime checks supply the connected representative runtime evidence.

An old returned decoder rlib initially rejected a valid terminal zero-child frame;
the driver was then pinned directly to current lib.rs and passed unchanged source.
No production repair or new installation is inferred from that stale-artifact
negative. Source mode16/particle237 has an asset preload but no demonstrated
ordinary draw producer; it remains explicitly classified, without invented
spawning. Actual source draw feedback is an explicit committed operation;
read-only DTOs/repeated cached frames do not service clocks, while the next
unpaused turn consumes each selector latch once. No arbitrary draw/no-draw history
equivalence is claimed. Evidence: generated/animation-acceptance-20261117 plus
the previously installed842fd4c1 lifecycle checks. Batch2 is actual outdoor
reflection pixels and batch3 is joined Cave flight/corridor/creature; .3/.4 and
full campaign acceptance remain OPEN. No Rust/Unity production code changed.

Sequential renderer batch2 — mc-oi6.3 is now CLOSED. A test-only per-instance
omission at the named actor's reflected raster call leaves primary/shadow calls
and writes, source pixel-mode continuation, terrain input/commands and actual
authority/clocks unchanged. At pinned actual Day3 actor220 and Night1 actor51
water views, its omission removes26 and416 final palette-visible RGB pixels
respectively. The final exact check passes3.07s; actual terrain/actor overpaint
passes3.59s and the existing flat5/7, reflective26 and shadow option checks
pass1.72s at the same compiled source. Root viewed both real native composed
scenes and independently checked their RGB differences. These are controlled
native views, not newly sampled physical-display water encounters. Prior exact
native→Unity/no-duplicate checks and actual installed Day/Night load evidence
remain the real client transfer evidence.

Bounded independent gate review found no production defect; the two-file change
is cfg(test) only. Installed Original native842fd4c1/player build-wuukj1s5 is
reused and --check-installed passes. UBS's lone whole-file critical is an unchanged
test-directory filename nonce, not a security token; no added critical found.
Evidence: generated/outdoor-reflection-20261117. Existing projectile impact/hide/
reclaim and source camera/order checks are reused, not repeated. Batch3 now owns
the remaining joined installed Cave flight/corridor/creature path; mc-oi6.4 and
the assembled campaign remain OPEN.

Sequential renderer batch3 — the new same-session Cave4 native path passes9.29s.
It uses the exact installed package/owner: source switch4/group11 constructs
switch577/group50 and authored Spider600; source switch125/group40/command584
opens controller206. Both live height planes and consecutive native frames
change with the same controller/Spider incarnations. After two declared player
approach controls,17 ordinary hover-turn commands and forward/climb/free-look
commands show1,213 actual palette-visible Spider pixels on a step that moves the
player, then enter the real corridor strip and cross its far east edge at
(52798,26252,3081). Source Cave floor/ceiling checks hold throughout. Root viewed
the native flight image and actor-omitted diagnostic comparison. The omission
changes only a same-state clone; no live terrain, actor or camera is overwritten
during flight. Earlier failed routes are preserved: admitted Spider rows alone
were not visible, and northward flight missed the east-west corridor. Assertions
were retained and the source flight controls corrected. This uses the existing
speed-index0 native harness, not a claim of identical fixed-step commands in
the normal GameRuntime speed schedule.

This is useful connected native evidence, not a continuous installed-client
mission route. The two initial player relocations are not GameRuntime commands
and cannot be promoted into a strict runtime replay. Root subsequently flew
the declared incoming Cave4 save through fixed, manually selected public input
legs, rendering each source step and cold-replaying every emitted save without
receipt changes. The actual tick1247 switch121/group34 publishes command574
and Spider587; tick1241 remains an ordinary pre-trigger checkpoint. The normal
installed Original launcher loaded it with the real first-four-role resolver.
Ordinary UI flight produced corridor sample47, the real Spider32 attack/web and
life loss; normal P/Save wrote tick5182 at(37846,21219,4769), and later tick5243
at(39298,20981,4701), life7660. The latter save cold-replays to exactly the same
snapshot; its current Spider is slot3, not a fixed/reused address assumption.
The installed mapped native remains842fd4c1. This history has no completed prior
missions: it is an explicitly incoming Cave checkpoint, not earned Levels1–3.
The installed captures show actual cave flight and attack, but do not yet isolate
the Spider's final pixels against a changing corridor; native connected evidence
does. No teleport API, receipt rewrite or mission-completion automation was added.
Evidence: generated/cave-runtime-flight-20261117, including the normal UI Save
log, captures, private saves and strict cold-proof receipts. Existing installed
Cave movement and exact Cave native→Unity transfer remain separate proofs;
bounded consumer review found no stale-geometry/missing-consumer defect.

A second bounded installed run now saves tick1611 at(37150,21439,4440), yaw621,
life7660, and shows actual blue Spider leg pixels at the corridor edge. Ordinary
R resets to the Cave entrance; source pause Load/confirm restores that saved
pose/health. A subsequent normal Save preserves tick, terrain hash, effect head
and journal. It is not byte-identical: recorded paused_frame operations advance
source presentation state and its state hash while gameplay is paused. The
final emitted save passes strict fresh-owner replay and exact snapshot equality.
No save validator was changed. Music was disabled through normal F6, and a12s
single-application PipeWire monitor capture contains non-silent output. Against
the hash-checked actual Cave bank, first8192-sample normalized correlations are
0.793 for DOORC2/sample47 and0.809 for DRIP3/sample67, with actual native start
and finite EndLoop events in the same log; other overlapping drips are not
individually certified. This is software output, not hardware listening proof.
Both owned player/display processes were stopped. A fresh bounded review found
that the older actor/terrain pixel overlap assertion proves visibility, not
distinct floor/ceiling occlusion. The stronger test-only plane-omission check
now passes8.93s. At fixed real open Cave views, later floor geometry hides14/13
Spider RGB pixels in normal/rolled views; ceiling hides91/76. A fifth view meets
source flight clearance and hides444 real Spider RGB pixels under later floor
triangles whose own vertices used the source near clamp. Diagnostic provenance
defaults off, preserves scene indices and exists only in test builds; all search
hooks were removed. The actual depth64 actor rejection remains intact. Real
group34/50 constructors leave living authored Spiders587/600 on opposite
controller206 half-planes; the western one is not adjacent to the doorway.
These controlled views are not a continuous installed two-creature encounter.
The failed searches record gate refinement, not rendering defects. Three
affected Cave primitive/order checks pass. The finite mc-oi6.4 acceptance is
delivered by this connected evidence plus installed flight/reset/load and exact
transfer checks; full missions and human play remain separate.

Only test controls, diagnostics and an existing test module changed. Native
842fd4c1/player build-wuukj1s5 is reused; the qualified Original --check-installed
passes. Whole-file UBS includes legacy/test warnings and an unchanged test
temporary-directory nonce falsely labeled a security token; no added critical
finding was identified.
Evidence: generated/cave-connected-20261117 (final traversal plus failure
receipts), and the earlier controlled Cave normal/rolled/near/plane checks.
No full mission, human combat, FPS or hardware-audio claim is made.

### Current Original HUD and minimap — 2026-11-17

The campaign map now reads the current typed Bee, Dragon, dynamic Worm,
Archer, Castle and balloon owners. Their retired raw rows stay suppressed
while the typed actor occupies the slot. An ordinary reuse resumes the new
occupant's source classification. Raw-owned model22 and the existing Firefly
projection remain on their respective paths. The C ABI is unchanged.

Current-role Unity checks passed NightSpire acquired hands, tiers, indexed
icons/bars and paid casting; Day3 paid Castle and balloon markers and matching
indexed status after cold attach; authored class5 markers; and the real
source34 Bee's paid Fireball hit, movement, hide and next-slot reuse. The
source34 case controls player aim and position through public input. It
establishes exact indexed pixels at the unoccluded Bee, later effect painter
order on impact and the class10 replacement. The Day3 entry declares incoming
spell carry. The earlier native Night/Day marker checks and the repaired Day
reset/acquisition/replay test passed. Its old visibility expectation failed
on the prior native too. The source-order, marks, and replay gates are intact.

Strict rig workspace all-targets check and release build passed. Of 626
compared rig inputs, only the later cfg(test) visibility assertion differed at
release capture. Original native
15b4e827fe4dc2f67b026a7a75190c7a210aaab923b3736cc529fa800a6a7145
was installed in build-1w4ofvfa without a Unity player rebuild. The qualified
receipt matches 476 local Rust inputs and both installed plugin copies.
Ordinary saved NightSpire launched with the new mapped native on the first
attempt. Exact tests and receipts are in generated/hud-minimap-20261117 and
generated/gameplay-audio-20261117/installed-night1-audible. The C# scanner was
partial and changed-hunk review found no critical finding on added lines.
Full mc-oi6.5 remains open for remaining marker qualification. These checks
do not certify full missions or playtest.

The Cave6 qualification fixture now supplies the actual Level4 and Level5
package roles required by the existing resolver. On the installed 15b4e827
native, the fixture's real Cave6 entry cold-replays with no spells and with an
opt-in declared incoming Fireball. The latter displays the Cave HUD and
Fireball menu cell in the installed player. Fixed normal native commands
create Fireballs at ticks2–3 before the authored Thief claims the sole
spell at tick4. On the fixed-input approach the Thief drops the same holder78
and ordinary overlap reacquires it at tick27. The live GAME snapshot restores
the ready left-hand tier0 Fireball and its menu entry. The paused tick40 save
cold-replays exactly with life10000 and mana440. Normal semantic menu input
reassigns holder78 to the right hand, updates the published HUD icon and
casts real Fireball projectiles. The installed player renders the reacquired
spell and HUD after physical resume and cast. The tested local mana decline
also occurs without casting while the first Thief is active, so this is not
a separate cast-debit witness. The route declares incoming Fireball and does
not represent earned previous-level carry or the whole Cave mission. No
production entry rules changed.
Receipts: `generated/hud-cave6-20261118.vrlCHd/` and
`generated/gameplay-audio-20261118/cave6-reacquired-fireball/`.
The live Day3 map also removes a Castle marker when its source Castle is
removed, while its still-living collector marker remains. Collector death
through that same map consumer still needs a direct witness.

### Original gameplay audio — 2026-11-17

The current installed banks now have connected software playback evidence.
Ordinary Day3 shore flight in the qualified Original produced both wind/ocean
crossfades, a paid Fireball and positional impact, Archer arrows and a local hit.
Owner waveforms correlate with the isolated output; the return used a second
manually chosen leg in the same process. On new native 15b4e827, ordinary
NightSpire resume and cast produced Night crickets, Fireball and positional
impact from bank1. Cave corridor flight used bank2 drips65–69 and the real
door47 EndLoop. The full 2.214-second door sample matched the owner waveform
at 0.9989 correlation with steady gain. Its EndLoop carries multiplier4 and
the finite voice is ineligible for fading, as in source. The actual same
generation and voice later played Spider32; the captured onset matches the
owner sample, with capture ending before the rest of that sample.

The new Day3 test uses a cold-valid incoming replay built by the existing Rust
qualification case, with declared incoming spells. The managed test uses
ordinary steering and checks both source 60 Hz fades, voice identity, duplicate
suppression and reset release through the current Unity consumer. It passed
on native 15b4e827. Existing native sidecar/reset, ten-voice arbitration and
finite completion/reuse checks passed. This batch changes test sources only.
An exact installed qualification-input mismatch from editor patch backups was
repaired by restoring the receipt-owned backup and moving the added backups
out of Assets. The normal qualification gate remains intact. Captures and
bounded input records are under generated/gameplay-audio-20261117.

Full mc-oi6.8 stays open. The earlier captures have no identified audible
Night creature and use door47 as the real Cave finite witness. Software
capture does not establish hardware listening or full mission play.

Installed Cave6 now supplies the spell and impact witness. Normal resume
and cast from the declared incoming Fireball replay generated local source9
and positional source3 in that order on 15b4e827. Stereo channels aligned
at the same offsets for each bank2 owner WAV despite overlap, at peak
normalized correlations 0.351 and 0.609. The player was tied to a private
Pulse server and had no stream on the main server. The actual source62
creature sound also matched its owner WAV at 0.941. Evidence:
`generated/gameplay-audio-20261118/cave6-fireball-first-ticks/`.
The follow-up uses the cold-valid tick40 save after genuine reacquisition.
Physical resume and cast again produced source9, source3 and source62.
The pre-impact launch prefix matched at 0.998 on both channels and the
positional impact matched at 0.866 on the left channel. Evidence:
`generated/gameplay-audio-20261118/cave6-reacquired-fireball/`.

The installed Night1 approach reached live audible-distance Goats. Its exact
public replay produced three source46 Bleat dispatches without a corresponding
audio journal entry or playback. The Goat slot callback omits the existing
source sound preparation join. A narrow correction is prepared for review.
Publication is pending the active simulator-file owner's handoff and matching
build/replay verification. Earlier saves with those dispatches can have
different journal receipts once publication is fixed. Evidence:
`generated/gameplay-audio-20261118/night1-creature/goat-join-negative.json`.

The installed Original cold-loaded the existing genuine Pav entry replay
with native 15b4e827 and all 30 package roles on 2026-11-18. A private audio
server captured the owned player stream. The single TRACK26 request played
the 4999 ms owner segment, with 0.923 normalized correlation across that span.
The initial shared-server capture was silent and supplied no playback
evidence. Private capture and routing receipts are in
`generated/pav-speech-private-20261118-ojqzzzjw/`. This verifies audible
software output for the existing secret announcement and leaves ordinary
advancement, completion and full mc-oi6.10 acceptance open.

An ordinary New Game opened the installed campaign map with the same native
and all 30 package roles. It played one TRACK01 briefing request at 9999 ms
with no repeated request while the map remained open. In the private mix,
the phase-matched owner voice had gain 1.002 and correlation 0.366, compared
with 0.060 for the largest off-alignment match. Eight voiced one-second
windows held the same alignment with fitted gain within 7% of unity.
Capture and routing receipts: `generated/map-briefing-private-20261118-301cw4zy/`.

Title checkpoint: canonicala77b8176/97a122e5 are pushed as Original82742e3e/
dfe22e00; all5 initial and2 follow-up owned blobs match. Installed Original
build-ky1onwev uses unchanged native58339e62 and the same627 native inputs;
--check-installed passes. This C# batch needed no Cargo rebuild. Source Exit
button/Escape now opens the actual x352/y26/height80/record407 scroll; only a
confirmed, fully closed dialog emits Quit. Cancel preserves Title and preferences,
and held/repeated keys cannot activate the next modal. Cursor39 yields to existing
Keys110/Language37 owners and restores visibility. Original nine-row idle/hover
help follows source100Hz clocks, unsigned199/200 boundary, one advance per source
draw, delayed gold and blank cooldown. Source Title F1 now applies the retained
help flag before drawing, including inside Exit, while respecting media/settings
ownership. The actual SDL/key-buffer producer preserves scan59 with modifiers;
Title Shift/Control F1 work without changing in-game graphics shortcuts. Existing
four fire/incense loops remain; no extra dragon loop or New Game confirmation
was invented.

Canonical116-source typecheck and changed-path/gate reviews pass; UBS remains
partial, with no new production warning/critical reported. Actual Title GUI,
profile/F1/modifiers/release, source clock and Exit check passes7.239s; existing
named-slot scroll passes13.641s; required full menu/NewGame/loading/save/load/
Title/attract check passes29.952s. Old fixture defaults initially selected legacy
packages/menu/media and omitted the real50ms media-release step. Only actual
resource arguments and ordinary release servicing were corrected; guards and
assertions remain intact. Root viewed normal installed Title, original popup,
cursors and Exit. Physical F1/ShiftF1/ControlF1 and Exit F1 retain exact private
profile restoration; Cancel stays usable; confirmed Exit is reaped by waitpid
with status0 and clean shutdown. Owned display/player cleanup is complete.

The first UI script wrongly used Escape during Keys capture; that edits a key
rather than closing, and affected only an ignored private fixture profile. The
corrected run used Keys Done unchanged and isolated profiles. Another checker
mistook an unreaped process entry for a running player despite earlier clean
shutdown; the final actual waitpid result replaces that inconclusive gate.
Evidence: generated/title-dialogs-20261116 (installed-2 cursor/held-Escape captures
and installed-3 final F1/Exit proof). This is not human combat, FPS, hardware audio,
full missions or unavailable-language proof. Broader open acceptance stays open.

Graphics checkpoint: canonical4b22ef80 is pushed as Original75d4acd4; all18
owned blobs and627 native inputs match. Original build-ce2k54te/native
58339e62fc0301f34fce24c5fbb1568c579f94e90f94ac45f2ed365c56db8cff is installed;
--check-installed passes. Shift+F1/F2/F3/F4 and Options8/9/10/11 now share guarded
GAME actions and original50-draw notices. CONFIG no longer owns the four flags;
legacy profile Install validates/removes only their obsolete boolean keys,
preserves duplicate/unknown-field refusal and never rewrites on read. GAME retains
across retry, mission and same-process Title/New Game; a fresh process defaults
ON. Pinned speed5/global sky-buffer admission remains independent of map type.
Active lighting keeps its existing verified policy history as sole authority;
only map packets need an optional lighting header. Old unsaved map values are
not inferred from receiving profiles. No existing lights/projectiles are retired
by disabling future light production.

Seven focused native core checks pass, including actual Fireball light admission,
atomic guards, notices, lifecycle/cold replay and the genuine released-writer
lighting-OFF save with exact byte resave. Actual public-ABI old-save load/resave
also passes; immutable receipt125cfedc is unchanged. Initial gate failures used
the wrong package/owner roles, missing FONT1 on a raw cache owner and a sequential
renderer at restored tick354. Corrections retain all strict guards/expectations.
The real Nyphur view discriminates reflections35778/shadows11 pixels; the real
NightSpire tick1 view discriminates sky33642. Every pass reverses exactly without
camera/actor mutation or gameplay step; the combined check passes31.420s. The
paused cache/revision check passes5.825s. Managed real keys/save/retry146.041s,
Options32.424s and held-release barrier0.012s pass. Workspace all-targets43.00s,
affected test typecheck42.50s, canonical116-source typecheck2.367s and strict Rig
native build pass. Final test-only changes leave production native identical.
UBS remains legacy/partial, with no added-line Rust critical found; the C# scan
does not establish whole-file cleanliness. Independent changed-path/gate reviews
found no further concrete defect in their bounded coverage.

Root viewed normal canonical map/one portal/NightSpire and physical Shift+F1..F4,
P and original-font Options all four OFF/ON with matching mapped native, zero
retries/errors; owned smoke processes stopped. This is not human flight/combat,
FPS, hardware audio, full missions or general historical-save certification.
Evidence: generated/graphics-controls-20261116. RCH's first successful8m45 compile
could not finalize during transient local disk exhaustion; the original completed
job was recovered without replaying it. Root free space returned to18GB without
project deletion; cause remains unconfirmed. Automatic approval review rejected
earlier recursive old-player cleanup; those folders remain intact. Full existing
beads/dependencies stay OPEN. Digital joystick remains an explicit unsupported
host path, without a demonstrated current-host producer or hardware-emulation
expansion; physical-device and distinct unavailable locale coverage stay honest.

Help Mode checkpoint: canonical7cf0fb39 is pushed as Originalcb2058b5; all20
owned blobs match. Original build-xgpo_q20/native
f74c1c8042302a194c71048d9f65df57242de8e903e13794a639e8686922ddf9 is installed;
--check-installed passes. Bare F1 and Options0 share guarded atomic CONFIG/help
policy and400/401 type3/50 notice publication. Existing exact-boundary native
help-policy history makes immediate save/cold replay strict; no fake gameplay
step or profile overwrite during replay. ON reinitializes actual retained popup
rows/ages/priorities and active/pending state; OFF hides it, both edges clear only
pause hover and keep Options open. Unrelated ability notification is preserved.
Pinned source sub_871E0 unconditionally returns1, so No Help Data is not claimed
as an ordinary reachable producer; real missing/invalid assets retain the actual
asset-binding failure route. No host filesystem availability guess was added.

Five native focused checks pass, including atomic failure/preview/guards,
policy+notice immediate exact cold/resave,50-draw clock and F2 replay regression.
The first new assertion incorrectly expected public gameplay hash to include
HELP: explicit native true->false passed while that hash stayed equal. Source
inspection proved HELP belongs to reset/conformance domains. The corrected gate
retains full snapshot equality except the independently checked notice and adds
actual reset AND conformance changes, isolating policy from the notice. It passes;
the test-only correction leaves the entire production native binary byte-identical.
Workspace all-targets43.29s, targeted gate typecheck, strict Rig build and all627
native input comparisons pass. Canonical116-source typecheck passes2.421s; UBS
has no added critical Rust or managed warning/critical finding, with C# partial
analysis/legacy totals explicitly retained. Fresh bounded source review found no
further concrete defect in the reviewed seams.

Real selected-glyph hint/popup/F1/save-load check passes128.619s, Options/cold
32.864s and held F1 barrier0.012s. Its first Load exposed a genuine qualified-
client rebind bug: preference preparation synchronized an OLD borrowed campaign
projection against the NEW runtime. The qualified client now uses canonical's
existing Load/recovery transaction without relaxing guards. The dedicated real
paused-load/audio/old-root disposal/malformed-save guard passes23.498s. That test
initially selected an obsolete menu cache; only its resource arguments changed
to qualified current owner data and private profiles, retaining every assertion.
An obsolete test expectation that F5 saves was corrected to the existing explicit
save request; delivered physical F5 sound assertions remain intact. No v8 data
retagging occurred: a stale label was corrected to actual current v9.

Root viewed normal canonical map/one portal/NightSpire and physical F1/P,
original Help Mode OFF/ON Options and400/401 notices; matching mapped native,
zero retries/errors, owned processes stopped. Actual popup rendering and warm
Load/recovery are separately established by the matching managed checks, not
claimed as human flight/combat/FPS/hardware-audio/full-mission/historical-save
proof. Evidence: generated/help-mode-20261116. All full existing beads stay OPEN.

Flight-assistance checkpoint: canonical cdcc8498 is pushed as Original235baa78;
all22 owned blobs match. Original build-a75ffnon/native
f327d31480c075c2aae3086265bee96e3a93be1d934998d329a9ea2317ce4fa8 was installed;
--check-installed passed. Real bare F2 and Options4 share the guarded atomic
profile/398..399 type3/50 notice action. Current supported campaign is explicitly
offline; AI rows and replay are not multiplayer admission. Suppressed states
retain the source mouse/counter; accepted pause/resume remaps only the exact
same input owner, rejected calls mutate nothing, actual reset starts a fresh run.
The source queues recenter before its later handler, retaining the observed
sample instead of manufacturing an immediately centered sample.

Fresh bounded review exposed a real precision gap: at sensitivity10, actual
sampler0/.45 values both round to physics byte0. The real-client49-turn regression
failed35.167s with sequence1 instead of0. A validated finite exact steering
observation now reaches Rust before quantization, is consumed before recording
the derived gameplay command, and leaves physics bytes/fixed ABI unchanged.
Legacy omission retains exact old command bytes; signed zero is canonical.
Native real changing/stationary samples, rejection, profile/cold replay,
48->pause/resume->49 recenter and real reset controls pass. Nine focused checks
select and pass; workspace all-targets passes44.23s and strict Rig build passes.
All627 native inputs match published source/Rig/returned/Editor/installed.
These627 are provenance inputs, not a test target.

All five real managed controls pass: key/consumer93.337s, Options/cold32.059s,
the formerly failing precision path plus stationary positive37.670s, all encoder
shapes/negative bits/legacy bytes0.014s, held-key barrier0.010s. Canonical116-source
typecheck passes2.380s. The encoder gate initially mistook two XML representations
of the identical empty JSON object for data loss; diagnostic input/encoded/XML
evidence justifies normalizing only empty-object text, retaining all member/bit/
legacy assertions. UBS remains partial/legacy, with no critical added Rust line
or added managed finding; fresh bounded source review found no further concrete
defect. Canonical compact identity refresh is source/typecheck coverage only;
actual managed/installed coverage uses the qualified full-identity cohort.

Root viewed normal canonical map/one portal/NightSpire scene/HUD, physical F2/P,
original Options FLIGHT ASSISTANCE ON/OFF and both original notices; matching
mapped native, zero retries/errors, owned player/display stopped. Controlled
native/managed recenter checks are not installed human steering/FPS/hardware
audio/full missions/historical-save proof. Evidence: generated/flight-assistance-
20261116. No Unity art/owner data change; full existing beads remain OPEN.

Audio checkpoint: canonical b8b44284 plus Loading-contract fix124f58f7 are pushed
as Original8ef5e2b0/6ca36e0d; all22 original owned blobs and both fix blobs match
their published commits. Original build-h_lrtfo6/native
fee7453b25fd53d84f5fe119bf3f72a8f881b470ed74eeabca4053ee478aca1a was installed;
--check-installed passed. Actual guarded F4/Options3 gates future speech without
stopping current CD; F5/Options1 retires native/managed samples and CD on BOTH
edges; F6/Options2 stops/restarts actual current-map music without a fake step or
fade advance. Source notices390..393/469..470 type3/50 replay through the existing
notice owner. Sound-off drains real semantic events without stale sidecar lookup,
keeps the cursor/music path and does not replay missed SFX on enable. Enabled
ownership checks remain strict. F5 no longer invokes replay save; named slots
remain. Product cold flags persist; original CONFIG did not store these flags.

Actual availability comes from qualified sound banks, current-map music and the
retained validated27-clip speech owner. Music qualification is lazy at the real
active-entry resource query; identity reads remain pure and recovery stays usable
after an asset error. An early all30-music preflight regressed the same warm native
bootstrap24.24s to34.04s and was removed. Three alternating old/final pairs now
have medians24.0693/24.0884s with matching I/O scale; this is a warm native slice,
not whole-client startup/FPS. No new cache platform or fixed ABI enlargement.

Atomic preferences, real authored transport audio, actual native controls/guards/
pool/recovery, workspace all-targets and strict Rig native build pass. All627
source/header/upstream inputs match; these are provenance inputs, not tests.
Actual managed paused audio/cold profile passes33.059s; physical controls, later
real speech cues and real Day land ambience retirement/reacquisition pass120.865s.
The Cave cannot produce that Day loop; the fixture uses the existing real returned
Day parent and ordinary frames, without injecting a voice. Initial real-client
checks exposed and fixed disabled-SFX sidecar and stale managed generation joins.

The first installed smoke FAILED at Loading because a shared C# response DTO
required active-entry-only audio facts. Evidence is preserved in installed-1;
no retry. Separate strict response DTOs fix that contract. The extended actual
Loading->Playing/audio check passes91.628s and canonical116-source typecheck
passes2.407s. UBS is partial/legacy with no finding on these added lines, not a
clean scan. The rebuilt matching installation then passes normal canonical
map/one portal/NightSpire scene/HUD, physical P/F4/F5/F6 and original Options all
off/on; root viewed captures, mapped native matches, zero retries/errors, owned
processes stopped. Evidence: generated/audio-controls-20261116 and loading-recovery.
No human combat, FPS, hardware audio, whole missions or historical-save proof.
Full existing acceptance/dependencies remain open. No owner art changes.

Player Names checkpoint: canonical1a83f09b is pushed as Originalb2123b87;
all31 owned blobs match. Original build-02d3en9v/native
2063859fda7d0a8773b6509658a19b1d33de3192eff1119eabfeecd83fcc53f0 is installed;
--check-installed passes. Actual source case17/auxiliary byte207 drives the
existing current class3/model0|1 A4/name/life carrier, original selected FONT1,
normal primary painter callback and real label/health pixels. No GUI name overlay
or asset-inferred actor was introduced. Names default visible, remain outside
GAME/CONFIG/SLEV and gameplay hash, retain receiving process state on retry/mission/
warm load, and cold-default only after actual saved receipts verify. The actual
local402/403 type3/50 notice and its reset digest/history still replay exactly.

Real unfiltered Level2 entry plus354 existing public flight/look commands reaches
living Nyphur112, PlayerState1/life3085. Strict cold replay/resave and actual FONT1
raster pass17.399s; no inventory/cast/actor-pose staging. Eleven affected native
checks pass, including actual Day3/Cave12 viewport, label cache/missing-font
recovery, rival cleanup, Default Keys, source glyph traversal and CABI guards.
The source clears the surround BEFORE DrawWorld; canonical writer and streaming
mode26 now clip terrain before writes, allowing source lowres inset labels to
remain in the surround. Native and GPU exit history preserve current exterior
pixels rather than resurrecting prior MAIN. Workspace all-targets and strict
native build pass; all627 source/header/upstream inputs and Rig/returned/Editor/
installed plugin hashes match. These627 inputs are provenance, not tests.

Actual managed Options/lifetime passes106.17s; exact GPU history passes0.526s.
Actual selected FONT1/current Nyphur GPU glyph and health check passes36.486s:
native and GPU match243 changed pixels in [128,51..154,59], exact on/off/on
reversal and query/replay purity. The initial new assertion compared the wrong
hash domain; it now checks actual ResetReceipt notice digest changes and unchanged
public gameplay hash. Initial GPU checks had an inactive presentation parent;
the corrected host activates only the real presentation hierarchy and retains
all glyph/health assertions. No receipt/assertion weakening or injected pixels.
The real restored-client route now binds existing preferences/locale BEFORE first
world composition. Canonical116-source runtime typecheck passes. UBS has no added
critical finding; whole-file legacy and partial C#/shader analysis remain limits.
Evidence: generated/player-names-20261116.

Root viewed canonical --load-save of that genuine public-flight replay with all30
resource roles, actual Payahandra scene/HUD and source Player Names ON/OFF Options;
matching mapped native, no retry/error, owned player/display stopped. The pause/
Options overlay covers most of the rival label, so full installed glyph visibility
is not claimed separately from the exact matching managed GPU proof. This is a
declared Level2 entry, not earned campaign progress, human combat, FPS, hardware
audio, full missions or historical-save proof. No owner art/Unity asset import.

Viewport checkpoint: canonical f0e11884 is pushed as Original81233f67; all20
owned blobs match. Superseded Original build-_rry9xbm/native
74ccbbf0c5260639c6be18c8079ee0281e52d46cb817889d20c9e2c04dd6e3c5 was installed;
--check-installed passed. Source Options index7 and physical brackets drive a
guarded GAME viewport16..40: menu clamps, '[' enlarges and ']' shrinks only above17.
The saved size survives cold/warm load, retry, mission and Default Keys; omitted40
preserves old save bytes. No CONFIG field, fake gameplay operation, notice or
fixed ABI enlargement was added. Projection, sky, source sprite clipping and
destination operations use actual viewport geometry; source palette-derived
surround clearing precedes full-target HUD/minimap. The fixed map pane retains
its separate view. The actual GPU exit-history compositor also clears the
surround before applying its pointwise source LUT; odd-size vertical margins
retain source truncation. Selected source glyph record13 now labels the enabled
option correctly as ALTER SCREEN SIZE.

Real Day3/Cave12 projection, clear surround, fixed-map override and full40 reversal
pass0.665s; source projection/sky, all three sprite-pass clips and the preserved
internally-clipped visibility boundary pass. GAME lifecycle passes1.19s, genuine
old-v9 receipt migration13.58s and CABI guards51.62s. Workspace all-targets and
strict native build pass; all626 source/header/upstream inputs and Rig/returned/
Editor/installed native bytes match. These626 inputs are provenance, not tests.
The actual GPU LUT/viewport check passes0.623s. The initial accumulated settings
test exceeded its existing180s timeout; every new viewport assertion moved into
the focused real-client SourceViewportChangesRealWorldAndKeepsFullHud, which
passes107.38s. Its prior SourcePanels body independently matches delivered
deacbbc5 byte-for-byte. Actual Options row/control and held-bracket checks pass
42.20s/0.012s. No assertion/timeout gate was weakened or Rust rebuilt for that
test-only separation. Canonical116-source runtime typecheck passes2.32s. UBS has
no added critical Rust finding; legacy totals and partial C# analysis are limits.
Evidence: generated/viewport-20261116.

Root viewed normal canonical map/loading/NightSpire, physical P/bracket shrink,
actual Options shrink and subsequent enlargement with full-sized HUD and original
indexed text; matching mapped native, one portal, zero retries/runtime errors,
owned player/display stopped. The final rapid-key capture remains inset and is
not exact-count/full40 installed reversal proof; that reversal is established by
the native and managed checks. This is not human combat, FPS, hardware audio,
full missions or historical-save proof. All30 resource roles remain available;
their mission paths were not repeated.

Speed checkpoint: canonical deacbbc5 is pushed as Original7ffcf607; all23 owned
blobs match. Superseded installed build-ozzihvaw/native4641e38e was qualified before
the viewport installation. Real F3/Options primary and secondary select source
0/1/2 with1/4/8 UpdateEntities passes per outer frame; PlayerEvents/presentation
run once. Actual steering, mouse suppression,50-frame381/382/383 notices,
recovery/reset, cold0 and warm receiving speed are connected. Speed is auxiliary,
not GAME/CONFIG/SLEV. Ordered process assignments/actions preserve same-boundary
save/replay; all saved receipts verify before cold/warm process adjustment.
Native actual-player cadence/lifecycle passes1.03s, real frame admission0.40s,
actual revival0.30s, old-v9 migration13.84s and CABI53.19s. The first warm-load
test incorrectly called a Title-only API from Paused; fresh Title targets corrected
the fixture without production or receipt changes. Actual8-pass managed execution
then exposed a real pause/audio join: native retires voices at accepted pause,
but Unity previously waited to observe Paused on a later drain. All accepted
pause paths now synchronously use the existing audio rebind/retirement; strict
ownership remains. SourcePanels passes176.91s after that fix; other two controls
pass44.18s/0.012s. Native/source closure and canonical116-source compile pass.
Root viewed installed F3 SPEED FAST and Options SPEED NORMAL/FAST, normal
map/load/terrain/HUD, matching native, zero retries/errors; owned smoke stopped.
Evidence: generated/speed-20261116. Human combat/FPS/hardware audio/full missions/
historical saves were not qualified.

Brightness checkpoint: canonical b5088a64 is pushed as Originalf916f016; all19
owned blobs match. Superseded installed build-kvk4b54z/native889b4704 was qualified.
Bare F7 cycles source0..4; Options clamps. Actual paletteMod>=3/submode0 admission,
endpoint refade, startup/four-step signed RGB6 fade, immediate save/cold restore
and shared world/HUD/help palette consumers are connected. Optional replay
palette boundaries preserve the genuine old-v9 prefix; CONFIG writes remain
atomic and generic Playing preferences restricted. Duplicate managed brightness
transforms were removed. Focused palette/preferences/receipt/CABI checks, strict
Rig build, all626 inputs and real managed help129.75s/Options44.70s/SourcePanels
117.15s pass. A test-only old camera-history assumption was corrected by drawing
the actual current generation before asserting full unchanged palette equality;
no production guard or expectation was weakened. Root viewed installed F7 and
Options brightness, original map/load/NightSpire/HUD, matching native, zero
retries/errors, owned smoke stopped. Evidence: generated/brightness-20261116;
the same human/FPS/hardware-audio/mission/historical-save limits apply.

Flat-shading checkpoint: canonical a99893ec is pushed as Original49481c14;
all26 owned code blobs match. Original build-j89o0ece/native
6451e447039315dfb3c1fd4f4a1b3dc0c3bfdf77a71927f0334505602f9db6bc is installed;
--check-installed passes. Actual Shift+F7 and Options row14 share a guarded
source GAME setting and original442/443 notices. It defaults off and survives
save/cold restore, retry and mission entry, independently of CONFIG. Optional
stored false remains omitted so existing receipt bytes stay unchanged. No fixed
ABI or MCPO struct was enlarged.

The actual indexed world consumes source mode7 on outdoor primary terrain and
both Cave floor/ceiling. Four-corner signed wrapping intensity sum>>18 selects
one palette row for both cell triangles; existing scan conversion/UV coverage
is reused. Reflections retain mode5 and reflective primary material retains26.
Same-tick setting changes invalidate scene/map caches without stepping gameplay.
The connected outdoor check found the old5/26-only preflight refusal; admitting
exactly5/7/26 preserves primary-only7/26 and every other refusal. That correction
was verified through the final matching native's real managed outdoor consumer,
without recompiling the large unit-test binary for that narrow gate change.

Focused kernel, GAME lifetime/legacy replay, real Cave floor/ceiling and CABI
guard/cold checks pass (1.84s/0.54s/0.54s/51.36s for the latter four). Workspace
all-targets passes before the narrow selector correction; the final strict Rig
native build passes with all626 input hashes and returned/installed bytes
matching. These are source/header/data inputs, not tests. The actual Original
managed outdoor flat/reversal/popup/cold/retry check passes112.62s; Options
row/442/443 passes42.81s and held-key release passes. The first managed setup
correctly rejected symlink cache aliases; only the two affected checks were
rerun with their existing regular cache directories. No gate was weakened.
Canonical Editor runtime offline typecheck passes on116 actual sources. Final
raster UBS reports zero critical; broader legacy warnings and partial C#
analysis are retained as limitations. Evidence: generated/flat-shading-20261116.

Root viewed normal canonical map/loading/NightSpire, physical P, both Shift+F7
variants and the actual Options flat entry in the installed Original client;
matching mapped native, one portal, zero retries/runtime errors, owned smoke
stopped. This proves the installed display control and launch/load, not human
combat, FPS, hardware audio, full missions or historical saves. All30 resource
roles remain present; their mission paths were not repeated. Full mc-5r5.2,
mc-oi6.5/mc-oi6.6 acceptance/dependencies remain OPEN. The three current owners
are now implementing the connected brightness capability above.

Castle/balloon checkpoint: canonical0c081394 is pushed as Original2572257b;
all22 owned code blobs match. Original build-fsmin12b/native
f583f3671a86d4a6e7fc998717bc4ea87e59506d1a60a0683a5e8254f5ae8835 is installed;
--check-installed passes. Real owner/stage and ordered live collector slots now
reach the original44–53 icons, signed wrapping bars, owner-mana aggregate and
raw authored goal ticks. Stage zero retains its distinct balloon admission;
missing slots are not compacted. The actual shared contact/area-damage path now
writes the collector's canonical mailbox. Castle hit selection uses source ID,
not merely ownership. Normal balloons retain their source shared-A4 hit carrier;
no local-player flash producer was invented.

The admitted source HUD tail follows objective/menu publication and consumes
clocks only in the source world/TopBar path. Accepted frame-policy history
replays that boundary; missing-history v9/v8/v7 retains the old verified prefix.
A genuine released-writer v9 save passes exact receipt migration. Full JSON
and MCPOv5 share typed status. Required zero/one castle_rows avoids Unity's
null-class-to-stage-zero normalization; original-byte streaming validation
preserves missing/null/shape refusal before JsonUtility normalizes it. No fixed
DayHud ABI extension, client clock, shortened lifetime or weakened receipt.

Root's real source4 paid Castle→flyer→completed root→natural collector→contact/
life→death/removal/reset check passes0.32s, including admitted hit clocks and
pure reads. Incoming carry/travel and detached alias/stage/scalar controls are
declared, not earned campaign proof. Source queues the1000-mana debit for the
following player visit; the matched neutral control remains1000 while the cast
reaches0. Collector maintenance follows the actual next even-phase callback.
The genuine old-v9/new-boundary/cold/rejection check passes13.89s. Workspace
all-target, strict Rig native and source-closure checks pass; all626 input hashes
and returned/Editor/installed native bytes match. These are inputs, not tests.

Actual Original managed paid-Castle/collector/cold/HUD check passes11.49s. It uses
a declared incoming Level3 carry/Loading control,38 fixed public reverse inputs
and real FullStop into a source-admissible site, then ordinary casting, build and
collector callbacks. It cold-loads with the same full source-animation role and
checks original icons and every collector life/mana bar pixel. Rare stage/holes/
negative-life/full-mana/goal controls remain detached component checks. Repaint
and normal Editor cadence preserve the native tick and replay. Wrong old BUILD
and cache fixtures were corrected; switching an absent animation catalog to a
full one was correctly refused, not bypassed. The actual integration defect was
startup preferences advancing the restored epoch1→3 while the scene/HUD stayed1;
the accepted boundary now republishes existing presentation owners at the same
gameplay tick, preserving all epoch guards and audio's no-extra-fade policy.

Both native full/compact and exact canonical C#/TypeScript decoders agree on the
actual paid-owner/collector status; all five browser codec checks pass. The
canonical Editor runtime assembly compiles offline with its original116sources,
266references/132defines/analyzers, zero errors/warnings; this checks canonical
optimized APIs separately from Original's older full-snapshot cohort. UBS has
no added-line critical Rust finding; C# analysis is partial, not a clean scan.
Root independently reviewed the source/gates and executed the connected checks.
Evidence: generated/top-status-20261116.

Root viewed the installed original Castle/balloon/player icons and bars in the
actual Day3 scene and physical-P pause menu through the canonical Original
launcher, using the exact three-package full-role saved control. Matching mapped
native, zero retries/runtime errors; owned player/display stopped. This proves
installed cold load/render, not earned campaign traversal, human combat, FPS,
hardware audio, complete missions or historical saves. Normal all30 resources
remain available, but this smoke did not re-run their campaign paths. Full
mc-oi6.5/mc-oi6.6/mc-5r5.2 acceptance/dependencies remain OPEN.

The connected panel-transparency batch is implemented, verified, committed and
installed: Shift+F6/Options13 share one guarded Rust GAME setting and local
440/441 type3 notice. Real save/cold restore preserves it; successful normal
LevelInit/retry resets Day/Night to transparent and Cave to opaque. Old GAME
absence resolves the map default only after validating the old receipt. CONFIG
and Default Keys remain independent. JSON/MCPO carry accepted state; minimap
admission is a pure guarded query. The indexed compositor preserves arbitrary
ordered minimap blends over current world/WEB, then source HUD backgrounds,
opaque icons/glyphs/bars and recursive objective/help. Cave minimap and full map
remain opaque. No CPU world readback or RGBA approximation supplies this path.

Real native Night/Cave save/reset/notice checks pass1.764s; CABI old-save,
guarded coverage/purity and reset check passes51.014s; MCPO lifecycle passes0.366s.
Workspace all-targets and strict Rig native build pass; all626 captured inputs
and native8cbb514708623c0468c5ae086c88218268863a7f4d3f982348d2d375cb39008c
match. These626 are source/header/data hashes, not tests. Actual Cave indexed
minimap invariance/HUD change passes34.15s; the actual Day paused F6/help-retention,
same-tick indexed pixel, save/cold/retry check passes82.77s. Pause-help recursion
passes41.90s; the held-control release barrier and GPU indexed HUD/minimap/WEB
orientation controls pass. Two test-only defects were corrected without changing
expected gameplay/pixels: a ushort boxed key missed its int-keyed descriptor;
the pause test used a retired v3 package instead of the qualified installed
resource set. Only those two failed checks were rerun. Detached GPU data proves
component composition, not earned gameplay. The old loaded-v1 minimap refusal
test was extended but not executed; its historical fixture is not new runtime
proof. Fresh bounded minimap review found no defect. UBS is partial for C#
analysis, with no critical/new changed-hunk warning; legacy Surface warnings
remain. Evidence: generated/panel-transparency-20261116.

Canonicalc123114d is pushed as Originalbb6a71a8, excluding unrelated Modern
ancestry; all33 owned code blobs match. Matching Original build-r2ejdd67/native
8cbb514708623c0468c5ae086c88218268863a7f4d3f982348d2d375cb39008c is installed
and --check-installed passes. Root viewed the normal map/loading/NightSpire
route and physical P, both Shift+F6 variants, source Options13 and resume:
actual original440/441 notices and indexed panel changes are visible. One
portal,30 resource roles, matching mapped native, zero retries/no runtime errors;
owned player/display stopped. Pixel comparison falsified an initial visual
suspicion of outside-world changes: paired paused and Options world pixels
outside the exact HUD/minimap/notice/option-label regions are identical. This
is installed display-control/launch-load proof, not human combat/FPS/hardware
audio/full missions or old saves. Full settings/HUD/help acceptance remains open.

Next confirmed HUD gap is source actions31/32's eight-update hand-selection
visibility on full map. Ordinary world HUD/topbar and map hand visibility have
distinct predicates; do not invent a GUI timer or derive flash from holder
validity. Detailed Castle/balloon HUD parity also remains incomplete. Full
mc-oi6.5, mc-oi6.6 and mc-5r5.2 acceptance stays open.

Hand-selection visibility is now implemented, verified, committed and installed.
Real accepted31/32 retriggers its global byte8 after the PlayerEvents prologue
decrements both. Source local-player startup ApplyEvents does not dispatch that
prologue; reset/ordinary/secret owner transitions retain the actual global pair.
Cold replay reconstructs saved state; real warm load retains the receiver pair
only after all saved receipts verify, recording the existing context-change
boundary. Newv9 saves verify oldv8/v7 under their historical producer prefix and
enable new assignments only at the explicit migration boundary; old receipts
are neither weakened nor rewritten. Actual full/JSON/strict MCPOv4 carry the
pair. World HUD remains TopBar-controlled; full map draws only each nonzero
hand after source menus/notices, independently of TopBar, with real indexed
notice-glyph overlap before transparent frame shading. No GUI clock or RGB
index inference is used.

The full real owner selection/expiry/query/invalid/preview/mark/reset/new-owner/
cold/warm/oldv8 check passes14.21s; actual CAPI cold/warm/rejection boundary
passes0.89s. Source hand/help and independent-hand checks, F6 lifetime1.84s,
MCPO purity/lifecycle0.416s and range-error mapping pass. Actual managed map
selection/TopBar/F6/expiry/pixel/repaint check passes58.11s; existing Ctrl/help
passes62.75s. Long notice/CR overlap is an explicitly detached component
control using actual glyphs/LUT, not a claimed native-produced long notice.
All five browser codec tests pass, including real native parity; a separate
actual accepted Fireball assignment gives full/render/TS and exact canonical
C# decoder parity at[8,0]. The latter compiles the real codec/DTO sources with
unused external dependencies stubbed; it is not Unity JSON/native integration
proof. The qualified Original managed checks supply that separate live path.

Strict Rig test compile/native/workspace checks pass; all626 inputs and native
875cb19902ed9dc136538f86b8719bb841fca82ff79c941d5376eae25ecdfa41 match.
One compile failure was an old test's retained-context type assertion; its fix
preserves production bytes. The build command then hit its configured300s
deadline, not OOM; the proper test --no-run command completed6m44s with the same
cache and configured1800s limit. The separate legacy global-context runtime
check fails before construction on a hardcoded Rig generated path; no candidate
behavior was exercised or qualification claimed for it. The new real owner
and CAPI checks cover the changed warm/cold boundary. UBS whole-file legacy
totals are not candidate defects: no added-line critical finding; new warnings
are bounded indexing/validated invariants and test assertions. C# analysis
remains partial. No scanner suppression or assertion weakening. Evidence:
generated/hand-selection-20261116. Full HUD/settings acceptance remains open.

Canonical7cc76199 is pushed/remote-verified as Original9cdd13a1; all21 owned
code blobs match. Matching build-cloulkxe/native875cb19902ed is installed and
--check-installed passes. Root viewed normal map/loading/NightSpire, physical
Return full map with no stale idle hands/status strip, and Shift+F5/world HUD
hide/restore. All30 resource roles and mapped native match, one portal/zero
retries/no runtime errors; owned processes stopped without escalation. Installed
proof is idle-map/topbar/launch-load, not a fresh native hand-assignment visual,
human combat/FPS/hardware audio/full missions or old saves. The actual assignment
visual/lifetime proof is the connected managed check above. Source F5's
counterintuitive365/366 notice wording is preserved exactly; this is not a new
defect or reason to rebuild the unchanged checkpoint.

Next connected gap: Castle and balloon state exists but day_hud_for still
projects empty54/54 panels. Bind actual owner relation/stage, ordered live
balloon slots, life/mana/owner aggregate and source bars/icons through one
typed same-boundary projection and indexed consumer. Preserve the distinct
stage-zero predicates, real hit-clock/goal carriers and removal/reclaim; no
client timer or invented Castle/balloon state.

The spell-menu trace exposed14 genuine Castle-mana threshold errors across
Fireball, Possession, Speed, Morph, Shield, Invisibility and Beyond Sight. Loaded
SPELLS.DAT maxManaLimit_A is copied directly by LevelInit; it is not divided by64.
Canonical properties/holder profiles now use those raw values, retaining the
source family23 override. All78 loaded tier gates and the14 exact lower/equal
admission boundaries pass, along with all38 activation-module checks. A real
public Invisibility selection/cast check compares matched no-cast/cast frames at
Castle19,999/20,000 and verifies source payment/XP/first-pulse behavior (0.48s).
Its original zero-regeneration expectation was defective: both ordinary frames
receive source regeneration. Only that test was corrected; production is byte-
identical outside it, and the original exact debit predicates remain intact.

Native help metadata carries retained tier, actual selected title records,
Castle/caster gates, discovery/blink, costs and XP through JSON and strict MCPOv3.
Actual fresh-Pav JSON/compact parity passes29.34s; existing HUD-tier/publication
lifecycle/menu purity and Beyond Sight public acquisition/readiness checks pass.
The final workspace all-target check passes44.8s; strict Rig native build passes
171.4s and all626 captured inputs/native bytes match. These626 are input hashes,
not tests. Nativee51b38785177e6de53b8f27214c1a7a1b764a7dab963942c94d27ab946630274
is installed in the checkpoint below. Original source glyphs/three-line highlight and chained
indexed TABLES recursion pass0.50s; actual controlled Results->Uluth tier drag
passes54.96s. The Ctrl-hover/shared-clock failure was resolved below without
weakening gameplay expectations. Menu
transparency currently projects the supported map default; Shift+F6 remains an
explicit unfinished control. Full help/language/audio acceptance stays open.
Independent source review caught two new renderer defects before delivery: tier
panel shading depends on Castle admission plus caster mana, but icon transparency
depends only on the Castle gate; XP bar arithmetic narrows its quotient to signed
16 bits before sign/54 clamp. Both now match source. Their detached indexed
component controls preserve the real owner/replay and do not claim organic
Castle eligibility or extreme XP production. The actual source20 owner has no
Castle and must retain that real blocked predicate. Transparency is source
Shift+F6 (case14), not Shift+F7, which selects flat shading (case15); the older
native comment calling transparency F7 is stale and will be corrected with that
next native capability, without rebuilding an otherwise unchanged artifact.
The connected managed Ctrl-hover check now passes62.11s, including actual
source20 entry/selection, source85 titles and six-poll cycling, indexed XP/icons,
repaint/native replay purity, unavailable-category cleanup and map+spell mode.
Diagnostics proved HUD GUI.matrix changes replaced the delivered event pointer
with OS coordinates; the canonical HUD now preserves it through its temporary
transform. Root initially placed that small fix in a similar legacy HUD method
in the older qualified project; that own candidate placement was reverted and
corrected by named method. The subsequent detached pixel control had supplied
stored33x17 sprite dimensions while sampling real66x34 runtime geometry; it now
uses the actual descriptor scaling, retaining the same54px/color expectations.
Temporary diagnostic draws/logs are removed. Final changed-source UBS is partial
for C# compilation/taint limits with no critical/warning in the changed regions.
All previous passing native/codec/tier-drag/GPU checks were reused; no native
rebuild was needed for these managed/test corrections. Installed physical input
is separately verified below; the Editor-window check proves its covered component path.

Spell-menu batch is now committed canonical9152adcb and pushed/remote-verified
Originalccd69323, excluding unrelated Modern ancestry. Matching Original
build-afsiqvfu/nativee51b38785177 is installed; --check-installed and all626
source/header/data inputs match the strict Rig receipt. The678-output source
menu cache was independently rehashed and published under the player-install
lock. Root viewed normal canonical map/loading/NightSpire and physical Ctrl
hover selecting FIREBALL/RAPID FIRE/FIRE STORM, then POSSESSION/MANA MAGNET/MANA
LOCK with original glyphs, indexed panels/bars and selected-tier highlight.
Physical Ctrl release clears the bottom menu and popup. All30 resource roles
and mapped native match, one portal click, zero retries and no cursor/integrity/
loading errors; owned player/display stopped without escalation. This is
installed UI/launch-load proof, not human combat, FPS, hardware audio, full
missions or old saves. Evidence: generated/spell-help-20261115/installed-hover-1.
The next confirmed control gap is source Shift+F6/pause option13 transparency,
including real panels, notices and GAME/LevelInit lifetime. Full mc-oi6.6 and
mc-5r5.2 acceptance remain open.
Source SetKeysDialog's immediate default action keeps the dialog open and resets
ten keys, sound/music volumes127, showHelp and brightness11/13. It preserves
brightness12, selected language/font, minimap/topbar and other settings: langIndex_4
is scratch, SelectedLangIndex is the persisted language, and LevelIndex_0xc is the
local player index, not campaign progression. Cancel after an accepted reset must
retain it while discarding subsequent draft edits. Full original acceptance stays
open; no default-state payload from the client or whole-preference replacement.

Default Keys is now implemented, verified and installed. One checked native
preferences operation derives only the source reset from committed state, validates
the unchanged selected-locale binding and publishes through the existing atomic
profile transaction. The real source109 bitmap and sample14-before-reset reach the
Title key dialog; the dialog stays open at its first assignment. All678 cache outputs
were independently rehashed; exactly settings-109.rgba8 was added, all677 prior
outputs are unchanged. Incomplete three-sprite settings caches now reject early.
The focused native current-locale/field-preservation/guard/IO/Cancel/cold/live-owner
check passes0.22s. Its first invocation selected an oldv3 package and correctly
failed collision admission before runtime; only the environment was corrected to
the existing v5 package. No source gate or expected result was weakened.

Connected original Title->Keys->Default/current-profile/cold-input->normal Night
cast/save/load/resume/retry passes37.70s; the prior source-help refusal/receipt and
held-input release checks pass1.15s. Only the failed legacy fixture was rerun: its
typed JSON serialization had changed absent fields into empty strings. The final
fixture uses the existing .NET JSON reader to append only the real109 descriptor/
bytes/receipt, retaining all original missing-help/pause negatives. Two intervening
isolated-test compile failures were resolved without production or native changes;
the older Original project does not carry the shared project's Newtonsoft package.
No extra package was introduced and passing gameplay evidence was reused. UBS is
partial/legacy-scoped; final changed hunks have no critical/warning finding.

Strict Rig core compilation and production native build pass; all626 captured
source/header/data inputs and returned native bytes match. Original build-ha52skcb
with nativea828d36efe614d5f84f48f904a3659ba4e471bb85548de54cf412aea043b842c
is installed and --check-installed passes. Root viewed actual physical Title/Keys/
Default/Cancel, exact private-profile reset and a fresh process's UP binding/source
button. The cold process then reached normal all30-resource map/loading/NightSpire
terrain/HUD/help with one portal click and zero retries. Both owned smokes stopped.
This is UI/profile/launch-load proof, not flight, human combat/FPS/hardware audio,
full missions or a historical WAV root-cause correction. Evidence is in
generated/default-keys-20261115 and help-20261115/default-keys-*.
Deletion of the verified105.7MB superseded Linux-before-build-ha52skcb player was
blocked by DCG's recursive-removal approval gate; no bytes were removed or guard
bypassed. Current installation, owner data and caches are preserved; disk headroom
is sufficient. Full mc-5r5.2/mc-oi6.5/.6 acceptance remains open.

Next connected capability: the real successful switch31 constructor must request
the special source announcement, then source phases7/8 select TRACK26/27,
text284/285, palette/duck and cleanup. The separate wrapping source3E4 counter must
follow actual CollectLevelStats calls; unique completed-secret history is not its
equivalent. Existing real Jaleen area722->727->group44 row755/portal770 and Pav return
checks supply the producer/continuation. Keep this next batch source-traced and
connected through current speech/text consumers, not an isolated segment function.

The secret speech unit mismatch is now being explicitly resolved under
mc-oi6.10's original acceptance: the source table stores375 CD frames, but86F20
omits86EB0's75-Hz conversion before SDL's millisecond timeout. Actual unchanged
TRACK26/27 contain voiced100ms windows through4.3/4.6seconds, so raw375ms clips
both sentences just after their start. Reuse the existing source conversion
(4999ms with its pinned floating truncation), retaining table/selection, source
state/order, palette and cleanup. This is a deliberate backend-unit correction,
not exact parity with the SDL clipping bug. Evidence: generated/
secret-announcement-20261115/secret-media-duration.json. Managed/native exact
oracles check the corrected duration before delivery; no installed claim yet.

The actual Jaleen area722->727->group44 switch755/portal770 producer now passes
through special phase7/8, text284, TRACK26/4999ms, real secret entry, fresh Pav
tick0/zero-operation replay, cold byte/hash equality, paused phase delivery,
retry/reset, actual Pav completion/statistics and parent return. This connected
check passes34.23s; suppression/exhaustion passes0.16s and source units/u8 wrap/
statistics/reset boundaries pass. The first local guard invocation failed before
runtime because its RCH-embedded package path was unavailable; the existing
MC_JALEEN_PACKAGE override now selects the actual installed package. No gate
changed. Declared frontier/pose controls are not an earned campaign route, and
TRACK27 is conditional source coverage, not a demonstrated natural producer.

One workspace all-target check passes44.8s. Final core compilation387.6s and
strict Rig native build pass; native dc5009d619f5bbdda3a64bd454ed6bf6f289ee96c0afbc4c96acb48c91e396cb
and all626 source/header/data inputs match Rig. Existing source membership is
unchanged;626 is not a test count. Actual fresh-Pav C ABI JSON/MCPOv2 parity passes
26.11s, including the special record and formerly omitted signed wall counter;
all clients reject old wire versions and unbound selected records. The Original
client uses the full JSON view. Independent bounded transport review found no
introduced defect; UBS is partial/legacy-scoped with no changed critical finding.

The first managed cold check exposed an actual shared Cave-music consumer defect:
both clients treated every non-Night map as Day track2. Native source/profile/XMI
already selects Day2/Night1/Cave3. The managed validator now matches all three,
retaining ABI/tick/generation/timer/hash/reserved guards and rejecting unknown maps.
Real Pav Cave track3/wrong2 rejection passes0.93s; the failed connected cold speech/
selected original glyph/O/repeated-read/cleanup check now passes39.54s. The earlier
ordinary-record check passes1.09s and was not repeated. The inactive Editor host
proves request/metadata/glyph/DSP-deadline/fade behavior, not active playback or
audible hardware output. Matching Original build-ibl36i6h/native dc5009d619f5 is
installed and --check-installed passes. Root viewed the actual cold Pav Cave scene,
original284 glyph panel on physical O hold, and its disappearance on release.
The active standalone logged exactly one TRACK26/4999ms request, Cave music track3,
and selected source32/record284; all30 resource roles and mapped native hash match,
zero retries, no disabled-source/integrity/load errors. Owned player/display stopped
without escalation. This is controlled cold-entry/UI/live-request proof, not earned
navigation, audible hardware, human combat/FPS, full missions or old saves.
Source is canonical ae63060c, pushed Original14283731; unrelated Modern ancestry
was excluded. Evidence: generated/secret-announcement-20261115. Full
mc-oi6.10/.6/.9 acceptance/dependencies stay open. Do not restart delivered secret
announcement or Cave-music admission diagnostics; continue the remaining original
help/text consumers, then installed Day/Night/Cave audio/presentation/performance.

Newest operator direction (2026-11-15): complete the source Shift+F5 display
control through HUD, original notice, help cleanup and ordinary F5 preservation.
The Astra diagnostic worker is stopped at Oliver's request; the installed route
passed with strict WAV hashes, while the historical corruption cause remains
unknown. QuietIsland owns the connected managed capability; BoldForge implements
the Rust state/replay/notification joins; root serializes builds/push/installation.
Source GAME saves retain the16-byte GameSettings block including minimap/topbar;
CONFIG.DAT does not. Use one accepted Rust carrier, the existing local notice
slot and source presentation clock. Preserve complete acceptance and all unrelated
work; no silent retry/hash weakening or isolated consumer handoffs.

Shared display-control batch is implemented and installed (2026-11-15): source
Shift+F5 and pause Options row12 now call the same guarded Rust operation. One
runtime-global minimap/topbar carrier survives mission/retry/map and GAME save
round trips without entering CONFIG/profile preferences. The existing local
participant notice slot receives source type3, replacing its prior notice;
the admitted PresentationFrame owns its50 ->0 ->-1 clock. Both Alt keys retain
source precedence; held-menu modifiers cannot leak a bare F5 save. Source Shift
dispatch precedes Ctrl spell-menu handling. Accepted hide closes ordinary help
only when showHelp permits it, and applies the source0x2ff minimap-off mask;
forced objective context and pause-hover help remain distinct. WEB/full map
remain visible independently. Source selects record365 on hide/366 on show;
the local English On/Off wording is counterintuitive and deliberately retained.

Five focused native checks pass: new and existing notice lifetimes, actual
session display/guards/reset/map+live save/cold receipts, CABI transitions, and
the existing fresh-help/reset boundary. One integrated workspace all-targets
check passes57.5s. Strict Rig native build passes48s; all626 captured source/
header/data inputs and returned bytes match. This is source hashing, not626tests.
Managed checks pass: five initial source/option/input regressions plus the one
connected paused/notice/cold/F5 check rerun145.96s after fixing the qualified
Original client's stale menu-action whitelist to the already published canonical
guard. Unknown actions still reject. The older client uses its full snapshot
getter in place of the canonical render-only getter; no compatibility wrapper or
guard relaxation was introduced. UBS is partial/legacy-scoped; changed-hunk
review finds no added critical source finding.

Matching Original build-3ct3fc6i/native
48d553ad3279f10fa10ec97114b984268201fec6f91f7be1429cdebf57bed490
is installed and --check-installed passes. Root viewed actual NightSpire left-
and right-Shift+F5 hide/show, source notices, the options row12 hide and retained
submenu, and ordinary F5 publication. No Shift action wrote a save; the actual
file records exactly three toggles. A fresh process restored its paused hidden
HUD, then physical Shift+F5/resume restored both HUD parts and F5 wrote the
four-toggle continuation. These are actual UI controls without flight/casting or
an automated playthrough; they do not establish human combat/FPS/hardware audio,
all-mission visual coverage or a WAV root-cause fix. Both owned players stopped.
Evidence is generated/help-20261115/icons-map-{native-checks-1,rig-proof},
icons-map-managed-{2,3} XML/logs and icons-map-installed-1/icons-map-cold-1.
Full existing settings/HUD/help acceptance remains open. Source SetKeysDefault
selector109 is still an absent control; ordinary key Apply/Cancel must not reset
display as a substitute. Remaining help/speech and earned intermediate Results
save -> cold restart -> next-mission carry proof remain as recorded below.

Pause-menu hover source is canonical `fc6d503a`, pushed/remote-verified Original
`1e0d98f1` on `codex/original-ending`; unrelated Modern ancestry was excluded.
Pause-menu hover help is implemented in the existing Original owners: actual
ReadPauseMenuEvents cursor/byte_aa -> rows86–90 -> selected source records/glyphs,
geometry/pointer -> leave/dialog/resume/reset cleanup. HSPR174–178 now retain
indexed planes and writer masks; disabled Load/Sound/Music rectangles apply the
source LUT twice before popup blending, including underlying-frame coverage.
Save/Options art stays unshaded. The real-menu component check passes7.031s with
all five owner texts/anchors, glyph pixels, the real Music150 -> LUT³57 pixel,
world-only negative, paused native identity/replay stability and lifecycle gates.
It declares a retained hash-qualified Night frame and a controlled GUI cursor;
it is not fresh-world or physical-mouse proof. EditorWindow Repaint substituted
the desktop cursor in the original test host; only that host input was corrected,
with production hit tests and pixel/lifecycle assertions unchanged. Owner-row/
receipt-tamper and Mana41 checks pass unchanged; all three existing shared GPU/
compositor regressions pass. Evidence: generated/help-20261115/pause-hover-unity-1,
-2,-3 and pause-hover-textbox-regression-1 XML/logs, pause-hover-menu-candidate-2
(all677 outputs independently rehashed). No Cargo/native change; ce48b7c0 native
retains606/606 matching inputs. This initial pause checkpoint did not install
the new C# player; the matching installation below supersedes that limit.
Full mc-oi6.6 acceptance
and its speech dependency remain open; do not claim sound is the only remaining
gameplay/presentation work. UBS is partial on C# flow/conditional budgets;
added profile-null warnings are ruled out by the non-null constructor-owned
dictionary, and changed-hunk review found no critical defect.

Wall-contact guidance is now implemented through the existing native JSON
observations and Original help tail: unmodified Level3 terrain material256 ->
actual Accelerate contact -> signed current-player counter -> source row3/record288
(Walls cannot be passed) -> original source glyph/frame -> full50-draw expiry.
The common bound-row scanner now tracks each row's own age/priority, selects the
highest priority then greatest positive overdue age with source-order ties, and
preserves pending objective precedence. Fresh review caught missing final-objective
masking; the corrected ordinary scan suppresses both3/41 after actual IsLevelEnd
while preserving forced80 and already selected popups. It reuses the exact existing
all_objectives_satisfied projection, not exit_sequence_active or a new timer.

Qualified Original actual contact/dry/reset check passes0.17s and emits real
full/render observations at ticks2/3/0 with counters1/0/0. The earlier added210-step
objective-expiry precondition failed because English/speech-enabled source startup
does not create an automatic textbox; the fixture now asserts actual initial and
contact textbox0 without inventing or shortening a timer. Its real geometry/pose,
ordinary movement, read stability, dry decrement and reset assertions remain.
The managed receipt-to-glyph check passes2.41s: actual full/native carrier validation,
selected owner text, literal W pixel232/94=index176, current-frame LUT blending,
expiry and paused native identity/replay stability. It declares controlled contact
sampling plus a separate native owner/retained Night frame, not continuous earned
travel, hardware input or installed Level3 visuals. Corrected source selection/
context precedence, original mana and both coherent-metadata-tamper checks pass1.39s.
All677 candidate outputs independently match receipts; prior15 source rows remain
unchanged. Rig workspace/all-target release check passes54.6s. Evidence lives in
`generated/help-20261115/wall-*` logs/XML, wall-contact-observation.json and
wall-help-menu-candidate-1. The existing Original JSON path is verified; separate
Modern MCPO binary publication is not extended or claimed qualified here.

Rust UBS reports whole-file legacy totals; no added critical, and added warnings
are assert/unwrap/create-only output in the focused test. C# UBS remains partial
on flow/conditional budgets; no added critical/warning. Fresh bounded source review
supports the fixed joins, not complete help certification.
Wall source is canonical `53e77024`, pushed/remote-verified Original `5059d5b7`.
The final source-composition check resolved the initial38 raw differences as one
materialized Git symlink plus37 effective differences:33 test-only paths, backend
tests/diagnostic wording, two missing live-class14 publication paths, and one file
omitting Modern-only observation modules. No new checkout or profile fork was
needed: the canonical Rust already matched the published Original tree. The
isolated Original project's generated Rust link now selects that canonical
workspace; both source trees remain preserved.

The build helper now accepts an explicit --native-source-root and uses that same
workspace for Cargo, header contracts and source capture. The receipt stores its
resolved root; subsequent native-only refresh and launch/check keep it rather
than silently returning to the old sibling. Legacy receipts/default builds retain
their documented behavior. The owned change is verified independently of the
uncommitted Modern player-retention work:20 clean-HEAD fixture tests pass3.61s,
22 tests with that separate work pass, and the exact older Original helper's two
source-root fixtures pass. Fixtures are mocked command/installation checks, not
live player proof. Stale menu-resource setup and duplicate-resource diagnostic
expectations were corrected against the existing production guard; rejection
checks stay intact. Source-root Cargo, mutation/header failure and payload
preservation checks remain strict.

The shared native `3bd7f66c38e9c42b638ece5775be67d3a45d0dc37efd0f0b8cc99b265018d7f9`
is built from canonical Rust. All626 captured source/header/data inputs match Rig;
this expands the older606-file subset to include the added ABI modules, headers,
TSV tables and fixture/data files actually captured by the helper. The existing
real class14 constructor -> live ABI observation -> hidden -> reset check passes
0.07s; it does not prove organic scroll acquisition. Its owner-package override
lets an offloaded test binary run against local owner data despite the embedded
worker manifest path, preserving the default and all original assertions. Both
pause-hover and actual wall-carrier managed checks pass7.17s with this exact
shared native in the isolated Editor. Build-helper source selection and --help
were exercised before the matching player installation below. Evidence:
source-root-*, public-original-* under generated/help-20261115.

The build helper's actual Cargo branch now defaults to the selected source
repository's disk-backed target, bypassing the retired workspace target symlink.
An explicit CARGO_TARGET_DIR remains authoritative; non-Git sources use the
project's Builds/native-target. Artifact reuse is unchanged. The clean-HEAD
20-case fixtures and exact older Original helper's two affected fixtures pass;
UBS has no critical finding and only existing unittest JSON warnings. These are
command/environment fixtures, not a real Cargo or player run.

The current shared native passed one full-boundary instrumented check112.71s:
160 WAV allocations matched their same-file bytes at read return, while protected
and before free, with no trapped write/mismatch. One subsequent normal
uninstrumented loader check passes103.01s with strict hashes retained. This
establishes operation of the current route, not a source-backed WAV correction:
the failing loader/audio code and lockfile are unchanged from the failing cohort.
Evidence: wav-integrity-boundary-1 and wav-integrity-normal-1 under
generated/help-20261115. Astra remains responsible for the unresolved cause and
next actual player qualification; installed build-af5fng7c is unchanged.

The matching normal player installation is recorded below. Unresolved integrity
cause, full tutorial/help/speech, selected non-ASCII installed proof and presentation/
performance acceptance remain open. The off_DB558 sentinel stream is credits;
do not invent a tutorial producer from it. Results-phase F5/cold restore is already
delivered below; do not restart that investigation. An earned intermediate Results
save -> cold restart -> next mission remains an evidence limit, distinct from the
installed controlled Results proof. Continue remaining real source guidance
without replaying delivered mission or spell/creature paths.

Source row1 now reaches the existing default-on help scanner and indexed
consumer: record286, "Press 'O' to show current objective", source age600,
priority2, anchor320/80 and50 draw lifetime. The new cache has677 verified
outputs;676 match the wall-help cache exactly. All six focused row1/mana/wall/
context/pause checks pass8.87s with shared native3bd7 and a clean Unity exit.
Literal owner P glyph, source geometry, precedence, expiry and native/render
read stability are checked; retained-frame component evidence is not installed
physical-input proof. A fresh independent source/gate review found no introduced
defect. UBS is partial for C# flow/taint budgets, with no added critical/warning.
The first Editor attempt compiled successfully then aborted in Mono domain
unload before loading libsim or running tests; the unchanged cold run passed.
No WAV or Mono root-cause correction is claimed. Shift+F5 display state remains
the next concrete settings join: source initializes minimap/topbar on, toggles
both, closes the popup on disable and masks affected ordinary help rows. It is
not persisted by the source CONFIG.DAT writer. Full help acceptance stays open.

Matching Original build-_qjz8bib is now installed with native3bd7f66c38e9,
canonical Rust source root,474 captured Rust/header/data inputs plus152 pinned
upstream inputs and210 qualified Unity inputs. Both installed plugin copies
match; --check-installed passes. Normal canonical startup with all30 resources
reached title -> source NewGame -> map -> one portal -> NightSpire terrain/HUD
on its first attempt, zero retries. Root viewed the actual original-font row1
popup in the scene, without forcing its selection or clock. Pause/wall code is
in this matching player; their controlled component proofs remain distinct.
Strict WAV guards remain; no failure capture was produced. Frontend audio,
speech and music Play-request logs are present, not hardware-output proof.
The prior intermittent WAV/Mono cause is unresolved, not fixed by this run.
Owned smoke processes are stopped. Evidence: generated/help-20261115/
wav-integrity-player-1 and objective-key-player-install-proof.json.

Operator-authorized cleanup retired ten unused Linux-before-build player
payloads, retaining compact receipts/current installation/active candidate.
About1GiB was removed and measured root headroom is28GiB. Cargo clean refused
the retired native target's missing cache marker; DCG separately blocked its
recursive deletion. That1.8GiB cache remains intact; no guard was bypassed.

Continue the existing all-30-mission outcome `mc-xa7.1`; the shared families
below are delivered checkpoints, not a reason to restart them. Original
gameplay remains separate from the Modern/Voxel asset work above.

All25 ordinary objective/exit paths and all five secret mission bodies now
have controlled connected passes. Jathnar23's strict final82 passes717.33s:
real Hydra/feature4/seven rival Castle-loss and banishment paths, remaining
Wyvern/Cymmerian deaths/splits/reclaim, paid Possession/current sphere merges/
collector/Castle credit necessary for65%, all seven goals and normal exit0A.
All five original parent-entry/secret-body/return/carry paths also pass below.
These use declared frontier, incoming inventory, prior bank, arrived contacts
and poses; they do not prove earned campaign progression, normal movement,
native all-mission visuals, human combat, FPS, hardware audio or old saves.
Full `mc-xa7.1`, opening and secret/carry acceptance remain open. Jathnar's
additions are cfg(test)-only; the installed runtime/native below is unchanged.

Oliver has now authorized parallel completion of campaign continuity, Original
frontend/controls and installed presentation/performance. The current native
team is VioletAnchor (`mission_revealed_way`) on `mc-zdo.6` cold-save/restore,
SilentBrook (`mission_night_spire`) on `mc-5r5.2` persisted bindings and dialog
input lifetime, and Sol/xhigh BoldForge (`original_integration`) on shared
client joins and `mc-oi6.8` source audio consumption. Root retains sole
Cargo/RCH/Unity build dispatch, acceptance, push and installation. Former
LavenderBeacon/GoldMoose registrations are confirmed retired; do not revive.
Full dependency/acceptance remains open; these are bounded connected slices.

2026-11-15 operator update: use ONE team and finish one connected capability
through integrated verification, commit/push and installation before starting
the next. Keep the current two Terra/xhigh workers and Sol/xhigh integrator;
root clears the existing integration queue and serializes all builds/delivery.
First finish the preserved `mc-oi6.6` objective glyph/textbox slice below. Other
points remain queued; no second team, broad audit or new planning round.

First coherent candidate joins semantic carry validation BEFORE campaign-save
publication; original accepted punctuation bindings and held-input release
barriers across settings close/resume; and actual source play-type4 audio
admission, ordering and finite-voice stop/completion. Source47/49 are finite
voices: a blanket per-loop120Hz change is not supported by pinned source.
Focused Rust map tampering/atomicity, stale guards, persisted preferences,
two-entry progression, paused replay, actual corridor audio and native finite
voice/arbitration/generation checks pass. Strict Rig workspace check passes;
native `15063781441a96dd5a7ceddbe3134df4e8c31ff75022c4451a705eb059c1a269`
has606/606 matching qualified source inputs. Final player `build-9izvzfji` is
installed, check-installed passes and root viewed normal map/loading/NightSpire
terrain/HUD, one portal click/zero retries, then stopped its owned processes
without escalation. Code is canonical `8ac701df`, pushed/remote-verified Original
`d9398768` on `codex/original-ending`; unrelated Modern ancestry was excluded.
This is launch/load, not human gameplay, hardware audio or FPS. Evidence:
`continuity-input-audio-qualification.json`, `continuity-input-audio-native-*.json`,
`continuity-input-audio-unity-11.xml` and `continuity-input-audio-final-installed-smoke/`
under `research/private/generated/campaign-original-environments/`.

The deeper current-client check found omitted already-committed preference/music
generation joins in the isolated Original client. Those exact canonical hunks
are now integrated there: Playing and Paused saves cold-load correctly; invalid
Load preserves identity, hash, preferences and owner count; resume advances one
tick. Final Restart exposed a stale managed snapshot captured before actual
primary-frame visibility publication. Real initial/retried replay hashes match
exactly. The client now refreshes that observation before audio consumption;
the complete Night NewGame/cast/Playing save/cold load/Paused save/invalid load/
cold restore/resume/Restart check passes34.46s with its original exact hash and
presentation assertions. Real GUI events require graphics; `-nographics` does
not draw the test's map window. The check now uses the normal launcher's exact
loading/menu/media roles; no validator was relaxed. The final installed player
contains these joins. Temporary root diagnostic writes were removed after
preserving their decisive initial/retried replay evidence; no Rust reset rewrite.

An actual source4/secret30 native objective check reproduced a separate managed
refusal of all source levels>=4. The focused default-language resolver now uses
the pinned25-entry ordinary tables and real secret30..34 rules. Both actual
native cases and prior-first-four/boundary/end regressions pass. This does not
complete generic original glyph/textbox composition, selected locale, tutorial
recursion or speech lifetime. Those full criteria, Results-phase save input and
Day/Night/Cave performance remain open. No all30 rerun or automated playthrough.
Preserve unrelated Modern/Voxel dirty work and its installed player.

The current connected `mc-oi6.6` slice is actual source glyph/textbox composition
over the current Day/Night/Cave indexed world. SilentBrook owns the canonical
compositor and its focused instance/shader consumer through integration;
VioletAnchor owns the existing menu-cache producer/typed loader for original
glyph/frame/profile data; BoldForge owns the runtime and menu forwarding joins.
Their narrow reservations and actual interfaces, not new planning, govern edits.
Root retains serialized builds, verification, candidate-cache promotion and
installation. Default-language selection passing alone does not close this.

2026-11-15 integrated qualification now passes with native
`9cca38e043d011b02114606425a26a4153c473f4e49193e180bf9ff76f8970e4`.
Actual source4 Day and secret30 Cave objectives reach original font/frame pixels
over the real composed indexed scene. The six retained source raster oracles,
GPU current-scene/map orientation/generation/lifetime/caller-target controls,
and full Night cast/save/cold-load/invalid-load/resume/retry/map/objective-clock/
speech/Title-disposal path pass (Night47.64s). The native material-root direct
open/frame/wrong-cache regression passes5.10s. Both source catalog guards and
the original strict hash/pixel assertions remain intact.

The native open route had omitted explicit material-only caches from canonical
owner binding, while loading those same assets into its renderer. It now binds
the explicit root; gameplay-component precedence and no-cache behavior remain
unchanged. A leaked active render target and missing declared shader properties
are fixed. The qualified Original runtime retains its existing full PausedFrame
observation API; no unrelated Modern optimization API was ported. Strict Rig
build passes56s; actual606/606 source hashes match the qualified snapshot with
only the owned CABI file changed from150637. The new stable generated menu-cache
alias selects the validated candidate without replacing prior cache data.
Player `build-m1qu78zm` is installed and check-installed passes. First normal
startup rejected the stable menu-cache symlink root under the unchanged strict
file guard. The launcher now resolves its default pointer before passing the
real directory; explicit overrides and file/hash guards remain unchanged. That
failed smoke is retained and was stopped without escalation. The corrected
normal all30-resource map/loading/NightSpire smoke passes with matching mapped
native, one portal click and zero load retries. Root viewed actual original
glyph/frame objective display while O was held and its removal after release;
no movement/cast input, combat, playthrough or FPS claim. Owned processes stopped
without escalation. Previous cache data and unrelated Modern work are untouched.
Implementation is committed as canonical `bdc35444` and pushed/remote-verified
Original `77e6732b` on `codex/original-ending`; installed native/player and
qualified source receipts above match this delivered capability. The next
remaining text-family work is selected-locale and tutorial/help consumption;
keep one team and finish each connected path before taking another.
Evidence: `research/private/generated/objective-glyph-20261115/`. This proves
the default-language objective capability, not selected locale, recursive help,
full campaign play, FPS or hardware audio. Keep full `mc-oi6.6` acceptance open.
All full campaign/frontend/audio/performance acceptance stays open. Static UBS
is partial for C# conditional/callable effects; the temporary-directory Guid
heuristic was a triaged false positive, with no critical changed-hunk defect
identified. DCG rejected removal of the two superseded generated Original player
folders (about203MiB total); both remain intact, without an override or retry.

Selected-source language delivery is now installed (2026-11-15): canonical
`05dfae71` plus viewport fix `80e1c63f`, pushed/remote-verified Original
`2f2accb1` on `codex/original-ending`. Player `build-js92rf9x` uses native
`aeb2c940ebb912e7507a331093c667c9911918f44940f6cfc36f6f2fad59a689`;
all606 qualified Rust/upstream inputs match Rig. This is the qualified Original
snapshot and owned deltas, not every unrelated canonical Modern source byte.
The actual GOG D2/L1 files are identical: one genuine English content identity,
no available non-ASCII records or second-language proof. Do not ask for another
pack, fabricate one or block other work on it.

The original recipe12 background, selected-pack flag and recipe12 cursor37 now
reach the real Language button/page. Settings share the title's fitted640x480
source viewport and the language page replaces its background opaquely. Native
checked install/apply compares the complete managed-preflight binding before
any resource/profile publication. Both managed preference clones preserve the
nullable default locale; Unity's null-to-empty clone had made the page ignore
Return. Unknown IDs remain rejected. Current map/objective/dialog consumers
share one retained source record/font binding.

Actual-owner native locale/cold-profile/valid-replacement/missing-font checks
pass; managed bad-font/default-null/cold-owner pass; final fullNight language
button/cancel/confirm/NewGame/cast/save/cold-load/invalid-load/resume/retry/
objective-clock/speech/Title check passes35.52s and key capture passes. The GUI
fixture now uses its actual viewport/coordinate transform and normal source
50ms media input-release frame; original assertions and timers were retained.
Root viewed installed source language/cursor/flag, cold key-dialog text, map and
live objective after full process restarts, with retained binding and matching
mapped native. Normal portal load used one click/zero retries; all owned smokes
stopped without escalation. Final fitted page has opaque sidebars; actual owner
art, source font metrics and pixel rendering remain in use. No replacement art
or automated mission playthrough was used; FPS/hardware audio remain unverified.
Evidence: `research/private/generated/locale-20261115/`. `mc-5r5.5` is closed
on its original available-content acceptance. Full frontend/help/subtitle
switching remains open in `mc-5r5.2`/`mc-oi6.6`; continuity and presentation/
performance remain unfinished. UBS is partial for C# callable/conditional/
dataflow analysis, with no critical changed-hunk defect identified. Failed
fixture/resource-path runs and the delayed attract-mode capture are retained;
no test, file/hash guard, source timer or reference raster was weakened.

Continue Oliver's points1–3 with the same one team, one connected capability
at a time. Reuse the delivered language and prior controlled mission/ending
evidence; do not restart all30 mission passes or run adaptive playthroughs.
Existing full acceptance/dependencies stay intact and unfinished beads stay open.

Results/map and named-slot continuity is now committed and installed (2026-11-15):
canonical `20911664`, shared carry/catalog correction `3579a38c`, HUD consumer
`68c95f94` and common cold attachment `b4a21d05`; Original-only remote
`2a21d23b` is pushed/verified. The qualified Original player is `build-af5fng7c`,
native `ed42fbf3c3f9cdd10e5b2ddd6843f89ccd627689f259655285eb42c385c65545`.
Strict Rig native build passes2m30s; all606 qualified inputs match. Four scoped
production Rust files differ from the prior qualified locale snapshot; extra
canonical backend tests are not a whole-tree parity claim.

Campaign F5 now retains render-frame edges through fixed dispatch, including
stationary Results whose source map is intentionally not Ready. Source dialogs
retain selection/draft after failed Save/Load; corrupt headers remain visible
without modifying the slot. Actual installed Uluth Results F5 writes the exact
controlled source packet, survives full process restart, and dismisses to a map
retaining all25 completions, statistics, full carry and CUT6 history. The actual
completed Ommosyth return cold-loads its Ammyridia map, retaining its frontier,
secret statistics and seven-family carry. Managed validation now matches Rust's
progressed-null-completed map projection; malformed frontier/briefing/phase and
unavailable/discovered-secret portal controls still reject.

Actual installed named Save, failed write, invalid-header Load, Cancel and valid
Load preserve the authoritative owner and prior file. Root viewed the reopened
original dialogs and error text. The completed-phase/cold-owner check passes
155.58s; full modal recovery passes17.93s. The Editor fixture requests ordinary
player-loop updates between yields; source animation clocks/timers are unchanged.

Real parent entry exposed a shared carried-spell rebuild dropping its validated
TMAP catalog. The candidate now reinitializes that immutable catalog's residency
on the unpublished rebuilt world, preserving source preload and rebase order.
Actual Ommosyth completion -> Ammyridia frame/feedback -> stale rejection -> cold
replay and simulation reset passes21.88s; independent bounded review found no
concrete defect. Full Night cast/save/cold-load/invalid-load/resume/retry passes
38.27s. A redundant managed HUD Levels1–3 restriction is removed after the
existing native full-registry/live-owner gate; actual2/3/20 HUDs plus invalid
selection/tick/generation controls pass. The installed one-click Ammyridia portal
now reaches real Day scene/HUD and a paused save with exact inherited carry,
retained map/secret history and suppressed repeat rewards, with zero load retries.

That actual paused source20 save now cold-attaches through the same typed
environment composer as ordinary loading, including registered secret identities.
The existing exact active-entry observation admits Playing or Paused; its
runtime/session/definition/source/artifact guards and JSON ABI are unchanged.
Native binding checks confirm observational reads preserve identity/replay bytes
and reject stale/Title/Loading/map requests. The old retained semantic Night route
is confined to its exact source identity and entirely undeclared presentation
configuration; declared-but-mismatched resources still fail. No neutral asset,
schema fallback, mission patch or new API was introduced.

Current map/Results/secret checks pass122.10s; focused actual parent cold scene/HUD,
exact paused F5, normal Load/new owner, carry/history/reward preservation and
one-tick resume pass108.62s. Unchanged full Night passes35.14s. These two connected
checks were separated after the expanded single method reached Unity's180s limit;
every assertion and the timeout remain. The live fixture now reads the normal
launcher's actual presentation roles: source1/2 differ from their gameplay roots.
A dynamic-source extraction was rejected by DCG and abandoned; static literal
metadata parsing supplied the fixture without executing the provider or changing
source/owner data. Original schema/hash gates rejected the incorrect fixture.

Root viewed the matching standalone cold paused Ammyridia scene/HUD and original
pause/objective text, then physically saved at tick21 with exact inherited carry,
progression, retained map/secret state and reward policy. Terrain/effect hashes
remain equal. Recorded paused outer frames advance source palette/global RNG/UI
phases, so the long-running standalone save is not byte-identical to its input;
no gameplay step was appended. Its owned smoke stopped without escalation.
This proves the current controlled ordinary/secret-return continuity slice,
not every save/media/mission acceptance. Full `mc-zdo.6`/`.4`/`.3` stay open.

NEXT: same one team on the remaining original help/settings/text consumers, then
installed Day/Night/Cave audio/presentation/performance. Root owns serialized
checks/push/install. Use actual source producers and current selected-record/font
bindings; do not restart delivered save/carry/catalog/HUD diagnostics or the
all30 controlled mission passes. Preserve complete original acceptance.

Prior help/narration source checkpoint is canonical `47ddb6ab`, Original branch
`941332df`; it is NOT installed. The real unaffordable Volcano
path now uses source help eligibility, typed assignment closure, selected GOG
record 329 and indexed original frame/font rendering. Help preference changes are
receipt-covered in replay v8. Normal objective timing/narration no longer stops
after the first four missions; all 25 ordinary source rows and normal secret
aliases are bound, with all 27 available GOG tracks verified. No other installed
language is available; L1/D2 are identical English, not non-ASCII proof.
Focused native checks and workspace pass; qualified native `0819741810f6` has
606/606 matching Rig inputs. Unity checks reached actual map briefing track 21,
Ammyridia objective track 20, O suppression/release, real hint pixels, natural expiry,
reset and same-value assignment closure. Final live-selection/cold-load check
is still pending: replay entry carry correctly remains Possession, while live
Volcano selection belongs to replayed commands. Two test oracles were corrected:
logical 640x480 coordinates must sample actual 320x200 scene UVs, and the complete
event interval must be read rather than the recent 256 HUD rows. The actual
blend region is non-discriminating against a flat substitute; do not claim that
negative passed. Final Unity check 10 failed before gameplay on a real environment
integrity mismatch. Its create-new diagnostic captured the exact rejected buffer:
77,889,580 bytes, offset 32,096,805 is `0xfc` instead of disk `0xfd` (one differing
byte). Python confirms captured SHA `692223b83edc` and disk SHA `03b3f9510485`;
the native digest agrees. This is a real read-buffer difference, not merely a
bad checksum report. Cause remains unknown; do not infer hardware failure,
overwrite owner data, weaken hashes or run retry/build loops. Evidence is
`unity-help-10-failed-read.bin` and `unity10-failed-read-comparison.json` in the
batch directory. All agents are frozen/completed; installation and expensive
qualification are held pending a concrete safe integrity resolution. The last
installed player remains `build-af5fng7c` with native `ed42fbf3c3f9`.
Special switch 31 announcements,
objective-expiry context help and broader recursive help remain open. Evidence:
`research/private/generated/help-20261115/`. Full `.6`/`.10` stay open.

The bounded read-integrity investigation has now stopped without identifying a
cause. Plain native frontend/map-load passes; private allocation pattern checks
found no error. A single-byte difference was also captured in C2GAME2 (offset
29,377,061, `0xf6` instead of disk `0xf7`). Both differing offsets are 549 modulo
4096; that observation does not identify hardware or a writer. A hash-boundary
probe observed 160 matching input/result hashes but hit the unchanged test timeout.
A lower-overhead diagnostic protected each exact native WAV allocation's interior
pages until its real free: all 160 protects/releases succeeded, no protected CPU
write was caught, and the full hint/selection/v8 cold-load test passed in 96.53s.
The guard is ignored diagnostic code, never a shipping workaround. GDB then
stopped on ordinary SIG36 during shutdown after NUnit wrote its passing result;
owned processes are stopped. Actual native GOT bindings for read/malloc/free/
realloc all resolve to glibc. These are bounded non-reproductions, not an integrity
fix or installed-client qualification. Original installation remains unchanged.
Evidence: `wav-guard-1.ranges`, `unity-wav-guard-1.xml`, `native-binding.json` in
the same help batch directory. The first binding probe lacked the plugin symbol
path and was stopped; the corrected binding-only probe stopped before gameplay.

Continue the SAME full help capability rather than extending diagnostics or
starting another family: BoldForge now owns the missing natural objective
textbox 1→0 `sub_88BA0` context-help72–80 producer and shared joins; VioletAnchor
owns actual selected source metadata/assets, SilentBrook the existing original
indexed presenter. Require actual narrow grants and a coordinated interface.
The source still assumes help-disabled at the expiry edge despite the delivered
configurable preference. Finish the real enabled path with natural timing,
paused/query/reset negatives and selected glyph/frame consumption. Root owns
serialized checks and delivery. Installation and full acceptance stay open;
no claim that byte corruption was fixed. Preserve original criteria and the
separate queued secret announcements/recursive-help work.

Current context-help source is canonical `d5b05613`, pushed Original `1a07d37e`
on `codex/original-ending` (remote verified). It is NOT installed. The batch ports
`sub_88BA0` stage selection/KeepPending to the existing ordered effect path,
then source pending selection, eight-pass delay, selected-language rows334–342,
original type1 top-centred frame/glyphs, fifty draws and cleanup. Hint41 shares
the validated metadata rather than a parallel guessed table. The real ordinary
native200-frame expiry check passes4.11s for help on/off and Playing/Paused,
plus pinned mapping, legacy source-order and foreign-text cadence controls.
A legacy source20 regression never entered runtime because its compiled RCH
absolute owner-package path is absent locally; retain that coverage limit.
The actual new647-output cache receipts match independently. Strict Rig native
`ce48b7c0a0828399acdf1777fa9c14be85950b809b3d14a9b12dc3b802502db8`
and all606inputs match the qualified Original snapshot. It is in the Editor
plugin only; the installed player remains `build-af5fng7c`/`ed42fbf3c3f9`.

The popup cadence and rehashed-metadata rejection checks pass uninstrumented.
The full actual O-release→row76→GOG338→current-scene glyph pixel→all50draws→
clear→existing mana41/V8cold replay case passes104.64s WITH the ignored read
sampler/protection probe. This is conditional behavior verification, not normal
client qualification or an integrity fix. Corrected test setup: English speech
correctly has no automatic objective textbox; actual O release exercises its
expiry, while the native speech-disabled control retains all200natural frames.
The missing-metadata negative now uses the previously admitted current locale
cache, rather than an ancient cache rejected earlier for missing language data.

Uninstrumented qualification again captured Game2's same6d73 SHA/one-bit change,
offset29,377,061 (`f6`vs`f7`), from another real cache root. A guarded run captured
Game1 source31 SHA522e7841524a: offset63,554,085 (`fa`vs`fb`), also549mod4096,
inside the successfully protected allocation, with no observed SEGV and57paired
protect/free releases. Do not infer a writer or continuous protection from that
alone. A subsequent bounded volatile pre/post-arm page549 sampler found no
difference across160reads; it is non-reproduction and covers only sampled bytes.
A standalone32GiB private allocation passed four patterns in29.75s; it does not
clear all RAM or establish a hardware cause. All probes are stopped; source/data
and installed player are intact. No further unchanged retries or broad diagnosis.
Preserve captures/comparisons and `context-*` evidence in the help batch directory.
Staged11-file UBS is PARTIAL for C# callable/conditional/flow budgets; none
of its14critical whole-file findings is on an added line. Existing Guid temporary
path/public-source-hash/secret-level-index false positives were triaged, not
suppressed. This is not a clean static-analysis claim. Formatting debt remains.
All teammates are frozen pending actual consumer failure; root owns landing.
Full `.6`/`.10`, recursive help, special announcements and installed qualification
remain OPEN. Installation stays held on the reproduced integrity blocker.

Evidence is `research/private/generated/continuity-20261115/`. Source completions
use declared frontier/inventory/placement controls; this is not earned human
campaign progress, human combat, hardware audio, FPS, full missions or old live
save proof. All owned cold smokes are stopped; no retries were sent. Failed
opening-only guards and fixture runs are retained. Two old presentation controls
failed on incompatible legacy cache/layout assumptions; their assertions and
owner data were not weakened or changed. UBS is partial for C# and reports legacy
whole-file Rust debt, with no added-hunk critical defect found in these changes.

Previous committed Original runtime is canonical `ef69ab51`, remote Original `ee9a7ee1`;
native `29688fd01c0303a2155d3ef29338662119f9fe1c753b23c18398e23edce1237e`
was the preceding installation. Learned spell ownership now survives physical-holder release
and reclaim through the source collection consumer; selection/casting still
needs a current holder. The real death→drop/expiry/reclaim→semantic carry→
next-entry holder/reset check passes45.87s, existing progression/two-entry/save
passes21.04s, strict Rig workspace all-target check passes31.7s. Independent
bounded source/gate review supports this fix; old v3 fixture preflight failures
are retained below. Strict native build passes151s with606/606 snapshot inputs
matching Rig, native-only installation/check-installed pass. Root viewed the
normal canonical Original map/loading/NightSpire terrain/HUD with that mapped
native, one click and zero retries, then stopped owned smoke. Evidence:
`learned-carry-installed-smoke/runtime-proof.json`, `learned-carry-native-*.json/log`
and `learned-carry-72-qualification.json` in the mission evidence directory.
This is launch/load plus a controlled continuation check, not human combat,
earned campaign, hardware audio, FPS, full mission play or old-save proof.

Previous Original runtime is canonical `50174e74`, delivered on
`codex/original-ending` as `d216723b`; native
`2d3d6c94e7a4a5ea02fdff79dfef26389ac0c40693be376d6a521fc21bbe13ae`
is installed. Reset Dragons now join their canonical typed root/body callbacks
without reconstruction, preserving current packets, pending contacts, original
generations, allocator, map links and RNG. Typed Dragon bodies now also reach
their real particle/presentation consumer, including authored Dragon bodies.
Exact source4 adoption/presentation check63 passes0.51s; nine affected Dragon
checks pass5.20s, source-order passes, strict all-target workspace check45.1s.
Actual original T'Klom objectives→five reset Dragon deaths/absence gate→whole
group6→Karakir completion→CUT1→cold parent carry passes223.86s on63. Frontier,
incoming inventory, contacts and admitted arrival pose remain declared controls;
this is not earned campaign or human movement proof. Strict Rig native build
passes174.9s with606/606 inventoried snapshot inputs matching the worker.
The qualified Original snapshot retains its separate source closure; canonical
test-only additions and unrelated Modern native work are not called installed.
Normal canonical launcher registered all30 resources and reached
map/loading/NightSpire terrain/HUD with the exact mapped native, one portal
click and zero retries. Root viewed the scene and stopped its owned processes.
Qualified installation receipt matches471 Rust/manifest inputs,200 Unity
inputs and142 player files. Evidence: `dragon-installed-smoke/runtime-proof.json`
and `dragon-native-*.json/log` under the mission evidence directory below.

Previous runtime `a403c3de`/`58a092e8` and native `c721b455143500eb2`
delivered these shared fixes: boss SEARCH preserves source cumulative class-prefix
iteration. Castle feature4 final loss writes the source current nonzero A4
address, including a freed physical row; it no longer rejects that valid source
case through a live spell-type filter. The earlier actual Uluth10/25 field and
Steal Mana/Duel pitch/roll/FOV collision fixes remain installed. Strict Rig native
build passed170.4s; source-order, Castle loss/A4 and boss approach checks pass.
Full Uluth46 passes284.32s: actual reset/waves/BUILD68→69→72→73, genuine300000HP
boss, real contact consumption, natural phases/death/reclaim, both goals and
living exit0A. Declared controls are safe player poses, renderer observations
at source visibility boundaries and arrived contacts, without HP/AI/timer edits.

Earlier `22a73da0` fixed the shared spell-acquisition cache to follow the current
generation/family after slot reuse. Cross-family and same-family replacement,
actual Invisible/Bee death/drop/expiry/reuse and source-order checks pass.
Cresidan21, Hodor22, Gleph18 and Malak24 connected objective/wave/cleanup/exit
checks pass, alongside the previously delivered ordinary missions. Evidence:
`research/private/generated/campaign-original-environments/`, particularly
`*-connected-30.log`, Gleph/Malak `*-connected-33.log` and
`spell-reuse-installed-smoke/runtime-proof.json`; the latest native smoke is
`dragon-installed-smoke/runtime-proof.json`.

The normal installed Original launcher registered all30 packages and reached
map/loading/NightSpire terrain/HUD with the matching mapped native, one portal
click and zero retries. Root viewed the captures and stopped the owned smoke.
This is launch/load plus controlled mission checks, not human combat, FPS,
hardware audio, an uncontrolled campaign playthrough or old-save proof.

Karakir/source30 now passes actual area-entry production, Firefly-derived mana,
paid Possession, collector credit crossing85, normal exit0A and restored
parent-continuation exit1A through real return/carry/reopen. Checks37 passed
22.61s/64.84s; local `91429dd9`, remote Original `9c4c5e45`. These are test-only
joins; no extra runtime change was needed. Prior Castle/below85 bank,
incoming carry, selected contact/poses and parent-frontier fixture are declared
controls, not earned prior campaign or live secret-portal proof.

Ymbul/source31 passes116.11s on43: real Skeleton/Archer absence chains, all six
new transports moving the actual player, paid mana crossing30 and living exit0A.
Local `b87de26b`, remote Original `bb44dcbe`, test-only. Source provenance,
full corpse/reclaim and same-generation living blocker controls are retained.

Ymbul now also passes actual retained-parent completion1A, exact mission
statistics, cold map-save restore and parent reopen with the final carry:
242.34s on49. Pav Durivium/source32 passes229.66s on52 through real Cave
opening/sealing/collision, all twelve lowering rings and Magic Mine acquisition,
Fire Spheres, eight Cymmerian roots/splits/reclaim, actual paid Possession,
collector credit crossing40 and living exit0A. Its retained-parent1A/statistics/
coldsave/carry/reopen passes356.77s. Local `bf758f1b`, remote Original `5dcdfcf5`.
The shared paid delivery extraction retains Karakir52 standalone29.46s and
parent82.34s regressions. These are test-only additions; native above remains
current. The parent frontier, incoming Possession and below-goal bank are
declared controls, not earned campaign progression or live portal entry.

Pav's source-selected collision can legitimately claim neighbouring spheres.
Its gate now follows the actual generation-bound paid projectile→impact→
owner mailbox→witnessed source-wave sphere→collector→exact credited payload.
A zero-strength tier0 pulse is valid for an unprotected sphere; payment is
independently required. No source rule, goal, HP, AI or timer was changed to
accept those outcomes. Changed-source UBS findings are triaged test failure
panics and lexical secret-mission/authentication false positives; no new
production defect was shown by those findings.

Beleem/source33 mandatory path55 passes51.36s through the real Sentinel absence,
source transports, nested27-root wave/one-survivor negative, actual group14
sphere production/settlement, paid claim/collector crossing45 and living exit.
Its actual retained-parent/statistics/coldsave/carry/reopen passes173.77s.
Local `3dbb9d4e`, remote Original `c2e91d03`. A cfg(test)-only clone prediction
reuses the actual holder constructor, source lists, frame and acquisition
kernel to choose the fixed shot; a live-state hash check proves prediction
does not alter the live world, and actual acquisition/claim/credit still must
pass. No production factoring or native refresh was needed. Optional source
groups3/7/11–13 now pass70.59s and the expanded parent path199.58s on57:
fourteen Fireflies, thirty-two Skeletons and three linked model22 roots reach
their real callbacks, sprite descriptors and selected contact consequences.
Local `155b7c83`, remote Original `17a0f1b6`; these are test-only checks.

Ommosyth/source34 passes74.53s on61: actual223-row group23 production, real
model58 child reclaim, Sentinel death/mana, paid25% crossing and group27 living
exit. Its retained-parent/statistics/coldsave/carry/reopen passes184.28s.
Local `f5056075`, remote Original `07c6cb3b`. Incoming Possession, poses and a
prior below-goal bank are declared controls. Actual source model58 retains its
terminal controller; only its child reclaims. Source allocation refusal while
the arena is full is retained. Optional Spider/Worm/BUILD51 consumers and
earned campaign/live portal proof are not claimed by these test-only additions.

Controlled objective/exit paths now pass for24/25 ordinary and all five secret
missions, with all five fixture-entered retained-parent return paths. Actual
unmodified T'Klom5→Karakir30 entry exposed a shared reset Dragon scheduling
gap: imported roots and their current linked bodies never joined the canonical
typed Dragon callbacks. The first focused gate confirmed state/mailbox/
provenance preservation but caught a second shared defect: typed Dragon bodies
were missing from presentation DTOs. Both joins and the real source4 portal
route are now delivered above. Ul'Buthnen/source7/external8→Ymbul31 now also
passes264.85s on66 through all actual objectives, area314, group31's House315,
BUILD23→24→28 destruction and exact group32, real portal/controller entry,
paid secret completion and cold parent return/carry. The ordinary-exit
regression on the current whole package passes7.67s through actual guarded
GameSession advancement, including a same-generation living Spider negative.

This found a real content closure defect: the old Level8 MCI3v2 export contained
BUILD23 but omitted its24/28 successors. A fresh unmodified whole export and
qualified2100-output environment now occupy the normal Original launcher
entry8. Local `e62c37cb`, remote Original `66824591`. Package hash is
`85636e26b079c1c62f566bfedb86bd5342b53fd2b570dcf4bdf1a56aaaf9665d`;
cache hash is `308267e2e2adfb0b9f1a313d416bbdfc22753d7d7d45596af6d99c34dcdc12bf`.
All160 current exporter input hashes and the existing embedded byte-exact
inventory/marker match the executed binary. Structural/package and cache
validation pass. The old package/cache remain untouched. MCP1 players v4→v5
and collision bank v2→v4 change encoding/closure; raw byte differences are not
called changes to source gameplay. The tested current package was selected by
override; the standalone default now names that same package.

Actual installed Original native2d3d opens and destroys the new package/cache
successfully0.827s; the normal30-resource launcher routing check passes0.243s.
No Unity/native rebuild is needed for this content-only follow-through, and
no native Level8 rendering or human-play claim is made. Proofs are under
`ul-buthnen-source-closure-20261114/` and mission `*-current-package-66.log`.
Three of the five original parent entry paths are now exercised with declared
frontier, inventory, arrived contacts and poses; they are not earned campaign
or normal player-movement proof. Jaleen12/source11→Pav Durivium32 passes364.00s
on68: original initial area722→complete group58→area727→complete group44,
actual portal/controller1A→Pav's real mission0A→statistics/cold parent/carry.
The initial area is a unique current imported source actor, not a later typed
birth; the original67 gate wrongly assumed the latter and failed before
production. No engine behavior or gate assertion was weakened to fix it.
Jaleen's source entrance is independent of mission objectives, so no invented
objective prerequisite or duplicate objective helper was introduced. Local
`b46eb684`, remote Original `6d48f96a`; test-only, native2d3d remains current.
The now-default qualified Level8 ordinary regression also passes7.20s on67.
Gleph18→Beleem33 now passes293.15s on70: actual four-objective completion and
rival Castle5→1 loss/banishment, exact group20→House372's BUILD27→41→49→
group21→original source portal/controller→real secret completion→cold parent
and carry. The ordinary regression passes98.75s on69. BUILD27 has138000 HP;
the initial69 gate sent only one65535 contact and failed on a still-living
House. Corrected contacts wait for each real source mailbox to be consumed,
without changing HP, damage rules, stages or lifetimes. Local `b03a641e`,
remote Original `d94bff4d`; test-only, native2d3d unchanged. Four original
parent entries now pass in70. Ammyridia20/source19→Ommosyth34 now passes52.15s
on74 using the existing release profile: real Wyvern absence/Hydra death→
whole group7→House24 BUILD43→44→45→whole group8 controller25/portal26→
secret completion→statistics/cold parent/carry. Contacts repeat after the
actual mailbox clears; this is not an independent exact contact-count census.
The ordinary regression passes185.19s on72. Its old MCI3v2 package lacked
BUILD44/45; the fresh unmodified whole MCI3v4 package and2172-output qualified
cache under `ammyridia-source-closure-20261114/` now occupy normal launcher
entry20. Actual installed native29688f opens/destroys them0.879s; normal
30-resource/default-new-game routing passes0.222s. Local `fab8e3c3`, remote
Original `b33b0f9b`, no Unity/native rebuild for this content/test-only batch.
All five original parent-entry paths now have controlled connected proofs,
in addition to the five fixture-parent paths. Declared frontier, inventory,
arrived contacts and poses are not normal movement or earned campaign proof.
The full `mc-6qf.4` acceptance remains open.

Fresh bounded post-fix source/receipt review at `e7e8ab87` found no further
concrete defect in reset Dragon adoption/schedule/presentation, controlled
T'Klom→Karakir or Ul'Buthnen→Ymbul paths. Inspected revision/hashes and
uninspected earned movement/native Level8/full-campaign scope remain explicit;
that review is not a full-engine or human-play certification.

Jathnar23's earlier diagnostic sequence is retained here. Run56 proved early real stage4
Hydra birth/lifecycle and all seven Castle losses, then failed4503.25s at final
Possession carry application because the selected family had no valid local
holder. Candidate64 puts the declared carry at the actual tick0 destination
materialization boundary; the focused current-holder/schedule check passes
9.95s. The full mission64 failed4839.15s at its final physical-holder check:
family1 still refers to sentinel1 but has no current holder (124 free slots).
It proved the earlier Hydra and all seven Castle paths; it is not a mission
pass. A short source chronology now isolates real player/NPC death and holder
release before repeating the long prefix. Package NPC A4 addresses initially
point to actual holders; source grounded death intentionally replaces nonzero
relations with1. Final index1 alone cannot establish an initializer or NPC-
alias defect. Fixed-retreat short73 passes134.21s, with actual player life
and original carried holder retained through the first Castle loss and128
following turns. Full release74 fails222.59s at actor531 because the gate
expected an inactive row's model to be zero. Source Events.cpp sub57F20 clears
class only and retains model/action; the actual class0/model9/action14 row
is valid. The gate is being corrected to require exact source draw-disable
write and retained packet/identity rather than invented zeroing. Frozen75
passes all seven Castle-loss paths, including the exact raw-packet writes and
live-holder reclaim, then fails531.16s because the declared local Castle597,
generation1 is no longer current during paid collection. No pre-turn Castle
state or destruction events were retained at that failure. Instrumented76
then reproduces the loss: real Cymmerian500 projectile hits and Wyvern316's
1600-payload hits (sometimes accumulated to3200) kill the local Castle at
collection turn192, with stage1→0/destruction/reclaim at193. Root and owner
checked the actual remc2500/1600 constructors, mailbox accumulation and Castle
loss order; no production mismatch was shown. The short76 continuation failed
before its Castle setup because that earlier reachable prefix had no settled
neutral sphere. Its losing extension was removed; exact patch/log are retained.
Max-clearance geometry77 still fails529.52s on a genuinely killed player during
collection (turn2587, life−1310), with original holder618 still current. No
geometry-only success or Castle repair is claimed. Actual threat defense78
completes96 creature generations, including43 genuine split children, through
admitted contacts, protected landing and natural death/reclaim. Its following
Castle constructor returnsNone;79 shows zero ordinary free slots/no typed
replacement candidate, then two ordinary free slots after one unforced source
turn. Castle creation proceeds without fake allocator changes. The paid check79
then fails566.83s on an obsolete original-sphere payload assumption. Frozen80
follows the actual paid projectile/effect, current target/merge lineage and
collector load instead; strict Rig release compilation passes378.8s and its
full mission check passes712.09s through the normal exit0A. Bounded independent
review finds that simultaneous unrelated mana could still cause that check's
threshold crossing. Frozen81 tightens attribution: only the paid subtotal
survives verified actual merges and collector generation, and removing it from
the numerator at the actual objective sample must leave the percentage below65
with the same frozen denominator. Independent source/gate review supports this
correction. Actual81 fails634.04s at that strict necessity check: the real
collector deposits30000 paid plus1993 unpaid merged mana, and the fixture bank
was only one unit below the goal. The unrelated portion alone can finish it.
Frozen82 keeps every strict81 assertion and uses the existing Pav prior-bank
control: one-time threshold minus the initial witnessed sphere's payload,
checked nonnegative before casting. Both bounded reviewers support this fixture
correction. Strict Rig82 release compilation passes379.0s and the full exact
mission passes717.33s with the strict paid-subtotal necessity assertion intact.
Root checked frozen source560bb9ad/mc2 observer20246d67 against the run; the
changed-hunk UBS scan has no critical finding. Its295 warnings/72 informational
findings are test failure assertions, guarded indices, deliberate source
arithmetic and bounded diagnostic allocations; legacy formatting/scanner debt
remains. No late bank/goal or production-rule edit was made. Installed Original
native copies still match29688f; no Unity/native rebuild is needed for these
cfg(test)-only additions. No native Jathnar visual or human/earned campaign
proof is claimed. See `jathnar-full-{75,76,77,78,79,80,81,82}-release.log`,
`jathnar-{80,81,82}-result.json` and `mission-source-82.json`.

The same observed House-successor defect class was independently checked across
all25 normal launcher packages: all593 authored class10/model45 records have
MCI rows for every retained BLDGPRM successor until0/repeat, with matching
parameters; all25 BLDGPRM files share the same hash, including fresh8/20. Root
reran the read-only parser against current source. Owner/presentation directories
exist; no missing successor was found. This is static data/consumer availability,
not admission, cyclic-chain semantics, atlas completeness or gameplay proof.
Receipt: `normal-house-successor-closure-20261114.json`.

Independent source diagnosis found a real shared carry consumer mismatch:
CollectLevelStats preserves learned ownership from persistent0x3E9 OR nonzero
A4 OR mission-acquired0x403, without requiring a live physical holder. The
Rust backend incorrectly rejected the stale nonzero relation. The collection
consumer now matches that union; physical selection/casting still requires a
current holder. Actual Goat-owned contact→human grounded death→drop/natural
expiry/reclaim→knowledge carry→fresh destination holders/reset passes45.87s
on72. The existing two-entry/progression/save check passes21.04s and strict
Rig workspace all-target check passes31.7s. Independent bounded source/gate
review supports the narrow fix. This is a declared continuation boundary, not
an earned victory. The two historical map-save/stale-guard checks stop before
carry behavior on an old Level1v3 package lacking the required collision bank;
those consumers are not called green. Added-hunk UBS has no critical finding;
whole-file legacy findings and formatting debt remain. Runtime local
`ef69ab51`, remote Original `ee9a7ee1` are committed/pushed and native29688f
is installed with the qualified launch/load smoke recorded above. Failed71 used a private test helper; corrected72 uses canonical
area-damage Contact. No source Castle filter, late inventory grant or fake
allocator reservation was added. `mc-paj.3` and full mission acceptance stay
open.
Run45's later Hydra gate failure is superseded only for that specific gate.
Keep full campaign, opening, secret/carry and presentation acceptance open.
Root serializes compilation, shared joins and landing; owners stay through
consumer failures. Controls preserve live AI, actual production, timers and HP.

## Active request: complete the shared world capabilities

Oliver's 2026-11-09 correction keeps voxel/Modern work in its separate thread.
This thread continues remc2-to-Rust single-player behavior and qualified
Original-client delivery. Preserve the already landed cross-thread work.
Oliver's current sequence is to complete all shared engine implementation,
then level assembly, with his human playtest after those stages. Continue
independent shared capabilities while recording unverified human gameplay.
Missing earned campaign saves or human victory evidence do not gate that work.

Latest installed shared checkpoint: canonical `994132fc`, compatible Original
runtime `22fd2a5a`, native `9f2399ea59db`. Fool's Mana, Magic Mine, Cave In,
support and model23 feedback now publish authoritative rewards at the actual
source boundary into core/fire state, including the Castle cap. Cave requires
the current nonzero holder; Mine publishes before fallible product allocation.
Acquired all-tier consumers, source-order/negative controls and independent
source/gate reviews pass. The strict release build and all662 remote inputs,
installed470Rust/200UI/142player inputs, both plugins and mapped library match.
One first-attempt Original map/load/NightSpire scene/HUD smoke passed; root
viewed the scene and stopped its owned processes. No Unity/art change or human
gameplay claim. Finite late families `mc-k5k.5` and Cave `.6` are now closed,
alongside directional `.2` and Army/field `.4`; full engine/campaign is open.
Alliance uses retained acquired all-tier evidence plus current affected holder,
allocation, projectile, parent/expiry controls and bounded source review; its
long whole scenario was not repeated. Proofs and exact limits:
`research/private/generated/shared-engine-20261111-integration/` batch3/candidate3.

The next connected batch keeps `mc-6qf.2` progression/counters and `mc-6qf.1`
activation open: real reward input through save/replay/carry/reset/results,
and configured actual final row368 through scheduled group28, BUILD destruction
and group29 boss production. Terra owners and the Sol integrator must identify
real defects or close existing finite coverage gaps without inventing missing
behavior. A configured constructor boundary is not an earned final mission;
recorded public inputs are fixed regression evidence, not an AI playthrough.

The current Level9 Evirith Gor route now verifies the real repeat-entry
switch as well. After authored group3 activates, the test controls only
player geometry on a clone of that live session. Row119 publishes group13
twice in order as `[120,125]`; it keeps the same controller lifetime,
does not rearm during overlap or after nine clear visits, and rearms on
the tenth. Both fresh emitters reach their normal action64 callbacks.
The same exact test retains and passes all prior mission assertions
(231.77 s, one test). No runtime code changed or player build was needed.
Full `mc-6qf.1` stays open. See
`generated/level9-repeat-switch-20261118/`.

Cymmeria's reset-active repeat-exit switch now has a separate direct package
check. The installed native publishes all 24 authored group1 rows in order at
tick 1 and has no repeat through tick 19 under ordinary default input. The
exact 5.32 s test then controls only player geometry. It verifies that staying
outside holds the countdown, the tenth inside visit rearms it, and leaving
again republishes the same ordered rows. The switch keeps its identity and the
new field reaches its ordinary callback. Only the test changed. See
`generated/level10-repeat-exit-20261118/`.

Cave4's subtype22 absence switch now has an exact connected check in the
existing Spider mission test (7.64 s). The real objective and proximity
chain admits row119 and ten authored Skeletons. A living Skeleton and
its fatal visit block the timer. After seventeen empty visits, sound41
precedes ordered group2 publication, and the controller hides and reclaims.
Ten XP scrolls and both successor switches are live; switch585 publishes
the genuine group41 corridor command, which runs its scheduled callback.
The check uses declared player geometry and typed damage contacts on the
existing fixture. No production code changed. The retained installed
Cave4 saves precede this trigger. See `generated/cave4-switch22-20261118/`.

The exact Level2 group27 check now uses the actual group8 House producer:
authored vault154 completes BUILD21 and its scheduled BUILD54 successor,
then publishes `[156,157]` during final demolition. The zero-slot control
publishes neither product. With one vacant address, pickup156 retains its
new incarnation while Meteor157 is refused, both records are consumed,
and the demolished House reclaims without a second publication. The
unconstrained branch verifies the Meteor's first scheduled visit and
three generation-bound Impact children, then exact reset. The check
declares the preceding objective and damage contacts; all construction,
allocation and reclaim execute in the real scheduler. The Rig-built
exact test passed locally in 6.16 s with the owner package/cache.
No production or installed player change. Full `mc-6qf.1` remains open.
See `generated/level2-group27-partial-20261118.DyLYYw/`.

In the earlier batch 4, configured final368/374 checks passed 7.388 s: ordinary switch scheduling,
ordered group28, BUILD successor/destruction to real boss callback, and real
row374 overlap to four authored model28 roots and following reclaim. That batch
did not qualify final392. The recorded-input
combat/save/carry check was interrupted by a kernel OOM before a result receipt
and produced no acceptance result.

The later candidate-10 final regression covers both remaining named rows.
After the actual boss terminal transition, row392 publishes group31
as `[393,394]` once and reclaims. Row394 then starts player action12 in
the same pass, clears the visible portal flag, reclaims, and reaches
victory Results. The connected test also verifies CUT6, credits and
guarded Results save through cold return to the map. It uses declared
prior resources and exit positioning. See
`generated/shared-engine-20261111-integration/candidate-10/ending-final-regression.log`
(661.05 s) and the installed retained continuation under
`generated/uluth-cold-continuation-20261118/`.

OOM remediation on2026-11-11: RAM-backed `/tmp` held about28GiB while swap was
full. The current Cargo cache has been moved byte-for-byte to SSD at
`/home/oliver/Projects/mc2-tmp26/cargo-cache-20261111`; all3255 regular files
(8,734,415,133bytes) and symlinks match, and the old target alias resolves there.
Further accounting found17GiB of Unity player copies in `/dev/shm`. Source,
saves and installed launchers were checked and preserved. Root removed the
inactive8.6GiB `/tmp/magiccarpet-level1-intermediate-builds-20261110` and21
inactive November3 player copies totaling4.3GiB; no archive was created.
Available memory rose from~6GiB to~27GiB, swap use fell from16GiB to~8GiB and
current memory-pressure stalls returned to zero. The diagnostic test peaked
near200MiB and timed out at180s without an acceptance result; no reward/save
failure or memory leak is established by that interruption. A temporary
per-job cap was diagnostic protection, not the operating memory budget.
DCG blocked both deletion and recoverable relocation of11 obsolete `/dev/shm`
player copies totaling13GiB; those remain untouched, alongside the latest
candidates and pinned Stone review. Root did not bypass the guard. Receipts:
`/home/oliver/Projects/mc2-tmp26/cargo-cache-relocation-20261111.json`,
`old-tmp-players-cleanup-20261111.json` and `shm-player-cleanup-20261111.json`.
Use actual measured RAM/swap/output headroom when resuming; large build/test
outputs belong on disk, not `/tmp`, `/dev/shm` or `/run/user/1000`.

Oliver subsequently authorized deleting obsolete temporary build/test outputs
instead of archiving them. Root removed19 inactive Nov3/4 output directories
and the just-copied old test backups; `/tmp` now has about20GiB free. Current
source, warm caches, active build and current installation were not removed.
Do not create more old-output archives or rollback work as a delivery step.
The previous Original checkpoint ran the secret continuation fix from shared main `c1f06874`
and compatible isolated build `5127083c`, with native `552d43962253`.
The immediate secret load from an unfinished parent replay retains source bit
0x10. Its action 12 result 0x1A routes through the active secret return frame,
including after restore. Direct map revisits use 0x0A. The existing focused
exit byte, route hash, retained-parent save, actual source4 entry and actual
source30 banishment checks passed. The save fixture stages entry and now uses
the current whole Level5 package because the older fixture lacks Castle BUILD1.

Rig and the returned library matched all 662 native inputs and the artifact
hash. Both installed plugins, 470 Rust inputs, 200 Unity inputs and 142 player
files matched. The six-package playtest launcher at
`/mnt/pinokio-shared/magiccarpet-secret-continuation-20261111/play-secret-campaign.sh`
loaded the Original campaign map and NightSpire with one portal click and zero
retries. Its new Level5 cache and existing source30 cache passed strict
environment validation against their exact packages. The earned Level5
frontier, actual 85% mana victory and the campaign return after a real win still
need human play. All related unfinished beads stay open. Build and test
receipts are beside the launcher.

The previous installed Original checkpoint was Rust `16be76a3`, client `2961a7a7`,
native `6f6bc45b13c88de14645f4b90bdef9e47f8219921ea692de6140806e0c0459ba`.
Both source commits are pushed. Full Original build `build-v9i5fr_r` is installed;
Root independently matched 470 Rust, 200 Unity and 142 player files, both plugins,
and all 662 Rig/native compile inputs. The normal canonical launcher reached
map/loading/NightSpire terrain/HUD with that mapped native, one portal click and
zero retries; Root viewed both captures and stopped the owned player/display.
Oliver authorizes the next four priorities in order: deliver current shared
fixes, retained playable secret entry, source-correct return/carry, then actual
campaign round trips/save/revisit across all five associations. These priorities
do not represent the entire remaining port or waive existing acceptance.

Step1 is delivered: stage-pulse/Castle-phase and additive character observations
are committed source, independently checked on frozen470-input58e7. Actual
group7→villagers, Castle holder phase, three character ABI consumers, source
order and configured secret entry pass. The original recorded public Castle
failure at3497 advances through3498 and120more ticks to3618 without error;
finalhash4fe8a46afa27fa80. Rig/returned release matches all662 inputs. New native
headers required genuine full Original rebuild build-kl71jtms; no PInvoke
migration was needed because Original does not use the additive character API.
Installed470Rust/200UI/142player files and both native hashes match. Root viewed
canonical map/loading/NightSpire terrain/HUD, oneportal/zero retries, stopped
owned smoke. This is launch/load, not human combat/FPS/hardware audio/full
missions/old-save proof. Evidence: generated/shared-refresh-20261110-integration/
candidate-1. Eighteen superseded owned test executables (~1.7GB) were removed;
source/logs/current marker proof/caches/player remain, without archival copies.

Step2 now reaches the actual retained source4 portal through public native
movement from the unchanged original spawn: real switch136, portal293/controller298,
full player phases through12 and guarded result0x1A at tick883. The only authored
content edit is original rows75/76 activation groups6→4. Existing WorkspaceStore,
package/provenance roots, other entities/objectives/positions, source arithmetic,
reset and stale guards remain intact. The installed resolver reads that same
retained workspace. The playable client now forwards `--authored-workspace` per
package through its existing parser/DTO/native JSON route; it changes no campaign
entry or frontier rule. Actual Unity revealed null-string serialization as `""`;
the existing request now uses DataContract optional omission, rejects explicit
blank values, and passes real default bootstrap/world open with current packages.

Step3 fixes confirmed loss of accepted secret completion statistics: Rust retains
one keyed row per secret through save/replay, preserves prior rows on abandon,
and publishes the same rows to receipt validation and Original results lookup.
The Rust state checks and three Original statistic/receipt fixture cases pass.
Actual source30 public menu banishment produces0x18 with stale/reset protections;
its fresh release ABI witness passes too. This is not an earned successful secret
exit, real outer transfer/carry, or a rendered results-table claim.

Step4's real data prerequisite is further qualified: all five genuine source30–34
packages already exist; no re-export or replacement content was needed. Current
installed native opens31–34 and advances16 ordinary neutral commands each, with
Root independently repeating those exact public checks. Source30's existing
current admission is retained. Source31's actual authored model28 actors and
transports also progress through the campaign DTO; neutral presentation_entities
is not used as their proof. This establishes data/admission, not all-five mission
completion or campaign round trips. Original acceptance and dependencies remain
open, including mc-6qf.3's unfinished switch/exit prerequisites.

Still required: source30's actual objective85→normal successful exit, an earned
parent campaign frontier, actual outer entry/load/return/carry/save/revisit and
all-five gameplay. Oliver reports no known earned Level5 save. Do not fabricate
frontiers, completion words or saves, and do not weaken source0 entry admission.
Human retained-variant play, combat/FPS/hardware audio/full missions/old saves
remain unverified. The legacy Title test dereferences an absent optional field
before the new encoder; historical v3 package defaults also reject current
runtime collision-bank admission. Both negatives are preserved, not greened by
changing assertions. BCL JSON inspection avoids a Modern-only test dependency.
UBS C# remains partial; large-file formatting/legacy scanner debt remains.

Evidence: `/mnt/pinokio-shared/magiccarpet-secret-lifecycle-20261110-integration/`
(candidates1/3/4/5) and generated/secret-packages-20261110-admission. Final resolver
and native JSON checks pass4.809s/2.534s, retained public flight124.354s, source30
numeric banishment3.518s, strict Rig workspace check21.722s. Six superseded test
executables (730,344,456 bytes) were removed after receipt hash verification;
current candidate5 executables/native, source, logs, caches and players remain.

Root CopperMountain owns this Original checkpoint. CrimsonForge owns the bounded
secret-package handback; the other available Sol owners are write-stopped after
integration. The capacity-stopped Terra worker remains stopped. Modern/Voxel
belongs to GentleBadger's separate task; its execution lanes are now returned.
Keep the isolated Original2961 snapshot and qualified private client intact.

The previous three sequential steps were delivered as bounded checkpoints: recovery
admission reconciliation, configured real secret entry, then the reproduced
possession ownership-transfer failure. Modern/Voxel remains separate.

Recovery reconciliation is committed/pushed `d16c0607`: source input1/3/15
admits an existing living human. Runtime behavior was already correct. The
false death-only oracle is replaced by four real invalid-owner/nonmutation
cases and a living same-actor/progression countercontrol; module6/6 and existing
public recovery pass. Do not add a health gate or restart this diagnosis.

Secret-entry source is committed/pushed `c34836d0`: distinct SecretPortal
binding14/4/9 and dynamic dispatch now reach the existing339 constructor,
world::portals sequence and guarded result0x1A. The owner module is a direct
move, with no old-path wrapper. Whole current source4 plus a declared pair75/76
group6→4 and two legal outside pose/aim controls reaches actual136, full source
cadence/sequence/audio/DTO, following reclaim/reuse and once-only public result
(17.431s). All other rows/positions/objectives/bindings stay intact. The current
MCI3/v4 Level5 package has identical authored bytes to the old source4 package;
the old v2 package lacks initialCastleBUILD1 and was not admitted by fallback.
Adjacent ordinary exit had an identical baseline palette oracle failure; source
PaletteChanges consumes5→10→0 while black remains installed. The strengthened
check observes those modes and actual all-black768-byte palette. Frozen C4
ordinary/entry checks pass13.750s/17.461s. Its whole-workspace capture refused
concurrent presentation ABI edits; separate exact Rig/core-artifact evidence
qualifies only that frozen core candidate. Final integrated C1 now passes the
same entry/ordinary-exit checks22.280s/18.353s, recovery6/6 plus public recovery,
all five marker/strict-reader/mailbox checks and source order. Strict Rig
workspace all-target check passes on that exact470-input source snapshot.

Possession transfer is committed/pushed `385c63a8`: the actual sub36AE0 scan now
receives current shared spell context, reads typed word94 before stale raw
shadows, updates both carriers and synchronizes matching imported records before
action78 can republish them. Fixed1..999 source scan, strict current-reader
validation, semantic IDs and unrelated stale identities remain intact. This
repairs missing context; the live Whirlwind replacement was not absent/expired.
The one Trader worker supplied the frozen two-file patch; Root integrated and
independently checked its source, gates and real public consumer. Fresh bounded
read-only review found no new concrete defect; full Possession remains open.

Actual whole Level7 repeats1260 recorded public inputs twice with identical
results, all1140 baseline pre-error hashes unchanged, no failure at former
tick1139 and finalhashc3f6992761a3436f. A separate public raw projection retains
marker13 through1140 and observes following-pass reclaim1141..1144. No actor,
inventory, mana or save mutation/adaptive player was used. Strict release build
and exact Rig/returned native match all470Rust+162upstream+30extra inputs.
The existing isolated execution worktree pins the committed engine; concurrent
uncommitted presentation CAPI/header/voxel work is preserved and excluded.
Original native-only refresh build-rtqcuygb records those470 inputs, both plugin
hashes,200Unity inputs and142player files. Root viewed canonical map/loading/
NightSpire terrain/HUD with the matching mapped native, oneportal/zero retries,
then stopped owned smoke. This is launch/load, not native secret-variant gameplay,
human combat/FPS/hardware audio/full missions/old-save proof. Evidence is
generated/marker-transfer-20261110-integration/candidate-1. UBS has4 legacy
wholefile critical findings, none on added lines; legacy formatting debt remains.
Four superseded recovery/portal test executables were removed (~650MB); current
proofs, logs, source, caches and installed player remain. No archive was made.

The prior group7 pulse-loss and Castle holder phase blockers are resolved in
the current58e7/f76 Original checkpoint above. Do not restart their diagnostics.

Full `.11`, `.6qf.6`, `.6qf.4`, `.7`, `.6qf.2`, `.bhq.10` and dependencies stay
open. Retained native variant, vanilla portal mission, outer return/carry and
all-five gameplay remain unqualified. The long/adaptive earned-Level1 runtime
test has NOT been run: no bots, forced objectives or group-completion proof.

Installed reward-jar checkpoint: runtime `d70dc599`, native
`97584d162e0a5909c9d4aa4dacc0c1bf998f662a3d0f3fcc4adbd2f7990d3058`.
Canonical `world::items::reward_source` now owns the source4EF30 controller,
typed default health10000, ordinary authored91/Imported98 wiring and actual
32CF0 consumers. The sphere path uses source50080→500C0, no extra child RNG
draws, five parent draws per birth, real speed/roll/particle dimensions and
protected140-life behavior. Shared progression joins current human/holder
classification, actual map context, signed increments, Castle SetSpell before
cap/tier, and existing runtime/core projections. Whole Level4 plus one declared
unused source-language91 row reaches real124/group37, jar126g1, actual emissions
and26-family XP, paid100 Possession192g1→impact205g1→claim53g2, natural hide139,
following reclaim, persistent parent and reset/reopen (132.739s). No shipped91
occurrence exists in the30-package census; genuine final-Volcano birth remains
separate earlier evidence. Prior spell carry/funding/poses are declared controls.
Protected children are excluded from Castle collection in source; no deposit
or natural active-holder/typed-nonspell SetSpell coverage is claimed.

Source/definition/shared-boundary/order/passive/scroll checks and workspace
alltargets pass. The one revival negative above remains an unchanged baseline
failure, not a clean-suite claim. C3 differs from C2 only in its fixture:
preserve configured rows while materializing prior carry, and let corridor195
settle naturally1→3 before approach. Fresh bounded source/gate review supports
the corrected route; no assertion weakening. UBS added-line findings are public
test equality false positives and an intentional invalid-map test failure;
legacy whole-file/format debt remains. Strict Rig release235.905s, all662 inputs
and worker/returned15497768bytes match. Original native refresh `build-h9h3w259`
matches470Rust/200UI/142player-file hashes, both plugins and unchanged contract.
Root viewed canonical map/loading/NightSpire terrain/HUD with matching mapped
native, one portal/zero retries; owned processes stopped. No native jar encounter,
human combat/FPS/hardware audio/full missions/old saves proof. Evidence:
`generated/jar-20261110-integration/candidate-3` and unchanged C2 checks. Obsolete
Jar C2 and Scroll C6 test executables were deleted after installation; logs,
source, caches, current Jar C3 and current player remain, with no new archive.

Installed Experience Scroll checkpoint: source `6f49e2a2`, native
`5824381c160d020314f96ebcf8dc9e01519b94fdaaf6e2cad6d3cbec3b3e823d`,
pushed metadata `0cde91bf`. Canonical items now own scroll/portal state,
constructors, visits and actual original sprite candidates; the missing typed
projection is fixed. Shared scroll rewards match enabled-only wrapping XP,
independent normal Castle cap7, all26 tier/selected-clamp visits, ownership/Cave
announcements and literal63/61 source audio. Actual whole Level2 sequence
produces four scrolls248–251; award/DTO/reopen/reset passes7.913s, final
retirement/reuse3.635s. Correct journal boundary binds replacement sphere398g2
to actual Goat callback73 phase48, after scroll398g1 retires. Prior source-order,
passive-item and reward counters pass on unchanged owned production; workspace
alltargets passes13.3s. Five fixed legal player poses and legitimate prior
semantic carry396 for the actual4XP→400 tier crossing are declared controls,
not earning396 or a full mission. Independent bounded source/gate review found
no remaining concrete defect in this checkpoint. Full `.11` remains open.
Strict Rig release227.256s; all662 inputs and worker/returned bytes match.
Original full player rebuild `build-gyyievt2` honors the changed shared header
while preserving all200 UI input hashes;470Rust/200UI/142player-file inputs and
both plugins independently match. Root viewed canonical map/loading/NightSpire
terrain/HUD with matching mapped native, one portal/zero retries; owned smoke
stopped. No native scroll encounter, human combat/FPS/hardwareaudio/fullmissions
or old-save proof. Evidence `generated/scroll-20261110-integration/candidate-6`
and prior unchanged checks. Removed four superseded C2–C5 test executables
(647,986,056bytes); logs/currentC6/source/caches retained, no archive.

2026-11-10: the connected prefix now PASSES on current source C3
(`generated/objective-20261110-integration/candidate-3`, 5.644s). Four held
paid FireballI requests debit400; actual source impacts consume500 then250
against original600HP Firefly165, producing the real300-mana sphere. Paid
Possession and the same-generation defender's exact Castle deposit cross40%,
then original reset6 publishes all six living5500HP Spiders. Positive
callback/epilogue receipts and exact phase+1 are captured before the mirrored
negative run overwrites thread-local receipts. This was an observation error,
not a source objective-pause defect. All no-cast/no-Possession gates remain.

Independent source review found a real Firefly discrepancy: the combined
consumer cleared both damage and owner, while source1BF90/1C560/1DDA0/1E1C0/
25610 clear only owner62 and retain damage5E. Root repaired the canonical
Firefly owner; its existing check now proves consume100→residue(100,0)→actual
Contact25→125→next consumption/residue125 (1.112s). Linked-child and actual Cave
adoption/attack/rebind checks pass1.047/2.741s. The paid-path ledger verifies
exact generation-bound impact writes, numeric aggregation, life and ownerless
residue at ordinary boundaries. It does not claim direct latest-writer
consumption metadata against every identical-value unrelated overwrite.
Fresh bounded source/gate review supports the two-file change. No pre-existing
Spider bytes were removed. C1 exposed the incorrect per-impact250 consumption
oracle; C2 reached the deposit/spawn path but read the negative clone's final
scheduler receipts. C3 corrects both and preserves full acceptance.

The prior Castle bank, carried spells/player funding and initial poses remain
declared controls. This is not earning all40% from zero, six ordinary Spider
deaths/exit, complete Level4 or full `.7` acceptance. Source `bbc831e0` is
committed/pushed. Strict Rig native build passes244.875s; all659 guarded inputs
and returned/worker bytes match nativeSHA477e80949eafef202fd18f17b33d079f5b90f92fe445ddbc43510d39f11dd20d.
Existing Original client was rebuilt as `build-8yrq40i1` for the current shared
header qualification; all200 UI inputs are unchanged. Root independently
verified467 Rust/200 UI/142 player inputs and both installed native copies.
Canonical map/loading/NightSpire terrain/HUD smoke passes with the matching
mapped library, one portal/zero retries; owned processes stopped. This is not
a native Firefly/Level4 encounter, human combat, FPS, hardware audio, complete
missions or old-save proof. Modern/Voxel Presentation remains untouched.
The complete connected Spider check now passes58.513s at proof `7695ab29`:
earned prefix -> six original Spiders -> paid Lightning/authored Meteor pulses
-> real Web/recovery/melee -> fatal paid Meteor -> ownership/deposit/reclaim.
Configured six-death absence passes6.182s; this is not six public kills or a
complete mission. The proof is cfg(test)-only, with initial resources/poses,
post-earned Castle200k and later empty-store controls explicit. Evidence:
`generated/objective-20261110-integration/candidate-9`.

Installed passive item checkpoint: runtime `8a471f1f`, native
`3b1ce93f46e29d7153e5edc1ed225c15698b523791c3975b9ceadf66bf430a34`,
pushed metadata `e56c4615`. Canonical `world::items` owns source14/0
construction/properties and its empty Update callback. Actual Level4 switch124/
group37/corridor195 plus one declared dormant source-language row verifies
constructor/target/particle77, changed floor with retained itemZ, queued contact,
common hide/reclaim and two normal neutral resets (13.420s). No shipped14/0
occurrence or positive stage-pointer overwrite is claimed. Source/registry,
definition and workspace checks pass; fresh bounded source/gate review found
no remaining concrete defect in this path. Strict Rig native passes238.077s;
all662 inputs and worker/returned bytes match. Native-only Original refresh
`build-7b0t4tzx` and470Rust/200UI/142player-file hashes qualify. Root viewed normal
canonical map/loading/NightSpire scene/HUD with mapped matching native,
one portal/zero retries, then stopped owned smoke. This is not a native item
encounter, human combat/FPS/audio, full missions or old saves. Evidence:
`generated/passive-item-20261110-integration/candidate-2`.

The first prefix candidate (C2 under
`generated/objective-20261109-integration`) typechecks and builds on Rig, but
fails its real Firefly-death predicate. Same-binary read-only GDB confirms all
four paid launches/impacts. Source/owner geometry places Firefly164 in front of
165 on the original+X approach. C3 corrects only the fixed approach to−X, adds
source target-score admission and decisive diagnostics, and retains all health,
kill/payment/deposit/negative gates; its typecheck passes. C4 optimized build
passed in524.038s. Its exact runtime still fails: the first250hit leaves350HP;
the second flyer collides, but the following area pulse misses the fleeing
Firefly by10units (X223 versus strict213 overlap). Source hit reaction sets
phase7 then speed76×3; bounded source review found no port defect. The same
four casts held on turns0–3 is the queued source-justified discriminator, not
a verified result. To land the independent scenery capability, Root preserved
the complete C3 source/diff and failed C4 evidence, then reverted only the
experiment's1002 added cfg-test lines. All original Spider bytes and gates
are unchanged. No successful mana crossing or full route is claimed.

An independent bounded `.13` review initially treated models4/5 as orphan
constructors because no rows appeared in29 inspected packages. Root required
the generic-factory admission to be checked before closure. The reviewer
withdrew that conclusion: source `4A1E0 → 4A310 → 4A190` admits both through
enabled constructor-table entries, while Rust rejects them and omits actions
12/15. TopazAnchor completed this original-scope missing static-scenery path,
including typed source-default life300/particle48 bindings, real construction
and the source-empty callback's common phase/reclaim consumers. A configured
source-language control is labeled as such; no shipped occurrence is claimed.
Its ten-file edit window was explicitly serial: TopazAnchor wrote those
canonical files; Root built the separate frozen dispatcher and remained
read-only there until handback. Mail returns grant IDs
but normal renewals return0 even for Root; no force release or coordinator
setup is used, and verified renewal is not claimed. After frozen handback,
independent review found that the raw Imported prefilter still omitted12/15.
Root joined those exact actions and corrected one signed expected-value tuple
in the configured check. Rig workspace/alltargets passes38.27s and the typed
property serde/reset-immutability test passes11.279s. Frozen C2 optimized build
passes550.004s; its real configured source-language path passes0.090s, registry
0.022s and ordinary source order0.100s. Independent source/gate review supports
these joins after the prefilter fix; final ten-file UBS has zero added-line
critical findings (23 legacy whole-file critical findings remain). All659
guarded inputs match canonical/dispatcher/Rig. This proves configured4/5
construction, target/DTO, scheduled empty callback/common phase, canonical
burial hide/following reclaim and exact reset; no shipped occurrence or native
encounter is claimed. Source `9a0a65b8` is committed/pushed; strict Rig native
build passes226.393s and worker/returned SHA128a372376b772a3 matches exactly.
Native-only Original installation `build-lul3mta2` matches all467 Rust inputs
and both plugin copies. Root viewed canonical map/loading/NightSpire scene/HUD
with that mapped library, one portal/zero retry; owned smoke processes stopped.
Fresh finite-family acceptance reconciliation found no remaining concrete
admitted gap alongside unchanged prior Tree/Stone/Dolmen/moving-scenery
evidence. `mc-bhq.13` is CLOSED with original acceptance/dependencies unchanged.
Evidence: `generated/static-scenery-20261109-integration/candidate-2`.
This is not native4/5 encounter, human combat, FPS, hardware audio, full missions
or old-save proof. The wider shared engine and objective/activation/flow
acceptance remain open.

Oliver authorized continuing the existing remc2-to-Rust delivery plan on
2026-11-08 after the creature runtime pass. Cave mushrooms `mc-bhq.1`, Cave
corridors `mc-bhq.2` and authored Cave paths/rooms/features `mc-bhq.5` have their
finite acceptance qualified below and are present in the installed runtime.
Trees, stones, dolmens and remaining scenery `mc-bhq.13` are now qualified.
Paid Tree burning, Crater/Dolmen grounding/recharge, Castle/scenery burial and
outdoor moving-scenery construction/fire collateral are installed below.
Continue the existing activation/objective work above, then progression/flow.
Use each family's existing original acceptance and canonical owners; complete
ordinary production, real children and lifetime/consumers.
Root CopperMountain implements shared joins and serializes Cargo/RCH, acceptance, commit/push and
installation; keep one accountable family owner through any consumer failure.
Keep the complete acceptance and dependencies. Then continue existing
terrain/scenery, activation/objective and progression/flow capabilities, one
connected runtime result at a time. Do not restart delivered spells/creatures.

### Current connected batch: scenery, world geometry, activation (2026-11-08)

Oliver explicitly queued these three capabilities in this order. Keep their
original acceptance and dependencies; this is execution of the existing plan.
Root CopperMountain owns shared joins and sole Cargo/push/installation.

1. **Dolmen identity and collision bindings — partial `mc-bhq.13`.** NavyBridge
   implemented the missing neutral Dolmen role, source-default 1024/1024
   dimensions and readonly initial semantic-to-live binding. Root integrated the
   existing authored-group constructor and scheduler validator. Frozen C16
   passes the real paid Crater/contact/recharge/reset path (0.185 s), configured
   constructor/current contact boundary/reset (0.083 s), real backend/schema
   consumer plus duplicate Cave Mushrooms (0.082 s), source order (0.066 s) and
   definition serialization/immutability. Source is committed/pushed in
   `e646e767`; matching native/client delivery is installed below.
2. **Source terrain deformation through voxel collision/presentation — partial
   `mc-bhq.8`.** TopazAnchor found an actual shared gap: source spells changed
   the floor planes while voxel-enabled height queries retained the enable-time
   surface. The candidate rebases terrain columns with source interpolation,
   preserves world-position removals and Building volumes, and publishes a base
   hash. Root joins synchronize before subsequent source consumers, including
   same-callback Volcano child placement and Gravity pull; the client refreshes
   the base while preserving removal intent and impact cursors. The immutable
   source binding participates in deterministic hashing. C18's two native
   source-projection/hash controls pass. All 12 focused client checks pass in
   a private source copy; its existing native is `dc5ea148`, so this is managed
   geometry proof, not combined native/client casting. Final C19 public Crater
   passes in 0.327 s using the unchanged qualified Level 2 entry and real house
   12. It checks paid casting, changed composed collision/base/revision, prior
   removals/buildings, natural cleanup, a no-cast control and exact serialized
   input replay from the same declared controlled baseline. Existing voxel
   configuration/cold replay passes in 1.212 s. Source is committed/pushed in
   `e646e767`; matching native/client delivery is installed below. The older Level 1
   carry fixture correctly fails its admission guard and is not bypassed.
3. **Real completion to later-group scenery — partial `mc-bhq.7`.** CrimsonForge
   traced Level 7's real all-rivals objective to source 271 and group 10's eight
   moving objects. Existing production wiring was present; no new objective
   implementation is claimed. Keep this item queued. C16 failed an inadmissible
   initial player pose; C18's corrected pose reached real paid impacts but did
   not establish all default-health deaths/Castle removals. Source Meteor II
   deals 1600 here, and the fixed encounters do not guarantee moving-target
   admission. Expanding this into a mission-combat script would violate scope.
   The owner preserved its exact source/patch/logs and reverted only the new
   test, proving the remaining file equals HEAD. No runtime, old predicate or
   original acceptance was changed; no positive mission result is claimed.

C15/C17 test-access compiler negatives and C16's pose failure remain preserved.
C16's optimized binary matches all 657 frozen dispatcher/Rig inputs; source
terrain work was explicitly excluded from that earlier snapshot. C18 includes
the combined Rust source with no excluded compile inputs. C19 removes the
speculative mission-combat test and corrects only the Crater entry fixture;
its focused test-target typecheck and optimized compilation pass. All 657
canonical/dispatcher/Rig inputs match. Final C19 paid Crater/Dolmen (0.186 s)
and source order (0.069 s) pass; unchanged C18 production workspace/all-target
and source-projection controls remain applicable. Rust UBS's two added-line
critical heuristics are public entity identity comparisons, not secrets. C#
semantic scanning is partial; compiler/12 focused Unity checks and bounded
review supply the managed evidence. Whole-package formatting debt remains.
Use `generated/scenery-20261108-integration/candidate-16`, `candidate-18` and
`candidate-19` under `research/private/` for commands and results. Unity crashes
even on `-version` inside minimal bwrap, while the same binary works outside;
no cause below that boundary is established. A private source/cache copy avoids
that wrapper without modifying the qualified player. Its first Goat-resource
lookup failed; an exact baseline control and the restored unchanged candidate
then pass. All negatives remain; no first-attempt or old-save claim.
Do not replay successful unchanged
checks. Full `.13`, `.8` and `.7` stay open.

**Installed batch 1/2:** Source `e646e7673885c4dcaa4c481761da841ab1b9619a`
is committed/pushed. Matching optimized native
`8acb9d956b69aa47d789ce653c5f207f153032ad13f232f05d33a094af2fc0f9`
built in 202.647 s with all 657 canonical/dispatcher/Rig inputs matching before
and after. Original native-only refresh `build-m4344n2r` passes its unchanged
source/contract gates. Root viewed normal canonical map/loading/NightSpire
terrain/HUD, verified the mapped plugin hash and stopped owned processes after
one portal click with no retry.

The separate existing Presentation client was rebuilt with the three changed
C# files and that exact native. All 628 private authored inputs are unchanged
across the build; all 170 payload files are pinned. The default Presentation
pointer now selects C19's `presentation-player`; the prior pointer/player and
prior native bytes are preserved. A single installed default voxel launcher
run reaches native projection and NVIDIA/Vulkan scene readback over ticks1–91.
All32 source buildings remain known; the selected voxel building38 correctly
leaves31 regular building meshes. A copied Modern-only oracle initially
expected32 meshes/slot−1; the same saved run was qualified against the actual
voxel consumer and frozen slot38, without another run or runtime change.

Old preset replay definitions correctly reject the new Dolmen structure. Fresh
opening/Goat/Archer sidecars are new current-native executions of the recorded
public inputs, preserving the old files. All280/440 inputs and warm/cold state
equality pass; actual Goat94/model1 and Archer273/model4 remain alive. An extra
root identity check initially used wrong remembered model selectors; source
selectors corrected that check, without runtime edits. The unchanged owner
layout passes all32 rows/1220 cells plus generation/tick/no-mutation controls.
Details, negatives and receipts are under C19. This proves installed launch,
projection and focused Rust capabilities, not a native paid-Crater visual,
human combat/FPS/hardware audio, full missions or old-save migration. No art
change was introduced. Item3 remains queued with its full acceptance open.

### Installed outdoor moving scenery and fire collateral — partial `mc-bhq.13` (2026-11-08)

Implemented, committed and pushed source `3353744e`, on the shared presentation
base `88319817`. Matching Original native `181c6007e782` is installed through
qualified full player build `build-4lbjf6mv`. Typed `BallisticObject` life rules
(source default `400 + draw % 2000`) reach actual initial readonly carriers and
the shared authored 7/8 constructor. The canonical owner is now
`world::scenery::ballistic`; the old native-world module was removed. Ordinary
NewEvent/map/particle 322/323 construction preserves three draws, flags 12,
half-speed axes and genuine Cave null-before-allocation. Initial life binding
is independent of placement-edit permission. The 81 actual retained ordinary
initial carriers preserve source-default maxima; Cave 21's duplicate readonly
Mushrooms retain their existing binding behavior.

At the committed integrated C14 revision, whole real Day 3 authored 245/source 245
passes the public paid Fireball I → actual Tree ignition → GroundFire → original
moving object path in 0.235 s. All 38 source turns remain live: actual 50-damage
fire pulses and their accumulated 50/100/200 mailboxes take life 988 → −12, with
real bounce, Smoke 273/generation 1 presentation, natural hide and
following-boundary reclaim.
Every contributing fire generation is traced through actual lethal Tree
receipts to paid projectile impacts; source continuous damage is 5 for Trees
and 50 for this object. Holder charges are exact; player debits are verified
in aggregate. Equal-elapsed no-cast and real reset pass. Player pose/carry are
declared controls; actors, objectives, terrain, RNG, allocator and lifetimes
are not forced. These are focused Rust gameplay checks, not a human campaign.

Typed property/backend consumer and duplicate-Cave regression, constructor/
physics/Cave controls, existing paid Tree lifecycle, numeric source ordering,
definition serialization and integrated workspace/all-target check pass.
Independent source/gate review found no further concrete changed-path defect.
UBS has no added-line critical finding; whole-file formatting/scanner debt stays
explicit. C10's 100-damage writer remains unattributed; C12 reached death/smoke/
cleanup but failed a wrong direct-250-only oracle. Both negatives and the C11
missing-import check failure are preserved. The corrected gate requires actual
source mode, generation, mailbox arithmetic, ancestry and payment, without
weakening gameplay acceptance. The older opening semantic census still fails
identically on C9/C12 before Day 3 due prior environmental-effect expectations;
the narrow actual Day 3 backend loader passes. No broad census rewrite was made.

All 465 Rust, 162 upstream and 37 extra inputs (657 unique) match clean committed
dispatcher/Rig/source and the installed artifact. The additive native-header
change correctly refused native-only refresh: Unity content was unchanged,
and the existing verified native was reused for a full qualified Original
rebuild without another Cargo build. The nearly-full RAM cache was copied
byte-identically to SSD; old cache/link and previous player remain preserved.
Root viewed normal canonical map/loading/NightSpire terrain/HUD with the mapped
matching native, one portal and zero retries, then stopped owned smoke processes.
This is launch/load, not native ballistic-encounter visuals, human combat, FPS,
hardware audio, full missions or old-save proof. Evidence: C10/C11/C12/C13/C14
and `ballistic-launch-smoke` in `research/private/generated/scenery-20261108-integration/`.

Full original `.13` acceptance/dependencies stay open. Later-group 7/8 factory
wiring is implemented; the real Level 7 group 10 triggering path remains
unqualified. Continue the remaining existing scenery producers/typed bindings
before activation/progression work; do not replay this delivered checkpoint.

### Installed Crater/Dolmen grounding and recharge — partial `mc-bhq.13`

Source `e8e0d408` is committed/pushed; optimized Original native
`7022c4c8cb2c` is installed through `build-vkj6uua2`. The shared Dolmen action6
now publishes its reached Z to the current matching raw source row, after
source65080's old-pose contact test. Its common phase tail also reaches the
raw row/record66. Only Z and phase are written; other packet fields and current
occupants are preserved. This fixes the source-visible carrier join for every
Dolmen routed through that shared handler, rather than changing a mission.

Final C6 passes the whole real Level2 authored411/Dolmen108 path in 0.179 s:
declared carried CraterI and Castle/player funding, public left-hand activation,
actual payment and generation-bound projectile/ring, exact ring-callback floor
change, reached ground write in current targeting and original sprite39,
raw/typed phase, natural hide/next-boundary reclaim/map unlink, equal-elapsed
no-cast control, actual contact selecting a max-derived high recharge rate and
its unsaturated credit at the next player callback, and typed/raw/target/DTO
reset. No NPC, objective, terrain, RNG, allocator or lifetime is forced.
The same active holder is allowed to finish its natural countdown; callback
receipts distinguish the player's rate selection from its later suppression.

C5 production is byte-identical to C6; only the test oracle differs. Reused
C5 workspace/all-target check (53.730 s), actual source-order control (0.069 s),
existing contact control (0.025 s) and acquired/all-tier public disaster path
(1.643 s). Bounded source/gate reviews, diff checks and Rust parsing pass.
Whole-file formatting debt remains. UBS has no added-line critical finding;
its four unchanged simulation-file matches confuse secret missions with
security tokens. Final Crater-only scan has zero critical findings.

The earlier C3/C4 negatives stopped before casting on invalid FOV/hand-timing
assumptions; they did not reproduce the production gap. C5 reached the real
grounding/cleanup prefix but its late end-turn recharge expectation ignored a
possible active-holder writer. C6 uses actual ordered player/holder callbacks.
No pre-fix runtime counterexample is claimed; the missing writeback was traced
directly against source and the connected path now passes with the small join.

Root verified all 654 unique inputs and exact Rig/installed bytes, viewed the
normal canonical map/loading/NightSpire terrain/HUD with the mapped matching
plugin, one portal and zero retries, then stopped owned smoke processes.
Launch/load is not native Crater/Dolmen gameplay, human combat, FPS, hardware
audio, complete missions or old-save proof. No Unity/art rebuild. Evidence:
`research/private/generated/scenery-20261108-integration/candidate-6`, reused
C5 production controls, retained C3/C4/C5 negatives and `dolmen-launch-smoke`.
Full original scenery acceptance/dependencies stay open. The following batch
resolves the specific burial/retirement join; it does not close the family.

### Installed Castle/scenery burial — partial `mc-bhq.13` (2026-11-08)

Implemented, committed and pushed `d1e983db`; matching Original native
`69fcd85cf224` is installed through `build-666g7e_b`. Actual Castle BUILD
child10/42/action44 now reaches shared57390 through live actor domains at
every source stage/cell, before material work and the separate height pass.
Preserves signed linear scratch offsets (including valid non-square overlap),
repeated material/topology RNG order, timer/hold behavior and all-cell Cave
clearance/Day closure. A hidden Dolmen still runs its current-pass callback
and common phase; its typed view retires only at the following raw reclaim
boundary, leaving one actual map unlink and allocator release.

Verified C9 real whole Level2 authored411/Dolmen108 case PASS0.118s: declared
carried Castle/funding/player pose, public paid flyer, actual owned root and
retained BUILD1 child, exact57390 cell/old generation, hidden packet and DTO
exclusion, numeric callback/phase order, natural following reclaim, cleared
physical class0 target read, actual free slot/no map discovery, no stale
Dolmen scheduling, deferred no-cast and raw/typed/DTO reset. No scenery/NPC,
terrain, RNG, allocator, objective or lifetime forcing. Source-identical C7
layout/clearance, public Castle tiers/failures, Day3/Cave24/Level9 startup,
shared Tree burial and numeric-order checks pass; strict Rig workspace/all
targets passes49.65s. The old model42 clock check fails identically on C6
and C7 at byte3b0 versus1; source60480 overrides that byte to zero. No assertion
was weakened or this existing mismatch hidden.

Preserved C7/C8 negatives: C7 stops before casting on House11 site exclusion;
C8 corrects the fixed site/hidden-flyer read and reaches reclaim, then stops on
an invalid None expectation for a retained physical pointer. C9 requires the
source-correct cleared packet plus independent allocation/discovery/DTO
absence. Bounded debugger controls established only those setup/API facts;
the final normal executable supplies the positive. Independent production and
gate review approves the joins. UBS has no added-line critical finding;
whole-file formatting/scanner debt remains explicit.

During C9, another task edited three canonical Rust/ABI inputs. Its work is
preserved and excluded from this delivery. The original guard failure remains;
Root independently verifies all654 inputs in clean committed dispatcher/Rig,
before/after artifact identity and installed bytes, rather than claiming the
dirty canonical checkout matches. One normal canonical Original map/loading/
NightSpire terrain/HUD smoke passes with the matching mapped native, one
portal and zero retries; owned player/display stopped. No Unity/art rebuild.
This is not native burial gameplay, human combat, FPS, hardware audio, full
missions or old-save proof. Evidence: `generated/scenery-20261108-integration/`
C7/C8/C9 and `castle-burial-launch-smoke` under `research/private/`.

Full `.13`/`.10`/`mc-k5k.9` acceptance/dependencies remain open. The then-next
outdoor7/8 constructor/property and real Day3 paid collateral→smoke→cleanup
capability is now installed above. The former forced-actor lifecycle control
was not used as its positive. Night Level7 group10 has eight model7 rows; its
objective-gated triggering path remains unqualified. The incorrect outdoor7
Cave-only null-adapter join is fixed; genuine Cave7/8 null behavior remains.
Root CopperMountain owns shared integration/builds; NavyBridge owned the
connected family implementation through verification.

### Installed paid Tree burning — partial `mc-bhq.13` (2026-11-08)

Source checkpoint `8eae0efe` is committed/pushed; matching optimized Original
native `e36c2c64a7d9` is installed through `build-gqmv2iqa`. The new source is
verification-only (`cfg(test)`); it qualifies existing gameplay rather than
claiming a new production behavior. Real Level3 authored row14/Tree95 receives
two distinct public, paid FireballI impacts: actual mailbox subtraction
300→50→−200, attributed GroundFire254/generation3 construction at turn10 and
its same-pass callback, original sprite descriptors, natural fire hide and
following-turn160 reclaim/map unlink, and the persistent stump. The deferred
no-cast world uses the same declared player pose/aim and leaves the tree intact.
No target health, actor RNG, lifetime, allocator or objective state is forced.

The connected check passes in 0.477 s; affected imported damage, constructor and
lifecycle/RNG controls pass in 0.095/0.078/0.169 s. Package all-target typecheck,
format/diff checks and independent bounded gate review pass. UBS reports no
critical finding; added-line warnings are deliberate test assertions/access.
An extra Level7 self-alias control cannot open its old MCI3v1 package because
ordinary BUILD39 is absent. It stops before the Tree callback; that rare coverage
is not claimed or weakened. Full scenery acceptance and dependencies stay open.

Root verified all 654 unique inputs and the exact worker/installed artifact,
viewed canonical map/loading/NightSpire terrain/HUD with the mapped matching
plugin (one portal, zero retries), then stopped owned smoke processes. This is
launch/load, not native Tree combat, human play, FPS, hardware audio, full missions
or old saves. No Unity/art rebuild. The same bounded direct-Rig recovery used
the existing warm target without changing compiler/profile/guards; core/native
builds took 507.980/200.029 s. Evidence:
`research/private/generated/scenery-20261108-integration/candidate-1` and
`original-launch-smoke`.

The subsequent Crater/Dolmen checkpoint above qualifies the shared terrain,
contact, targeting and presentation path. Preserve source Stone/Dolmen
no-destruction behavior. NavyBadger owns the connected family; Root implements
shared joins and serializes verification.

### Qualified installed Cave paths and rooms — `mc-bhq.5` (2026-11-08)

The existing implementation is committed at source checkpoint `dfb6eb15` and
present in installed Original native `74632f319df3`. Root verified seven existing
focused checks using exact C2 executable `975930593d50`, without another compile
or source edit. Real generation across Cave missions 4/6/8/12/15/16/21/24 matches
all five planes, map heads, retained 176-byte rows, exact free-stack order and
global/terrain RNG against owner boundaries. Reset and all 65,536 floor/ceiling
samples reach the ordinary LegacyTerrain consumer. Mission 4's 274 allocations and
real room 13 construction produce usable dual-plane geometry. Existing focused
controls also pass constructor/default/failure, typed group properties, no-op
class2/7, linked paths, dynamic80 hide-only and separate shaped phases/RNG.

NavyBadger and the independent bounded source reviewer found no concrete
remaining finite-family producer or immediate-consumer gap against pinned
remc2. Root independently inspected the source and exact runtime receipts.
The installed corridor gate covers the same constructed-Cave movement and
indexed-renderer boundary; it was reused rather than adding a duplicate test.
The Mission4 startup check's first ordinary step asserts tick only, so it does
not prove an error-free first live turn. Configured ambient prerequisites do
not prove a natural birth. No full-mission, native Cave, human/FPS/hardware-audio
or old-save claim follows. Matching 654-input native proof and the just-completed
canonical launch/load smoke are reused unchanged. No duplicate build or smoke
was needed. Evidence: `cave-corridors-20261108-integration/candidate-2/`
`cave-features-*` and `cave-feature-*` receipts, plus the installed proof above.
Original acceptance/dependencies remain intact; broader assembly stays open.

### Installed Cave corridors — `mc-bhq.2` (2026-11-08)

Source `dfb6eb15` is committed/pushed; matching optimized Original native
`74632f319df3` is installed through `build-393_ge8i`. All 654 unique inputs and
the worker/installed artifact match. Root viewed canonical map/loading/NightSpire
terrain/HUD with that mapped plugin, one portal and zero retries, then stopped
the owned player/display. This proves launch/load, not native Cave traversal,
human play, FPS, hardware audio, complete missions or old saves. No Unity/art
rebuild was needed.

The finite runtime was already implemented in `world::terrain::cave_corridors`.
Fresh source review found no production mismatch. NavyBadger replaced the
connected check's direct opening-command allocation with actual Mission4
switch124/group37/command582 and switch125/group40/command584. Declared player
approaches remain within real source bounds and legal live Cave geometry;
no controller, switch, group, RNG or terrain is forced. Actual activation,
generation-bound scheduled command consumption, hide/following reclaim, both
floor/ceiling orientations, collision, Cave pixels, settlement, reset and invalid
initialization removal pass. The two connected checks pass in 0.840 s and the
constructor/removal check in 0.028 s on final C2. Sound47 preparation already
used source play type4; Root corrected stale census metadata1 to4 and the actual
scheduled prepared-audio gate passes. The unrelated global audio census limit
remains recorded; this is not a hardware-audio claim.

C1's first player pose was rejected inside the sealed strip before activation.
C2 changes only that declared pose and strengthens overlap/collision admission;
all prior predicates remain. Independent gate reviews found no remaining concrete
gap. Changed-line UBS has no critical finding; existing production formatting
debt is retained. RCH cancelled its initial compile via `stuck_detector` at
201.571 s. Root kept its guards unchanged and used the configured Rig's existing
target for this bounded batch, with unchanged compiler/profile and all 654 input
hashes checked before/after: final core compile 477.170 s, native 200.424 s.
This is documented direct-SSH recovery, not a successful RCH claim or watchdog
fix. Evidence: `research/private/generated/cave-corridors-20261108-integration/`
(`candidate-2`, preserved C1 negative and `original-launch-smoke`). Original
acceptance/dependencies are retained; broader campaign assembly stays open.

### Installed Cave mushrooms — `mc-bhq.1` (2026-11-08)

Implemented, committed and pushed at `25b13176`; optimized Original native
`e666b72d57db` is installed through refresh `build-n3sma5cg`. All 654 unique inputs
match canonical source, isolated checkout and Rig. Root viewed the canonical
map/loading/NightSpire terrain/HUD with the matching mapped plugin: one portal,
zero retries, then stopped the owned player/display. This is launch/load, not
native Cave combat, human play/FPS/hardware audio, full missions or old saves.
No Unity/art rebuild was needed.

NavyBadger completed the owned mushroom callback; CopperMountain integrated its
shared Smoke constructor/current-address seam. Source `651B0` now allocates Smoke13
before grounding/water, clears ownership through the current occupant and keeps
the persistent dry stalk. Canonical constructor draws, failed allocation RNG,
admissible replacement, current-position aliasing, hidden packets and following
reclaim stay source-correct. No gameplay moved into the client or scheduler.

C1's four focused checks pass, including non-Cave/exhaustion, damage boundaries
and actual replacement/failure controls. C2 adds only a 64-line test continuation;
production is byte-identical. Its real unfiltered 56-mushroom Cave12 path passes
in 0.219 s: ordinary open/step/reset, declared carried FireballI and player pose,
public paid cast against original slot30, exact 250 accepted damage, stalk/smoke DTOs,
natural smoke hide/reclaim, then declared water material on that same stalk and
ordinary hide/following-pass unlink/reclaim. The water/pressure controls are
explicit configurations, not natural terrain-change or exhausted-owner proof.
Workspace all-targets check passes on unchanged production; final package
all-targets check passes. UBS's two changed-line critical flags are test-assertion
false positives; C2 adds no critical finding. Legacy whole-file debt remains.
Two fresh bounded source/gate reviews found no further concrete finite-family
gap. Root independently verified the integrated evidence and matching delivery;
the original acceptance and dependencies are retained. Evidence:
`research/private/generated/mushrooms-20261108-integration/candidate-2` and
`original-launch-smoke`; C1 retains the four focused checks and source-order gate.

## Delivered creature runtime checkpoints

Oliver requested working through the remaining creature list on 2026-11-04.
Continue the existing original `mc-1pb` acceptance and dependencies; this list
tracks delivery, not missing-code counts. Settlement `.16`, Skeleton `.22`,
Firefly `.23`, Manticore17 `.6`, Sentinel18 `.7`, leaping creature21 `.8`
and paired-body creature22 `.9` are delivered. Mana-hunting creature23's connected
runtime is installed; `mc-1pb.10` retains its open shared-system dependencies.
Cave melee creature24 `.11` and splitting creature25 `.12` are delivered and
closed. Spell-thief26's connected runtime is now installed; `mc-1pb.13` retains
its original open `mc-k5k.7` dependency. Deep One28 `mc-1pb.15`, Worm3
`mc-1pb.2` and Spider20 `mc-bhq.3` are delivered. Shared fields `mc-bhq.12`
and Hydra27 `mc-1pb.14` are delivered and closed. The proper original
names of models21/22/23 remain unverified. Name each active creature alongside its source model in
progress updates. Root
serializes shared joins, Cargo and installation with the small native team.
Complete one creature through verification and installation before starting the
next. Parallel workers tackle disjoint parts of that same capability. Use the
existing source and installed checkpoints rather than restarting delivered work.
Each family must connect ordinary production, source scheduling/arithmetic,
behavior and spell interaction, consequences/rewards, natural cleanup,
objective consumers and original presentation/assets. Preserve the delivered
spell checkpoints and already qualified creature paths; repair shared defects
in their canonical owner when a real connected failure exposes them.

AmberDune and CalmLynx delivered creatures23–25 through their actual producers,
combat, consequences, cleanup and original presentation. CopperMountain
serialized checks and delivery. FrostyPond (previously AmberDune) and RusticGrove
have now completed creature26's finite runtime, Deep One28, Worm3, Spider20,
the shared-field prerequisite, Hydra27 and shared combat. Player lifecycle,
rival wizards and final boss now also have installed runtime checkpoints below;
retain their original open shared-system dependencies. Read each
original acceptance and current source; extend its existing family.
`mc-1pb.10` remains open while its original
`mc-bhq.10` shared-system dependency is open; move the actual required joins forward
without bypassing or rewriting those dependencies.

- [x] Wyvern16 — `mc-1pb.5`.
- [x] Castle defenders15 — `mc-1pb.4`.
- [x] Dragon0 — `mc-1pb.19`, including the current-parent/predecessor corrections below.
- [x] Bee2 — `mc-1pb.20`.
- [x] Archer4 — `mc-1pb.21`.
- [x] Skeleton9 — `mc-1pb.22`.
- [x] Firefly19 — `mc-1pb.23`.
- [x] Manticore17 — `mc-1pb.6`.
- [x] Sentinel18 — `mc-1pb.7`.
- [x] Leaping creature21 — `mc-1pb.8`.
- [x] Paired-body creature22 — `mc-1pb.9`.
- [x] Mana-hunting creature23 runtime — `mc-1pb.10` (bead retains open shared dependencies).
- [x] Cave melee creature24 — `mc-1pb.11`.
- [x] Splitting creature25 (port name Cymmerian; original name unverified) — `mc-1pb.12`.
- [x] Spell-thief26 runtime — `mc-1pb.13` (bead retains open `mc-k5k.7`).
- [x] Deep One28 — `mc-1pb.15` (original sound-name binding verified).
- [x] Worm3 — `mc-1pb.2`.
- [x] Spider20 full ordinary web/contact/death capability — `mc-bhq.3`.
- [x] Hydra27 — `mc-1pb.14`.
- [x] Builder/Villager/Trader — `mc-1pb.16`.
- [x] Single-player rival-wizard runtime — `mc-1pb.17` (original shared-system dependencies remain open).
- [x] Player death, banishment and revival — `mc-1pb.18`.
- [x] Final boss10 runtime — `mc-1pb.3` (bead retains open Meteor dependency).
- [x] Shared actor contact/damage/linked death/collectible mana — `mc-1pb.24`.

Goat (`mc-1pb.1`, already closed) and the delivered Spider20 family are inputs. Existing
18/25/26 checkpoints are installed inputs, not reasons to replay unchanged
long checks. An unchecked family may already be substantially implemented.
Original full acceptance remains open until its evidence is independently
verified and matching native delivery lands. No automatic heartbeat is recreated.

### Installed canonical rival family/lifecycle — `mc-1pb.17` (2026-11-08)

Implemented, committed and pushed at `16d953fe`; matching optimized Original
native `2d58e46e` is installed through refresh `build-fsmagut5`. All654 unique
inputs match canonical source, isolated checkout and Rig. Root viewed the normal
canonical map/loading/NightSpire terrain/HUD with that mapped native, one portal
click, zero retries, then stopped the owned player/display. This is launch/load,
not native Mission18 combat, human play/FPS/hardware audio/full missions or old
saves. No Unity/art rebuild was needed.

Actual rival state/behavior and action2/3 death, scattering, banishment and
sequential Castle revival now belong to `creatures::rival_wizards`. The old
implementation files are moved, not duplicated. Narrow borrowed domains replace
whole-session lifecycle methods; shared composition retains the flight-journal
and ordinary reclaim/rebuild barriers. Root's normalized comparison confirms
the roster move preserves behavior. Current-slot/generation checks, source
allocation order, partial failures and following-pass reclaim remain intact.

Frozen C2's nine exact checks pass: distinct imported roster/shared RNG,
all-rival dispatch/tail, actual paid Meteor, configured mode14, actual Morph,
source order, remote Morph scatter, real whole-Mission23 full1200 Castle revival
(155.705s), and final-Castle loss (0.862s). The configured mode14 gate now asserts
the exact68DE0 before/after debt and unchanged immediate mana, resolving the
prior lower-bound review finding. The last-loss check observes actual owner425,
Castle450 and holder428 against living peer451/holder454 through teardown,
current A4/roster removal and ordinary no-rebuild. It reuses the same compiled
ELF and qualified whole package; no new control or source edit was needed.
Workspace/all-targets release check passes with462 Rust inputs and37 extra
inputs unchanged. Fresh bounded source/gate assessment supports rival runtime
qualification and identifies no further concrete family producer/consumer gap.
It does not require every tactic to occur naturally in one mission. Original
external acceptance/dependencies remain OPEN; `.17` is not force-closed.

Evidence: `research/private/generated/rival-ownership-20261108-integration/candidate-2`
and its Original smoke. Rename-aware UBS found no changed-hunk critical; legacy
formatting/whole-file findings remain. Failed C1 compile and its exact source
are preserved; the retained scatter test now calls the canonical helper with
its original assertions. Completed return artifacts are verified on Rig.

### Installed final boss paid combat and natural death — `mc-1pb.3` (2026-11-08)

Source `e013f088` is committed/pushed; matching Original native `f0c2dee3` is
installed through `build-fyccf0jm`. All654 unique canonical/isolated/Rig inputs
match. Root viewed canonical map/loading/NightSpire terrain/HUD with the mapped
matching native, one portal and zero retries, then stopped both owned processes.
This is launch/load, not installed-client boss combat, human play/FPS/hardware
audio, whole missions, reward91 collection or old-save proof. No Unity/art change.

The actual whole-Level25 boss120/g1 retains its default300000 life. Paid Meteor
first hits and the continuous remaining996 consumptions/250 paid launches reach
the exact source300 cap and terminal300→8. Natural phases13/14/15, resident345
timer, original sprite/audio, nonliving/absence membership, hide, following
reclaim and active-counter0 pass. Death Meteor326/g282 and Volcano180/g259 reach
the real scheduled reward91 product926/g3. Byte36E03 is Volcano mode, not mission
victory. The separate BUILD68→69→72→73→group29→record379 producer check passes;
its ordinary damage-mailbox controls do not prove public spell demolition or
the preceding mission route.

Source fixes complete phase0 progress60/38×38 burial before approach, signed16 XY
proximity, success-only projectile A4 feedback before sound, and success-only
Volcano mode clear before its current-owner read. The real fight exposed an
ordinary Eruption→Fire Spheres sector77 collision: its typed constructor axes
were dropped by the shared target reader. The canonical projection now preserves
those stored particle340 axes independently of visual42/orbit angles/raw shadow.
C8's failing full-fight gate is byte-identical to passing C9; no admission,
payment, HP, timer or failure assertion was weakened.

C9 full defeat PASS27.835s, real BUILD/burial/A4 consumers1.086s, natural Firefly
production/parent-loss2.702s; current-sector, outgoing mailbox, acquired Fireball
and source-order checks pass. Release workspace all-targets PASS39.770s. C6's
affected approach/phase0/four-projectile/exhaustion/death-owner controls passed
against the same boss/integration production code; C9 adds the shared collision
projection and diagnostic. Fresh bounded source/gate/acceptance reviews
found no additional concrete defect; changed-line UBS critical0, legacy whole-file
and formatting debt retained. Core ELF SHA028f060d independently matches Rig.

Controls are disclosed: prior group29 progression, carried MeteorII, real Castle
funding, equal current/max200000 player life, initial placements/aim, natural
quiet waits, one westward placement and unchanged held input. No NPC life, RNG,
action or lifetime is forced in the defeat run. C4's end-turn mailbox-owner gate
was corrected to the actual store boundary plus same-generation refill evidence;
C6's blocked east firing point and C7/C8 negatives remain preserved. These are
fixed-input runtime checks, not an AI playthrough. Four summons and exceptional
branches retain separately configured coverage. Evidence: generated/
boss-audio-20261108-integration/candidate-9 and original-launch-smoke-9 under
research/private. The creature runtime checklist is covered; original bead
acceptance/dependencies stay unchanged and OPEN, including `mc-bhq.4` and whole
campaign-ending assembly. Do not restart these delivered creature checkpoints.

### Installed final boss audio, source tails and death ownership — `mc-1pb.3` (2026-11-08)

Implemented, committed and pushed at `351793fc`; matching optimized Original
native `dedc2c40` is installed through refresh `build-cj8_6p33`. All654 unique
inputs match canonical source, isolated checkout and Rig. Root viewed the normal
canonical map/loading/NightSpire terrain/HUD with that mapped native, one portal
click and zero retries, then stopped the owned player/display. This proves
launch/load, not native boss combat, human play/FPS/hardware audio/full missions
or old saves. No Unity/art changes were required.

Boss attack, approach, pressure and death sounds now reach prepared native audio
at their source-time reads. Null-emitter centering/pitch, current ID-addressed
emitters, mute/listener order and non-advancing RNG reads are preserved. The
missing ground/facing tail now uses current player state, frame timing and source
turn arithmetic. Source58490 is signed-16 XY distance despite its XYZ name;
boss targeting/protection use that actual arithmetic. Summon gates, death-child
owner reads after allocation and family ownership of the Volcano reward are
corrected without replacing generic constructors.

Frozen C3 passes the real whole-Level25 boss's natural Firefly production,
flight, audio and parent-loss cleanup (2.714s), and actual BUILD68 demolition
through group29/boss production and approach audio (1.015s). Configured controls
cover four child branches, projectile construction, exhausted allocation,
ground/facing, protection and death sound/owner order; native audio drain and
source-order checks pass. The workspace release all-targets check passes.
Fresh bounded source/gate review found no introduced defect. UBS found no critical
on changed executable lines; legacy formatting/whole-file findings remain.

C1/C2 negatives are preserved. Their natural-child fixture funded current player
life without matching maximum life, allowing source12A70 to clamp the declared
survival control. C3 funds the same200000 as both current/maximum, leaving boss,
NPCs, RNG, lure, lifetime bound and assertions intact. The pressure callback
control proves family call order; production movement was inspected statically.
Configured death controls do not establish a naturally earned defeat.

Evidence: `research/private/generated/boss-audio-20261108-integration/candidate-3`
and its Original smoke. Full boss acceptance stays OPEN/unchecked. Next is the
successful Volcano constructor's missing source-global clear before its owner
read, then one real paid Meteor hit through same-generation boss consumption
and the complete natural death sequence. Preserve default300000 boss life and
source300-per-callback cap. The Volcano mode flag is not mission victory;
whole final-route/ending assembly remains in its original owning beads.

### Installed rival Morph steering/full activation — `mc-1pb.17` (2026-11-08)

Source `b4c87923` is committed/pushed; matching optimized Original native
`68f5cce4` is installed through refresh `build-nioe6r6x`. All653 unique inputs
match canonical source, the isolated build and Rig. Root viewed the canonical
map/loading/NightSpire terrain/HUD with that mapped native, one portal click,
zero retries, then stopped the owned player/display. This is launch/load,
not native Mission18 combat visuals, human play/FPS/hardware audio/full missions
or old saves. No Unity/art rebuild was needed.

Both source161A0 calls now use full14E10; steering reads signed A0 descriptor14
instead of the unrelated A4 movement latch. Source narrowing, roll-write order
and steering without the Morph holder are preserved. C4's real whole Night18
selector14 path, all six rivals/111 holders intact and only a declared human
midpoint position control, PASS2.881s: own Morph189 creates body238/gen2 at38,
queues the exact7500 current-parent charge, follows its actual callback-time
parent, presents original DTO/audio, runs the full301 visits, hides and retires
on the following pass. The semantic receipt validates the real direct or current
generation-bound fallback; shared0 is not mistaken for failed admission.

The explicitly configured secondary path PASS0.168s includes actual child210's
clock18 construction, Castle209's state2/action4/owner publication and following
60F00 capacity79600, then same-pass12910/161A0/Meteor194, real tier-I cost6000,
queued/next-upkeep debit, flyer motion and natural hide/reclaim. Its controls
include mode14, actor/human geometry, funds, cooldowns and avoidance byte45e=3.
It proves the6000 holder price and at-least-cost debit; a fresh independent
review found that its exact68DE0 before/after assertion must be strengthened.
RedElk owns that finding in the next canonical-family batch; the existing
ordinary Meteor exact-charge oracle remains covered. No production defect was
found in this bounded review. Full `.17` stays OPEN, including canonical family
ownership/lifecycle composition and all original dependencies.

C3 actual rival-Meteor/source-order/malformed-holder checks remain applicable
after byte-identical production and other-test-body comparison; C4 changes only
the configured construction preparation. Earlier negative gates/source snapshots
are preserved. Package formatting debt remains; changed-source UBS had no critical
added-line finding. Evidence: `research/private/generated/rival-mode14-20261108-integration/candidate-4`,
its reuse/closure receipts and Original smoke. Completed build outputs are
byte-preserved on Rig; current and immediate previous natives remain local.

### Installed rival-selected spells — `mc-1pb.17` (2026-11-08)

Source `7d9c8399` is committed/pushed; matching optimized Original native
`1ba22da8` is installed through refresh `build-swuv2zwb`. All653 unique build
inputs match canonical source, the isolated build and Rig. Root viewed the
canonical map/loading/NightSpire terrain/HUD with that mapped native, one portal
click and zero retries, then stopped the owned smoke. This is launch/load,
not native Mission18 combat visuals, human play, FPS, hardware audio, full
missions or old saves. No Unity/art rebuild was needed.

The corrected source15170/15F20 resource, applied-tier and admission rules,
14E10 selected-family aim/direct branches and5F660 active-holder rules now reach
ordinary rival casting. The whole unfiltered Night18 path retains all six
rivals,111 holders, original resources/health and neutral input. It observes
natural Castle accounting, selected Meteor payment, exact accepted-writer and
target-generation damage, real raw0 explosion-child DTOs and natural hide/reclaim.
Earlier misses must pay and finish cleanup; the invisible17 controller is not
invented as a sprite. C3 PASS0.652s. Its only delta from C2 removes an unrelated
optional raw/legacy snapshot filter; actual scoped writer sequence and full
impact generation still bind exact HP consumption. C2's source-identical
configured boundaries, roster, dispatch, actual Bee/Fireball, source order and
full natural Castle1200 revival PASS156.451s are reused. Fresh bounded review at
the committed revision found no introduced defect in these five files.

Full `.17` remains OPEN with original acceptance/dependencies. The subsequent
mode14 correction is installed above; canonical rival-family ownership and
remaining acceptance still require implementation and verification.
Seven adjacent legacy failures reproduce identically on the previous installed
C7 binary; their later assertions remain unproved. Old Mission18 lacked Castle
BUILD resources: a new whole package from the qualified current160-input exporter
is used read-only, preserving old data. Evidence:
`research/private/generated/rival-spells-20261108-integration/candidate-3`,
with C2 regression reuse and the saved baseline comparison. Completed outputs
are byte-preserved on Rig; derived scanner caches are losslessly archived.

### Installed player lifecycle — `mc-1pb.18` (2026-11-08)

Source `7487708b` is committed/pushed; optimized native `3965057f` is installed
with the matching Original player `build-edg_38r2`. All653 unique native build
inputs match worker/local source and the installed artifact. The two focused
Original Unity checks pass with200 source/settings inputs unchanged. Root viewed
the canonical map, loading artwork and NightSpire terrain/HUD with that mapped
native, zero retries, then stopped owned smoke. Bead `.18` is closed after
independent bounded original-acceptance/source/gate review and root verification.
This is not a native death-encounter visual, human combat/FPS/hardware-audio,
whole-mission or old-save claim. The previous player and native remain preserved.
Rival wizards `.17` and final boss10 `.3` remain unfinished; preserve their
original dependencies and deliver the next real missing connected capability.
RedElk now owns `.17` through integration: actual imported rival profiles and
their own holders must reach source AI decisions, real casting/Castle/mana,
hostility and defeat/revival consumers. Inspect current code before treating
the bead's old Nyphur-only assessment as truth. Select one concrete unmet join
for the next batch; reuse delivered spell and lifecycle paths. CopperMountain
owns shared integration/checks/delivery. `.17` retains its open prerequisite
acceptance and remains open; no dependency bypass or roster-wide claim.

RedElk and RusticGrove delivered this original capability with CopperMountain
owning shared joins, serialized Cargo and Original installation. It connects
ordinary lethal combat through actual held-spell scattering, owned mana
and death-marker binding, source death camera/banishment, natural rival Castle
revival or no-Castle removal, and the actual flow/objective consumer. Preserve
full original acceptance and source1200 countdown/allocation-failure behavior.
Read current source instead of the bead's historical file paths. Extend its
canonical family with owned state/behavior; keep scheduler/ABI wiring thin.
AmberStream's voxel joins landed at `b0db8ae4`; their shared source windows were
handed back. Keep their unfinished isolated diagnostics and client work separate
from this capability. Later rivals/boss acceptance stays queued with its existing
dependencies intact.

The lifecycle candidate now implements source fall/camera, current A4 Castle
revival, full-arena burst/marker/holder births, mutable source spawn XY and public
no-Castle recovery, plus per-participant original-font notices with a separate
guarded, replayed presentation clock. The following records the qualification
sequence; current installation is stated above. Candidate3
core/CAPI compilation passed with460 unchanged Rust inputs. The current-carrier,
generation-bound Morph/Bee repair passes its focused discriminator1.092s and
existing acquired all-tier/follow/expiry/reclaim check47.639s; source order0.617s
and actual CAPI paused-clock/guard/replay5.760s pass. Candidate2's six family and
public recovery controls remain unchanged. The full Mission23 natural1200 check
failed1455.246s on Skeleton187's damage-target236 lookup. Its exact late target
packet was not captured. RedElk's reviewed repair reads retained/current typed
identity only at the actual source-reached reaction, preserves inner20940 versus
outer73/79 tails, prior nonfatal word24 and cadenced axis-read order. Actual74
still uses20C50; the misleading private20940 helper was renamed without an alias.
All seven focused Skeleton timing controls pass on C5; its retained/reused
control stopped because the fixture discarded the available real collision bank.
That test now supplies the same bank, actor domains and player as production.

C4's actual FIRST-cast carriers and source68D50 establish missing Castle
prerequisites: LightningI30000 with no Castle, MeteorII80000 with only50000.
Both public activations were accepted; source correctly suppressed production.
C5 uses one Castle-free paid FireballI provocation and the actual80000 Castle
entry requirement, preserving health, geometry, timing and downstream predicates.
No spell-admission production change was needed. C5 release compilation passes
with460 unchanged inputs; actual Mission23 defeat, full natural1200 countdown,
real Castle revival and new holder generations PASS172.526s. Family controls,
public recovery and CAPI pause/guard/replay also pass. Its human encounter stops
at the declared contact point's terrain-clearance guard; C6 derives legal Z from
that same XY and adds strict source envelope/range checks. Castleless death,
banishment, objective and notice checks pass up to the final dropped-holder
reclaim assertion. A genuine missing scheduler join left ordinary/persistent
families0–2 without68FF0 callbacks. Root's reviewed fix admits those pickup roles
while retaining dedicated active0/1/2 routes and dynamic/78 precedence. The same
expiry assertion remains unchanged. C6 integrates this join and both exact
fixture prerequisites onto delivered ABI base `d6665e38`. Release compilation
passes with461 unchanged inputs and serializedcodegen1. Actual Castle revival
through its full1200 countdown PASS181.159s; Castleless defeat, banishment,
objective and all26 dropped-generation natural cleanup PASS2.024s. The real-bank
retained-address control, active Fireball/Possession paths, public recovery,
source order, CAPI pause/clock/replay and additive world-ABI boundary pass.
The C6 human encounter reached actual lethal damage, fall, scatter, marker and
owned mana, then stopped at the marker's original presentation assertion. The
typed marker was excluded by the dynamic-slot raw fallback without a matching
typed DTO join. C7 adds the source-owned marker presentation helper and one
canonical consumer join; the original check remains unchanged. C7 compilation
passes with461 unchanged inputs; the complete actual human encounter, original
marker/burst DTOs, source camera, notice clock and public recovery PASS1.239s.
CAPI pause/clock/ordered replay PASS0.652s and marker hide/reclaim PASS0.082s.
Independent bounded original-acceptance/source/gate review found no further
concrete defect. C6 and C7 differ only in the two pure marker-projection joins;
reuse C6's qualified gameplay/1200 checks. Completed C6/C7 outputs are
hash-verified before/after on Rig. New-family UBS has no critical or production
warning; source wrapping arithmetic is intentional, with test/legacy findings
and whole-package formatting debt retained. The integrated strict-Rig workspace
all-target check PASS51.165s with461 Rust and37 extra inputs unchanged. It
returned no local compiler artifacts; no cleanup or repeated check is needed.
Frozen Original-only runtime and test C# compilation passed with68 immutable
source inputs; both actual Unity checks and matching native/player delivery
subsequently qualified as recorded above. Original24df remains preserved as
the immediate previous native.

Evidence is in `generated/player-lifecycle-20261107-integration` under
`research/private/`. The next source candidate uses the existing native
worktree with owned files only, excluding another owner's unfinished cfg fixture.
Its applied Original UI patch contains only this notification consumer. Local
Cargo cache bytes/links/timestamps were verified and relocated to existing RAM
storage at the same target alias; duplicate returned metadata and completed test
ELFs were byte-preserved on Rig. The inactive Original UnityLibrary cache also
retains its5249 entries, byte/link/mode/time-verified at its original path via
an existing RAM alias. Keep the5GB root floor and measured7.2GB heavy
stage reserve; `/tmp` has only about0.85GB free. Do not remove source, data, current
players or remote warm caches to create room.

### Previous installed delivery: shared creature combat (2026-11-07)

Implemented, verified, committed and pushed at `1e43eb6c`; installed Original
native `24dfac3f528a38d09fc2806318aa2a021211ff0d4421737ddf505178a804636f`
(139087984 bytes), refresh `build-9puvqwkk`. `mc-1pb.24` is CLOSED after fresh
bounded original-acceptance review and independent Root verification.

Actual Cave4 Spider kill now continues through its original same-generation
2750-mana sphere, one paid public Possession, real collector pickup, once-only
Castle credit and following reclaim (51.665s). Every original web/contact,
Meteor/credit/death/objective/survivor predicate remains. Declared carried spells,
player placement and a post-claim empty Castle store preserve the real gathering
gate; this does not prove earned campaign progression. Actual paid FireballI
through both published Goat600/1200 max-life definitions reaches the original
impact/mailbox and250 life loss (3.813s), with real parent charge, later player
debit and equal-elapsed no-cast controls. NPC pose/life/RNG remain unchanged.

Admitted player contact now follows source11900 u16 narrowing and contact-mailbox
semantics, including zero/ownerless/dead writes; actual5EFA0 retains its later
life guard. Natural protection expiry, zero/positive hit/cooldown/regen, Castle
transfer, wrapping/residue and linked/safe3/4 controls pass. Safe lanes remain an
explicit repair of invalid C++ arithmetic; full spell casts retain their own
acceptance. Single-slot arena lookup preserves generation/current-carrier
priority; actual Hydra attack/paid body-head-root regression passes422.204s.
No speedup/FPS claim follows from that bounded measurement.

Evidence: `generated/shared-combat-20261107-integration/candidate-6` under
`research/private/`, with explicit byte-identical production reuse of C5/C1.
Workspace alltargets17.877s and strict rig native65.213s pass. All452 Rust,
162 upstream and37 extra inputs (644 unique) match the rig and qualified Original
installation. Separate pending voxel source/ABI/client work is excluded and
preserved. Root viewed canonical map/loading/NightSpire terrain/HUD under the
initial white spawn flash, verified the mapped native, and stopped owned smoke;
one portal click, no retry/gameplay input. This is launch/load, not native combat
visuals, human combat, FPS, hardware audio, full missions or old saves. No Unity/art
change. Legacy formatting/whole-file scan debt remains; no critical added-line
finding in the inspected owned changes.

Use original configured RCH with `RCH_CONFIG_DIR` unset, existing socket and
strict rig dispatch. The failed ExFAT staging experiment remains unused; its
successful supported C1 recovery and the accidental unchanged C3 compile are
explicitly recorded, not counted as new verification. Heavy stages require7.2GB
measured headroom above the5GB floor; keep latest caches/current player/previous
native. Known returned outputs and superseded incremental sessions are preserved
on rig; nine closed scan reports are losslessly compressed with decompression
hash receipts. Current24df and previous Hydra05d2 natives remain local.

### Previous installed delivery: Hydra27 (2026-11-07)

Implemented, verified, committed and pushed at
`98f2c2ee903f8d8aa1834c34057d718cec2823e1`; original finite `mc-1pb.14`
is closed with acceptance and dependencies unchanged. FrostyPond/RedElk owned
family behavior, RusticGrove the connected checks, and CopperMountain the
shared joins, builds and installation. Bounded independent source/gate review
found and rechecked the repaired enabled paths; root verified actual results.

The family now preserves descriptor97 turning/material/pitch/slope rules,
216's timed flags/relocation and two RNG draws,217's fatal early return,
218's alignment-before-range animation reset, literal head-tail scratch/map
copies and source body-state admission. Fatal local head flags survive packet
commit. Original233/234 disabled callbacks and222 unsupported classification
remain intact. Source15/17/22/23 freeze audio at their literal current-carrier
calls into prepared Flight; fatal collapse does not invent idle5/6 sound17.
Ordinary Level23 exposed the shared eighth spawn setter11 omission; the canonical
initial Castle input now includes it and reaches actual player7/Castle611.

Frozen C4 unfiltered Level23 gate passes508.489s: real authored producer,
all51 original parts, both850 projectile lineages into actual player damage
and prepared audio, then one paid MeteorIII actual body661/g2 collision into
eligible head damage capped76 and original root646/g2 mailbox1600/attacker424.
Its carried spell/resources, objective setup and fixed lure are declared controls;
this is not a default-health defeat. Actual Level22 configured near-end head
health then fatal contacts reach source death, real bound target objective,
all51 hide/following reclaim and burst lifecycle in6.968s. Four new source-order
controls and affected capacity/reuse/reward/startup/order regressions pass.
Workspace all-targets42.505s and strict rig native75.516s pass. All452 workspace,
162 upstream and37 additional inputs (644 unique) match source, rig and client.
UBS's six whole-file criticals do not touch added lines; added findings were
triaged. Family behavior formatting passes; shared-file formatting debt and one
test path-wrapping hunk remain explicit, without a style-only rebuild.

Installed Original native
`05d22ec76a3ef8c1c62f42beaa948099e0f3e15ed0b1f919b5c8603c7a1b58f0`
(139097344bytes), native refresh `build-qu2uk4jo`; no Unity/art change. Root
viewed canonical map/loading/NightSpire terrain/HUD under the spawn flash with
that mapped native: one click, zero retries, no gameplay input; owned smoke
stopped. No native Hydra encounter visuals, human combat, FPS, hardware audio,
full missions or old-save proof. Current/previous natives and previous full
player remain. Completed returned artifacts are byte-preserved on rig; transient
native transfer exceeded the former output budget, so future heavy stages need
the actual measured transfer peak included. Evidence:
`research/private/generated/hydra-20261107-integration/candidate-4/` and
`research/private/remc2-unity-native/Builds/delivery-hydra-20261107-qualified-smoke/`.

### Previous installed delivery: shared fields before Hydra27 (2026-11-07)

Implemented, verified, committed and pushed at
`06046071a0466257b8ff75b7a268fcecc607e75b`; original finite `mc-bhq.12` is
closed with acceptance and dependencies unchanged. Transport21/22/20 and
Storm-child23 now freeze positional audio at their literal source calls, before
player copy or second child allocation, and reach the real native consumer.
Retained ordinary transports already have startup flag2; their arrival gate
preserves it. A separate real constructor proves first21 and no repeat.

Source33/action35 has typed default300/512 properties, actual NewEvent RNG
projection, increment/decrement/old-life behavior and the full scheduling join.
The connected check exposed a second admission list silently skipping35; that
canonical join is fixed. Bare constructor35 returns an actual unstamped class0
NewEvent and is registered distinctly from null. Storm38 uses each current child
selector and sequential postwrites. Bare default10/0 and payload100, ordinary9/9,
null entries and partial allocation remain distinct. No ordinary33/35 campaign
producer is claimed; configured Storm calls their real constructors. Disabled
constructor37 and enabled configured/restored follower behavior retain their
existing classification.

C4 full natural300 controller life/hide/following reclaim passes9.141s; ray RNG,
exact call-time/final map links, registry and source order pass. The old packet
oracle compared before-second-birth state with its later map link; the repaired
gate explicitly proves that sole link change and keeps all other fields equal.
Unchanged C3 evidence covers actual transport2.299s, native voice1.770s, acquired
all-tier Lightning/Storm10.460s, bare child presentation/reclaim2.035s and
configured null/capacity controls8.456s. C3/C4 differ only in admission35,
constructor35 registration and that test oracle. Workspace all-targets39.088s
and strict rig native68.872s pass. Fresh bounded reviews and root's exact-source
checks support finite acceptance. Focused formatting passes; UBS has no critical
added-line finding. Whole-file legacy debt remains.

Installed Original native
`8d7df89de38eb4e7fb2676e7b7cb07048d2152a48ba11f3aab72d1fd48aca566`
(139069232bytes), native refresh `build-cfkkf4tl`. All452 workspace inputs,
162 upstream and37 additional inputs (644 unique) match source, rig and client.
No Unity/art change. Root viewed canonical map/loading/NightSpire terrain/HUD
under the initial spawn flash with matching mapped native; one click, zero
retries, no gameplay input. Owned smoke stopped. This proves launch/load, not
native field encounter visuals, human combat, FPS, hardware audio, full missions
or old saves. Current/previous natives and the previous full player remain;
completed RCH outputs are byte-preserved on rig. Evidence:
`research/private/generated/transport-audio-20261107-integration/candidate-4/`
with unchanged C3 checks and
`research/private/remc2-unity-native/Builds/delivery-shared-fields-20261107-qualified-smoke/`.
At that checkpoint Hydra27 `mc-1pb.14` was the next owned capability; its
subsequent complete installed delivery is recorded above.

### Previous installed delivery: Spider20 (2026-11-07)

Implemented, verified, committed and pushed at
`0b1d0186620f66acdf044fdc7222c257187e24cd`; `mc-bhq.3` is closed with its
original acceptance and dependencies unchanged. The family owns its source
constructor, actions160–165/167, Stage branches, typed properties and assets,
web producer and death lifetime. Source166 remains unsupported. Corrections
include descriptor-backed turning, current player A4 authority and feedback,
source duration arithmetic and linked state, canonical SourceArena death
allocation, and positional attack/first-hit audio reaching the actual journal.

Frozen candidate8's actual whole Cave4 group6 encounter passes11.992s:
record137 produces its own9/21 → same-generation10/66 web, exact780 damage
and prepared audible first-hit receipt, authoritative movement/status/overlay
DTO and natural recovery. Its contact contributes100 through the actual source
mailbox arithmetic and the same accepted writer to HP and DamageApplied.
One paid MeteorII causes the original's fatal subtraction, one kill credit,
mana/burst presentation, natural hide and following reclaim. The other five
originals and equal-elapsed no-cast original survive. Declared entry resources,
prior carry/task state, fixed lure, paid Lightning provocation and fixed shot
pose are controls; the check also permits one disclosed player-only contact
placement. No NPC pose, RNG or lifetime is forced. This is not earned mission
progression or an automated fight.

Earlier negatives are retained: tierIII legitimately credited two distinct
Spiders, so the representative cast uses the source-supported sufficient tierII
without relaxing the +1/survivor controls. The contact oracle now observes the
real100 contribution rather than incorrectly assuming an empty damage mailbox;
the held aim is set after public selection, matching source update order.
Unchanged-production C2 motion/Stage/web/current-A4/lifetime/reuse/source-order
controls and C3 real six-original absence/proximity checks remain valid. The
absence control uses configured lethal contacts; the adjacent Manticore status
check is a configured constructor/projectile/consumer control. Neither is an
ordinary multi-enemy fight. Fresh bounded independent source/gate/evidence
review found no further concrete defect in the finite family. Workspace
all-targets passes39.153s; strict rig native build passes66.429s. Changed-source
UBS has no critical on added lines; broad legacy scanner/formatting debt remains.

Installed Original native
`e012fc8275700d825e8a4b79b385dd66ab684f7be3902803617aa7d56b25ec63`
(138949400bytes), qualified build `build-l38nnukl`. All451 workspace inputs
(401 Rust source files),162 upstream and37 additional inputs (643 unique)
match source, rig and the qualified Original client. The additive standalone
character header from the separate Modern work required the existing full
Original build qualification; all authored Original Unity inputs are unchanged.
No qualification gate was bypassed. The previous Worm player and native remain.

Root viewed normal canonical map, original loading art and NightSpire terrain/HUD
through the initial spawn flash with the matching mapped native. One held portal
click, zero retries and no gameplay input; owned smoke processes stopped cleanly.
This is launch/load proof, not native Cave/Spider pixels, human combat, FPS,
hardware audio, full missions or old-save proof. Current native copies were
SHA-verified and deduplicated; completed RCH outputs are byte-preserved on rig.
Evidence: `generated/spider-20261107-integration/candidate-8/` and
`Builds/delivery-spider-20261107-qualified-smoke/` under `research/private/`.
The subsequent shared-field prerequisite is delivered above. Keep remaining
creature and shared-system acceptance open until its own complete delivery.

### Previous installed delivery: Worm3 (2026-11-07)

Implemented, verified, committed and pushed at `5034ad97ff92a98db16995133fb46cdf3f9056fc`;
`mc-1pb.2` is closed with its original acceptance and dependencies unchanged.
The family owns actions24/25/26/27/28/31, full StageVars, root plus sixteen
parts, projectile/player feedback and original sprite/audio consumers.
Source30 remains unsupported; its default descriptor does not select it.
Constructor and death now use canonical SourceArena allocation, preserving
partial prefixes, current occupants, source tails and following reclaim.

Frozen candidate7's whole authored Night1 switch/group encounter passes33.299s:
all sixteen body sprites/motion, ordinary target acquisition/9/0 shot/audio,
one paid Meteor tail impact, actual attributed body-to-root fatal damage,
credit/products, natural hide/reclaim and the real absence successor.
Whole imported Day3 root201 plus sixteen parts passes2.783s; four other roots
survive and still block family absence. Equal-elapsed no-cast controls survive.
Declared entry resources/objective/loadout, fixed lure and one player-only shot
placement are controls, not earned progression or an automated fight.

Real impact allocation-pressure and constructor prefix/preflight/refusal controls
pass1.463s; they configure exceptional pressure, not naturally full gameplay.
Affected motion/Stage/projectile/expiry/current-carrier/Alliance/writer checks,
source order and original-generation all-five absence pass. The latter uses
configured kills, not ordinary five-Worm combat. Gate fixes preserve strict
causal predicates: typed accepted-mailbox observation was missing, and freed
addresses may legitimately be reused by death products. Negatives remain saved.
Workspace/all-targets passes54.753s; strict rig native build72.780s.
Fresh independent finite source/gate/receipt review found no further concrete
Worm defect. Changed-source UBS has no critical on added lines amid legacy
whole-file findings; package formatting still reports broad existing debt.

Installed Original native `9c757c0241263300c329dffb846db1c7eb89263641eed981514d87fee8117393`
(138585728bytes), qualified refresh `build-vraodwgv`. All449 workspace inputs
(400 Rust source files),162 upstream
and37 additional inputs (641 unique) match the committed isolated snapshot,
rig and qualified Original client; foreign dirty Modern source is excluded.
Root inspected normal canonical map/loading/NightSpire terrain/HUD, including
the initial spawn flash, with the matching mapped native. The completed smoke
used one held portal click, no load retry or gameplay input, then stopped its
owned processes. Earlier helper input/display/timeout failures are preserved;
they are not counted as clean successful smokes. Native Worm encounter visuals,
human combat/FPS/hardware audio/full missions/old saves remain unverified.
No Unity/art change. Current and immediate previous Deep One native remain;
completed RCH outputs are byte-preserved on rig with before/after SHA checks.
Evidence: `generated/worm-20261107-integration/candidate-7/` and
`Builds/delivery-worm-20261107-qualified-smoke/` under `research/private/`.
Next is Spider20's actual web/contact/combat prerequisite, shared fields, then
Hydra27; its original blocked status is retained rather than bypassed.

### Previous installed delivery: Deep One28 (2026-11-07)

Implemented, committed and pushed: `126e11bf8f00acbe0c98442619ae8c3ed412cc40`.
The original name is independently bound by `DeepOne_38` in remc2's
`WavIndexes.h` and this creature's actual sound38 call. Its canonical family
owner now supplies constructor/action224–229/231 behavior, source movement,
both descriptor93/94 forms, resident animation-header timing, water splash,
melee, StageVar helpers, attributed death/rewards, hide and following reclaim.
Action230 retains its exact unsupported, default bit8-clear classification.
Shared joins repair valid null StageVar relations, the stale authored model23
preflight whitelist, and current typed model53 yaw/geometry and child publication.
They preserve canonical source state, admission, arithmetic and update order.

Verified with real whole Night14 owner data and the existing runtime. The
authored attack/form/splash/melee/audio path passes105.193s; configured absence
and source31 scheduling controls pass11.826s/4.762s. Eighteen focused kernel and
StageVar checks passed on the earlier frozen family candidate; its one failing
authored encounter was repaired and passed separately, not relabeled an all-green
family run. The corrected current model53 geometry/yaw/child check passes1.049s;
source-order passes0.614s. Candidate17's connected public Meteor check passes
170.716s: actual switch/group16 produces all28 authored members, including511/512;
the same original511 naturally transforms, then a real paid MeteorIII reaches
its exposed body through collision, supplies attributed fatal damage, awards
credit/mana/presentation and reaches natural hide/following reclaim. The other
original Deep One512 survives and still blocks whole-family absence. An
equal-elapsed no-cast control keeps511 living.

That connected check declares the prior objective/resource loadout, a fixed
initial lure and one player-only placement at the observed exposed-form
boundary. It fires once with fixed aim; it does not earn the prerequisite
mission fight, adaptively track targets or edit NPC life/pose/RNG/lifetime.
Earlier fixed-input Meteor/Lightning/Volcano negatives remain preserved. An
adaptive tracking candidate was rejected and never run. Under real pool
pressure, source NewEvent fallback clears family heads; homing admission remains
unchanged. The passing path proves the real collision boundary, not homing
acquisition in that cleared-list state. Configured rare-state controls remain
distinct from ordinary owner gameplay.

Candidate17 workspace/all-targets check passes43.567s on healthy strict rig RCH;
native build passes66.878s. All445 Rust plus162 upstream inputs and37 additional
workspace inputs (637 unique) match current source, isolated build and rig.
Independent bounded source/gate review and changed-hunk inspection found no
remaining concrete defect in the reviewed capability. Changed-source UBS retains
legacy whole-file findings with no critical on added lines; package formatting
still has existing broad debt. No acceptance was weakened to obtain green.

Installed Original native:
`e81f2dc905bb4b469058d5eb6a9509063ba5b3203f8c9ab3d26256b14c7ee918`
(138442608bytes), pin `generated/two-form-creature-20261107-integration/candidate-17/`.
Qualified native-only player refresh is `build-u7jgxa2g`. Root inspected canonical
map, loading and NightSpire terrain/HUD with the matching mapped library; one
portal click, zero retries, then owned smoke processes stopped. Native Deep One
encounter visuals, human combat, FPS, hardware audio, full missions and old saves
remain unverified. No Unity/art change. Evidence lives under
`research/private/generated/two-form-creature-20261107-integration/` and
`Builds/delivery-deep-one-20261107-batch1/`. Current/previous native pins and warm
caches remain; completed artifacts and old compiler-crash outputs were moved
byte-for-byte to the existing rig archive with before/after SHA receipts.
Next: Worm3 `.2`, then multipart27, single-player rivals, player death/revival,
boss10 and the remaining shared actor boundaries. Other families' original
unfinished dependencies remain open.

### Previous delivery: spell-thief26 (2026-11-07)

Implemented and verified; committed/pushed at runtime
`50873e9ec3ffa03da9e9abf849b740ea8f4e4cd2`; installed Original native
`c20286859fdc52f448dba4114a2f11a505a2a329ab6e9b7ba2c3747665764bbd`
(138,582,336 bytes). Original `mc-1pb.13` stays OPEN with its acceptance and
`mc-k5k.7` dependency unchanged. The creature parent and full engine remain open.

Constructor and enabled208–213/215 now preserve source movement subtype5,
stationary/roaming peer direction and byte39 eligibility, linked callback211's
early return/movement/speed/avoidance order, and full1D5D0 StageVar helpers.
Unsigned/array-pitch avoidance in210 remains distinct from signed/fixed256 in211.
Frozen lists enumerate scans; direct pointers resolve current fields, including
the callback-local actor and source-reached sentinel. Signed mana subtraction
wraps then clamps only below0. Ordinary Castle model2 remains excluded; typed
model0/1 Castle controls are explicitly configured current-carrier exceptions,
not proof of an ordinary Castle drain producer.214 stays source-unsupported.
Entry sound62 now freezes its call-time pose/silence state and reaches the real
Flight journal/native audio command before damage or movement, without a duplicate
tail receipt or added source RNG consumption.

Actual Level6 reset thief73 drains the actual player and steals genuinely
acquired Speed; action78 detachment, real pickup and exact Shield-left/remembered
return-hand consumers pass with its own audible62 (5.293s). Existing one-Army
pickup placement control remains declared. The new Shield approach changes only
player position512 west and checks every current eligible pickup AABB: the old
Shield-center pose also overlapped Beyond Sight and correctly equipped12 first.

Actual Level10 switch313/gate2 → full group15 → authored119/current generation
→ StageVar2 guard-point movement/steering passes10.820s. Completion of actual
objective index2 is a declared prerequisite, not earned mission progression;
the copied selected-index3 shorthand had correctly failed to activate the switch.

Actual Level6 switch63/player overlap → all six group13 authored roots → exact
authored378 pinned before combat → paid carried MeteorIII9/3 →10/17 latest scoped
fatal mailbox writer →212/213 credit → mana/death-burst DTOs/living-root loss →
hide/following reclaim passes5.664s; same-elapsed no-cast retains that living
generation. Player spell/Castle/resources are prepared before the trigger, then
one fixed512 firing pose uses the actual open Cave room. Its flat source terrain
was independently sampled. No NPC fields, RNG, lifetimes or160-turn combat bound
were forced. Candidate2's authored304 instead died from source four-retry movement
at phase7 before the first Meteor writer at10; corpse pulses remain rejected.
Earlier imported73 is InitialPackage, not fresh AuthoredRecord production.

Fresh bounded independent review supports the finite family and actual consumers.
Candidate1's25 passing family controls, entry-audio3.167s and source-order0.617s
are explicitly reused on unchanged production. Its three failed family gates were
corrected with source/geometry evidence, including both old fatal-mailbox population
expectations1→0; all three reruns pass on candidate2. Candidate3 changes only the
public combat encounter. This is not a fabricated28/28 single run. Workspace
all-targets passes38.993s; native build62.130s. Whole-file UBS critical findings
remain legacy with none on added lines. Existing package-formatting debt remains.

All445 Rust,162 upstream and37 extra inputs (637 unique) match canonical,
isolated and rig source/artifact;196 qualified Original Unity inputs match.
Root viewed canonical map/loading/NightSpire scene/HUD with the exact mapped
native, one portal click/zero retries, then stopped owned smoke processes.
No Unity/art change. Native thief26 encounters, human combat/FPS, hardware audio,
full missions and old saves remain unverified. Current/immediate previous natives
are regular/hash-preserved; completed build returns are SHA-preserved on rig ext4.
Evidence: `generated/spell-thief-20261107-integration/` candidates1–3 and
`Builds/delivery-spell-thief-20261107-batch1`.

### Previous delivery: splitting creature25 (2026-11-07)

Implemented and verified; committed/pushed at runtime
`3f842e8b4e26552b26627d3be3111137c063d123`; installed Original native
`ae71ebabfd93087ef61026ccd0b08acb22de64ef29b8abca7d91609637bd08e2`
(138,593,632 bytes). Original finite `mc-1pb.12` is closed with acceptance and
dependencies unchanged. The creature parent, full boss and full engine stay open.

The canonical family/shared joins now use actual subtype5 turning, promoted
peer-position arithmetic, the ordinary constructor across authored/Army/boss
producers (Army alone zeroes mana), current A4 ownership/target state and current
Castle/raw contact carriers. Small prekill awards once; every qualifying death
callback applies the literal source next-player header increment, preserving
the result consumer and full header state/hash. Boss minion25 sound37 now reaches
the native audio journal using the source's current NewEvent ID/pose/mute state;
ordinary pursuit37 retains its different physical-pointer emitter convention.
Other boss behavior/audio remains in its original unfinished family.

Actual unfiltered Level10 authored283 → parent330/gen2 → projectile409/gen2 and
its own audible37 → children404–406/gen2 → all natural landings → selected child
60-damage Contact and scheduled Castle life subtraction/DamageApplied → three
one-time credits → no resplit/hide/following reclaim passes4.713s. One declared
fixed player-Castle placement precedes the lethal hit and all child births; no
NPC, phase, RNG or lifetime is forced. Original authored31 production and its
source-correct four-retry movement fatal remain mandatory negatives; the live
attacker is pre-pinned to an actual current authored group8 model25 generation.
No Other/stale writer, unrelated sound or unconsumed mailbox can satisfy the gate.

Real Level24 absence, allocation/split lineage, current-contact/A4/header controls,
enabled200–205/207, unsupported206, shared StageVar modes and boss17→16 natural
expiry are covered by the saved focused runs. Boss phase/mode/attempt and one
parent-death control are declared, not a full natural boss fight. Earlier
unchanged-path evidence is reused explicitly, not presented as one35/35 run.
The final workspace/all-targets check passes40.698s and native build77.728s.
Fresh independent bounded source/gate review supports the finite acceptance.
Whole-file UBS findings remain legacy, with no critical added-line finding;
package formatting still fails existing debt and was not rewritten.

All445 Rust,162 upstream and37 extra inputs (637 unique) match canonical,
isolated and rig source/artifact. Native-only installation passed its qualified
Original gates. Root viewed canonical map/loading/NightSpire scene/HUD with the
exact mapped native, one portal click/zero retries, and stopped owned smoke.
The initial blank startup is retained. No Unity/art change. Native model25
encounters, human combat/FPS, hardware audio, full missions and old saves remain
unverified. Current/immediate previous natives are preserved; completed build
returns are SHA-preserved on rig ext4. Evidence:
`generated/splitting-creature-20261107-integration/` candidates1–6 and
`Builds/delivery-splitting-creature-20261107-batch1`.

### Previous delivery: Cave melee creature24 (2026-11-07)

Implemented, verified and committed/pushed at runtime
`144c5cbca8ab2c559047e31a21aaa0305a1cacbe`; installed Original native
`245a8764e20f161487e6fdf442381f4c902ea50eeebe61c41cbe66253540b676`
(138,561,840 bytes). Original finite `mc-1pb.11` is closed with its acceptance
and dependencies unchanged. The creature parent and full engine remain open.

Five source defects are repaired in the canonical family and shared joins:
movement now publishes its accepted/rejected source proposal while preserving
the hold branch; turning uses source subtype5 rather than the ignored170
argument; inherited child damage triggers the point route's hit branch;
actor-following applies target separation after peer avoidance; and callback194
prepares sound7 into the real journal/audio stream at its source entry pose.
All192–199 are enabled in the pinned source table.

Actual unfiltered Level16 switch92 activates group23 and authored404. After
natural startup protection expires, a pre-pinned current authored family24
generation moves, supplies the exact contact writer consumed by player life and
DamageApplied, and produces its own audible sound7 (4.292s). The witness is
validated as authored group23, semantic Creature24 and raw5/24; no Other,
model17, stale or unpinned writer qualifies. Record404 production remains
mandatory; it need not be the particular attacker selected by the live crowd.

Prior-carried public MeteorIII, a real Castle and declared player resources,
entry/aim and objective prerequisites reach the original404 fatal mailbox,
credited196→197, actual mana/burst DTOs, living-root loss, hide and following
reclaim (7.944s). The firing pose clears actual Cave floor/ceiling/FOV; a fixed
34-turn held input and identical no-cast input replay remain within the original
160-turn bound. There is no adaptive tracking or NPC/RNG/lifetime edit. Aggregate
mana spending is a consistency check, not individual-debit attribution.

Candidate1's other family controls, real absence2.575s and source-order0.661s
pass on byte-identical production. Final workspace/all-targets passes24.347s,
native build50.202s. Fresh bounded source/gate review found no further concrete
finite-family defect. Six whole-file UBS critical findings remain legacy;
none are on added lines. Formatting/scanner debt is retained. Earlier melee
and Meteor negatives remain failures: retained velocity/protection/crowd
attribution and an invalid firing corridor/impossible one-impact kill required
honest scenario corrections, with original causal predicates preserved.

All445 Rust,162 upstream and37 extra inputs (637 unique) match canonical,
isolated and rig sources/artifact;196 qualified Original Unity inputs match.
Root viewed normal canonical map/loading/NightSpire scene/HUD with the matching
mapped native, one click/zero retries, and stopped owned smoke processes. The
initial blank startup capture is retained. No Unity/art change. Native Cave24
encounters, human combat/FPS, hardware audio, full missions and old saves remain
unverified. Current/previous natives remain regular, and completed returns are
SHA-preserved on rig ext4. Evidence: `generated/cave-melee-20261107-integration/`
candidates1–3 and `Builds/delivery-cave-melee-20261107-batch1`.

### Previous delivery: mana-hunting creature23 (2026-11-07)

Implemented and verified, committed/pushed at runtime
`6ee32f709b9a068600374986098dfaff7d3ea862`, and installed as Original native
`19bc05bc8c004eda4eac7b238255e3959e2ab16c00c9b99cabc62df1aff79fa6`
(138,552,464 bytes). Original `.10` acceptance/dependencies are unchanged;
the bead stays open with shared `.10/.12`, and the full engine remains open.

Actual unfiltered Level18 root29 follows the ordinary source callbacks.
Public carried Steal Mana III hits actual rival184 outside its current Castle209,
debits the Castle and creates16 real mana spheres. Original sphere253/gen1
merges from500 to1500, is claimed at349, genuinely lifted, and consumed at361:
hunter mana100→1600, hidden retained sphere, then following original-generation
reclaim. The corrected connected gate passes66.783s. Carry/XP/resources, a
constructed player Castle and player entry/travel are declared controls;
the rival/hunter poses, health, RNG and timers stay ordinary. The test waits
naturally outside Castle protection; a protected prefix correctly clears hits.

The separate real hunter→9/9 Ray→10/23 Lightning→actual player damage path
passes8.827s. Public carried Meteor III now supplies the exact original hunter's
fatal mailbox writer, credited188→189, mana/burst DTO, hide, living-root loss
and following reclaim, with error-free same-elapsed no-cast control (4.497s).
The shared KillEntity birth now uses the canonical death-burst producer,
preserving post-mana current reads, suppression, aliasing and final hide while
joining existing provenance/events. The earlier burst failure established
missing registration, not missing rendering.

Source movement/targeting, current A4 Castle geometry/music, retained claims,
source-order damage tails and all enabled184–189/191 are connected;190 remains
explicitly unsupported. Configured Stage191 modes, current-parent physical reuse,
allocation failure, contact/reward/statistics and actual frozen semantic absence
are checked. The longer run exposed remote Morph's stale shared holder projection;
the canonical generation-bound holder/body relation now admits its visible body
and actual callback/phase epilogue (27.710s). No hidden-body bypass is claimed.

Final shared-kill24 lifecycle/alias and26 death checks pass; source order passes
0.598s, warm-context save/replay6.703s, workspace/all-targets42.687s and strict rig
native build65.828s. A fresh bounded source/gate review found no further concrete
finite-family defect. Whole-file UBS/format debt remains; the only added-line
critical is the intentional cfg(test) dispatch-error panic. No production finding
was left untriaged. Failed gates and the interrupted/recovered candidate6 build
remain recorded; they are not counted as successful outer checks.

All445 Rust-workspace,162 upstream and37 extra inputs (637 unique), exact worker/
returned native bytes and196 Original Unity inputs match. Root viewed canonical
Original map/loading/NightSpire terrain/HUD with the matching mapped native,
one click/zero retries, and stopped owned smoke processes. The initial blank
startup capture is retained separately. No Unity/art change. Native23 encounters,
human combat/FPS, hardware audio, full missions and old saves remain unverified.

Evidence: `generated/mana-hunter-20261106-integration/candidate-13`, candidate11
capture and earlier supporting/negative receipts, plus
`Builds/delivery-mana-hunter-20261107-batch1`. Completed build returns are
SHA-preserved on rig ext4; current and immediate previous native pins remain regular.

### Previous delivery: paired-body creature22 (2026-11-06)

Finite original `.9` acceptance is implemented, verified, committed/pushed at
runtime `9cedba178dd4ad80999359be6f6738751f7c3bfc` and installed as Original native
`79f03d8862b4cbcaf98680e1841e0eb7951ef71267a73b9f916a5856ec85ee0a`
(138,342,288 bytes). The bead is closed; the creature parent and full engine stay open.

The actual unfiltered Level7 switch194/group5/row218 creates the complete
length15 body. Public prior-carried Possession reaches actual projectile/impact,
ownership, colour, motion, prepared audio4/48 and original DTO consumers.
All14 shipped authored22 records already use length15. A separately declared
additional length3 constructor in the whole Level7 runtime proves natural
1024-countdown growth to5, real public Possession, Castle approach and128-visit
shrink delay, pairwise shrink, deposit, hide and following reclaim. No actor
pose/action/life/RNG/timer is forced. Original376/gen1 deposits to Castle372/gen1;
another actual22 writer380/gen1 remains separately visible in the exact receipt.
The prior spell carry and Castle/player entry resources are declared controls.

Fixed owner A4/nonlocal typed-Castle mana access and source signed saturation,
current movement-proposal writes, and the missing prepared-audio journal join.
The longer run exposed Firefly211 selecting Castle372 with no current yaw.
Castle pose now has its own constructor/reset fields and hash, distinct from
BUILD collision axes. Constructed/poisoned-shadow/hash checks pass; imported
pose copying is source-reviewed. An optional old Level23 whole-package check
still stops before adoption at missing initial BUILD1; full terrain `.8` remains open.

Configured canonical1B5F0 cleanup proves all15 original parts' mana/hide/DTO
loss/reclaim and actual frozen NoLiving22 timing. This is not terrain57390 burial,
ordinary damage lethality or an earned boss victory. Source182 remains unsupported;
unshipped183 fatal-tail behaviour has no invented positive route.

Final family14 checks PASS179.640s, current fields6 PASS6.054s and connected
cleanup/absence PASS2.706s. Source-order and strict audio census pass on candidate3;
the census's omitted Dragon/Meteor owner inputs were repaired without changing
its scanner, strict assertions or negative controls. Workspace/all-targets
PASS44.150s; strict rig native build PASS92.133s. All445 Rust,162 upstream and37extra
inputs (637 unique), worker/returned artifact bytes and196 Original Unity inputs
match. Bounded independent source/gate/acceptance review found no remaining
concrete finite-family defect. Whole-file formatting/scanner debt remains;
no added critical finding was identified.

Root viewed canonical Original map/loading/NightSpire scene/HUD with the matching
mapped native, one click/zero retries, then stopped owned smoke. The early blank
startup capture is retained with its proper label. No Unity/art change. Native22
encounters, human combat/FPS, hardware audio, full missions and old saves remain
unverified. Evidence: `generated/paired-creature-20261106-integration/candidate-4`
and `Builds/delivery-paired-creature-20261106-batch1`. Completed binaries and redundant
scanner outputs are SHA-preserved on rig ext4; identical old player payloads were
consolidated without changing bytes or paths. Current and previous native pins remain regular.

### Previous delivery: leaping creature21 and Dragon correction (2026-11-06)

Leaping21 `.8` and the reopened Dragon0 `.19` corrections are implemented,
verified, committed/pushed at runtime `35b73120c8aeb2b1724bfdd2d7d86be8d93bb106`
and installed in the qualified Original player `build-avvn2_bj`. Its native is
`ba5918a4d17f1e1d48281d8100b14d138de17753b172728ea8f32137a876247b`
(138,317,464 bytes). Full boss `.3`, the creature parent and full engine stay open.

Actual unfiltered Level 8 authored production reaches jumping,9/0 payload 500,
same-generation impact, consumed player damage, prepared42 audio and original
DTO (10.241s). Public carried Meteor reaches original fatal attribution,
mana/death/hide/reclaim, with no-cast control (5.344s). Configured contacts on
the real group prove task5's frozen-list absence boundary (2.919s). Water/Cave,
allocation exhaustion/reuse and parent-loss exceptions remain declared controls.

The actual Level 25 boss117/gen1 naturally produces21 child386/gen12. It reaches
17->16, jumping/landing, a real payload500 shot, movement and DTO at turn1598;
the connected check passes in144.322s. Group29 progression and player position/entry resources
are declared controls, not earned mission proof. The isolated dead-parent branch
retreats only the player: action170 phases65..96 retain the child, range cadence
returns169, the next175/1E580 at phase97 hides it, creates10/0 and removes its DTO,
then the following pass reclaims it. No boss/child RNG/action/timer is forced.
The prior immediate-hide assertion was wrong for a child still attacking.

Source21850's secondary RNG draw, per-session shared movement proposal, current
parent/target and splash-carrier joins are corrected. Dragon reads the current
fixed parent/predecessor while creator provenance remains generation-bound.
Four-family live-address reuse/dead-current cleanup and the affected real Dragon
parts control pass. Earlier ordinary Dragon acceptance remains qualified.

The boss timer now consumes actual source animation preload, ordered qualified
render demand and residency, carried through reset/hash/cold replay. Literal221F0
adds16 **animation records** and reads the low16 active bits; it is not frame count.
Checked Rust preserves defined reads and explicitly rejects undefined bounds.
The qualified 504-entry bank fits source cache budgets; pressure eviction and
animated pixels are not newly claimed. Replay-save/v7 rejects older gameplay
histories lacking ordered demand; campaign-map saves are unchanged.

Workspace/all-targets passes in46.072s; strict rig native build in69.441s. All445 Rust,
162 upstream and37 extra/637 unique inputs and exact artifact bytes match.
Original was rebuilt for the v7 contract: all196 Unity inputs are qualified,
with only the transport version literal changed. No art or Modern inputs changed.
Root viewed canonical map/loading/NightSpire terrain/HUD with the matching mapped
native, one click/no retry, and stopped owned smoke. Two early blank startup
captures remain labelled as such. Native21 combat, human play/FPS,
hardware audio, full missions and old saves remain unverified. Independent bounded source and
separate gate review found no further concrete family defect. Whole-file format
debt and C# scanner semantic limits remain; no critical added-line finding.

Evidence: `generated/leaping-creature-20261106-integration/candidate-11`, prior
candidate2..10 negatives/positives, and `Builds/delivery-leaping-creature-20261106-batch1`.
Completed build outputs are byte-preserved on rig ext4; current/previous native
pins, previous player `Linux-before-build-avvn2_bj`, source/data and warm caches
remain preserved. The original complete creature22 `.9` acceptance became active at that checkpoint and is qualified above:
ordinary authored paired-body production, real Possession growth/shrink/motion,
combat/death, exact partial-pair failure and whole-chain mana/root-count cleanup.
AmberDune owned that capability through integration; CalmLynx implemented shared
joins. Root CopperMountain serialized Cargo, push and installation. Existing
family code was present; its stale absent-code bead assessment was not authority.

### Previous delivery: Sentinel and death-map correction (2026-11-06)

Sentinel18 `.7` is implemented, verified, committed/pushed at runtime
`41b828f438ad3f8ed4f10b0e738b2ccc6924021c` and installed as Original native
`18c459364710458eb3ec1b07af1ddcda63fd37b0df9224a8e9ac8e7fbc57bd91`.
Actual Level24 group2/record234 produces original220/gen1, naturally selects
the player and emits all five source-ordered fan births. An exact projectile →
impact generation → latest mailbox writer → player life check passes6.802s.
One declared player interception follows an actual shot; no NPC pose, action,
health, RNG or lifetime is forced. Group progression and player resources are
controls, not earned mission proof.

Fixed three public Meteor commands reach the original fatal callback, mana,
owned burst/source ID/original DTO, living-list1→0, hide and following reclaim
(13.517s). Exact original credit0→1 and real Spell Thief26 collateral205/gen0
credit1→2 are accounted for; GDB identified the latter's actual internal writer.
No-cast control retains the original. Two test-only observer blocks are the
only candidate3 difference from candidate2; production is byte-identical.

Per-NewEvent current actor/target reads, signed birth yaw, StageVar arithmetic,
literal no-ops, current typed/retained parents and post-expiry tails are fixed.
Constructor mana derives from default maximum life before authored overrides;
descriptor86/particle286 live in the MC2 adapter. Source150 remains unsupported;
no Sentinel entry sound is invented. Configured partial allocation/replacement
and positive-life typed-parent controls pass; prior15 affected family checks
remain applicable. The original finite acceptance/dependencies are unchanged.

The old death-map assertion exposed a real source1C930/57F10 regression, also
present in Manticore17: hide had removed map linkage ahead of57F20 reclaim.
Both exact fallbacks are corrected; Sentinel and Manticore controls pass1.005s
and1.009s. Manticore `.6` was reopened for that correction and is requalified
and closed again with its earlier gameplay evidence retained.

Fresh bounded source and separate gate review found no further concrete gap.
Source order/current-carrier checks and workspace/all-targets40.274s pass;
strict rig native79.132s, all444 Rust+162 upstream+37 extra/636 unique inputs
and exact137035392 artifact bytes match both installed Original plugin copies.
Root viewed canonical map (`map-ready.png`), loading artwork and NightSpire
terrain/HUD with the mapped native, one click/no retry, then stopped owned smoke.
The earlier black startup capture is retained, not presented as the map.
No Unity/art rebuild. Native Sentinel encounters, human combat/FPS/hardware
audio/full missions/old saves remain unverified. Allocation pressure and prior
Level24 absence-objective evidence are labelled configured controls.
Package formatting drift and the prior Dragon audio-census source-list issue
remain explicit; UBS has no added critical finding. Full engine/parent stay open.
Evidence: `generated/sentinel-20261106-integration/candidate-3/`, retained
candidate1/2 negatives and positives, and `Builds/delivery-sentinel-20261106-batch1/`.

### Previous delivery: Manticore

Manticore17 `.6` and its original finite wind/status prerequisite `mc-bhq.14`
are delivered and closed without changing acceptance or dependencies. Runtime
`3834b46c8da97b42aa9786c3118d80408427db4b` is committed/pushed; native
`c72c82d5649125e2aa42bf00842efbcf13b5e74ca388fd5c350e2ef89cd6a708`
was installed and qualified in both Original plugin locations; Sentinel now
carries its gameplay fixes and the map-retention correction. Source helpers
now preserve Alliance expiry, linked current-address reads, signed arithmetic,
guard/follow/wander tails and source update order. Constructor mana derives
from default max life before authored health overrides; current subspell owns
melee damage. Source85/87/285/audio58 bindings stay in the MC2 adapter. Attack
audio reaches the native event at callback entry; death preserves current IDs,
mana/owner ordering and the owning generation. Unsupported142 stays unsupported.

The actual original Level9 actor67 takes a public Fireball provocation, changes
stance and delivers source350 melee through the actual player mailbox/life.
Its real9/20 projectile reaches10/65 and the same live player's slowed accepted
movement, recoil, palette, eight-visit recovery and generation cleanup (7.63s).
Fixed public Meteor casts kill the original67/68 at unchanged10,000 health;
caster/holder→9/3→direct10/17 generation→latest writer→fatal callback, rewards,
hide/reclaim and task5 absence pass4.92s. The10/0 children are visual, not the
Meteor damage writer. Carried spells, entry resources/Castle/XP, objective
selection and finite player positions/input are disclosed controls; no NPC
state, RNG, lifetime or health is forced. This is not earned progression or
payment proof. The unrun adaptive Fireball proposal was rejected.

Actual Level2 switch121/group15/Wind122 forces player movement, runs its natural
lifetime, clears flags and reclaims all11 linked generations (15.56s). Actual
Level15 transport moves the player (1.75s); scheduled65/66, stacked slowing/web
recovery and supported orphan37 controls pass. No ordinary37 producer is claimed.
Sixteen family checks pass on candidate2; two stale-package checks retain their
assertions and pass on candidate3 with qualified Level9 and the matching Level2
package/cache. Configured allocation, retained-parent and reuse controls are
exceptions, not natural pressure proof. Firefly shared-writer42.84s and source
order0.62s pass. A fresh bounded source and separate gate/acceptance review found
no further concrete in-scope defect. Workspace/all-targets37.865s and strict rig
native53.719s pass. All444 Rust+162 upstream+37 extra/636 unique inputs and exact
worker137051672 bytes match the canonical source and Original installation.

Root viewed canonical Original map/loading/NightSpire terrain/HUD with that
mapped native, one portal click/no retry/private-save load/gameplay input, then
stopped owned smoke processes. No Unity/art rebuild. Evidence:
`generated/manticore-20261106-integration/candidate-3/`, candidate2 positives and
preserved negatives, and `Builds/delivery-manticore-20261106-batch1/`. Native
Manticore encounters, human combat/FPS/hardware audio/full missions/old saves
remain unverified. Package formatting still fails amid drift; UBS has zero
added critical findings with whole-file limits. The audio census currently
fails on DragonFireball because its unchanged source list omits the unchanged
canonical Dragon emitter; source comparison reproduces the same baseline gap.
No validator was weakened and no all-green whole-workspace test claim is made.

### Previous delivery: Skeleton and Firefly

Skeleton `.22` original finite acceptance is delivered and closed. Runtime
`d88bd191b45e1145451d27800b43eca4bf0d48dc` is pushed; matching native
`87fbc53ebcff26511d04ef57c69a12fd7c4fb94df4c10f446217c78ae992ce19`
was qualified in both Original plugin locations; Firefly and Manticore carry
those fixes forward. All source79
helper cases and literal default returns now have their distinct behavior;
enabled75 is integrated and unsupported78 remains explicit. Alternate-mode/75
controls are configured source exceptions, not invented ordinary producers.
The prior real constructor/arrow/conversion/death, L3 point/L5 follow/L16
conditional follow/Alliance/objective evidence remains applicable.

The actual scheduled expiry check exposed a missing raw10/73/action80 admission.
Its canonical exact-tuple fix reaches the already implemented source3A630 body;
no phase/life assertion was weakened. Candidate13's negative and source snapshots
are preserved. Candidate14 passes full unshortened16-callback73 and Impact0 aging,
hide/presentation/following reclaim0.964s; owner branches0.499s, order0.629s,
core69.033s/444stable and workspace35.995s/444+37stable. Independent bounded
implementation and separate gate review found no further concrete defect.
UBS has zero added critical findings; legacy formatting/scanner debt remains.
Strict rig native52.893s, worker137077272 bytes and all636 unique inputs match.
Root viewed canonical map/loading/NightSpire terrain/HUD/spawn flash with that
mapped native, one click/no retry/private-save/gameplay input; owned smoke stopped.
Evidence: `generated/skeleton-20261106-integration/candidate-14/`, preserved13,
and `Builds/delivery-skeleton-20261106-batch6/`. No native Skeleton encounter,
human combat/FPS/audio/full-mission/old-save or pressure-alias claim is made.
Parent/shared-engine and other creature acceptance remain open.

Firefly `.23` original finite acceptance is delivered. Runtime
`f61d888a8866032c8a8e7e26290f30fe20e431ed` is committed/pushed; native
`c8982abd4374cc3b6b09f060df54d99c698fe538d6c5d5684e4a53c3720f19d5`
was installed and qualified in both Original plugin locations; Manticore carries
those fixes forward. Source4C6B0 is one
ordinary constructor: boss births retain300 mana; Army applies its own zero.
Typed class properties, semantic/Morph bindings and original descriptor88 assets
reach their consumers. All enabled152–159 paths are retained;158 remains unsupported.
Idle/roam, point/guard, actor-follow and linked/damage helpers now read their
actual current carriers and source arithmetic/order. Modes6–9 do not require a
StageVar descriptor; real Morph12 keeps its existing dedicated raw-body consumer.

Candidate8's actual whole Level7 switch205/group9 publishes20 authored roots;
one original generation delivers contact300 and ranged9/0 payload500 through
accepted writes, the exact player callback and life loss (43.224s). Actual Level1
record108 takes three public Fireballs600→350→100→-150, reaches156→157, hidden
presentation and following reclaim; original living count loses one and the
other four keep the real staged absence objective pending (2.212s). This does
not newly prove per-cast payment or earlier mission progression. Actual Level25
boss construction/flight17→16 and configured parent-loss expiry/reclaim passed
44.164s in candidate3; its producer and stage body are byte-identical in8.
Group29 is a disclosed upstream prerequisite, not a public BUILD-demolition proof.

The25 family checks pass28.243s, current-word32 owner control1.000s and source
order0.659s. Configured helper/current-field/selector controls are explicit
source exceptions; they do not establish every ordinary pressure/replacement
producer. Current word32 reads never silently borrow a typed carrier's stale raw
shadow; unsupported current owners remain explicit. Existing acquired Night/Cave
Morph and public Army lifecycle evidence is retained rather than replayed.
The actual Cave death control proves original132/gen0 reclaim and legitimate
same-address10/0 gen1 reuse. Independent bounded source and separate gate review
found no further concrete defect in the inspected family. All-target workspace
check passes45.641s with444 Rust+37 extra inputs stable. Package formatting
still fails amid existing drift; parsing/compilation and diff checks pass.
Scoped UBS has zero added critical findings; whole-file backlog is not a clean scan.

Strict rig native75.522s and all636 unique Rust/upstream/extra inputs match
canonical source, isolated build and worker; exact worker artifact136983120 bytes
matches the installation. Root viewed the normal canonical Original map, loading
art and NightSpire terrain/HUD/spawn flash with the matching mapped native, one
portal click/no retry/private-save load/gameplay input; owned smoke processes
were stopped. No Unity/art rebuild. Evidence:
`generated/firefly-20261106-integration/candidate-8/`, preserved candidates1–7,
and `Builds/delivery-firefly-20261106-batch1/`. Native Firefly encounters, human
combat/FPS/hardware audio/full missions/old saves remain unverified.

Next is creature23 `.10`; paired-body22 `.9`, leaping21 `.8` and Sentinel18 `.7` are qualified above. Retain all delivered
source evidence and finish each original finite acceptance before moving on.
Do not replay unchanged long checks. Parent/shared-engine and other families
stay open. Root CopperMountain owns serialized integration/delivery; the small
native team continues one creature at a time.
Preserve the separate Modern task and current/previous native. Older completed
build proofs were byte-verified and archived to rig ext4; source/data/caches and
unrelated dirty work were preserved. No heartbeat was recreated.

### Preserved settlement and Skeleton delivery evidence (chronological)

Finite settlement `.16` is implemented, verified, committed and installed.
Runtime/source `ae2e84d1` and client qualification `24df66cc` are pushed;
native `9d9af55f3c61697c8892fd2b7c63970cc590ad1c675b265d076ce5b8e37facfc`
matches both installed Original plugins. The matching Original Unity rebuild
passes20.006s using that existing native; its444 Rust and196 player-input
receipt binds the actual production scripts. The focused real preflight check
passes133.016s and rejects fully rehashed opposite-profile cache pairs before
level open. Candidate26 retains exact source/rig/native input verification.

Root viewed the canonical Original map and NightSpire terrain/HUD/spawn flash
with that exact mapped native on the first portal click: no retry, private-save
load or gameplay input. Loading-bound/frame/open events pass and both owned
player/display processes are stopped. Evidence:
`generated/settlement-20261105-integration/candidate-26/` and
`Builds/delivery-settlement-20261106-batch3/runtime-proof.json`.
The earlier batch2 cache rejection and all diagnostic negatives remain saved.
The fresh bounded default-family review plus the connected lifecycle evidence
below supports original finite acceptance; no unsupported callback or invented
direct Trader-to-Castle deposit is claimed. This is not native NPC encounter
visuals, human combat/FPS, hardware audio, full missions or old-save proof.
Parent/shared-engine and other creature acceptance remain open.

Next is original Skeleton `.22`, with `.21` and `.16` prerequisites now delivered.
MagentaPuma owns its real production-to-consumer capability; AzureGate implements
required shared joins and root serializes Cargo/push/install. Reuse the existing
actual Builder/Villager conversion and child-scheduling evidence. Trace current
constructor/SetEvent144 arithmetic and typed properties, ordinary arrow203 to
actual damage/ownership, cleanup and actual absence consumers before editing.
Source75 is enabled but has no demonstrated ordinary producer in the inspected
paths; source78 has no dispatch implementation. Preserve that distinction rather
than inventing producers, forcing actors/groups or scripting whole-group combat. Workers
remain responsible through integration; original acceptance/dependencies stay.

Skeleton `.22` current property/arrow batch is implemented, verified, committed
and installed; full family acceptance remains open.
Typed life/speed/subspell and400/600 arrow properties plus the MC2 descriptor/
particle binding now reach raw construction, legacy promotion, semantic defaults,
field births, movement/reaction and actual conversion. Source SetEvent144 still
derives mana from initial maximum life before later authored health overrides.
The bounded review caught literal selector consumers in the shared orchestrator
and legacy tuple; those actual joins are fixed, without callback/order changes.
Real switch279→group19 Skeleton birth→generation-bound9/13/203/400 arrow→accepted
player mailbox→later damage/life loss PASSES17.091s. Player-only initial placement
and interception of an already launched arrow are explicit controls; no NPC,
RNG, phase, lifespan or group was forced. Configured Level5 watcher209/11/22
count-edge passes3.294s:39 genuine baseline roots,38 declared hides, final
declared arrived pulse, real death/countdown/publication/reclaim/reset. It does
not prove natural whole-group combat or absence.

Candidate2 core+CAPI compile passes107.009s/444stable, with independent rig/local
test-binary hash/extent. Typed constructor, unchanged constructor admission,
legacy promotion, Alliance/expiry/pressure and actual reset-health controls pass.
An affected authored-health check failed identically on the preserved installed
settlement26 binary: it read only the legacy projection after promotion to the
authoritative raw arena. The corrected observer preserves all seven expected
authored rows, exact239/current generation, raw health mirrors and1000/2000
reopen/reset checks. It passes1.825s on candidate3; core compile62.366s and
workspace/all-target42.396s pass, all444Rust+37extra inputs stable. Only this
cfg(test) observer differs from passing candidate2 gameplay; other443 inputs and
the entire production prefix are byte-identical, so that evidence is reused.
Candidate1's compiler-detected usize conversions and test import repair remain
recorded; no admission, gameplay or assertion was weakened. Evidence:
`generated/skeleton-20261106-integration/`. Full `.22` remains in_progress;
ordinary paid Skeleton death/count loss is next; matching delivery follows below.
Package formatting and whole-file scanner debt remain explicit. Known completed
build artifacts are byte-preserved on rig ext4 before local removal; three closed
test binaries were hash-verified into ext4 with stable aliases to free426MB in
tmpfs. Source/data, warm remote caches, current/previous natives and Modern work
are preserved.

That batch is committed/pushed as `b3b7da835a34c67823d871f555dd39c3423e6bf1`.
Strict rig native build passes68.094s; native
`63311e3a89d2042379369286f9c300573df802bdb0aee38b3ff1a4ae95830656`
and its136948168-byte extent match the worker. All444Rust+162upstream+37extra
categories (636unique paths) match canonical/isolated/rig inputs. Native-only
installation passes2.323s; both Original plugins and the444-input installed
receipt match, with no Unity rebuild or art change. Root viewed canonical map,
original loading artwork and NightSpire terrain/HUD with that exact mapped
native: one portal click, no retry/private-save load/gameplay input. Initial
startup capture was blank while initialization continued; no startup timing or
FPS claim. Both owned smoke processes are stopped. Evidence: candidate3 and
`Builds/delivery-skeleton-20261106-batch1/runtime-proof.json`. No native Skeleton
encounter visual, human combat, hardware audio, full-mission or old-save proof.
The immediate previous9d9 native/player is preserved. Next is one ordinary paid
Skeleton kill through strict negative life, source76/77 death/hide, following
reclaim and real living-root/target loss; four250 hits leave1000 life at zero,
so a fifth accepted hit is required. Keep `.22` open through this consumer.

Fresh original-family review atf3b538f7 found another real enabled gap: the whole
Level5 package86cb7654 binds39 reset Skeletons to StageVar1 kind3/flags3 and
AxisScalar(4), with live Archer targets. After natural emergence, source1D8C0
selects/caches a current model4 actor; the existing Skeleton action79 adapter
instead treats4 as the coordinate(4,0) and calls its mode1 kernel. This is an
ordinary behavior divergence, not a hypothetical custom route or the configured
absence control. AzureGate owns its canonical target-follow implementation and
real unmodified Level5 producer/consumer check, with narrow shared-root leases;
Magenta retains the family and has frozen the separate UNRUN paid-kill gate at
skeleton.rsaaa937d3. Serialize that file's handback. Root batches both checks
after a coherent fix; a production change requires a new matching native.
Neither the installed property/arrow batch nor the configured absence control
closes this missing behavior. No level-specific patch or forced actor/timer is
allowed, and unsupported/default-unreachable callback classification stays.

That actor-follow fix and the paid-kill path are now implemented, verified,
committed and installed from candidate6. Actual unmodified Level5 reset/emergence→
mode3→current Archer cache/following/map/presentation/reset passes2.718s.
Configured near/hit/lethal/route-continuation/null/direct-pointer and unsupported
flee controls pass3.470s on production-identical candidate5. The real Level7
switch279/group19/authored454 paid path passes48.905s: five causal250 hits,
1000→750→500→250→0→-250, actual76/77, credited word24/player with no model9
killed-percent increment, neutral500 mana/burst, hide/presentation and following
original-generation/target/living-root loss. Declared controls are player-only
200000 life/mana/capacity, equipped Fireball and one fixed firing pose/aim;
existing acquired-spell proof is reused, no NPC state/RNG/timer was forced.
Candidate5's failed outer-turn mailbox assertion is preserved. The corrected
test uses actual callback-entry/exit state and the latest accepted writer before
that entry, retaining exact payment/impact/generation attribution. Its bounded
enabled-only observer is cfg(test); the non-test dispatch call is unchanged.
Core6 passes106.600s/444stable; exact worker/local executable hashes match.
Source order passes0.638s and workspace/all-target36.442s passes444Rust+37extra
stable. Whole-file formatting/scanner debt remains, no added critical finding.

Runtime `a9d10c5abd7c7a4c30b758d24b16ea0ed07ce643` is pushed. Strict rig native
build passes49.478s; `12ee86299242d5c079b5532f0e5f182eb063d4ddb8ccf01f7dfad84855c6e04a`
matches the worker's136983592 bytes, all636 unique source inputs, both installed
Original plugins and the444-input installed receipt. No Unity/art change.
Root viewed the canonical map and NightSpire terrain/HUD with that mapped native,
one portal click and no retry/private-save load/gameplay input. The first30s
observation expired before load-open; the same process completed naturally.
Loading-bound/frame/open and night-presentation events pass; owned smoke is
stopped. Evidence: candidate6 and `Builds/delivery-skeleton-20261106-batch2/`.
Fresh bounded source/gate review found no further concrete defect in those joins.
No native Skeleton encounter, human combat/FPS, hardware audio, full-mission or
old-save claim. Completed returned auxiliary files736MB were byte-verified on
rig ext4 before local removal; current/previous natives and warm caches remain.

The original-family admission check also matched all22 existing L3–24 authored
payload hashes against the pinned source-record closure. Besides L3 kind1 and
L5 kind3, L16 binds97 Skeleton rows to kind4 targeting Creature24; that target
is absent in its reset, so callback admission depends on actual group timing.
No other bound kinds were found in those22 inputs. This does not prove all
other source StageVar modes globally unreachable: remaining fallback modes,
uninspected L1/2/25/secret routes and custom selectors stay unqualified. Full
`.22` remains open. The new current whole L16 package passes export6.439s with
all160 current exporter inputs and marker verified. Its actual flags7 route and
group23→27 chain exposed a missing canonical source12870 group-end rearm.
Azure owns that shared join and its active-count boundary; Magenta owns the
real switch/group/Creature24-follow consumer. No NPC/group/timer forcing or
mission-specific patch is permitted; preserve full acceptance and dependencies.

The source12870 group-end join is implemented at both successful publication
exits, before allocator reset, and bounded by the actual active StageVar count.
Candidate8's conditional L16 switch92→group23→proximity361→group27→Skeleton399→
current Creature24 follow passes3.103s, including generation, cache, movement,
map and presentation. Objective3 completion2 and one player position are declared
prerequisite controls, not natural upstream objective completion/full-mission proof.
Candidate7's dynamic-only switch observer failed against the real imported92;
the corrected observation retains exact raw/authored identity and requires an
actual negative-path callback. Production and443 other inputs are identical.
Core8 passes65.953s/444stable, workspace22.479s/444+37stable. Source-order0.687s,
active-count bounds1.016s and previous L5 follow2.698s pass on identical production7.
Independent bounded gate review found no further concrete defect; its target
selection oracle does not independently establish candidate-list completeness.
Root also inspected the actual source/frozen-list loop. UBS has30 legacy
whole-file critical findings, none on added lines; formatting debt remains.
Runtime `340c06acb6dda09ef01e02aba7037be96220515b` is committed/pushed. Strict rig
native build46.745s yields `7eca80a15ff6a513642a0568169d7ee10c2e239dd3f64be416cf38c4ee8521d2`;
worker136983720 bytes and all636 unique canonical/isolated/rig inputs match.
Both installed Original plugins and444-input receipt match; qualification passes.
Root viewed canonical map, original loading artwork and NightSpire terrain/HUD/
spawn flash with that mapped native on one portal click, no retry/private-save
load/gameplay input. Loading-bound/frame/open/night-presentation events pass;
owned smoke is stopped. Evidence: candidate8 and delivery-skeleton batch3.
No Unity/art change, native Skeleton/Creature24 encounter, human combat/FPS,
hardware audio, full-mission or old-save proof. Closed auxiliary736MB and two
older native proof copies274MB are hash-preserved on rig ext4 before local
removal; current7eca and immediate previous12ee remain local. Free~6.59GB.

Full `.22` stays open: uninspected source inputs and other StageVar fallthroughs
are not proven equivalent or unreachable. A review's combined75/78 claim was
corrected by root/source inspection:75 is an enabled remc2 callback (20E50),
currently rejected in Rust; no actual producer was demonstrated in the inspected
Skeleton constructor/custom scans/StageVar paths. Bit8 controls78 selection,
not75. Keep enabled-orphan reachability distinct from unsupported dispatch and
do not invent a positive producer or claim full family acceptance from this batch.

Skeleton point-route batch11 is implemented, verified, committed/pushed as
`5ec744b026d63452d995fbeb99a030c6b9c5a388` and installed with native
`6bd0cb61b23f083089b44831425863bbd8e41f1d913de9d69a311aa67181c816`.
Actual untouched Level3 eleven reset roots naturally emerge and follow their
authored point, with heading/RNG/movement/map/presentation checks passing0.821s.
Current peer fields are read lazily over frozen membership after source gates;
first overlap stops the scan. Completed movement/map/population changes survive
a later peer-read error; pre-movement errors do not publish movement. Configured
membership/read-order and actual-wrapper failure controls pass; these are not
claims of naturally occurring carrier failures. Core67.412s, workspace36.426s,
source-order and affected focused checks pass. Bounded independent review found
no further defect; UBS has zero added-line critical findings and legacy debt.
Strict rig native74.784s, all636 unique inputs and both installed Original plugins
match. Root viewed canonical map/loading/NightSpire terrain/HUD with that mapped
native, one portal click/no retry/private-save/gameplay input; owned smoke stopped.
Evidence: `generated/skeleton-20261106-integration/candidate-11/` and
`Builds/delivery-skeleton-20261106-batch4/runtime-proof.json`. No Unity/art change,
native Skeleton encounter, human combat/FPS/audio/full-mission/old-save proof.
Metadata `cf67631b` preserves full open `.22` acceptance. Current/previous native
and closed build proofs are preserved. Next same-creature batch: MagentaPuma owns
enabled75 in the canonical Skeleton module; AzureGate implements thin scheduling
joins. Use clearly declared configured-source exception controls while ordinary75
admission remains unproven. Firefly remains queued, without edits.

The linked75 batch12 is now implemented, verified, committed/pushed as
`c6d50265` and installed with native
`2a0b5762ecf26b015784b0ba9a2cb3cf67b9f31aed2c098378c70599e9bf8d4c`.
Configured source branches pass0.518s and scheduled lethal damage through natural
hide/reclaim passes1.388s; these do not demonstrate an ordinary75 producer.
Real L5 following, registry/source-order, core69.499s and workspace15.737s pass.
Bounded source/gate review found no further defect in this batch; UBS has zero
added critical findings, with legacy formatting/scanner debt retained. Native
rig55.326s, all636 inputs, worker137024344 bytes, installed444-input receipt and
both plugins match. Root viewed canonical map/loading/NightSpire terrain/HUD
with that native, one click/no retry/private-save/gameplay; owned smoke stopped.
Evidence: candidate12 and `Builds/delivery-skeleton-20261106-batch5/`.
Full `.22` remains open: unimplemented source-distinct alternate79 modes and
source-default returns must not be called equivalent to the point fallback.
Magenta owns those canonical helpers, Azure the thin join and scheduled consumer
check. Keep configured exception coverage explicit; no invented ordinary
producer or source admission change. Firefly waits. Current2a0b and previous6bd
remain local; older closed native pins and returned build outputs are hash-
preserved on rig ext4 before local removal. No native Skeleton encounter,
human combat/FPS/audio/full-mission/old-save proof is claimed.

The following settlement/Archer notes are chronological evidence, including
preserved intermediate failures and superseded pending-installation statements.

The previous Dragon/Bee batch is implemented, verified, committed and installed.
Runtime `aeba96fc687271fa2cb3b32182f245f4545f8cc4` is pushed; matching native
`4d875c2e74bd480f3f603ce2112ef9b2b7ac32c95b457b5df3e602d9d15962bd`
is installed and qualified in the Original client. Strict rig native build
passes66.955s; all444 Rust and162 upstream inputs match the frozen candidate,
isolated source and installation. Native-only installation/qualification pass.
Root viewed the canonical map, loading and NightSpire terrain/HUD with the exact
mapped native on the first attempt: one portal click, no retry or gameplay, no
private save. Both owned smoke processes were stopped. Evidence is candidate19
under `generated/dragon-bee-20261105-integration/` and
`Builds/delivery-dragon-bee-20261105-batch1/runtime-proof.json`. Finite `.19` and
`.20` original acceptance is delivered; parent/shared `.24` and terrain `.8`
remain open. No human combat/FPS, hardware audio, full missions, old-save or
native Dragon/Bee encounter visual claim. Source-default orphan/null and custom
selector limits below remain explicit. Archer delivery follows below.

### Archer delivered; settlement prerequisite next

Archer runtime `c7f3b55cbef98ac66114471f60960cf66e23f78b` is committed/pushed.
Matching native `fdeda626922471321f557d14ed3076675d36cb1a8ad3f59b25d5e5a72e811af8`
passes strict rig build66.301s, native-only installation and installed qualification.
All444Rust+162upstream inputs match canonical/isolated/rig source; the installed
receipt's444Rust inputs match exactly. Root viewed canonical Original map,
loading artwork and NightSpire terrain/HUD (including spawn flash) with the
matching mapped native: one portal click, no retry, no private save. Owned smoke
processes were stopped. Initial startup captures were blank while initialization
continued; no startup timing/FPS claim. Evidence is candidate5 under
`generated/archer-20261105-integration` and
`Builds/delivery-archer-20261105-batch1/runtime-proof.json`.
Finite `.21` acceptance is delivered; `.24`, Skeleton `.22`, settlement `.16`,
other creature families and the shared engine remain open. This is focused
producer/consumer verification plus launch/load, not native Archer encounter
visuals, human combat/FPS, hardware audio, whole missions or old saves.

Active next: complete existing settlement `.16` (Builder/Villager/Trader production,
household/population and actual trade consumers), then Skeleton `.22`, whose
original dependency on that family remains intact. Retain the small team and
reuse current evidence; source inspection must identify the actual unmet link
before editing. MagentaPuma owns the complete capability, AzureGate implements
shared joins, BrightCastle supplies bounded source/consumer diagnosis and root
GreenCave serializes Cargo/push/install. `.16` is in_progress; existing real
checks must distinguish missing code from incomplete evidence.

Current settlement source trace corrects the friendly Trader wording: source
237B0 enters an eligible House, increases occupancy, then hides/reclaims the
Trader. It does not deposit current mana in a Castle. House mana is the source
fixed BLDGPRM scalar, independent of occupancy; ownership and next-boundary60F00
census feed actual wizard capacity and Castle/defender consumers. No invented
direct deposit is added. Fresh wholeLevel7 unforced resetTrader→House occupancy/
hide/reclaim passes72.120s on the installed Archer source; seeded House→next
boundary Castle projection passes0.382s and is labelled a narrow control.

Pending four-file settlement candidate1 compiles on strict rig147.632s with all
444inputs stable. Builder typed health now reaches the existing definition/
constructor and validator: nondefault40000 construction/reset passes0.546s;
signed-dword range control passes remotely10.869s. Trader's Level2 row-number
guard is removed behind its typed constructor; freshLevel22 reset/adoption and
separate real-row237 constructor control pass0.973s. Captured reset Trader147
was already present: the earlier suspected lost actor is explicitly withdrawn.
The separate constructor control is not an ordinary second birth.

The new paid public House12→Builder→House→paid Possession check initially fails
7.950s at the unchanged100-turn guard:20paid casts, House life20000 unchanged.
Candidate2's capped trajectory shows real impact at that House's centre. The
shared special model45 damage predicate incorrectly reads signed mask cells;
pinned10C80 actually dereferences uint8_t*, whose promoted0..255 comparison with
-1 always succeeds. Actual-header C++ type/exhaustive-byte proof confirms this,
including0xff. The canonical helper now preserves selector admission and caller
strict-axis checks without inventing signed transparency, negative-index
padding or a new asset format. Regular signed-cell collision API is unchanged.

Candidate3 proves demolition, natural Builder construction and public House
claim, but its immediate payment-delta assertion fails26.294s. Source Possession
queues negative mana_regen; upkeep consumes it on the next player visit. The
corrected source-timed assertion requires that actual queue and exact next
debit. Candidate4 full unchanged gameplay path passes27.232s: House12→same
Builder121/gen2→constructed House120/gen67→paid Possession/owner/census and
presentation. Actual writer/missing-bank/strict-axis control passes1.025s;
integrated workspace/all-target check passes46.715s, all444Rust+37extra inputs
stable. Rig/local compiled binary hashes/extents are independently compared.
Original negatives remain; package overrides are now recorded explicitly.

An affected existing model76/FireSpheres test has a preexisting row112 switch
failure, reproduced on installedc7 in0.369s. Switching export-only reset to the
normal semantic session was insufficient. Candidate5's same7-turn trace confirms
real25/group10/[112] activation and a current root119, whose generic-effect
constructor deliberately stores marker65535 rather than authored112. Its real
sound is a Flight PrepareSound/site3 projection, not the legacy economy event.
Sol corrects these two obsolete observations using publication/consumed-row
evidence, unique new generation-bound handle and the actual sound journal;
the current selector reader must also use the typed arena/source_target view,
not raw retained entity shadows. Candidate7's corrected existing gate passes
2.774s with original timing/count/RNG/payload/owner/reset/replay assertions.
Final Builder typed-health and Trader adoption/constructor checks pass0.443s/
0.986s. All444current inputs match candidate7; the only change since passing
candidate4 public/workspace evidence is that model76 test observer. Production
and the public House consumer are byte-identical, so their evidence is reused.
Fresh independent bounded review supports the unsigned-byte/caller/gate fix
without a concrete introduced defect. No production, wait, admission or
asset-format change was needed to repair these obsolete test observations.
Finite `.9` is not reopened. Full `.16` remains open for its actual Villager
death/population/target/reclaim and Builder/Villager conversion consumers, not
closed on this useful House batch. This coherent batch is implemented, verified,
committed/pushed as `7afb95566c0c0259a3ac45421595ff2b10bda952` and installed.
Matching native `bb1d511247b23e450bd255b0ffcf14f513551cb2f4d2067d282e49532dfea829`
passes strict rig build51.384s; independent SSH hashes/extent match the returned
artifact. All444Rust+162upstream inputs match canonical and isolated source;
both Original plugin copies and its444-input installed receipt match. Native-only
installation/qualification pass. Root viewed normal canonical map, original
loading artwork and NightSpire terrain/HUD/spawn flash with that mapped native:
one portal click, no retry, no private save. Owned smoke processes are stopped.
Evidence: settlement candidate7 and
`Builds/delivery-settlement-20261105-batch1/runtime-proof.json`. No Unity/art
change, native settlement encounter visual, human combat/FPS, hardware audio,
full-mission or old-save proof. Archer native remains preserved as the immediate
previous checkpoint.
Actual paid Villager death is now verified in candidate10 (15.805s): real
Fireball launch generation→impact generation→original77/gen0 accepted fatal
mailbox250/life0→-250/player credit, living presentation then immediate hidden
removal, semantic living-root loss and following source reclaim/target removal.
Candidate8/9 negatives remain: end-of-turn mana deltas falsely classified all
launches unpaid. The repaired gate observes the real holder's queued negative
cost and later exact player upkeep debit in existing ordered CF10 receipts,
and pins the latest accepted carried mailbox rather than any older equal tuple.
No actor, timing, RNG, payment or damage expectation was weakened. Compile
62.058s/444stable and independently matched rig executable; root reviewed the
gate and UBS has no critical added line. Only the existing cfg(test) module
changed; all production and other443inputs match installed7afb, so no duplicate
native build/smoke is required. This is real in-process gameplay evidence, not
a native Villager encounter visual or whole-family completion.
MagentaPuma next owns actual Builder/Villager Skeleton conversion through the
ordinary Level7 producer, with AzureGate supporting shared joins. Source excludes models12–15
from the global creature-population census, so an invented global decrement is
not an acceptance gate. Full `.16` and its original dependencies stay open.
The first conversion candidate11 used the dynamic-switch list for imported
reset279 and failed before stepping. Candidate12 corrects that observer using
the actual11/0/sourceID19 reset carrier; ordinary group19 publication succeeds,
but the natural encounter does not converge before the player dies (72.183s).
No conversion is qualified and no mission/actor/admission patch is justified.
Inspect another real authored encounter before repeating a long run.
Fresh bounded `.16` review found a concrete conditional consumer omission:
typed settlement constructors install mana, but Builder101/Trader117/Villager109
skip source1C930→36BA0(false) for positive values. Source defaults remain zero;
do not invent a default-mission positive-mana producer. MagentaPuma owns the
complete family and dynamic joins; AzureGate implements the plain-Villager/shared
death-mana join through existing sphere/cleanup consumers. Source RNG before
allocation, owner clearing even on failure, current-occupant continuation and
burst/hide order must hold. Builder sound10 also moves to its existing MC2 asset
adapter without changing the default. Full acceptance stays open.
Candidate14 core+CAPI compiles70.883s/444stable; independent worker/local
executable hashes match. All focused mana/collection/reclaim controls pass,
including post-build Builder12/action109 with one RNG draw. Real Trader raw
header/terrain retirement is now joined before economy release; its prior
retained119/gen1 failure is preserved. Legacy control initialization was also
repaired to begin its actual source turn. Source-order, paid Villager16.018s
and public House→Builder→paid claim26.418s pass. This source is not installed.
The real House85 conversion route still fails25.280s: genuine paid impacts at
its live centre reach no accepted mailbox. Its starting29 was construction,
not combat life; natural live state is25000. A proven retained-data blocker is
now identified: reset House85 selects mask30, but the qualified Level7 bank has
only60/65. The C++ exporter uses complete construction selectors only for
Level2; Rust qualification also pins that closed profile toLevel2. Root owns
the shared exporter closure/build; AzureGate owns strict generic qualification
and compatibility with existing profiles, and MagentaPuma remains responsible
for the connected gameplay consumer. Retain genuine map-bank bytes and source
selector provenance, with missing/wrong banks rejected; never manufacture masks
or weaken the source gate. Fixing this omission alone does not yet qualify the
entire collision/list/schedule route or either settlement conversion.
The canonical exporter now closes the actual model45 selector set separately
from initial-Castle terrain footprints. New generic CollisionFact4 packages
require real MCI3v4/BLDGPRM and independently derived authored/reset/Builder/
successor inputs; legacy authoredv1/v2 and Level2v3's exact byte pin remain.
Fresh wholeLevel7 has22 masks, including30 and excluding Castle-only1, all
dimensions/payloads independently matched to actual pinned Night TAB/DAT.
All15 other binary members are unchanged; a newLevel2 export retains all15
binary members and its v3 pin. Exporter6919a0c0 has160 matching compiled inputs
and its embedded provenance marker. Python qualification plus rehashed omitted/
extra/wrong-bank/profile negatives pass0.16s. Generic declared hashes retain
their existing trust model; no arbitrary rehashed-pixel authentication claim.
Legacyv2 packages remain authored-only, and canonical launcher content currently
installsLevels1–4; this newLevel7 package is connected verification input, not
a newly installed Level7 campaign route. Candidate15 compilation found only
missing test imports; the exact two-import repair is frozen for candidate16.
Candidate16 compiles60.354s/444stable and the exact rig/returned executable
hash matches. Real Rust v4 qualification/ordinary-session bank arrival passes
0.796s. The unchanged House85 gate now reaches real paid demolition and actual
House-born Builder373/gen34 conversion by Skeleton381/gen1 into11/gen1, then
fails79.996s because its hide assertion reads the retained raw shadow. The
conversion owner actually hides the typed house_resident; census/presentation
read that carrier. Magenta is correcting this observation while retaining
same-pass hide, presentation loss, following reclaim and child scheduling.
Source17 then joins the real missing raw hide flag before NewEvent, so the
family's existing owning reclaim can retire that same address. Its full
House85→Builder373/gen34→Skeleton381/gen1→child11/gen1 path passes78.327s,
including typed hide/census/DTO, following original-generation/target reclaim
and child callback. Compile63.008s/444stable and independent rig SHA match.
An affected old Archer control failed identically on16/17: it forced74, which
correctly routes20C50; nested20940 conversion requires73/mode1. Source18 repairs
only that configured entry and passes1.538s, retaining all failure/count/
identity/reset assertions. Compile62.426s/444stable, independently matched.
Production is identical to17. The separate actual Villager encounter remains
unverified: one genuinely paid shot lands about750 units short; exact pulse/
target/scanned-cell/overlap shows a legitimate miss, not a damage-admission
defect. Owner is correcting only the player's firing line and observing the
actual terminal reason; no NPC state, RNG, timing or acceptance changes.
Full `.16` also retains a confirmed all-map asset join: canonical Level1 and3
legacy banks each omit nine eligible Builder choices. The new generic v4
producer/qualifier currently applies beyond those legacy profiles; a fresh
first-map export still cannot supply that complete closure. After installing
the connected working batch, extend the common owner-data contract to those
opening maps while preserving their existing qualified profiles/rollback.
Do not close `.16` from Level7 success or label this a mission behavior patch.
The connected working batch is now verified through candidate24: actual paid
selected Skeleton374/gen1/auth454 takes250, reacts/moves/retreats; independently
pinned same-group Skeleton377/gen1/auth457 converts original Villager336/gen0
into64/gen18. The full gate passes45.956s including real hidden census/DTO,
following original-generation/target reclaim and child callback. The incorrect
selected-converter hypothesis is retired with all negative sources/logs saved.
The test-only census lacked converted-origin candidates in a formerly unbound
slot; adding those real keys retains all identity/living/generation filters.
Its current/hidden/inactive/stale-key/reused-generation control passes1.033s.
Candidate24 compiles62.864s/444stable with independent rig executable match.
The integrated workspace/all-target check on22 passes38.875s with444Rust+37extra
inputs stable. Later changes are confined to core test controls/observation;
their final core compile covers those deltas. Source-order19 passes0.618s.
All production since17 is unchanged; its real Builder conversion and14 death/
mana/collection checks are reused. Fresh bounded source/gate reviews found no
new concrete defect in those joins. Implemented/verified; commit/install follow.
This batch is committed/pushed as `f27fb6b20515c2e1376bd71bb02bd8350ef26cf4`
and installed with native
`b24b6014a6955cb9dca4856d903b56ad7079f5a0a498f66fcdf1c16939425512`.
Strict rig native build passes64.502s; exact worker/returned artifact hash and
136940480-byte extent match. Canonical/isolated/rig inputs match444Rust,
162upstream and37extra compile-input categories (636unique paths). Both
Original plugins and its444-input installed receipt match; native-only install
and installed qualification pass. Root viewed canonical Original map and
NightSpire terrain/HUD with the exact mapped native: one portal click, no
retry/private-save load; both owned smoke processes stopped. Loading-open
events passed, but the loading-art screenshot was too early and shows the map,
so no viewed-loading-art claim. No Unity/art change, native NPC encounter
visual, human combat/FPS, hardware audio, whole mission or old-save proof.
Evidence: candidate24 and `Builds/delivery-settlement-20261106-batch1/runtime-proof.json`.
The previous bb1d native and player are preserved. Full `.16` remains open;
next implement the complete first-map collision input contract noted above,
then original Skeleton `.22` acceptance without bypassing its dependency.
The C++ UBS module failed on
unsupported control flow; it provides no clean-scan claim. Rust/Python UBS has
zero critical added-line findings; whole-file format/heuristic debt
remains. Cleanup keeps all proof binaries/current/previous natives intact;
only disposable dispatcher caches and a closed Nov4 incremental generation
were removed under Oliver's explicit build-file cleanup authorization, retaining
the full local code-generation cache and remote warm generations. Two closed
metadata-only local check caches were also pruned to provide measured output
headroom; source, owner data, proofs and current/previous native pins stay.
The proposed four older Archer
proof-binary prune was blocked by DCG and was not executed or bypassed.
For the next settlement checks, diagnostic executables2/3/5 were byte-preserved
on rig's ext4 proof archive using the existing preservation helper, with SHA
verification before and after the move. Receipts retain their exact locations;
current candidate7, all source/packages and current/previous natives stay local.
The obsolete candidate6 observer executable is preserved there by the same
before/after verified move; its negative receipts remain local.
Further completed settlement executables1/8/9/11/12 are preserved on the same
rig archive with before/after SHA receipts. Current installed/previous natives
and latest positive test pins remain local. Measured native returns reached
about988MB, so native preflight now requires6.2GB free, retaining the5GB floor
and a concurrent-write margin; no threshold was lowered.

The first-map settlement input join is implemented and verified in candidate26,
not yet installed. Fresh whole Level1/3 campaign v6 retains each map's real
player/reset/allocator dialect and carries MCI4, BLDG and all22 source-closed
model45 masks. Legacy profiles remain independent; a bounded review caught and
root repaired standalone Level3 emission so its owner-v5 contract remains.
The current exporter has160 checked compiled inputs/provenance marker; actual
standalone3 verification passes5.838s and both changed first-map twin-export
gates pass33.16s. The earlier campaign export binary's sole source delta is
the standalone-only condition; its campaign branch is unchanged.
Real first-map packages plus newly built ENV owner caches open through the
ordinary Rust consumer, retain masks/footprints and reject rehashed missing
selectors/old profiles: PASS8.754s. Real configured Builder-row19 House
construction, model45 mailbox writing and scheduled damage consumption pass
2.615s on both maps. This is a configured constructor control; ordinary births
and full lifecycles retain the earlier actual Level2/7 evidence. Strict rig
core67.434s/444stable and workspace/all-target16.181s/444Rust+37extra pass;
returned executable hash/extent independently match rig. No scheduler change.
The profile now survives decode/preflight and binds BLDG into actual semantic
constructors and owner-cache identity. Both common ENV caches validate with
the current package and full64-slot original sound banks. L1 still requires
its actual Night-v2 language/font consumer: new Night presentation build
passes25.494s, checks real BLDG/MSPRN bytes and retains exact original visual
qualification. Nine focused old/new Night/rehash gates pass; Python v6/v4/
legacy gates pass, as do17launcher tests. Default launcher bindings now select
these actual packages/caches; no Unity/C# or art change. A fresh bounded review
finds no additional concrete enabled default settlement-family gap, conditional
on the remaining native installation and normal launcher proof. Full `.16`
stays open until that delivery; `.22` keeps its original dependency. Human
combat/FPS/hardware audio, full missions, old saves, native settlement encounter
visuals and every eligible row's individual contact remain unproved.
Touched-file formatting still reports legacy/line-wrap debt; source/gate review
and changed-hunk scanner triage found no added critical defect. C++ UBS remains
partial, not a clean-scan claim. Current/previous native and player stay local;
older completed proofs were byte-preserved on rig's ext4 before removing local
copies, with before/after hashes. No owner data/Modern work was cleaned.

Source `ae2e84d1` is committed/pushed and native
`9d9af55f3c61697c8892fd2b7c63970cc590ad1c675b265d076ce5b8e37facfc`
is installed/qualified after strict rig52.796s and independent136943424-byte
artifact/636-input matching. The first normal launch showed the map, but its
single portal click FAILED Night presentation qualification before mission
open; `.16` is not delivered yet. All1249 old/new cache products match except
three truthful exporter-provenance fields in the frame-census JSON. The fix
retains the old exact content fingerprint and qualifies the new one against
actual v5/v6 package schemas, with all other hash/tree/seal checks retained.
Actual preflight must pass the matched package to that validator before Rust
opens a level. The previous package-free call exposed a rehashed cross-pair
alias and is also repaired. Root copied only those two production C# files and
the focused existing-module test into the qualified Original snapshot; Modern
work is preserved. A matching Unity player rebuild is now required, reusing
the same9d9 native. Short ext4 TMPDIR is required for Unix socket length and
tmpfs headroom. Unity compiled the C# changes; the new rehashed-pair control
first exposed its own XML-JSON writer's `[ ]` versus canonical `[]` mismatch.
The producer-digest assertion stays intact while that test-only codec is fixed.
Do not close `.16` or start `.22` implementation before the final positive/
cross-pair check and actual matching player launch/load succeed.
The corrected focused Unity control now PASSES133.016s with the actual complete
four-package prefix and each map's matching ENV owner cache. Both genuine
v5/v6 Night pairs and package-free qualification reads pass; fully rehashed
opposite-content pairs reject through actual preflight while still Loading
with no session, before GameRuntimeOpenLevel. Earlier negatives remain saved:
the JSON writer's single empty-array space, inadmissible one-map catalog and
legacy-audio fallback caused by the test's omitted ENV input. No runtime
admission or digest assertion was weakened. The test's negative changes only
frontend presentation after a genuine native bootstrap. Production C# remains
OwnerAssetCache9a413493/Presenter99e6994f; matching Original player rebuild is
next, reusing9d9 without another Rust build.

The following
records the Archer integration and its preserved negative results.

Oliver explicitly resumed the full existing creature queue and authorized
cleanup of build outputs when space is tight. Continue this list with the
small native team, one connected capability through delivery before starting
the next: Terra/xhigh MagentaPuma owns Archer, Terra/xhigh BrightCastle supplies
bounded source/gate review, Sol/xhigh AzureGate actively implements shared joins.
Root GreenCave owns sole Cargo/RCH/push/install (prior CrimsonBay credential was
unavailable; no retired NTM identity was reused). Existing acceptance/order stays.

The initial failed return below remains preserved. A single fresh direct
retrieval of the already compiled executable now matches remote SHA78060ed3
exactly; all444 Rust and162 upstream inputs match before/after retrieval.
No rebuild or validation weakening was used. Evidence:
`candidate-1/verified-retrieval-receipt.json`. The first actual Archer gate ran
and failed8.037s at a genuine shared Fireball clear: legacy NightSpire players
have a sealed573-byte record and no optional auxiliary, while the old writer
unconditionally addressed encoded offset589. AzureGate implemented the
canonical typed/encoded auxiliary setter and all seven same-field writers,
preserving legacy/extended formats and dispatcher/publication order. The exact
legacy/extended contract check passes1.287s on unchanged shared source.

The active consumers are authored Archer combat/death→class11/17 countdown,
real reset House12 demolition→HouseSpawn Archer→arrow damage, real Level12
group6 StageVar2 point-guard→player target/arrow, and separate actual Level3
group5 Skeleton conversion. Root/gate review
requires original-generation fatal credit/reward observations and exact arrow
mailbox attribution; owner-ID-only damage events are insufficient. StageVar
selected-target motion and actual converting predator are recorded separately
when source scheduling does not guarantee they are the same actor.

Candidate2 strict rig compile/return now passes115.411s, all444 frozen inputs
match. Legacy/extended auxiliary contract passes1.287s. Actual authored
combat/death/absence passes16.828s and House demolition/spawn/arrow damage
passes38.446s. Their remaining parent-at-birth proof gap is corrected in the
next candidate with a cfg(test) receipt at the actual owned arrow constructor;
the passing earlier runs are diagnostic, not finite `.21` closure.
The combined Level3 StageVar/conversion test fails45.111s: source data shows
StageVar1 selects a Worm exemplar, not Archer. The test hypothesis was wrong;
no gameplay change is justified by that failure. Current owner inputs instead
bind Level12 StageVar2 to Archer exemplar93 as kind2 point guard at130,91.
A NEW whole Level12 package is exported/structurally verified under
`level12-owner-current/level12`, using the pinnedbe08 exporter with all160
source hashes and actual embedded marker verified. Candidate3 passes all four
actual consumers: authored combat/absence16.918s, House37.847s, natural
conversion19.132s and Level12 guard5.616s. Exact arrow-birth and accepted-mailbox
receipts now join the real child's generation to later actual player damage;
conversion records the real producer/prey/child handles at the writer.

An event-vector reset hypothesis was also withdrawn after checking actual
callers: `begin_turn()` clears only in test-only callers; ordinary outer turns
retain economy event history and reset a scratch cursor. Absolute writer event
cursors remain valid across ordinary turns. The owner reverted only that losing
test candidate, preserving birth/generation/reward and real timing predicates.

Root recorded hashes and removed327 isolated dispatcher-generated return-cache
files/obsolete executable (846029425bytes), with their strict rig regeneration
path documented. Source, owner assets, current/previous native pins, remote/local
warm caches and failed-artifact evidence were preserved. This restored7.2GB at
that point; measure again before each expensive stage. A second bounded cleanup
removed326 regenerated dispatcher return copies/700100448bytes after checking
none were open/mapped, restoring6.48GB. The same source/data/cache/native and
failed-artifact preservation boundaries hold.

Archer's typed max-life1000 now feeds ordinary/House/semantic consumers, keeping
constructor mana=max_life>>1. MC2 adapter-owned descriptor75 and idle/attack/arrow
bindings reach actual kernel/packet consumers; a counterfactual supplied-binding
check confirms that join. Independent bounded review found no concrete defect.
Candidate4 strict rig compile passes106.833s with all444inputs unchanged. Its
whole Archer module passes21/22, including all four real paths and the binding
check. The remaining existing control expects flags0x14 after reflection, but
source NewEvent8→AddMap4→test reflection16 requires0x1c. The corrected control
first requires initial0x0c then exact0x1c, preserving its other death/reflection/
mana assertions. Candidate5 strict rig compile passes65.965s and the corrected
exact control passes0.145s; production is byte-identical to candidate4, whose
other21 tests passed. The single integrated workspace/all-target check passes
41.796s; all444Rust and37 extra compile inputs remain unchanged. The initial
preflight stopped before Cargo because an older include inventory also listed
three now-changed Rust files; those are correctly checked against candidate5's
444-input snapshot, not the previous runtime hashes. Changed-source UBS reports
no critical added-line finding; whole-file heuristic/formatting debt remains.
Source subsequently committed and installed as recorded above. Skeleton `.22`
still depends on settlement `.16`; finish that existing
prerequisite before claiming full Skeleton acceptance. No dependency is bypassed.

The following records the earlier stop and preserved starting point.

`mc-1pb.21` stays in_progress with MagentaPuma as accountable owner. Its whole
Level1 group3 actual arrow/combat/death/reward/hide/reclaim→class11/17 countdown→
group11 Speed gate is preserved, UNVERIFIED, in canonical `level2_class5/archer.rs`
at SHA `e929c487d8647c69ca6e075f9156bba1abc87c351accc0446c44324d319d3e93`.
No production Archer behavior changed. Remote candidate1 compilation succeeds
57.07s, but RCH102/E309 refuses publication: the145942160-byte returned executable
SHA `e1093bdf3018299e75229ce77456b080e644f45feb40ef09c1e482316032f226`
differs from remote compiled SHA
`78060ed3ff0a48d1e62f874e76a2fcbc41c82df4d8e719f4e81a65b9af4acc40`.
One bounded comparison confirms all444 remote source inputs match candidate1.
The bad artifact was not executed; no build/copy retry was made. Failed bytes,
logs, frozen source/patch and comparison remain under
`generated/archer-20261105-integration/candidate-1/`. Cause is not established.
Root reverted only its isolated execution copy to the installedaeba96fc after
preserving it; canonical uncommitted Archer work stayed intact. Installed/current
native hashes still match4d875c2e. Workers were stopped at that checkpoint;
operator-directed resumption and exact verified retrieval are recorded above.

Remaining Archer acceptance includes actual house production and natural
Skeleton conversion/StageVar consumers. Legacy tests force hide/predator state
or directly call construction, so they do not establish those ordinary paths.
The earlier Level3 StageVar hypothesis was subsequently falsified as recorded
above; it must not be used as an Archer producer claim. No natural conversion
is claimed yet. Exact cfg(test) conversion and arrow writer receipts are now
implemented and the next candidate consumes them. An older closed Nov4 compiler generation was
SHA-preserved on healthy rig for headroom; latest Nov5 local/RCH caches and the
current/previous native pins remain intact. This does not qualify Archer or
the rest of the shared engine.

The previous Wyvern/defender batch remains a preserved delivered checkpoint.
Runtime `793b7ef62973d4153ab1372ffee3afc559585813` is pushed; matching native
`ecbf096983e4b60b3fe63a24a1c734ff569d5d12efd08b31c155a3c39a198b18`
was installed in the qualified Original client and is the preserved previous
native. Strict rig native build passed
48.397s; native-only installation and installed qualification passed. All444
Rust and162 upstream inputs match rig/canonical/isolated source, and the
installed receipt matches all444 Rust inputs. Root viewed the normal canonical
map, loading and NightSpire terrain/HUD on the first attempt, with the exact
mapped native, then stopped both owned processes. One portal click, no retry or
gameplay input. The isolated display had software/no-sound output; hardware
audio is not qualified. No private save was written. Evidence:
`Builds/delivery-creatures-20261104-batch1/runtime-proof.json` and
`generated/creatures-20261104-integration/candidate-20/` and earlier connected
runs. All444 Rust inputs are frozen. Candidate20 differs from18 only in the
new sphere boundary's source-pass/resource prerequisites; production is byte
identical. The strict rig workspace/all-target check passes22.907s.

Actual owner18 Wyvern production/route/target inheritance, real projectile
damage, sprite207 and prepared sound39 pass. Its full reward path on18 passes
91.451s: five public Meteor casts, source kill credit0→1, original target172
objective, natural hide/reclaim, actual neutral30000 sphere, public Possession,
collector217/Castle215 deposit and exact131000 census/player capacity. This run
has no rival contest;11 separately passed contest/recapture. Corrections include
source target-overlap precedence and linked-point-hit retaliation. Player-only
test aim accounts for the real hand offset and the still-moving sphere; damage,
RNG, life and ownership outcomes are not assigned.

The actual hostile Castle→defender path on18 passes18.731s: exact MCI BUILD1..5
binding, ordinary stage5 construction/model15 production, movement/aim/arrow,
prepared33/34 and attributed player damage, paid Fireball retaliation/death,
following reclaim, cleared owner relation and replacement. Initial resources
and player travel/aim are declared controls. Source collision axes, full probe
extent, wrapped guard-Y, linked-point reactions, allocation throttle, independent
arrow depth and promoted Z arithmetic are corrected. Public Castle, source
order/reset and focused source/pressure controls pass; pressure controls do not
establish ordinary self-alias reachability. This owner package lacks BUILD6/7;
those are not invented or claimed qualified by this path.

Connected collection exposed missing current fields across fixed-address reuse.
Speed trails now supply their real zero axes/word96 through hide/reclaim;
CastleAuxiliary supplies its current target/attacker/stage fields. Failure-time
diagnostics identified the remaining cached leader as a real ManaSphere10/39,
not the later9/20 packet seen after the whole turn. Sphere word96 now reads the
actual collector slot, survives flag-only release/collector retirement, and is
retained at reclaim. Candidate20's actual sphere→collector assignment→Cymmerian
current read→collection/hide/reclaim boundary passes0.627s, including poisoned
raw rejection and a separately labelled retirement prerequisite. The original
100-mana replay runs all1024 iterations without dispatch errors on18; its final
combat assertion still fails from insufficient mana/dead player and is not a
gameplay pass. Fresh bounded source and independent gate reviews support the
changed paths; package formatting remains non-clean, without a clean-format claim.

The batch owners handed back frozen source: Terra BoldHollow/AmberOtter, Sol
CrimsonGlacier/BrownMountain. Root AmberWolf independently verified and delivered
the integrated batch. Finite defender `.4` and Wyvern `.5` acceptance is
qualified. The original Fireball `.10` prerequisite is closed after a current
bounded source/acceptance assessment by Sol and root against its actual
acquired all-tier, capacity/alias, water/expiry and public CAPI evidence; no
unchanged old positive was rerun merely for closure. The next affected Bee
batch will exercise that shared boundary again. Full `.24` stays open. Next
connected families are Dragon `.19` and Bee `.20`. Unresolved
Level23 initial Castle3/2 identity at0,0 remains in `mc-bhq.8` comment99; the
fresh whole package is structurally/exporter-qualified, not runtime-qualified.
No human combat/FPS/hardware audio, all missions, old-save or arbitrary custom
particle-override claim. Retired pane maps and the deleted heartbeat stay retired.

### Delivered batch: Dragon and Bee

Dragon ScarletValley, Bee LavenderAnchor and integrator CrimsonGlacier handed
back the original frozen source. Oliver resumed this batch on2026-11-05.
Current root CrimsonBay uses Terra/xhigh MagentaPuma (`bee_runtime`) and
BrightCastle (`dragon_connected_review`, now implementing Dragon ownership).
StormyCrane (`dragon_runtime`) and Sol/xhigh CyanBridge hit confirmed model
capacity and are stopped; available owners continued their preserved work.
Root owns shared joins and serialized build/install.
The batch is now committed/pushed asaeba96fc and installed as4d875c2e; the older
793b7ef6/ecbf0969 checkpoint remains preserved as the immediate previous native.
Frozen copies, hashes
and the original owned patch are under
`generated/dragon-bee-20261105-integration/preserved-source/`. Finite `.19` and
`.20` acceptance is delivered; shared `.24` stays open. Fireball `.10` is closed.

Dragon fixes source4B240 admission/partial body allocation, source-ID threat
comparison, promoted vertical arithmetic and each handler's distinct coordinate
promotion/separation. Bee production was unchanged; its new connected gate
uses actual group21, contact/audio, public paid Fireballs, all three deaths,
credit, hide/reclaim and the original absence objective. The shared definition
now admits source signed-dword Bee health instead of rejecting values above
32767. Backend gates add real body targeting/damage, current projectile
reaction, kill credit, natural cleanup and the original guardian objective.

Current bounded evidence is in `generated/dragon-bee-20261105-integration/`.
Bee's actual Day group21/contact/audio/public paid Fireball→all3kills/credit/
natural hide/reclaim/original absence passes on2 (10.804s). The unchanged Bee
owner's remaining19 focused controls pass on3 (11.62s, root-observed execution
receipt; raw stdout was not separately captured). Signed-dword definition
admission passes on1 (11.704s). Actual whole Level25 group2 terrain14/1
construction→group3 close markers→48 visits→terrain/flight query/audio and
following marker reclaim pass on2 (12.051s); both group activations are declared
progression controls. Full terrain `.8` stays open, including Level23 startup.

Dragon's ordinary public player shot reaches real raw10/0 effects, exact
three-part damage250 and later root adoption; its focused body gate passes on7
(139.186s). The separate bounded guardian encounter exposed omitted source68BD0
reaction and an incorrect body-row guidance census. Canonical guided acquisition
now consumes the source frozen heads/current fields and writes the selected
Dragon's reaction before movement/collision/Rebound. The actual guardian exchange
passes17.018s on12 and17.736s on13 with paid casts, generation-correlated lateral
reaction, body damage, source credit, all34 deaths/rewards/hide/reclaim and the
original reveal objective. Player-only root-directed1536 shots give the real
reaction time before the384 body shots; no actor/RNG/timer forcing or AI player.
Actual launch/impact occurrence and target generations are observed at their
own producer/writer boundaries; sound owner IDs and legacy typed-effect lists
are not projectile/raw-effect identity.

The real boss path exposed a missing source primary-render→visibility input.
The existing indexed source compositor now emits qualifying primary completion
receipts; internal clipping follows literal source behavior, and shadow/
reflection or rejected traversal cannot emit them. Root joins private frame
identity/source view to authority and replay, including neutral presentation's
cloned-state boundary and paused-safe typed input. Actual frame/identity controls
pass15.708s and serialized/direct replay-input consumption passes24.98s on7.
The real CAPI paused/cached/fresh-presentation check passes51.357s on9; it is not
a nonempty boss full saved-session restoration proof. Generation-bound tail
provenance and linked-constructor publication now survive root reclaim/reuse;
the focused reuse control passes1.014s on7. With the matching newly imported
whole Level25 environment, the actual boss advances past visibility wait and
summons real Dragons. Earlier apparent expiry was combat death. The corrected
ordinary boss path now passes47.925s on12 and49.491s on13: actual17→16, lethal
combat removes the player from frozen class3, the same living child counts down
by4 to-2 with its original parent still live, real10/0 allocation, hide and all16
body generations reclaim. No assigned life/timers or parent-loss substitution.
The later delivered checkpoint above qualifies finite `.19` acceptance.
Acquired all-tier Fireball passes13.559s and the Bee combat path10.526s on12;
the corrected guidance/collision census and source-order pass. Candidate9 CAPI
compile passes74.459s;13 isolated core compile126.380s and14 compile69.290s.
All failed logs remain preserved; no current candidate installation claim.

Original acceptance review found missing typed Dragon ownership. `DragonClassProperties`
now owns consumed constructor/movement/combat values, `MC2_DRAGON_ASSETS` owns
descriptor/particle/sound bindings, and the family produces attack projectiles,
current-target feedback and audio before the scheduler's current-occupant tail.
The wildcard import is removed. Shared projectile construction consumes the
family payload; Firefly keeps its source500. The default binding check passes
1.075s on13. A separate unfiltered Day3 group1 Dragon attack passes9.434s on14:
actual live projectile/Impact/root/player generations, attributed500 damage and
sound8. It used one disclosed player-only contact position control; no NPC life,
RNG, target, timer or capacity writes. The older Skeleton-controlled test fails
before its attack setup and is not a positive claim. Four affected shared flyer
constructor/alias/lifetime controls pass7.628s and source-order0.580s on14.

Bee `.20` additionally names class11/model15, beyond the already-qualified type7
objective. Real Level5 row237/slot263 is its producer; the old retained package
lacks initial Castle BUILD1. Root regenerated the entire Level5 package with
the byte-qualified exporterbe08/160 matching compile inputs/actual embedded
marker. Structural validation passes; authored rows and ten reset Bees remain
unchanged. Set `MC_LEVEL5_PACKAGE` to this batch's `level5-owner-current/level5`.
Candidate14 reaches all ten original-generation hide/reclaim, natural16..1 then
seventeenth-clear-visit publication, exact12 successor Bees and switch reclaim.
Its added all-ten-player-credit requirement fails: real rival damage kills one
Bee and actual construction kills three. Candidate19's corrected gate passes
40.555s: all ten original generations have a successful causal writer, hide and
reclaim; paid player lethals have exact observed census credit; the original
switch visits16..1 and publishes the exact twelve group5 rows on its seventeenth
clear visit, then hides/reclaims. Source construction can rewrite an already-dead
retained actor's credit. The gate requires the first construction receipt to see
live life, every later receipt to see negative life, and the last owner to match
observed death credit. It rejects dead-only, wrong-generation, revival/re-kill
and final-credit mismatch chains. No fixed player-kill count, actor state forcing
or weakened admission. The actual two-owner burial control checks this writer
order independently. All earlier failures and candidate15's pre-Cargo headroom
stop remain preserved. This passes the connected consumer, not native visuals.

Dragon's unfiltered Level9 group18/row403 actual subtype route moves its root/body
to the real mode1 terminal on17 (3.242s). Current all30 owner-package inspection
finds112 Dragon rows, with actual bound routes1/2/6; levels4–25/31–35 are separately
matched to packed owner inputs. Levels1–3 lack the sidecars used for that check;
their missing sidecar qualification is explicit. Default Army/Morph selectors do
not produce Dragon mode13; custom selector reachability is unqualified. Source
enabled-null/orphan callbacks retain classification. Guardian, normal outgoing
500-damage attack and actual boss17→16 natural expiry pass on16 (19.849s/10.803s/
53.266s). Later candidates change tests/observers only. Final candidate19 compiles
on healthy rig in65.222s; all444 inputs match canonical and isolated source.
Independent review supports the final Bee gate. The full25-file UBS scan has no
added-line errors; its legacy whole-file errors and formatting debt remain.
The final two-file scan flags an intentional cfg(test) failure panic, not a new
production panic. No findings or assertions were suppressed. The actual two-owner
construction writer control passes1.013s. Strict rig workspace/all-target check
passes4.974s after restoring missing ignored compile inputs in the existing
isolated worktree:24 atlas CSVs match their embedded expected hashes, and the
recording probe is byte-identical to canonical. Both failed missing-input runs
are preserved; no gameplay code or gate was changed to fix that environment.
All444 Rust inputs and53 additional literal/atlas inputs match before and after
the passing check. Source/native delivery and its limits are recorded above.
The native return exceeded the former5.75GB stage budget and temporarily left
4.91GB free. Root started no further stage until735560955 returned auxiliary
bytes were SHA-verified/preserved on healthy rig, restoring5.65GB. Current and
previous native pins remain regular local files; future native preflight is
raised to5.90GB from the measured return. No source/data/warm-cache deletion.

After unrelated `journal.rs` changed during12, root separately verified all444
rig inputs still matched that compilation manifest. Its proof excludes the newer
local journal. Further verification uses the existing native isolated worktree,
with only owned Rust changes copied and exact before/after hashes, preserving
other writers. Separate commits12f45229/9dc7a58e are inherited, not this team's
work. The isolated base is now49c68f15; canonical latera6a65532 is a disjoint research
document commit. No new worktree or infrastructure was made.

Reference filters below; reuse completed evidence rather than replaying it:

- `sim-definition`: `level_definition::tests::bee_authored_health_admission_preserves_source_dword_range`.
- `sim-core`: `native_world::creatures::bee::tests::authored_day_bees_contact_player_and_public_fireball_drives_absence`.
- Dragon prefix `native_world::level2_class5::dragon::lightning_consumer_tests::`:
  `dragon_constructor_respects_source_free_stack_and_partial_chain`,
  `selected_dragon_tracks_current_projectile_and_clears_hidden_threat`,
  `dragon_separation_uses_source_ids_and_each_callback_axis_rules`, and
  `dragon_boss_level25_natural_summon_and_expiry`.
- Backend prefix `campaign::backend::tests::`:
  `campaign_backend_carried_fireball_hits_one_real_level_three_dragon_row_through_public_command`
  and `campaign_backend_level_three_public_guardian_exchange_reaches_reveal_objective`.

Bee uses the qualified Level2 package (`MC_CAMPAIGN_PACKAGE` or its existing
default); backend uses existing real Day3 descriptors/seed65. Set
`MC_LEVEL25_PACKAGE` explicitly to the existing whole
`generated/mc-bhq10-owner-v3-level25-20261029` for the boss check: the all-spells
ordinary-package root contains only Level18. Boss group29 is a declared prior
progression control, not a claim of the preceding BUILD mission chain. Actual
boss RNG/selection,17→16 and natural targetless expiry now execute. The actual
Level5 class11/15 consumer, final source checks, owned commit/push and matching
Original delivery now pass as recorded above. Do not repeat these delivered
checks or unrelated long creature/effects checks.

**Resolved build resource stop:** concurrent local scheduler tests and a separate
Modern-performance baseline Cargo build reduced root free space below5GB
(observed low0.56GB; later4.31GB), below both the floor and measured core5.45GB /
native5.75GB stage budgets. No new expensive checks were started by this team.
The external performance work is now committed through12318238. Its changes
are inherited source, not this team's edits. No Cargo process was active at
resumption and all four preserved creature files still matched their hashes.
Root byte-preserved two closed Nov3 local incremental graphs on healthy rig:
20 files/1531926916bytes match SHA256 before and after relocation. Receipt:
`generated/dragon-bee-20261105-integration/closed-incremental-archive/receipt.json`.
This recovered headroom to6.21GB without touching source/data, current local
or RCH warm caches, installed player or immediate previous native. Rig is
healthy/idle; root started one serialized integrated compile. Keep the5GB floor
and measured core5.45GB/native5.75GB stage budgets. No repeated baseline tests
or destructive cleanup were used. The latest qualified installation remains
793b7ef6/ecbf0969 until a new batch actually qualifies and is installed.

Additional closed Nov3 compiler graphs were byte-preserved on healthy rig after
the candidate9 return reduced free space below the stage budget. The extra
single graph and three delivery-headroom graphs retain exact SHA256 before and
after relocation; receipts are `closed-incremental-extra-archive/receipt.json`
and `closed-incremental-delivery-archive/receipt.json` in this batch. Newer local
and RCH warm caches, all source/data and current/previous natives remain intact.
Completed test executables and returned auxiliary files are also retained on
rig with per-candidate hashes; the earlier bad exFAT copy is never used.

## Delivered request: remaining spell E2E paths

Oliver explicitly requested completing all remaining spells on 2026-11-04.
Execute the existing beads and preserve their original acceptance/dependencies.
Each entry includes source tracing, ordinary acquisition/public all-tier casts,
actual consequences/consumers, natural cleanup and presentation, then focused
verification, coherent commit/push and matching installed native. Existing code
and traces are inputs; an unchecked entry is not an absent implementation.

- [x] Heal5 — `mc-k5k.1`
- [x] Rebound8 — `mc-k5k.1`
- [x] Teleport10 — `mc-k5k.1`
- [x] Meteor9 — `mc-bhq.4`
- [x] Crater16 — `mc-k5k.3`
- [x] Earthquake17 — `mc-k5k.3`
- [x] Volcano18 — `mc-k5k.3`
- [x] Gravity Well20 — `mc-k5k.4`
- [x] Whirlwind21 — `mc-k5k.4`
- [x] Fool's Mana22 — `mc-k5k.5`
- [x] Magic Mine23 — `mc-k5k.5`
- [x] Cave In25 — `mc-k5k.6`

The14 delivered spell-specific checkpoints (Fireball, Possession, Castle,
Speed, Morph, Shield, Invisibility, Beyond Sight, Lightning, Steal Mana,
Duel, Tremor, Army and Alliance) remain the baseline; repair them only when
an actual shared-consumer finding requires it. Shared authored-terrain evidence
does not substitute for Crater/Earthquake/Volcano spell-casting acceptance.
Root serializes Cargo/RCH and delivery; independent owners retain their paths
through integration. Do not close broader families on this list alone.

## Previous installed checkpoint: all-spells batch

Implemented and verified source `7bf5439845bc11fb54f8b0fe42c21eb0c6899e8e`
is committed and pushed. Matching native
`845964e70e7a6cbbc9d31cdabe0704dbb16502926eff8a04dd5539dc9d650371`
is installed in the qualified Original client. The actual rig-built binary,
local pin, asset plugin, player plugin and loaded process mapping match.
All444 Rust and162 upstream input hashes match rig; all444 Rust inputs match
the installed receipt, frozen candidate15 and canonical/isolated source.
Native-only installation and `--check-installed` passed without a Unity build.
The prior native `ab1095b7514a` and its original install receipt are preserved.

One normal canonical launcher smoke reached the original map, loading artwork
and NightSpire terrain/HUD on its first attempt, with the matching mapped
native. Root viewed the map and scene, recorded the loading-open event, then
stopped both owned processes. Only one portal click; no retry or gameplay
input. The private save destination was separate but no save file was written:
this is launch/load evidence, not saving or old-save proof. Details:
`Builds/delivery-all-spells-20261104-batch1/runtime-proof.json` and
`generated/all-spells-20261104-integration/candidate-15/`.

The remaining twelve spell paths above now have connected positive checks.
The frozen source is `generated/all-spells-20261104-integration/candidate-15/`
(`inputs.json`:444 Rust inputs). Candidate14's same-binary acquired checks pass:
Heal/Rebound/Teleport62.786s, Meteor11.585s, Crater/Earthquake/Volcano31.976s,
Gravity/Whirlwind12.346s, Fool's Mana42.754s, Mine23.819s and Cave In32.450s.
These use actual owner packages, authored acquisition and public casts, with
declared objective/progression/resources/encounter-position controls. They
observe actual damage, terrain, ownership/target state, presentation and
natural cleanup as applicable; no forced RNG or shortened source lifetime.

Candidate15 differs from14 only in the field test's retired-Villager setup and
Gravity's missing successful AddEventToMap04 publication. All10 field checks,
10 Gravity controls, the Mine constructor/tier/ordinary-dispatch boundary3,
acquired Mine19.901s, six Mine field controls and BadStone source/semantic
binding1.025s pass on15. The boundary control supplies captured payloads
explicitly; it is not33 actual acquisitions. Actual capture, release,
attributed rival damage and natural expiry are covered by the acquired Mine
scenario. Candidate12 ordering/formula and candidate10 acquired Tremor/Steal
Mana regressions passed; their relevant production joins remain unchanged.

Implemented shared joins include registry-backed persistent spell admission,
current fixed-address parent/A4 Castle/aim/price reads, the complete Rebound
product allowlist, canonical reusable guided constructors, real Mine9/30 and
10/85 scheduling, all real6DCA0 captured products with source-null cases,
per-shot recoil/charge/fanout order, and source Mine hover/visibility. Gravity
now initializes its child differently from Wind as source677A0/678E0 require.
Fool's Mana has actual motion/attraction, Cave/water/ground tails, map identity,
merge/color and retaliation. Source my_sign32 is a sign mask; a new actual
neighbor-cell merge/reclaim check passes1.779s. Authored BadStone construction,
source action9, sealed semantic bindings and source Cave-only null2/8 admission
complete previously blocked ordinary acquisition routes.

Independent fresh bounded reviews found one merge-radius defect, now fixed
and independently rechecked, and no further concrete defect in their inspected
paths. They inspected the original producer/consumer source, implementation
and gate changes separately, recording hashes and omitted coverage. Earlier
failed checks exposed real missing joins and erroneous observations; no source
gate was weakened. Rival100-visit protection now expires naturally away from
its Castle before the controlled damage encounter. Typed Firefly/Villager and
Fireball observations read their actual carriers rather than retired packets.

UBS static analysis covers25 changed Rust files, with57 whole-file legacy
critical heuristics and no critical on added lines; the final field/Gravity
delta scan completed with zero critical findings. Formatting check remains
non-clean across the existing package (1832 diff headers); no formatter
writes or broad cleanup. Full raw
review/compiler outputs and completed executables are checksum-preserved on
rig with receipts; source, owner data, warm worker caches and installed player
remain intact. Quarantined bad mounted-copy evidence was not reused.

The strict remote workspace all-target check passes34.591s with all444 input
hashes unchanged; strict matching native build passes55.419s. The checkboxes
record these delivered spell-specific checkpoints, not closure of broader
family acceptance or open dependencies. Remaining shared-engine and campaign
beads stay explicit; no dependencies or acceptance were rewritten. The finite
support-family `mc-k5k.1` is closed after its original acceptance and installed
delivery; the other five families retain their open dependency/full-family
acceptance. This is
not human combat/FPS/hardware-audio, whole-mission or old-save qualification,
and does not complete the shared engine. No Unity/art change. Workers and
bounded reviewers finished; the old heartbeat remains deleted.

## Previous installed checkpoint

Current installed batch: prepared Volcano9/Earthquake15 terrain production,
runtime `9d660088d63a6100f04a0554ed7cc4a8377fc0de`, matching native
`ab1095b7514a4f925fddfef378468d263537342d6ac59c82ad8bb09061c65fd1`.
Source is committed/pushed and the native is installed in qualified Original.
Real `DisId=-1` phase1 records now call their typed constructors, repeated
ascending ApplyEvents scans and real descendants, preserving common phase
tails and immediate startup reclaim. Eruption18 is constructed before startup
hides/reclaims it without its gameplay callback. Later BUILD composition now
handles missing source material10–15/20–22, inclusive lowDiff<=8, protection
and four-corner closure in the canonical shared terrain consumer.

Actual unchanged whole Level13 (42 Volcano roots) and Level22 (three Quakes,
one Volcano) require source RNG, exact allocator/free-stack, all five produced
planes, map heads, every slot's activity and full live MCE3 packets, followed
by ordinary session publication and FlightWorld height/material queries:
PASS8.052s. The original new gate had incorrectly passed MCE3 to the MCE1
parser; the corrected strict MCE3/v1/176/1000/header/extent comparison keeps
all activity/live-packet expectations. Day3 ribbon1.012s, Cave24 ribbon2.676s,
Level9 lowering rings2.379s and source-order0.857s pass on that same compiled
candidate6. Fresh independent bounded production/gate review found no concrete
defect, verified443 input hashes and both19-member package manifests. UBS
completed both changed Rust files with no critical added-line finding; its
one whole-file critical is the unchanged non-secret Castle-holder equality.
Startup formatting passes; world_types has existing outside-hunk drift.

All443 Rust and162 upstream inputs match rig, and the installed receipt matches
the443 Rust inputs. The returned native exactly matches the actual rig-built
library; pin, asset plugin and player plugin all hash `ab1095b7514a`. One
canonical Original map-to-NightSpire load succeeded on its first attempt with
that mapped native; root viewed map and terrain/HUD, verified the loading-open
event/private save, then stopped owned player/display. No load failures in
this smoke; the earlier intermittent WAVE failure's cause remains unresolved.
No Unity/art changes in this batch, native Level13/22 demonstration, human
combat/FPS/hardware audio, whole-mission or old-save claim. Evidence:
`generated/volcano-quake-20261104-integration/candidate-6/` and
`Builds/delivery-volcano-quake-20261104-batch1/`. Full original `.8` acceptance
and dependencies remain OPEN; finite `.9` stays CLOSED. Next assess remaining
original terrain-family acceptance against actual source/evidence before
choosing another concrete unmet capability; do not replay delivered paths.

Completed test binaries/RCH returns are preserved on rig with SHA receipts
before and after checksum-verified relocation; active native stays on local
ext4. The bad mounted candidate4 copy remains excluded and retained as
evidence, with no claimed hardware/storage diagnosis. See the recovery record
below. The automatic heartbeat was deleted during the blocked interval; this
explicitly requested resumed batch did not recreate it. Other workers' Modern
changes remain outside this delivery.

Previous installed batch (`17cf7060`/`50d6e9af2de1`): actual Level9 group0 lowering rings57/113 now use
canonical Rust constructors with validated physical NewEvent identities16/69.
Source phase/RNG precede the first callback; physical69's first radius update
is preserved. Initial PlayerEvents Castle stamps run before numeric callbacks;
real root146 then creates BUILD child161, which runs in that same ascending
pass. Immutable authored stages/Life, the first Update global RNG advance,
child byte3B/flags and successful root state4 match source. This terrain arena
produces geometry; unrelated actor startup remains under its existing owner.

The initial Castle's real BUILD1/2/3 inputs are now exported in MCI3 v4,
with exact Rust/Python closure and typed-schema validation. Original BUILD0
is a zero-sized source no-op. Older v1/v2/v3 contracts remain unchanged;
old packages are preserved. Fresh whole Level9/2 packages and separate160-input
exporter/binary-marker checks are under `generated/ribbon-20261104-integration/group-zero-exporter/`
and `group-zero-owner-package-v4/`. No captured mutable Castle clocks or
post-feature planes seed production. The real Level9 check requires all five
constructed planes, actual publication, exact lowered-plane player clearance,
subsequent ordinary deformation and natural hide/following reclaim (2.281s).
It rejects captured-clock dependence and a stale higher collision plane.

Focused v4 missing-input/legacy controls, Day3 ribbon (0.979s), Cave24 ribbon
(2.734s), source-order (0.790s), real public Castle failure/retry/upgrades
(62.342s), acquired all-tier Tremor (22.936s) and workspace all-targets
(30.385s) pass. Fresh independent bounded source/gate review found no further
concrete defect in these joins. UBS completed Rust/Python scanning with four
unchanged critical false positives and no critical added-line finding; its
C++ analyzer failed on unsupported control flow, so that scan is partial.
C++ was compiled and its actual exported packages verified; no clean C++ UBS
claim. Broader formatting debt remains. Evidence: `generated/ribbon-20261104-integration/candidate-14/`.

Implemented and verified runtime `17cf7060` is committed and pushed; matching
native `50d6e9af2de1` is installed in the qualified Original client. All443Rust
and162upstream inputs match rig and installation. Root viewed the canonical
map/loading/NightSpire terrain/HUD with that mapped native and stopped owned
smoke processes. First load hit a music-WAVE hash failure although the actual
disk WAVE matched its unchanged descriptor; one same-process Retry loaded
successfully without source/data changes. That intermittent failure's cause
remains unresolved; this is not a clean first-attempt launch claim or an audio
fix. No Unity/art change, native Level9 play, human combat/FPS/hardware audio,
full-mission or old-save proof. Full `.8` acceptance/dependencies stay OPEN;
finite `.9` stays CLOSED. Fresh bounded original-family review at `c533e693`
found the next concrete missing producer: actual pre-feature `DisId=-1`
Volcano9/Earthquake15 are selected by source GenerateEvents phase1, but Rust
startup rejects them and retains captured geometry. MaroonMarsh owns the
startup-module join through existing typed constructors/callbacks, repeated
ascending ApplyEvents scans, real Eruption allocation with immediate startup
reclaim, following phases and ordinary geometry/collision publication. Fresh
whole source packages for Level13 (42 Volcano roots) and Level22 (three
Earthquakes and one Volcano) are qualified under
`generated/volcano-quake-20261104-integration/current-owner-package/`; all160
exporter compile inputs and its binary marker match current pinned source.
Full acceptance remains open; this extends shared generation, not mission
content. Root serializes checks and delivery. Verified completed native,
exporter and old report bytes are preserved on mounted disk to restore build
headroom; warm objects and installed player remain. The exporter executable
now lives at `/mnt/pinokio-shared/magiccarpet-build-proofs-20261104/volcano-quake-headroom/qualified-exporter-be08`;
its disposable build-directory copy was removed after verification. Existing
CMake target relinks it from warm objects if needed; no symlink is left at that
reusable build path. Current and previous native artifact paths remain stable.

Resolved during current batch: phase1 Volcano9/Earthquake15 and ApplyEvents are
implemented in `cave_features/startup.rs`. The real Level13 check exposed
missing canonical BUILD material10–15/20–22 handling; `world_types.rs` now
implements source mapping, lowDiff, protection and closure. Candidate4 passes
Level13 terrain/global RNG, exact free-stack, all five feature planes and map
heads. It then hits a test-format defect: post-feature packets are MCE3 but
`parse_entities` requires MCE1. The corrected gate validates MCE3/v1/176/1000
and retains activity/full live-packet equality. Candidate5's slice-method
compile error was corrected; candidate6 now passes both Level13/22 and all
later ordinary publication/collision gates. Earlier candidates remain failed
evidence, not qualified artifacts. The fresh final bounded review and delivery
proof are recorded above.

Operator authorized resuming integrity recovery and delivery. A second preservation
failure has concrete evidence: a mounted copy of candidate4's test binary
differs at one byte, offset82043581, source0x24 versus copy0x20. Source SHA
`06a2fe76a5376d7db1e4d091a365c0e7fb396b73bccd83c0e95065b75c143e60`
still matches its compiled receipt; the copy hashes
`c03f37474fc566744fa0f7f812aa66f83a5a2a4efdda540bb8905356011d8994`.
Direct aligned disk reads confirm the differing byte in each file; this was
not merely a buffered-read discrepancy. No hardware/storage cause is established.
The bad mounted copy is retained as evidence and excluded from delivery. The
good original and244 completed RCH return files are now preserved under
`/data/projects/magiccarpet-dispatcher-proofs-20261104/candidate4-recovery` on rig.
All245 SHA hashes matched before and after checksum-verified rsync relocation;
no source, data, warm worker cache or installed player was touched. This
preserving alternative recovered388592432 bytes after DCG had refused the
earlier deletion command. Receipts remain in candidate4. Root free space was
5.89GB before the resumed check, above the unchanged measured stage budgets.
Keep active artifacts on ext4 and use the verified rig archival path, not the
bad mounted copy. Qualified candidate6 core outputs and native auxiliary
outputs are similarly hash-preserved on rig under `candidate6-qualified` and
`candidate6-native-aux`; the new native pin remains local and regular. Current
installation is now9d660088/ab1095b7514a as recorded above. Keep `.8` open.

Previous installed checkpoint: supported outdoor startup (`8ec3db4b`, native
`3b841258c2d1`) runs canonical typed MapType producers and ordinary geometry
publication on Day3 with source lighting/no ceiling; Cave24/Cave4/Tremor and
source-order controls qualified that batch. Current and immediate previous
native artifacts remain byte-preserved on disk.

Previous installed checkpoint:
Current installed shared capability: authored Creature18
and Cymmerian25 lifecycles now feed the original Level24 absence objectives and
real45→46 ribbon geometry/collision, under existing `mc-bhq.8` acceptance.
The complete unfiltered current-exporter package retains task2/18/group2 and
task5/25/group7. Actual groups2/7 → natural creature/split death → both original
absence objectives → following reclaim pass (candidate5,30.899s); actual ribbon
construction reaches live collision (candidate7,2.713s). Only18/25's existing
complete-lifecycle eligibility was admitted after bounded source assessments
and real family qualification. Full creature and terrain beads stay open.

Creature18's actual wrapped XY tracking and current fixed-address parent reads
are corrected. All17 owner checks pass (426.259s), including actual Secret34
group23 → ordinary five-shot fan → player damage → mana/death → original DTO
and reclaim. Creature25 now preserves source zero-life large roots through
split, protected landing hit-discard, raw countdowns, vertical hit relaxation,
water visual/speed distinctions and exact global denominator updates. Its26
focused source checks pass. The actual Level10 buildings → group8 → row31 guard
→ own shot/prepared sound37 → three small bodies → natural landing → real Castle
contact → small death/no-resplit → hide/reclaim/frozen removal also passes
(candidate7,15.214s). Constructor10/57 was missing from ordinary authored groups;
the family now exposes the existing Fool's Mana constructor with58DA0 binding,
without holder/payment/parent/tier overrides. Sound37 freezes its real Flight
journal/native audio command before movement, beyond the diagnostic receipt.

The old32-visit guard deadline stopped at phase33 after scan20 acquisition;
source pursuit attacks at the later scan40. The check now derives two source
cadence windows (41 visits), with every existing expectation intact. Diagnostic
source/logs are preserved in candidate6; no forced phase/RNG/action or shortened
lifetime. Fresh independent production and gate reviews found no further
concrete defect in their bounded coverage. Source-order and workspace all-target
checks pass; both canonical hash-consistency checks pass. Candidate5→7 changes
only this test file; production is byte-identical. Evidence is under
`generated/ribbon-20261104-integration/candidate-7/` and referenced candidates.
The inherited audio-census gate fails MeteorLaunch identically on installed
baseline and candidate; it does not invalidate the actual guard audio receipt.
Package-wide formatting and whole-file UBS debt remain: no critical finding
falls on an added line. All original acceptance/dependencies remain open.

Runtime `5c648cb7` is verified, committed and pushed; matching native
`530814669e15` is installed in the qualified Original client. All443Rust and
162upstream inputs match rig and installation; root viewed the normal canonical
map/loading/NightSpire terrain/HUD with matching mapped native, then stopped
owned smoke processes. This is launch/load proof, not native Level10/24 combat,
FPS, hardware audio, whole missions or old saves. No Unity/art changes. The
separate committed shared observation-hash improvement is included in this
native refresh. Current and immediate previous native are preserved on disk.
Root CalmBeaver owns sole Cargo/push/install. The existing
healthy RCH daemon listens on `/run/user/1000/rch.sock`; set RCH_SOCKET_PATH per
command because the default cache socket is stale. No worker restart needed.
Previous Tremor/Creature17 checkpoint (`ddfb0188`/`6a9ce089c86a`):
The actual Creature17 prerequisite is corrected: all 12 owner checks pass on
candidate5, including real melee after natural initial protection, slowdown,
stance, follow/expiry and retained addresses. Its real Level9 absence sensor
passes with clone-only objective activation controls, and group3 now reaches
the existing model17 constructor. Fresh bounded source review supports these
joins; the full creature bead/dependency remains open.
The connected Tremor check now passes all three tiers (candidate7, 21.360s):
actual Level9 switch/vault demolition → authored165 pickup → public acquired
hand/carry → paid cast → real flyer/controller → terrain/attributed Goat damage
and XP15 → same-pass player clearance → natural hide/following reclaim.
The old retained package lacked BUILD23→24→28 successor inputs. Rebuilt the
existing exporter and generated complete Level9/2 packages under
`generated/tremor-20261104-integration/current-owner-package`; the old packages
are preserved. Package structure/BUILD closure checks pass. A separate check
matches all 160 exporter compile-input hashes and its binary provenance marker
to current source/pinned remc2; campaign verification's source flags alone do
not check those hashes. No runtime fallback or injected BUILD record.
The existing test-only damage observer now reads the actual typed Goat mailbox,
with raw fallback, matching its real writer; receipt predicates are unchanged.
Source-order, acquired Fireball and workspace all-target checks pass on the
byte-identical candidate5 production. Fresh bounded review supports these joins.
Runtime `ddfb0188` is verified, committed and pushed; matching native
`6a9ce089c86a` is installed in the qualified original client. All 443 Rust and
162 upstream inputs match rig and installation. Root viewed the normal original
map, loading artwork and NightSpire terrain/HUD with the matching mapped native,
then stopped its smoke processes. Spawn flash is present in the scene capture;
this is launch/load proof, not a native Tremor demonstration, human combat, FPS,
hardware audio, full missions or old-save proof. No Unity/art changes in this
batch. A separate Modern performance commit is independent of this installation.
Full terrain and creature acceptance/dependencies remain open. Legacy package
formatting and whole-file scanner debt remain; no changed-hunk critical defect
was found. Evidence: `generated/tremor-20261104-integration/candidate-7/`.

Previous installed checkpoint: ordinary checkpoint smoke lifecycle
`ee6fd24c`, native `69410e96a7fe`.

Delivered on 2026-11-04: ordinary model59/action64 now uses the shared emitter
callback and canonical Smoke13 constructor. Pure adoption retains the existing
typed child without another allocation or RNG draw. Natural parent expiry now
hides the current occupant, preserves its final packet and common phase, and
reclaims its address at the following owning boundary. Children retain their
own NewEvent identity and reach original sprite selectors. The source 32-free
constructor gate and success-only parent RNG tail remain intact.

The actual descriptor-backed row342 → group23 → row343 path passes through its
unshortened 800..899 lifetime, original DTOs, child hide/reclaim, parent hidden
packet and following scheduled reclaim (55.814s). Canonical RNG/admission,
source-order and actual Ambient65 lifecycle checks also pass on byte-identical
candidate2 production. Candidate4 differs only in the existing gameplay check:
it now uses the production CampaignSession descriptor, with identical authored,
entity and terrain inputs. The previous export-only reset lacked semantic
bindings and failed identically on the installed baseline. No runtime fallback,
filtered group, shortened timer or weakened expectation was introduced.

Also restored the existing authored model4 FOV-width copy to match remc2; no
shipped MC2 producer for that dormant route was established or invented.
Fresh bounded source review found no additional concrete defect in the changed
checkpoint path. Source is verified, committed and pushed; the matching native
is installed. All 443 Rust and 162 upstream inputs match rig and the original
client. Root viewed normal map/loading/NightSpire terrain/HUD with the matching
mapped library and stopped its smoke processes. No Unity/art changes. Evidence:
`research/private/generated/checkpoint-smoke-20261104-integration/candidate-4/`.

Finite `mc-bhq.9` acceptance is complete and the bead is closed. A fresh
independent assessment found no concrete unmet requirement across its typed
content, owned callbacks, real damage/presentation consumers and source
lifetimes. Root checked its source/evidence against the original acceptance;
prior acquired Fireball/Fire Spheres, Volcano, Possession, Cave and Ambient
checks remain relevant. This does not close the full shared engine or the
external spell/creature families. The latest checkpoint checks free-slot
eligibility, without asserting later NewEvent reuse of that exact parent
address. Native visuals for every effect, human combat, FPS, hardware audio,
full missions and old saves remain unverified. Format/scanner legacy debt stays.

Previous Ambient/theft checkpoint `bae0a599`/`a911fd3b0eee` stays delivered:
unfiltered Level6 groups, six authored model26 roots, Ambient60 children and
natural lifetimes; current-A4 theft, detached return and remembered-hand pickup;
all 19 existing model26 owner checks pass. `mc-1pb.13` remains open. Evidence is
under `generated/ambient-smoke-20261104-integration/candidate-5/`. Prior
effect/contact `b395b5e4`/`e739c2ff0f23` also remains delivered. Do not restart
these checkpoints.

Previous ordinary Cave smoke checkpoint: `3555793f`, native `fa95bcc1ff52`.

Ordinary Cave smoke checkpoint delivered on 2026-11-04: implemented the missing
`sub_58630` ambient producer after `sub_159E0` and before `sub_60F00` and numeric
dispatch, with the source player/turn gates, wrapped scan and RNG order. Its
Ground86 callback now uses the canonical FadingSmoke87 constructor and preserves the
current parent position, sound, decrement and animation after allocation. Cave
map surgery preserves retired packets' own link words.

The actual unmodified Cave4 source-pass check covers ordinary birth, declared
turn/RNG prerequisites, normal and captured/exhausted allocation, same-pass tail,
original sprite/audio descriptors and natural hide/reclaim (7.84s). Five affected
constructor/RNG/Cave/order checks, workspace all-target check and native build
pass. All 443 Rust and 162 upstream inputs match rig and the original client.
Runtime `3555793f` is committed/pushed; native `fa95bcc1ff52` is installed. Root
viewed the normal map/loading/NightSpire scene/HUD with the matching mapped
library, then stopped its smoke processes. No Unity/art changes. Evidence:
`research/private/generated/ground-smoke-20261104-integration/candidate-1/`.

Full `mc-bhq.9` and shared-engine acceptance remain open. Exhaustion, snapshot,
candidate isolation and observer positions are explicit check controls, not a
natural owner-package exhaustion claim. Native Cave visuals, human gameplay,
FPS, hardware audio, whole missions and old-save migration remain unverified.
Package formatting and whole-file scanner debt remain; no critical changed-hunk
defect was found. Continue remaining real producers/consumers; do not restart
this delivered path.

The requested Possession → Fireball → Morph sequence is delivered. Remaining
shared-engine acceptance stays in the existing open beads; do not restart these
completed capability batches.

Oliver resumed the core team on 2026-11-03: two Terra 6.1/xhigh workers and
one Sol 6.1/xhigh integrator, with root coordination and independent checks.
The requested mana economy → rival combat → Lightning → Steal Mana → Duel
sequence is delivered in the installed checkpoints below. Do not restart those
completed paths. The full shared-engine acceptance remains in the open beads.

Previous mana contention/collection checkpoint: runtime `a9d4fb80`, native
`115137a73a34`.
The connected mana boundary is delivered: actual earned death sphere → rival
selection/contest → public reclaim → defender collection → exact Castle deposit
and another paid cast. Actual authored Scavenger selection → approach → claim →
lift → consumption → reclaim also passes. The Scavenger credits its own mana;
it does not deposit to a Castle. Fixed its record offsets, horizontal distance,
source movement/transition order, collision offset and frozen-list/current-row
selection. Nyphur uses the same actual mana head and current ownership.
Existing invalid control setups were corrected without relaxing their source
expectations. The authored target check retains its identity, death, retirement,
HUD and neutral handoff assertions, with explicit travel prerequisites.

The owner-data gameplay checks, source-order controls and workspace all-target
check pass; final test-only geometry does not change verified production code.
All443 Rust and162 upstream inputs match rig and the installed original client.
Root viewed the normal launcher → original map/loading → NightSpire scene/HUD
with the matching mapped native and stopped the owned smoke processes. Human
combat, FPS, hardware audio, complete missions and old-save migration remain
unverified. Wider `mc-bhq.10` and shared-engine acceptance stay open. Evidence:
`research/private/generated/mana-collection-20261104-integration/`.

Shared effects/lifetime checkpoint delivered on 2026-11-04:

- Implemented the source two-by-two wrapped 3D GroundFire proximity search and
  post-allocation velocity tail from the current BurningDebris occupant. Hidden
  fire remains searchable until its source reclaim.
- Completed final packets, phase and map-link retention across shared
  effects, Fire Spheres and Castle/Possession owning sweeps. Real lists unlink;
  the retiring packet retains its own link words with map bit and class cleared.
- Verified actual acquired all-tier Fireball impact → damaging descendants →
  target life/presentation → hidden packet → next-pass reclaim/reuse. The real
  authored Volcano → Eruption → unforced debris → GroundFire → DarkSmoke path
  passes, including source cancellation when its floor changes. Positive cases
  declare objective, stable-position and allocation prerequisites explicitly.
  A proposed Fire Spheres fallback test was removed because it bypassed the
  source's 26-free-slot admission; that broader alias reachability is unproven.
- Committed/pushed runtime `e72f6c58`; installed native `9b9911936657` in the
  qualified original client. Nine relevant checks, workspace all-target check,
  native build and all 443 Rust plus 162 upstream input comparisons pass. Root
  viewed the normal map/loading/NightSpire scene/HUD with the matching mapped
  library, then stopped its owned smoke processes. No Unity/art change.

The existing `mc-bhq.9` family acceptance stays open. This checkpoint does not
claim every remaining class10 producer, rare alias, whole mission, human combat,
FPS, hardware audio or old-save migration. Continue its remaining real shared
producer/consumer paths before broader closure; do not restart the delivered
checkpoints. Evidence: `research/private/generated/shared-effects-20261104-integration/candidate-5/`.

CalmBeaver owns the sole build/push/install slot; MaroonMarsh implements shared
joins with the feature owners. The Terra owner hit model capacity; work was
preserved and root/Sol completed the batch. Discover available agents on resume;
do not retry unavailable models in a loop.
Use exclusive file windows. Original-client delivery uses the verified isolated
`magiccarpet-original-client-70e76b9e-20261103` input snapshot while unrelated
Modern renderer work is present in the canonical Unity source.
The canonical launcher now selects the qualified original client through
`Builds/Original` (launcher commit `841c4709`); explicit Modern selection remains
separate. Root verified this normal route with the matching installed native.
Source/native qualification gates remain intact.
Keep the bead's wider acceptance and dependencies open. Existing delivered
Castle spell tiers remain the baseline. Disk headroom is restored before heavy
stages; preserve warm caches, current player and immediate previous native.

Duel checkpoint delivered on 2026-11-03: runtime `cd65b670`, native
`955cf9b7df8e`. Actual secret34 group23 pickup → inventory carry → public
all-tier cast reaches a real rival, tether movement and tier drains, source
payment/XP/presentation, holder/death/range release and natural cleanup/reset.
Corrected the zero source projectile axis, per-visit current holder/target reads,
normal/dying human consequence publication and current dying-rival consumers.
Local and rival flight now read the canonical Duel rules. Ordinary Nyphur1
preserves its source no-op. Triggers/resources/positions and lethal pulses are
explicit controls; the acquired cast/tether itself is real.

The acquired check, affected tether/source controls, actual Steal regression,
workspace all-target check and native build pass. All443 Rust and162 upstream
inputs independently match rig and the installed receipt. Root viewed original
map/loading/NightSpire scene/HUD with the matching mapped library and stopped
smoke processes. No Unity/art edits. Human gameplay/audio/FPS/full missions and
old-save migration remain unverified. Alive unknown nonplayer tier2 A4 aliases
remain explicitly unsupported/unverified; no speculative model was invented.
The full `mc-k5k.2` and its broader dependencies stay open. Evidence:
`research/private/generated/duel-20261103-integration/candidate-1/`.

Steal Mana checkpoint delivered on 2026-11-03: runtime `73bf009f`, native
`c1a7b623aca7`. Actual acquired/public all-tier casts reach a genuine rival,
source payment/XP, real two-Castle tier2 debit/spheres, presentation and natural
flyer/field cleanup. Fixed signed32 percentage order, pre-allocation XP, sphere
launch velocity, and the connected shared Cave sphere movement/attraction path.
Sphere roll/speed now reach canonical constructors, observations and hashes.
Actual Cave dispatcher controls pass alongside the real acquired path. Test
positions/resources/carry destination and collision planes are explicit controls.

The workspace all-target check passes after two CAPI test-literal updates;
production core sources match the tested candidate. All443 native Rust inputs
and162 upstream inputs independently match rig and the committed artifact.
The original client loaded map, loading art and NightSpire scene/HUD with the
matching mapped native; smoke processes stopped. No Unity/art edits. Human
combat/audio/FPS/full missions remain unverified. Two sphere checks still fail
identically on the pre-Cave binary; fixture versus production cause is unproven,
assertions unchanged and broader acceptance remains open. Existing formatting
and whole-file UBS debt are not clean-scan claims. Evidence:
`research/private/generated/steal-mana-20261103-integration/candidate-5/`.
The full `mc-k5k.2` and dependencies stay open; follow the current checkpoint.

Lightning checkpoint delivered on 2026-11-03: runtime `0369d2f7`, native
`f89eef7e4f49`. Actual acquired/public casts pass all three tiers through
caster-attributed damage, payment, experience, original presentation and natural
cleanup/reset. Fixed storm-shot accuracy/experience and the canonical source
collision/status-record axis order. Unmodified Level16 now opens; its real
Creature24 producer/death/absence objective/reclaim and authored Dolmen
contact-to-recharge path pass. Authored Dolmen and terrain-marker constructors
reach their actual shared consumers without filtering owner data.

Focused checks and the workspace all-target check ran on rig through RCH after
local Rust1.98 compiler crashes; profiles and caches were preserved. Native
delivery used an isolated checkout of the committed source because another task
is editing C API audio and Unity files. All 443 Rust inputs and the complete
required upstream source closure match the tested/installed candidate. The
original player loaded map, loading art and NightSpire with the matching mapped
library; its smoke processes are stopped. Evidence is in
`research/private/generated/lightning-20261103-integration/candidate-6/`.
`mc-k5k.2` and `mc-1pb.11` remain open for their full acceptance. The bare Fireball
reclaim-helper check still fails identically on the pre-batch rival binary;
shared-effects acceptance stays open. Legacy formatting/scanner backlog and
uninspected rare aliases remain explicit. This is not human combat, hardware
audio, FPS or complete-mission proof. Steal Mana is the next connected batch.

Oliver resumed implementation on 2026-11-01 with two Terra 6.1/xhigh workers
and one Sol 6.1/xhigh integrator. Finish Castle/mana and Alliance end to end,
then Army and status spells. Commit, push and install working batches. The full
shared single-player engine remains the goal; trace remc2 into its real Rust
consumers and keep unfinished acceptance open.

Castle spell/mana checkpoint delivered on 2026-11-02: runtime `f10d20c6`,
installed native `3bfe49a21bb4`. All three spell tiers, source failure/cleanup
controls, earned mana through deposit and paid spending, and the death-marker
merge/census path pass focused owner-data checks. The matching player loaded
the campaign map and NightSpire with the original HUD. Human Castle play,
hardware audio and performance still need Oliver's playtest.

`mc-bhq.10` remains open for its broader settlement/economy acceptance;
`mc-k5k.9` retains that blocking dependency. Their open status does not request
reimplementation of the delivered Castle paths.

Alliance checkpoint delivered on 2026-11-02: runtime `4acdee29`, installed
native `a620baf8396d`. The actual acquired three-tier cast/takeover path,
representative allied attack through real target damage, and natural expiry
pass the existing owner-data scenario. Focused controls cover special creature
consumers, current parent records, allocation failures/reuse, shared A4 input
and Castle relations, arrow collision, restoration, and replay/carry. The current
occupant determines scheduler dispatch, including a reused player address.
The matching installed player loaded the campaign map, original loading screen,
and NightSpire terrain/HUD; its native library and all 437 Rust inputs match the
tested candidate. No Unity or original-art changes were required.

`mc-k5k.5` stays open for its combined families22/23/24 acceptance. Its bounded
checks do not establish every eligible creature's complete mission playability.

Army checkpoint delivered on 2026-11-03: runtime `c83fe2f4`, installed native
`85fc99aca629`. The acquired spell's nine-case owner-data route passes: source
payment/cooldown/recast, 9/24 flight, 10/72 rings, all three tiers and the four
Bee/Firefly/Cymmerian/Wyvern products, actual attributed outgoing damage, and
representative natural expiry/death/split/landing/reclaim. The original eight
cases remain; real Cymmerian and Wyvern rival-damage checks were added.
Fixed current parent/A4/target reads, Firefly current-position/distance reads,
allocation-failure charging and the queued-profile zero-counter update order.
The installed player loaded the original campaign map, loading artwork and
NightSpire terrain/HUD; native copies, the loaded library and all 437 Rust inputs
match the tested candidate. No Unity or original-art changes were required.

The exact Army native and install receipt are preserved as the status batch's
rollback; the earlier Alliance rollback remains available. `mc-k5k.4` stays open for combined
families19/20/21 acceptance. Progression/resources and some positions are explicit
test controls, not earned campaign progression or a human playthrough. Hardware
audio/performance remain unverified; the package-wide format check still fails
on broad formatting drift.

Status checkpoint delivered on 2026-11-03: runtime `11b49428`, installed native
`d87e112bc161`. Acquired/public casts pass all supported Speed, Shield, Invisible
and Beyond Sight tiers through movement, genuine Bee damage, actual Nyphur
selection and observation, with natural expiry and the real Bee death/drop/reuse
route. Source fixed-record/A4 publication now reaches current typed and retained
actors; Shield checks current Castle funding, queued free profiles survive cleanup,
and Speed preserves post-trail-allocation alias/payment/expiry order. Typed i32
Speed multipliers reach actual public movement, reset, carry and same-revision
replay without changing MC2 defaults. The source Invisible holder/A4 anomaly and
Bee exception remain explicit; latched attacks are not universally erased.

Final immutable candidate9 reuses the byte-identical core8 binary. All 437 source
inputs match its receipt and the installed native. Focused controls, acquired
consumers, affected C ABI acquisition/damage/labels, workspace all-target check
and native build pass; independent checks cover the new lifetime boundaries and
exact C ABI Shield damage. C ABI vault rewards use declared controlled BUILD21/54
damage; Shield's accepted mailbox is not claimed as natural combat. The installed
player loaded the original map, loading art and NightSpire terrain/HUD with the
matching mapped library. No Unity or original-art changes were required.

This status installation was the pre-Possession baseline. New definition keys reject
older-definition saves/replays; no migration or relaxed identity gate was added.
The exact Army native `85fc99aca629` and its receipt remain under
`research/private/generated/status-20261103-integration/candidate-6/rollback-army/`.
`mc-k5k.12` and parent `mc-k5k.7` retain their full acceptance and open dependencies.
The shared-effects dependency `mc-bhq.9` remains open, including an adjacent bare
reclaim-helper test failure reproduced on the pre-batch Army binary; its fixture
versus production-lifecycle cause remains unproven. Formatting is still non-clean;
UBS changed-hunk review found no critical candidate defect, not a clean whole-file
scan. Human flight/combat, hardware audio/performance and full missions remain
unverified. Evidence is in `research/private/generated/status-20261103-integration/`
and `research/private/remc2-unity-native/Builds/delivery-status-20261103-batch1/`.
Oliver resumed on 2026-11-03 with three sequential shared-engine batches:
Possession (`mc-k5k.11`), then Fireball and its required shared impact/damage
consumers (`mc-k5k.10`, `mc-bhq.9`), then Morph (`mc-k5k.8`). Finish the real
acquired-cast path, consequences and cleanup, verify, commit/push and install
each coherent batch before advancing. Keep the existing acceptance and unresolved
dependencies open. The installed status checkpoint is the starting baseline.

Possession checkpoint delivered on 2026-11-03: runtime `fae4f54f`, installed
native `ea88eadd5c89`. Actual acquired/public casts cover all three tiers,
House and authored mana ownership, stronger ownership pulses, attraction,
partial allocation, source timing/queued profiles, current parent state and
natural cleanup. Source-scheduled checks also cover model22, death-marker
relation transfer and Fool's Mana retaliation. Typed launch speed reaches
actual movement without changing the MC2 default. Root independently reran the
scheduled death-marker/model22 consumers and self-verified the public C ABI
House path. The workspace all-target check and matching native build pass.

All 439 Rust inputs, native copies and the live player's mapped library match
candidate8. The installed player loaded the original map, loading artwork and
NightSpire terrain/HUD; no Unity or art changes were required. One initial
smoke inherited Wayland instead of its private display, received no input and
was stopped; the corrected private-display load passed and its processes were
stopped. This proves launch/load, not human Possession combat, hardware audio,
performance or full missions. Use this Possession installation as the current
baseline; evidence is in `research/private/generated/possession-20261103-integration/`
and `research/private/remc2-unity-native/Builds/delivery-possession-20261103-batch1/`.

`mc-k5k.11` retains its open `mc-bhq.10` dependency. The exact adjacent house
census check still fails at 13148 versus 12648, reproduced unchanged on the
pre-batch status binary; its broader cause is unproven and no assertion was
weakened. The package format check still has legacy drift; changed-hunk UBS
review found no critical candidate defect, not a clean whole-file scan.
The two Terra workers hit model capacity and remained stopped; root and the Sol
integrator completed the sequential batches without replacement retries.

Fireball checkpoint delivered on 2026-11-03: runtime `9ccd9815`, installed
native `8afc2bf1ac9a`. Actual acquired casts cover all three tiers and a typed
speed override through flight, damage, attributed death, original impact and
FireSpheres descendants, natural cleanup and holder reset. Fixed current
parent/holder reads, source launch/terrain/water order, Magic Mine selection and
capture, canonical descendant allocation and local particle binding. Root
independently reran the acquired scenario and self-verified the public C ABI
flight/impact/presentation/reclaim path. Workspace all-target check and native
build pass; all 441 inputs and installed native copies match candidate6.

The matching player loaded the original map, loading art and NightSpire
terrain/HUD; its mapped native hash matches. No Unity or art changes. Evidence:
`research/private/generated/fireball-20261103-integration/` and
`research/private/remc2-unity-native/Builds/delivery-fireball-20261103-batch1/`.
Use this installation as the baseline for Morph next. The exact Castle reward
deposit and Nyphur mode7 pursuit checks still fail, both independently reproduced
on the pre-batch Possession binary; their broader causes remain unresolved.
`mc-k5k.10` and shared-effects `mc-bhq.9` remain open with their full acceptance.
Declared resources/positions are check controls, not earned campaign progress.
Human combat, hardware audio/performance and full missions remain unverified;
legacy formatting and whole-file UBS backlog remain, with no critical defect
identified in the changed hunks.

Morph checkpoint delivered on 2026-11-03: runtime `fbaf38b9`, installed native
`dd16bf15937e`. Acquired/carry casts cover Day Bee, Night/Cave Firefly, Cymmerian
and Wyvern, plus a typed Cave-tier0 Wyvern override. Actual control, current
parent following, original presentation, natural 201/301/455-visit expiry,
following-pass generation reclaim and reset pass. Fixed current A4 colour and
Castle reads, local/rival control isolation and flicker, award/charge ordering,
parent/holder replacement aliases and current word36 cleanup. Morph shares the
ordinary/Army Wyvern and Cymmerian constructors/counters; the typed Bee remains
its authoritative body rather than activating a duplicate raw shadow.

Root independently reran the six-case path (88.24s), replacement-address controls
and the constructor/counter/position-alias check. Relevant existing Army consumers,
native entity DTO/reset, scheduler, workspace all-target check and native build
pass. All 443 Rust inputs, native copies and the player's mapped library match
candidate7. The installed player loaded the original map, loading artwork and
NightSpire terrain/HUD; its test player/display were stopped. No Unity or art
changes were required. Evidence: `research/private/generated/morph-20261103-integration/`
and `research/private/remc2-unity-native/Builds/delivery-morph-20261103-batch1/`.

`mc-k5k.8` retains its full acceptance and open creature dependencies. Carry,
resources and positions are declared check controls, not earned campaign
progression. The native DTO check is not a Morph C ABI cast or human playthrough.
Human Morph combat, hardware audio/performance and full creature/mission coverage
remain unverified. Prior economy/rival failures above remain unresolved. Package
formatting still has legacy drift; scoped UBS review found no critical changed-hunk
defect, not a clean whole-file scan. Superseded proof executables were preserved
with hash-checked relocation receipts under `/tmp/magiccarpet-*`; source, warm
caches, installed player and immediate previous native were preserved. The Sol
integrator stood down at that checkpoint; the resumed work above supersedes it.

Mana economy checkpoint delivered on 2026-11-03: source `4eef9713`, native
`73d854280c95`. The real earned-mana loop already passed on the Morph baseline
(103.27s) and passes the integrated candidate (100.145s): public Castle,
Fireball kill, public Possession, scheduled balloon, exact deposit and paid cast.
The two reported failures were setup/oracle defects: the explicit claim adapter
omitted physical player ownership; the house census omitted a real neutral Archer
born with 500 mana during demolition. Corrected those narrow paths with exact
source-backed birth, generation and positive-census assertions; production
Possession, Castle dispatch and scheduler behavior were not rewritten.

Both original checks and the claim/reset consumer pass; root independently
reran the two corrected cases (3.632s/.457s). Native build passes, all 443 inputs
and native copies match, and root viewed the matching mapped player loading the
original map, loading art and NightSpire terrain/HUD. Its test processes were
stopped. No Unity/art change. Evidence: `generated/mana-economy-20261103-integration/`
(under `research/private/`) and `Builds/delivery-mana-economy-20261103-batch1/`
(under the Unity client). The earlier deposit/census negatives above are resolved
by this evidence; the rival pursuit failure remains the next investigation.
`mc-bhq.10` retains broader acceptance and stays open. Declared travel/progression
are check controls, not a human campaign playthrough. Hardware audio/performance
remain unverified; legacy package formatting and whole-file scanner backlog remain.

Rival combat checkpoint delivered on 2026-11-03: runtime `d5731432`, native
`ac3bf6d9fe17`. The actual authored Bee producer, ordinary rival selection,
Fireball cast, attributed damage and natural projectile/impact reclaim pass.
Current typed targets reach the frozen class-5 selectors without reactivating
their raw shadows; explicit physical word94 writes reach the current owner and
reject stale generations. AI Castle creation now uses canonical NewEvent for
raw and typed replacements, repairs map links immediately and refreshes the
same-pass decision tail. Normal rival Castle upgrade, allocation failure,
neutral ownership, numeric scheduling, target reuse and reset/replay controls pass.

Root ran the focused checks, workspace all-target check and native build against
all 443 matching inputs. An independent source review found no additional defect
in the inspected allocation/tail join. Root viewed the original map, loading art
and NightSpire terrain/HUD with the matching mapped native, then stopped its
private smoke processes. Source and remote push match; no Unity/art changes were
included. Evidence is `generated/rival-combat-20261103-integration/candidate-5/`
under `research/private/`, plus the Unity `Builds/delivery-rival-combat-20261103-batch1/`.
The exact previous native remains in the mana-economy checkpoint.

`mc-1pb.17` retains its full roster/repertoire/mission acceptance and stays open.
The Bee check controls its objective trigger, approach and Fireball-only inventory;
it does not prove a unique shot-to-impact correspondence among overlapping shots.
Mode7 controls its state each visit; its detached boundary checks now preserve
the source negative-burst refusal and explicitly ready the positive control.
These are not human combat or full-mission proof. Hardware audio/FPS remain
unverified. Rust1.98's large test binary compiled with `RUST_MIN_STACK=67108864`
after recorded compiler crashes; profiles, flags and warm caches were preserved.
Legacy formatting and whole-file UBS findings remain; no changed-hunk critical
was identified. Original-client launches currently use the verified isolated
`magiccarpet-original-client-70e76b9e-20261103/remc2-unity-native/run-campaign.sh`
while unrelated Modern renderer edits are present in canonical Unity sources.

Read this plan and your current bead before each assignment, follow-up, resumed
work session or heartbeat. Also reread after compaction and before declaring a
handoff complete. Continue useful work; do not produce a read-receipt report.

## Outcome and scope

Deliver faithful MC2 single-player behavior in the reusable Rust engine.
Shared systems own behavior; missions configure and exercise them. Keep
properties and asset bindings typed, and behavior in focused canonical modules.
Use original presentation. Oliver does the playthroughs.

First finish the preserved 16 shared-engine gaps: spells 7 and 13–24,
Meteor 9 identity; creature models 10,16,17,18,21,23,25–28; authored spawning;
objectives 1,2,4,6 and rival targets; missing scenery/projectile/effect callbacks;
final encounter; secret entry/return and carry; player-info/mark/cycle controls;
campaign-map hover/cursor/terminal effects; original settings dialogs.
Then continue the remaining existing shared single-player engine/system beads.
Existing beads retain their full acceptance. Multiplayer, an authoring platform
and a level-content expansion are separate. Do not restart planning.

## Execute the existing plan

1. Use the delivered Castle/mana, Alliance, Army, status, Possession, Fireball and Morph checkpoints above.
   Other preserved work remains in scope for later batches. Check live source
   and ownership first; do not restart a component already delivered.
2. Each feature owner follows the entire remc2-to-Rust chain: properties/assets,
   ordinary producer, schedule/dispatch, behavior and interactions, same-pass
   tail, following-pass cleanup, and actual gameplay/presentation consumers.
   Use the trace already in the bead and source. Implement missing links rather
   than writing another trace document.
3. A worker remains responsible through integration. A leaf handoff is a
   checkpoint, not completion or permission to abandon the feature. Integrators
   implement shared joins with the worker and return concrete failures to that
   owner. A missing dependency gets an implementation owner, not fake success.
4. Independent workers may finish other complete in-scope systems in parallel.
   Reserve disjoint files. One writer at a time owns each shared root; obtain an
   explicit edit window. Shared roots compose behavior rather than absorbing it.
5. At a coherent boundary, use the smallest checks that exercise the actual
   producer-to-consumer path. Reuse warm compilation; no workspace suite per
   tweak, bots, automated playthroughs, duplicate tests or weakened assertions.
   Passing a constructor test does not prove a creature or spell works.
6. Commit and push the coherent, verified implementation and install matching
   Rust/Unity artifacts. Run one delivery launch/load smoke. Deliver useful
   batches without waiting for all 16, while keeping unfinished beads open.
   Report separately what is source-ready, verified, committed and installed.
7. Continue the remaining systems, then use fresh bounded review of completed
   paths. Fix concrete findings. Claim completion only against original bead
   acceptance, with honest coverage limits and a matching installed build.

## Coordination and prompts

The current team uses native collaboration; the current capability section
above records live owners and model-capacity stops. Oliver's chosen roles remain
Terra6.1/xhigh workers and Sol6.1/xhigh integration when available. Discover actual
status and private Mail identity before resuming anyone; do not revive retired
names or retry confirmed capacity in a loop. Root owns acceptance, tracker,
serialized Cargo/RCH, shared joins when the integrator is unavailable, commit/push
and native installation. Earlier four-team NTM sessions are retired; use NTM only
if live state warrants it. Preserve dirty source, staged work, warm caches and
current installations.

Integrator ownership includes implementation of shared joins. Never hold a
shared-root reservation while waiting on another family: negotiate brief exact
file windows, integrate the coherent change and release. Workers remain with
their feature through the real consumer; do not accumulate unintegrated leaves.

For the current incremental build/install batches, check actual free bytes and
pending output size before expensive stages. The earlier blanket 20 GB hold was
replaced by the bounded headroom decision in mc-zdo.1: stop the next stage below
5 GB free or when its measured footprint exceeds available headroom. Preserve
the current player, immediate rollback and warm caches.

Every dispatch/follow-up must say: reread this plan and the bead; the positive
runtime result; the exact remaining link and owned surface; the integration
owner; the focused evidence and handoff boundary. Reference existing details
instead of repeating manuals or stale status. Verify native-agent receipt through
acknowledgment or actual work. If using live NTM, use literal canonical `%N` pane
IDs. Never print tokens.

The coordinator rereads this plan every tick, checks live work and messages,
and acts on specific blockers or idle actionable owners. No blind nudges or
status-only loops. Stay quiet on unchanged state. Preserve work and report once
if quota or access prevents progress. Stop the heartbeat only when the scoped
acceptance, fresh review and matching delivery are complete.
