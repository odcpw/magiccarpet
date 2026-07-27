using System;
using System.Collections.Generic;

namespace MagicCarpet.Native
{
    /// <summary>
    /// Input-only Level 1 route driver. It observes the same native snapshots
    /// that Unity presents and returns an ordinary player command; it has no
    /// mutation, teleport, damage, or mission-control path.
    /// </summary>
    internal sealed class Level1Autopilot
    {
        /// <summary>
        /// Player-facing observation assembled at the Unity boundary. Enemy
        /// health, actions, targets, RNG, damage mailboxes, and cooldowns are
        /// deliberately absent, so the controller cannot use hidden state.
        /// </summary>
        internal readonly struct Observation
        {
            internal readonly PlayerObservation player;
            internal readonly MissionObservation mission;
            internal readonly ActorObservation[] actors;
            internal readonly GoatObservation[] goats;
            internal readonly SphereObservation[] spheres;
            internal readonly CastleObservation castle;

            internal Observation(
                PlayerObservation player,
                MissionObservation mission,
                ActorObservation[] actors,
                GoatObservation[] goats,
                SphereObservation[] spheres,
                CastleObservation castle)
            {
                this.player = player;
                this.mission = mission;
                this.actors = actors;
                this.goats = goats;
                this.spheres = spheres;
                this.castle = castle;
            }
        }

        internal readonly struct PlayerObservation
        {
            internal readonly ulong tick;
            internal readonly ushort x;
            internal readonly ushort y;
            internal readonly short z;
            internal readonly short yaw;
            internal readonly short effectivePitch;
            internal readonly short rollAccumulator;
            internal readonly short targetSpeed;

            internal PlayerObservation(NativeSim.SessionSnapshot source)
            {
                tick = source.tick;
                x = source.x;
                y = source.y;
                z = source.z;
                yaw = source.yaw;
                effectivePitch = source.effectivePitch;
                rollAccumulator = source.rollAccumulator;
                targetSpeed = source.targetSpeed;
            }
        }

        internal readonly struct MissionObservation
        {
            internal readonly byte currentStage;
            internal readonly byte playerDead;
            internal readonly byte exitSequenceActive;
            internal readonly byte exitSequenceComplete;
            internal readonly byte hasCastleSpell;
            internal readonly byte hasSpeedSpell;
            internal readonly byte castleExists;
            internal readonly int securedManaPercent;
            internal readonly int playerLife;

            internal MissionObservation(NativeSim.Level1MissionSnapshot source)
            {
                currentStage = source.currentStage;
                playerDead = source.playerDead;
                exitSequenceActive = source.exitSequenceActive;
                exitSequenceComplete = source.exitSequenceComplete;
                hasCastleSpell = source.hasCastleSpell;
                hasSpeedSpell = source.hasSpeedSpell;
                castleExists = source.castleExists;
                securedManaPercent = source.securedManaPercent;
                playerLife = source.playerLife;
            }
        }

        internal readonly struct ActorObservation
        {
            internal readonly ushort authoredIndex;
            internal readonly byte kind;
            internal readonly byte active;
            internal readonly byte collected;
            internal readonly ushort x;
            internal readonly ushort y;
            internal readonly short z;

            internal ActorObservation(NativeSim.Level1ActorSnapshot source)
            {
                authoredIndex = source.authoredIndex;
                kind = source.kind;
                active = source.active;
                collected = source.collected;
                x = source.x;
                y = source.y;
                z = source.z;
            }
        }

        internal readonly struct GoatObservation
        {
            internal readonly byte active;
            internal readonly ushort x;
            internal readonly ushort y;
            internal readonly short z;

            internal GoatObservation(NativeSim.GoatSnapshot source)
            {
                active = source.active;
                x = source.x;
                y = source.y;
                z = source.z;
            }
        }

        internal readonly struct SphereObservation
        {
            // Stable presentation identity used by Unity's visible-object
            // registry. This is not simulation ownership, mana, velocity,
            // carrier assignment, or any other hidden gameplay state.
            internal readonly uint sphereId;
            internal readonly byte active;
            internal readonly ushort x;
            internal readonly ushort y;
            internal readonly short z;
            internal readonly ushort renderParameterIndex;
            internal readonly ushort baseTmap;

            internal bool IsLocalOwned => baseTmap == 67;

            internal SphereObservation(NativeSim.ManaSphereSnapshot source)
            {
                sphereId = source.sphereId;
                active = source.active;
                x = source.x;
                y = source.y;
                z = source.z;
                renderParameterIndex = source.renderParameterIndex;
                baseTmap = source.baseTmap;
            }
        }

        internal readonly struct CastleObservation
        {
            internal readonly byte active;
            internal readonly ushort x;
            internal readonly ushort y;
            internal readonly short z;

            internal CastleObservation(NativeSim.Level1CastleSnapshot source)
            {
                active = source.active;
                x = source.x;
                y = source.y;
                z = source.z;
            }
        }

        internal readonly struct Command
        {
            internal readonly sbyte Roll;
            internal readonly sbyte Pitch;
            internal readonly sbyte Flags;
            internal readonly short RequestedSpell;
            internal readonly sbyte Opcode;
            internal readonly sbyte Argument1;
            internal readonly string Phase;

            internal Command(
                sbyte roll,
                sbyte pitch,
                int flags,
                short requestedSpell,
                string phase,
                sbyte opcode = 0,
                sbyte argument1 = 0)
            {
                Roll = roll;
                Pitch = pitch;
                Flags = unchecked((sbyte)flags);
                RequestedSpell = requestedSpell;
                Opcode = opcode;
                Argument1 = argument1;
                Phase = phase;
            }
        }

        private const byte ArcherKind = 1;
        private const byte FireflyKind = 2;
        private const byte CastleJarKind = 3;
        private const byte SpeedJarKind = 4;
        private const byte ExitSwitchKind = 6;
        private const byte MultipartHeadKind = 7;
        private const int PossessionFlag = 0x20;
        private const int SelectedSpellFlag = 0x40;
        private const short CastleSpellFamily = 2;
        private const short SpeedSpellFamily = 3;
        // Four Archer spheres plus five visible Goat spheres provide the
        // conservative Level 1 build route. Exact mana values are hidden
        // simulation state and are deliberately not available to the bot.
        private const int RequiredVisibleLooseSphereCount = 9;
        private const ushort Group33FireflyAuthoredIndex = 306;
        private const ushort FirstMultipartAuthoredIndex = 307;
        private const ushort FirstGroup34MultipartAuthoredIndex = 309;
        private const ushort LastGroup34MultipartAuthoredIndex = 311;

        private static readonly Target FirstWaypoint =
            new(29_440, 54_272, 5_000, "first-waypoint");
        private static readonly Target SecondWaypoint =
            new(49_664, 54_528, 5_000, "second-waypoint");
        private static readonly Target CastleBuildSite =
            new(49_920, 43_776, 1_200, "castle-build-site");
        // Authored row 305 is a non-rendered overlap switch at tile 217,213.
        // A player can learn/replay this route as a fixed world waypoint; the
        // controller never observes the switch phase, extent, or allocation.
        private static readonly Target Group33SwitchWaypoint =
            new(55_680, 54_656, 3_000, "trigger-group-33");

        private readonly bool includePostCompletion;
        private readonly Dictionary<ushort, ActorTrack> actorTracks = new();
        private ushort? lockedActorIndex;
        private byte lockedActorKind;
        private bool recoveringFromArchers;
        private bool recoveringFromFireflies;
        private bool speedSpellRequested;
        private bool hasLockedSpherePosition;
        private uint lockedSphereId;
        private bool backingAwayFromSphere;
        private bool group33SwitchReached;
        private ulong group33SwitchReachedTick;
        private bool group33FireflyObserved;
        private bool firstMultipartObserved;
        private bool group34MultipartObserved;
        private bool recoveringFromPostCompletion;
        private string priorPhase = string.Empty;

        internal string PriorPhase => priorPhase;
        internal int LastAimYawError { get; private set; }
        internal int LastAimPitchError { get; private set; }
        internal double LastAimDistance { get; private set; }
        internal ushort LastAimActor { get; private set; }

        internal Level1Autopilot(bool includePostCompletion = false)
        {
            this.includePostCompletion = includePostCompletion;
        }

        internal static Observation Observe(
            NativeSim.SessionSnapshot player,
            NativeSim.Level1MissionSnapshot mission,
            NativeSim.Level1ActorSnapshot[] actors,
            NativeSim.GoatSnapshot[] goats,
            NativeSim.ManaSphereSnapshot[] spheres,
            NativeSim.Level1CastleSnapshot castle)
        {
            return new Observation(
                new PlayerObservation(player),
                new MissionObservation(mission),
                Array.ConvertAll(actors, actor => new ActorObservation(actor)),
                Array.ConvertAll(goats, goat => new GoatObservation(goat)),
                Array.ConvertAll(spheres, sphere => new SphereObservation(sphere)),
                new CastleObservation(castle));
        }

        internal Command Next(Observation observation)
        {
            var player = observation.player;
            var mission = observation.mission;
            var actors = observation.actors;
            var goats = observation.goats;
            var spheres = observation.spheres;
            var castle = observation.castle;
            UpdateActorTracks(actors, player.tick);
            Command command;
            if (mission.playerDead != 0)
            {
                command = Brake(player, "player-dead");
            }
            else if (mission.exitSequenceActive != 0 ||
                     mission.exitSequenceComplete != 0)
            {
                command = Brake(player, "exit-sequence");
            }
            else if (mission.currentStage == 0)
            {
                command = Navigate(player, FirstWaypoint, 720, false);
            }
            else if (mission.currentStage == 1)
            {
                command = Navigate(player, SecondWaypoint, 720, false);
            }
            else if (mission.currentStage == 2)
            {
                command = StageTwo(player, mission, actors);
            }
            else if (mission.currentStage == 3)
            {
                command = StageThree(player, mission, actors, goats, spheres, castle);
            }
            else if (mission.currentStage == 4)
            {
                command = StageFour(player, mission, actors);
            }
            else if (includePostCompletion)
            {
                command = PostCompletion(player, mission, actors);
            }
            else
            {
                command = EnterExit(player, actors);
            }
            priorPhase = command.Phase;
            return command;
        }

        private Command StageTwo(
            PlayerObservation player,
            MissionObservation mission,
            ActorObservation[] actors)
        {
            var jar = FindActor(actors, CastleJarKind, activeOnly: true);
            if (mission.hasCastleSpell == 0)
            {
                return jar.HasValue
                    ? Navigate(player, ActorTarget(jar.Value, 80, "collect-castle"), 420, true)
                    : Brake(player, "wait-castle-jar");
            }
            if (mission.playerLife < 8_500 && !recoveringFromArchers)
            {
                recoveringFromArchers = true;
            }
            var nearestArcherDistance = NearestActorDistance(player, actors, ArcherKind);
            if (recoveringFromArchers &&
                mission.playerLife >= 9_950 &&
                nearestArcherDistance > 6_000)
            {
                recoveringFromArchers = false;
            }
            if (recoveringFromArchers)
            {
                return FleeVisibleActor(player, actors, ArcherKind);
            }
            var archer = LockedOrNearestActor(player, actors, ArcherKind);
            return archer.HasValue
                ? AttackActor(player, archer.Value, "fight-archers")
                : Brake(player, "wait-archer-deaths");
        }

        private Command StageThree(
            PlayerObservation player,
            MissionObservation mission,
            ActorObservation[] actors,
            GoatObservation[] goats,
            SphereObservation[] spheres,
            CastleObservation castle)
        {
            if (mission.castleExists == 0)
            {
                var visibleLooseSpheres = 0;
                foreach (var sphere in spheres)
                {
                    if (sphere.active != 0 && !sphere.IsLocalOwned)
                    {
                        visibleLooseSpheres++;
                    }
                }
                if (visibleLooseSpheres < RequiredVisibleLooseSphereCount)
                {
                    var goat = LockedOrNearestGoat(player, goats);
                    return goat.HasValue
                        ? AttackGoat(player, goat.Value)
                        : Brake(player, "wait-goat-mana");
                }

                var distance = HorizontalDistance(
                    player.x, player.y, CastleBuildSite.X, CastleBuildSite.Y);
                if (distance > 700 || Math.Abs(player.targetSpeed) > 32)
                {
                    return Navigate(player, CastleBuildSite, 560, true);
                }
                var aimed = Aim(player, CastleBuildSite, 0);
                return new Command(
                    aimed.Roll,
                    aimed.Pitch,
                    SelectedSpellFlag,
                    CastleSpellFamily,
                    "build-castle");
            }

            var sphereTarget = NearestVisibleUnownedSphere(player, spheres);
            if (sphereTarget.HasValue)
            {
                return PossessSphere(player, sphereTarget.Value);
            }
            if (mission.securedManaPercent < 15)
            {
                return castle.active != 0
                    ? Navigate(
                        player,
                        new Target(castle.x, castle.y, castle.z + 600, "await-carrier"),
                        900,
                        false)
                    : Brake(player, "await-carrier");
            }
            return Brake(player, "wait-firefly-release");
        }

        private Command StageFour(
            PlayerObservation player,
            MissionObservation mission,
            ActorObservation[] actors)
        {
            var speedJar = FindActor(actors, SpeedJarKind, activeOnly: true);
            if (speedJar.HasValue && speedJar.Value.collected == 0)
            {
                return Navigate(
                    player,
                    ActorTarget(speedJar.Value, 80, "collect-speed"),
                    420,
                    true);
            }
            if (mission.playerLife < 8_500 && !recoveringFromFireflies)
            {
                recoveringFromFireflies = true;
            }
            var nearestFireflyDistance =
                NearestActorDistance(player, actors, FireflyKind);
            if (recoveringFromFireflies &&
                mission.playerLife >= 9_950 &&
                nearestFireflyDistance > 6_000)
            {
                recoveringFromFireflies = false;
            }
            if (recoveringFromFireflies)
            {
                return FleeVisibleActor(player, actors, FireflyKind);
            }
            var firefly = LockedOrNearestActor(player, actors, FireflyKind);
            if (!firefly.HasValue)
            {
                return Brake(player, "wait-firefly-deaths");
            }
            var attack = AttackActor(player, firefly.Value, "fight-fireflies");
            if (mission.hasSpeedSpell != 0 && !speedSpellRequested)
            {
                speedSpellRequested = true;
                return new Command(
                    attack.Roll,
                    attack.Pitch,
                    unchecked((byte)attack.Flags) | SelectedSpellFlag,
                    SpeedSpellFamily,
                    "speed-combat");
            }
            return attack;
        }

        private Command PostCompletion(
            PlayerObservation player,
            MissionObservation mission,
            ActorObservation[] actors)
        {
            var group33Firefly = FindActorByAuthoredIndex(
                actors,
                Group33FireflyAuthoredIndex,
                activeOnly: true);
            var firstMultipart = FindActorByAuthoredIndex(
                actors,
                FirstMultipartAuthoredIndex,
                activeOnly: true);
            var group34Multipart = NearestAuthoredRangeActor(
                player,
                actors,
                FirstGroup34MultipartAuthoredIndex,
                LastGroup34MultipartAuthoredIndex,
                MultipartHeadKind);
            group33FireflyObserved |= group33Firefly.HasValue;
            firstMultipartObserved |= firstMultipart.HasValue;
            group34MultipartObserved |= group34Multipart.HasValue;

            var nearestHostileDistance = double.MaxValue;
            byte nearestHostileKind = 0;
            foreach (var candidate in new[]
                     {
                         group33Firefly,
                         firstMultipart,
                         group34Multipart,
                     })
            {
                if (!candidate.HasValue) continue;
                var distance = HorizontalDistance(
                    player.x,
                    player.y,
                    candidate.Value.x,
                    candidate.Value.y);
                if (distance >= nearestHostileDistance) continue;
                nearestHostileDistance = distance;
                nearestHostileKind = candidate.Value.kind;
            }
            if (mission.playerLife < 8_500)
            {
                recoveringFromPostCompletion = true;
            }
            if (recoveringFromPostCompletion &&
                mission.playerLife >= 9_950 &&
                nearestHostileDistance > 6_000)
            {
                recoveringFromPostCompletion = false;
            }
            if (recoveringFromPostCompletion)
            {
                return nearestHostileKind != 0
                    ? FleeVisibleActor(player, actors, nearestHostileKind)
                    : Brake(player, "recover-post-completion");
            }

            if (group33Firefly.HasValue)
            {
                return AttackActor(
                    player,
                    group33Firefly.Value,
                    "fight-group-33-firefly");
            }

            if (!group33FireflyObserved)
            {
                var distance = HorizontalDistance(
                    player.x,
                    player.y,
                    Group33SwitchWaypoint.X,
                    Group33SwitchWaypoint.Y);
                if (distance > 760)
                {
                    return Navigate(
                        player,
                        Group33SwitchWaypoint,
                        1_600,
                        true);
                }
                if (!group33SwitchReached)
                {
                    group33SwitchReached = true;
                    group33SwitchReachedTick = player.tick;
                }
                // Row 305 samples strict XY overlap only on its own eighth-
                // phase cadence. Remain inside for more than one complete
                // cadence; do not observe or infer the hidden phase itself.
                if (player.tick - group33SwitchReachedTick < 24)
                {
                    return Brake(player, "hold-group-33-switch");
                }
                return Brake(player, "wait-group-33-firefly");
            }

            if (firstMultipart.HasValue)
            {
                return AttackActor(
                    player,
                    firstMultipart.Value,
                    "fight-first-multipart");
            }
            if (!firstMultipartObserved)
            {
                return Brake(player, "wait-first-multipart");
            }

            if (group34Multipart.HasValue)
            {
                return AttackActor(
                    player,
                    group34Multipart.Value,
                    "fight-group-34-multipart");
            }
            if (!group34MultipartObserved)
            {
                // Source row 308 waits until the first head and its dying
                // body chain leave the family list, then counts seventeen
                // eligible dispatches. None of that hidden state is exposed
                // to this controller; it waits for a visible group-34 head.
                return Brake(player, "wait-group-34-release");
            }
            return EnterExit(player, actors);
        }

        private Command EnterExit(
            PlayerObservation player,
            ActorObservation[] actors)
        {
            var exitSwitch = FindActor(actors, ExitSwitchKind, activeOnly: true);
            return exitSwitch.HasValue
                ? Navigate(
                    player,
                    ActorTarget(exitSwitch.Value, 100, "enter-exit"),
                    2_000,
                    true)
                : Brake(player, "wait-exit-switch");
        }

        private Command AttackActor(
            PlayerObservation player,
            ActorObservation actor,
            string phase)
        {
            // Keep the stationary Archer's separately qualified aim. Sweep a
            // narrow visible-height band around moving Fireflies so discrete
            // fixed-step shots do not all repeat the same near-miss.
            var verticalAim = actor.kind == ArcherKind
                ? 78
                : actor.kind == MultipartHeadKind
                    ? -96
                    : -300;
            // Archers are stationary and the direct, current-position aim is
            // already reliable. Only lead visibly moving Fireflies.
            var currentTarget = ActorTarget(actor, verticalAim, phase);
            var currentDistance = HorizontalDistance(
                player.x,
                player.y,
                currentTarget.X,
                currentTarget.Y);
            var target = actor.kind == FireflyKind && currentDistance <= 3_500
                ? PredictedActorTarget(player, actor, verticalAim, phase)
                : currentTarget;
            var distance = HorizontalDistance(player.x, player.y, target.X, target.Y);
            var movementFlags = actor.kind == FireflyKind
                ? FireflyCombatSpeedFlags(player, currentDistance)
                : actor.kind == MultipartHeadKind
                    ? MultipartCombatSpeedFlags(player, currentDistance)
                    : CombatSpeedFlags(player, currentDistance);
            var aimed = DirectAim(player, target, movementFlags);
            AimError(
                player,
                target,
                out var yawError,
                out var pitchError);
            LastAimYawError = yawError;
            LastAimPitchError = pitchError;
            LastAimDistance = currentDistance;
            LastAimActor = actor.authoredIndex;
            var flags = aimed.Flags;
            short requestedSpell = -1;
            const int yawTolerance = 32;
            const int pitchTolerance = 32;
            if (distance < 7_200 &&
                IsFiringWindow(player.tick) &&
                IsDirectlyAimed(
                    player,
                    target,
                    yawTolerance,
                    pitchTolerance) &&
                player.tick % 2 == 0)
            {
                flags |= SelectedSpellFlag;
                requestedSpell = 0;
            }
            return new Command(aimed.Roll, aimed.Pitch, flags, requestedSpell, phase);
        }

        private Command AttackGoat(
            PlayerObservation player,
            GoatObservation goat)
        {
            // Walk a small cross around the visible feet so fixed-step shots
            // cannot all pass the same side of the strict collision volume.
            // Every hit still comes from an ordinary projectile/explosion.
            var offsetPhase = (int)((player.tick / 120UL) % 5UL);
            var offsetX = offsetPhase == 1 ? 128 : offsetPhase == 2 ? -128 : 0;
            var offsetY = offsetPhase == 3 ? 128 : offsetPhase == 4 ? -128 : 0;
            var target = new Target(
                goat.x + offsetX,
                goat.y + offsetY,
                goat.z - 32,
                "harvest-goat-mana");
            var distance = HorizontalDistance(player.x, player.y, target.X, target.Y);
            if (distance > 1_600)
            {
                return Navigate(
                    player,
                    new Target(goat.x, goat.y, goat.z + 600, "approach-goat"),
                    800,
                    false);
            }
            var movementFlags = distance < 400
                ? SeekSpeedFlags(player, -32)
                : HoldSpeedFlags(player);
            var aimed = DirectAim(player, target, movementFlags);
            AimError(player, target, out var yawError, out var pitchError);
            LastAimYawError = yawError;
            LastAimPitchError = pitchError;
            LastAimDistance = distance;
            LastAimActor = ushort.MaxValue;
            var flags = aimed.Flags;
            short requestedSpell = -1;
            if (distance < 6_800 &&
                IsDirectlyAimed(player, target, 128, 256) &&
                player.tick % 2 == 0)
            {
                flags |= SelectedSpellFlag;
                requestedSpell = 0;
            }
            return new Command(
                aimed.Roll,
                aimed.Pitch,
                flags,
                requestedSpell,
                target.Name);
        }

        private Command PossessSphere(
            PlayerObservation player,
            SphereObservation sphere)
        {
            var sphereDistance =
                HorizontalDistance(player.x, player.y, sphere.x, sphere.y);
            if (sphereDistance < 800)
            {
                backingAwayFromSphere = true;
            }
            else if (sphereDistance > 1_400)
            {
                backingAwayFromSphere = false;
            }
            var target = new Target(
                sphere.x,
                sphere.y,
                sphere.z - 32,
                "possess-mana");
            if (backingAwayFromSphere)
            {
                var retreat = PrecisePossessionAim(
                    player,
                    target,
                    SeekSpeedFlags(player, -64));
                return new Command(
                    retreat.Roll,
                    retreat.Pitch,
                    retreat.Flags,
                    -1,
                    "back-away-mana");
            }
            var approach = new Target(
                sphere.x,
                sphere.y,
                sphere.z + 256,
                "approach-mana");
            if (sphereDistance > 1_800 || Math.Abs(player.targetSpeed) > 32)
            {
                // Fly directly down the visible firing line and begin
                // braking before the final possession range.
                return Navigate(player, approach, 2_200, true);
            }
            AimError(player, target, out var yawError, out var pitchError);
            var movementFlags = HoldSpeedFlags(player);
            var aimed = PrecisePossessionAim(player, target, movementFlags);
            LastAimYawError = yawError;
            LastAimPitchError = pitchError;
            LastAimDistance = sphereDistance;
            LastAimActor = ushort.MaxValue;
            var flags = aimed.Flags;
            if (sphereDistance < 2_200 &&
                IsDirectlyAimed(player, target, 64, 32) &&
                player.tick % 2 == 0)
            {
                flags |= PossessionFlag;
            }
            return new Command(aimed.Roll, aimed.Pitch, flags, -1, target.Name);
        }

        private ActorObservation? LockedOrNearestActor(
            PlayerObservation player,
            ActorObservation[] actors,
            byte kind)
        {
            // A stable visual target is useful for moving Fireflies. Archers
            // remain selected afresh from visible positions so one awkward
            // firing line cannot stall the whole encounter.
            var retainVisualLock = kind == FireflyKind;
            if (retainVisualLock &&
                lockedActorIndex.HasValue &&
                lockedActorKind == kind)
            {
                foreach (var actor in actors)
                {
                    if (actor.kind == kind &&
                        actor.active != 0 &&
                        actor.authoredIndex == lockedActorIndex.Value)
                    {
                        return actor;
                    }
                }
            }

            ActorObservation? best = null;
            var bestScore = double.MaxValue;
            foreach (var actor in actors)
            {
                if (actor.kind != kind || actor.active == 0) continue;
                var playerDistance =
                    HorizontalDistance(player.x, player.y, actor.x, actor.y);
                var speed = actorTracks.TryGetValue(actor.authoredIndex, out var track)
                    ? track.Speed
                    : 0;
                var clusterDistance = 0.0;
                foreach (var other in actors)
                {
                    if (other.kind != kind ||
                        other.active == 0 ||
                        other.authoredIndex == actor.authoredIndex) continue;
                    clusterDistance += Math.Min(
                        HorizontalDistance(actor.x, actor.y, other.x, other.y),
                        10_000);
                }
                var score =
                    clusterDistance +
                    playerDistance * 0.10 +
                    Math.Min(speed, 1_000) * 80;
                if (score >= bestScore) continue;
                best = actor;
                bestScore = score;
            }
            lockedActorIndex = retainVisualLock ? best?.authoredIndex : null;
            lockedActorKind = retainVisualLock ? kind : (byte)0;
            return best;
        }

        private static double NearestActorDistance(
            PlayerObservation player,
            ActorObservation[] actors,
            byte kind)
        {
            var nearest = double.MaxValue;
            foreach (var actor in actors)
            {
                if (actor.kind != kind || actor.active == 0) continue;
                nearest = Math.Min(
                    nearest,
                    HorizontalDistance(player.x, player.y, actor.x, actor.y));
            }
            return nearest;
        }

        private static Command FleeVisibleActor(
            PlayerObservation player,
            ActorObservation[] actors,
            byte kind)
        {
            ActorObservation? nearest = null;
            var nearestDistance = double.MaxValue;
            foreach (var actor in actors)
            {
                if (actor.kind != kind || actor.active == 0) continue;
                var distance = HorizontalDistance(
                    player.x,
                    player.y,
                    actor.x,
                    actor.y);
                if (distance >= nearestDistance) continue;
                nearest = actor;
                nearestDistance = distance;
            }
            if (!nearest.HasValue)
            {
                return Brake(player, "recover-no-visible-firefly");
            }

            var awayX = -ModularDelta(nearest.Value.x, player.x);
            var awayY = -ModularDelta(nearest.Value.y, player.y);
            var magnitude = Math.Max(1.0, Math.Sqrt(
                (double)awayX * awayX + (double)awayY * awayY));
            const double escapeDistance = 12_000.0;
            var target = new Target(
                ((int)Math.Round(player.x + awayX / magnitude * escapeDistance)) & 0xffff,
                ((int)Math.Round(player.y + awayY / magnitude * escapeDistance)) & 0xffff,
                Math.Max((int)player.z, 512),
                "recover-away-from-fireflies");
            return Aim(player, target, SeekSpeedFlags(player, 128));
        }

        private GoatObservation? LockedOrNearestGoat(
            PlayerObservation player,
            GoatObservation[] goats)
        {
            var active = new List<GoatObservation>();
            foreach (var goat in goats)
            {
                if (goat.active != 0) active.Add(goat);
            }
            if (active.Count == 0) return null;

            // Do not fixate indefinitely on one awkward firing line. Cycle
            // through the visibly active list every 1,200 ticks, selecting
            // only from the same sanitized observations a player can see.
            var selection = (int)((player.tick / 5_000UL) % (ulong)active.Count);
            return active[selection];
        }

        private SphereObservation? NearestVisibleUnownedSphere(
            PlayerObservation player,
            SphereObservation[] spheres)
        {
            SphereObservation? nearestToPlayer = null;
            var nearestPlayerDistance = double.MaxValue;
            SphereObservation? tracked = null;
            foreach (var sphere in spheres)
            {
                if (sphere.active == 0 || sphere.IsLocalOwned) continue;
                var playerDistance =
                    HorizontalDistance(player.x, player.y, sphere.x, sphere.y);
                if (playerDistance < nearestPlayerDistance)
                {
                    nearestToPlayer = sphere;
                    nearestPlayerDistance = playerDistance;
                }
                if (hasLockedSpherePosition && sphere.sphereId == lockedSphereId)
                {
                    tracked = sphere;
                }
            }
            var retainedVisualTrack = tracked.HasValue;
            var selected = retainedVisualTrack ? tracked : nearestToPlayer;
            if (!selected.HasValue)
            {
                hasLockedSpherePosition = false;
                backingAwayFromSphere = false;
                return null;
            }
            if (!retainedVisualTrack)
            {
                backingAwayFromSphere = false;
            }
            lockedSphereId = selected.Value.sphereId;
            hasLockedSpherePosition = true;
            return selected;
        }

        private void UpdateActorTracks(
            ActorObservation[] actors,
            ulong tick)
        {
            foreach (var actor in actors)
            {
                if (actor.active == 0) continue;
                if (!actorTracks.TryGetValue(actor.authoredIndex, out var track))
                {
                    actorTracks[actor.authoredIndex] =
                        new ActorTrack(actor.x, actor.y, tick, 0);
                    continue;
                }
                var elapsed = Math.Max(1UL, tick - track.Tick);
                var dx = ModularDelta(actor.x, track.X);
                var dy = ModularDelta(actor.y, track.Y);
                var instantaneous =
                    Math.Sqrt((double)dx * dx + (double)dy * dy) / elapsed;
                track.X = actor.x;
                track.Y = actor.y;
                track.Tick = tick;
                track.Speed = track.Speed * 0.75 + instantaneous * 0.25;
                track.VelocityX =
                    track.VelocityX * 0.75 + dx / (double)elapsed * 0.25;
                track.VelocityY =
                    track.VelocityY * 0.75 + dy / (double)elapsed * 0.25;
            }
        }

        private Target PredictedActorTarget(
            PlayerObservation player,
            ActorObservation actor,
            int verticalAim,
            string name)
        {
            const double projectileSpeed = 384.0;
            var relativeX = (double)ModularDelta(actor.x, player.x);
            var relativeY = (double)ModularDelta(actor.y, player.y);
            var predictedX = (double)actor.x;
            var predictedY = (double)actor.y;
            var flightTicks = Math.Sqrt(
                relativeX * relativeX + relativeY * relativeY) / projectileSpeed;
            if (actorTracks.TryGetValue(actor.authoredIndex, out var track))
            {
                var a =
                    track.VelocityX * track.VelocityX +
                    track.VelocityY * track.VelocityY -
                    projectileSpeed * projectileSpeed;
                var b = 2.0 *
                    (relativeX * track.VelocityX + relativeY * track.VelocityY);
                var c = relativeX * relativeX + relativeY * relativeY;
                var discriminant = b * b - 4.0 * a * c;
                if (Math.Abs(a) > 0.0001 && discriminant >= 0)
                {
                    var root = Math.Sqrt(discriminant);
                    var first = (-b - root) / (2.0 * a);
                    var second = (-b + root) / (2.0 * a);
                    var intercept = first > 0
                        ? first
                        : second > 0
                            ? second
                            : flightTicks;
                    flightTicks = intercept;
                }
                // The observed actor advances once as the submitted command
                // enters the ordinary simulation step.
                flightTicks = Math.Clamp(flightTicks + 1.0, 1.0, 20.0);
                predictedX += track.VelocityX * flightTicks;
                predictedY += track.VelocityY * flightTicks;
            }
            return new Target(
                (int)Math.Round(predictedX) & 0xffff,
                (int)Math.Round(predictedY) & 0xffff,
                actor.z + verticalAim,
                name);
        }

        private static ActorObservation? FindActor(
            ActorObservation[] actors,
            byte kind,
            bool activeOnly)
        {
            foreach (var actor in actors)
            {
                if (actor.kind == kind && (!activeOnly || actor.active != 0)) return actor;
            }
            return null;
        }

        private static ActorObservation? FindActorByAuthoredIndex(
            ActorObservation[] actors,
            ushort authoredIndex,
            bool activeOnly)
        {
            foreach (var actor in actors)
            {
                if (actor.authoredIndex == authoredIndex &&
                    (!activeOnly || actor.active != 0))
                {
                    return actor;
                }
            }
            return null;
        }

        private static ActorObservation? NearestAuthoredRangeActor(
            PlayerObservation player,
            ActorObservation[] actors,
            ushort firstAuthoredIndex,
            ushort lastAuthoredIndex,
            byte kind)
        {
            ActorObservation? nearest = null;
            var nearestDistance = double.MaxValue;
            foreach (var actor in actors)
            {
                if (actor.active == 0 ||
                    actor.kind != kind ||
                    actor.authoredIndex < firstAuthoredIndex ||
                    actor.authoredIndex > lastAuthoredIndex)
                {
                    continue;
                }
                var distance = HorizontalDistance(
                    player.x,
                    player.y,
                    actor.x,
                    actor.y);
                if (distance >= nearestDistance) continue;
                nearest = actor;
                nearestDistance = distance;
            }
            return nearest;
        }

        private static Command Navigate(
            PlayerObservation player,
            Target target,
            int arrivalRadius,
            bool descendAtArrival)
        {
            var distance = HorizontalDistance(player.x, player.y, target.X, target.Y);
            var targetZ = descendAtArrival && distance < 1_500
                ? target.Z
                : Math.Max(target.Z, player.z);
            var adjusted = new Target(target.X, target.Y, targetZ, target.Name);
            return Aim(player, adjusted, NavigationSpeedFlags(player, distance, arrivalRadius));
        }

        private static Command Brake(
            PlayerObservation player,
            string phase)
        {
            return new Command(
                0,
                0,
                0,
                -1,
                phase,
                0x27,
                1);
        }

        private static Command Aim(
            PlayerObservation player,
            Target target,
            int movementFlags)
        {
            var dx = ModularDelta(target.X, player.x);
            var dy = ModularDelta(target.Y, player.y);
            var horizontal = Math.Sqrt((double)dx * dx + (double)dy * dy);
            var desiredYaw = Angle11(Math.Atan2(dx, -dy));
            var desiredPitch = Angle11(Math.Atan2(-(target.Z - player.z), horizontal));
            var yawError = AngleDelta(desiredYaw, player.yaw);
            var pitchError = AngleDelta(desiredPitch, player.effectivePitch);
            var roll = ClampSByte(yawError * 0.45 - player.rollAccumulator * 1.25);
            // The source input axis is screen-relative: positive mouse/input
            // deflection drives the nose toward a negative 11-bit movement
            // pitch. The command sign is therefore opposite the geometric
            // pitch error.
            var pitch = ClampSByte(-pitchError * 0.30);
            return new Command(roll, pitch, movementFlags, -1, target.Name);
        }

        private static Command DirectAim(
            PlayerObservation player,
            Target target,
            int movementFlags)
        {
            var dx = ModularDelta(target.X, player.x);
            var dy = ModularDelta(target.Y, player.y);
            var horizontal = Math.Sqrt((double)dx * dx + (double)dy * dy);
            var desiredYaw = Angle11(Math.Atan2(dx, -dy));
            var desiredPitch = Angle11(Math.Atan2(-(target.Z - player.z), horizontal));
            var yawError = AngleDelta(desiredYaw, player.yaw);
            var pitchError = AngleDelta(desiredPitch, player.effectivePitch);
            var roll = ClampSByte(yawError * 0.45 - player.rollAccumulator * 1.25);
            // A projectile copies effectivePitch verbatim. Close to the
            // terrain, a positive pitch is the source's downward direction,
            // so weapon/possession aiming must follow the geometric error.
            var pitch = ClampSByte(pitchError * 0.30);
            return new Command(roll, pitch, movementFlags, -1, target.Name);
        }

        private static Command PrecisePossessionAim(
            PlayerObservation player,
            Target target,
            int movementFlags)
        {
            var dx = ModularDelta(target.X, player.x);
            var dy = ModularDelta(target.Y, player.y);
            var horizontal = Math.Sqrt((double)dx * dx + (double)dy * dy);
            var desiredYaw = Angle11(Math.Atan2(dx, -dy));
            var desiredPitch = Angle11(Math.Atan2(-(target.Z - player.z), horizontal));
            var yawError = AngleDelta(desiredYaw, player.yaw);
            var pitchError = AngleDelta(desiredPitch, player.effectivePitch);
            var roll = ClampSByte(yawError * 0.45 - player.rollAccumulator * 1.25);
            // Possession must cross a 256-unit sphere without an explosion
            // radius. The generic combat gain leaves the projectile roughly
            // 350 units high at Level 1's common firing ranges, so this
            // controller drives the copied projectile pitch to the visible
            // center before allowing a shot.
            var pitch = ClampSByte(pitchError * 0.90);
            return new Command(roll, pitch, movementFlags, -1, target.Name);
        }

        private static bool IsFiringWindow(ulong tick)
        {
            var phase = tick % 240;
            return phase is >= 60 and < 140;
        }

        private static bool IsDirectlyAimed(
            PlayerObservation player,
            Target target,
            int yawTolerance,
            int pitchTolerance)
        {
            AimError(player, target, out var yawError, out var pitchError);
            return Math.Abs(yawError) <= yawTolerance &&
                   Math.Abs(pitchError) <= pitchTolerance;
        }

        private static void AimError(
            PlayerObservation player,
            Target target,
            out int yawError,
            out int pitchError)
        {
            var dx = ModularDelta(target.X, player.x);
            var dy = ModularDelta(target.Y, player.y);
            var horizontal = Math.Sqrt((double)dx * dx + (double)dy * dy);
            var desiredYaw = Angle11(Math.Atan2(dx, -dy));
            var desiredPitch = Angle11(Math.Atan2(-(target.Z - player.z), horizontal));
            yawError = AngleDelta(desiredYaw, player.yaw);
            pitchError = AngleDelta(desiredPitch, player.effectivePitch);
        }

        private static int NavigationSpeedFlags(
            PlayerObservation player,
            double distance,
            int arrivalRadius)
        {
            if (distance <= arrivalRadius) return HoldSpeedFlags(player);
            var desiredSpeed = distance > 4_000 ? 128 : 64;
            return SeekSpeedFlags(player, desiredSpeed);
        }

        private static int CombatSpeedFlags(
            PlayerObservation player,
            double distance)
        {
            // A stationary target needs a stable firing line. Circling made
            // the controller repeatedly cross the narrow source projectile
            // collision column while its shot was in flight. Close to a
            // visible Archer/Goat, settle the carpet and aim through ordinary
            // controls; only translate to regain a useful range.
            if (distance > 3_600) return SeekSpeedFlags(player, 64);
            if (distance < 1_200) return SeekSpeedFlags(player, -48);
            return HoldSpeedFlags(player);
        }

        private static int FireflyCombatSpeedFlags(
            PlayerObservation player,
            double distance)
        {
            // Stay inside the source Firefly's 5,120-unit engagement radius
            // while retaining room to see and react to either ranged fire or
            // a charge. This uses only the visible actor distance.
            if (distance > 4_400) return SeekSpeedFlags(player, 64);
            if (distance < 3_000) return SeekSpeedFlags(player, -64);
            return HoldSpeedFlags(player);
        }

        private static int MultipartCombatSpeedFlags(
            PlayerObservation player,
            double distance)
        {
            // The multipart creature is large and durable. Keep an ordinary
            // visible firing line near the far edge of Fireball range rather
            // than stopping inside its body-chain contact envelope.
            if (distance > 6_200) return SeekSpeedFlags(player, 64);
            if (distance < 5_200) return SeekSpeedFlags(player, -96);
            return HoldSpeedFlags(player);
        }

        private static int SeekSpeedFlags(
            PlayerObservation player,
            int desiredSpeed)
        {
            if (player.targetSpeed < desiredSpeed - 16) return 0x01;
            if (player.targetSpeed > desiredSpeed + 16) return 0x02;
            return 0;
        }

        private static int HoldSpeedFlags(PlayerObservation player)
        {
            if (player.targetSpeed > 16) return 0x02;
            if (player.targetSpeed < -16) return 0x01;
            return 0;
        }

        private static Target ActorTarget(
            ActorObservation actor,
            int verticalAim,
            string name)
        {
            return new Target(actor.x, actor.y, actor.z + verticalAim, name);
        }

        private static double HorizontalDistance(
            ushort fromX,
            ushort fromY,
            int toX,
            int toY)
        {
            var dx = ModularDelta(toX, fromX);
            var dy = ModularDelta(toY, fromY);
            return Math.Sqrt((double)dx * dx + (double)dy * dy);
        }

        private static int ModularDelta(int target, ushort current)
        {
            return ((target - current + 32_768) & 0xffff) - 32_768;
        }

        private static int Angle11(double radians)
        {
            var value = (int)Math.Round(radians * 2048.0 / (Math.PI * 2.0));
            return value & 0x07ff;
        }

        private static int AngleDelta(int target, int current)
        {
            var difference = ((target - current + 1_024) & 0x07ff) - 1_024;
            return difference;
        }

        private static sbyte ClampSByte(double value)
        {
            return (sbyte)Math.Clamp((int)Math.Round(value), sbyte.MinValue, sbyte.MaxValue);
        }

        private sealed class ActorTrack
        {
            internal ushort X;
            internal ushort Y;
            internal ulong Tick;
            internal double Speed;
            internal double VelocityX;
            internal double VelocityY;

            internal ActorTrack(ushort x, ushort y, ulong tick, double speed)
            {
                X = x;
                Y = y;
                Tick = tick;
                Speed = speed;
                VelocityX = 0;
                VelocityY = 0;
            }
        }

        private readonly struct Target
        {
            internal readonly int X;
            internal readonly int Y;
            internal readonly int Z;
            internal readonly string Name;

            internal Target(int x, int y, int z, string name)
            {
                X = x;
                Y = y;
                Z = z;
                Name = name;
            }
        }
    }
}
