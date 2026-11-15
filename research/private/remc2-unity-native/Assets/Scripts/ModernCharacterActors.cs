using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace MagicCarpet.Native
{
    // One instance belongs to one native slot generation, never to the slot itself.
    internal sealed class VoxelDeathLifecycle
    {
        internal bool Exploded { get; private set; }
        internal bool RetainAfterRemoval(bool dying) => dying && !Exploded;
        internal bool TryExplode(bool dying, Func<bool> emit)
        {
            if (!dying || Exploded) return Exploded;
            Exploded = emit();
            return Exploded;
        }
    }

    // Joins supported native occupants to reviewed rigs. No gameplay is simulated here.
    internal sealed class ModernCharacterActors : IDisposable
    {
        private sealed class Actor : IDisposable
        {
            internal NativeSim.CampaignCharacterState State;
            internal ModernGoatPresenter Goat;
            internal ModernArcherPresenter Archer;
            internal ModernVillagerPresenter Villager;
            internal ModernFireflyPresenter Firefly;
            internal ModernWormPresenter Worm;
            internal readonly Dictionary<ushort, Part> Parts = new();
            internal Vector3 From, To;
            internal float FromYaw, ToYaw;
            internal Quaternion FromRotation, ToRotation;
            internal double ObservedAt;
            internal float Interval = 1f / 30;
            internal bool Visible, WasVisible;
            internal bool WasRendered;
            internal bool TerrainContactComplete = true;
            internal float TerrainSlope;
            internal Renderer[] Renderers;
            internal int RenderedFrame = -1;
            internal readonly VoxelDeathLifecycle VoxelDeath = new();
            internal bool AuthoritativeDeath;
            internal List<Voxels.VoxelFragment> DeathFragments;
            internal Vector3 DeathVelocity, DeathOrigin;
            internal bool IsDying => AuthoritativeDeath || (Goat != null ? Goat.IsDying : Villager != null ? Villager.IsDying : Firefly != null ? Firefly.IsDying : Worm != null ? Worm.IsDying : Archer.IsDying);
            internal bool VoxelIsDying => AuthoritativeDeath || SourceVoxelDeath(State.model, State.action, State.life, (State.lifecycleFlags & 1) != 0);
            internal GameObject Root => Goat != null ? Goat.Root : Villager != null ? Villager.Root : Firefly != null ? Firefly.Root : Worm != null ? Worm.Root : Archer.Root;
            internal bool Finished => Goat != null ? Goat.DeathFinished : Villager != null ? Villager.DeathFinished : Firefly != null ? Firefly.DeathFinished : Worm != null ? Worm.DeathFinished : Archer.DeathFinished;
            internal string Clip => Goat != null ? Goat.CurrentClip : Villager != null ? Villager.CurrentClip : Firefly != null ? Firefly.CurrentClip : Worm != null ? Worm.CurrentClip : Archer.CurrentClip;
            internal float ClipTime => Goat != null ? Goat.ClipTime : Villager != null ? Villager.ClipTime : Firefly != null ? Firefly.ClipTime : Worm != null ? Worm.ClipTime : Archer.ClipTime;
            internal void SetVisible(bool visible) { if (Worm != null) Worm.SetVisible(visible); else Root.SetActive(visible); }
            internal void Render(float dt)
            {
                if (RenderedFrame == Time.frameCount) return;
                RenderedFrame = Time.frameCount;
                if (Goat != null) Goat.Render(dt); else if (Villager != null) Villager.Render(dt); else if (Firefly != null) Firefly.Render(dt); else if (Worm != null) Worm.Render(dt); else Archer.Render(dt);
            }
            internal void Remove() { if (Goat != null) Goat.MarkRemoved(); else if (Villager != null) Villager.MarkRemoved(); else if (Firefly != null) Firefly.MarkRemoved(); else if (Worm != null) Worm.MarkRemoved(); else Archer.MarkRemoved(); }
            public void Dispose() { Goat?.Dispose(); Archer?.Dispose(); Villager?.Dispose(); Firefly?.Dispose(); Worm?.Dispose(); }
        }
        private sealed class Part
        {
            internal NativeSim.CampaignCharacterState State;
            internal Vector3 From, To;
            internal Quaternion FromRotation, ToRotation;
            internal double ObservedAt;
            internal float Interval = 1f / 30;
        }
        private sealed class Arrow
        {
            internal NativeSim.CampaignCharacterState State;
            internal GameObject Root;
            internal Vector3 From, To, BirthOffset;
            internal Quaternion Rotation;
            internal double ObservedAt, BornAt;
            internal float Interval;
            internal bool AlignAtBirth;
        }
        private readonly Transform parent;
        private readonly ModernTerrainMesh terrain;
        private readonly Dictionary<ushort, Actor> actors = new();
        private readonly Dictionary<ushort, Arrow> arrows = new();
        private readonly List<Actor> dying = new();
        private readonly Dictionary<ushort, NativeSim.CampaignCharacterState> observed = new();
        private sealed class VillagerArt
        {
            internal ModernCharacterAsset Asset;
            internal Voxels.VoxelGoatAsset Fine, Far;
        }
        private readonly Dictionary<ushort, VillagerArt> villagerArt = new();
        private VillagerArt fireflyArt;
        private ModernCharacterAsset wormHeadAsset, wormSegmentAsset;
        private Voxels.VoxelGoatAsset wormHeadVoxels, wormSegmentVoxels;
        private ModernCharacterAsset goatAsset, archerAsset;
        private Voxels.VoxelGoatAsset goatFine,goatMiddle,goatFar,archerFine,archerFar;
        private GameObject arrowTemplate;
        private ulong generation = ulong.MaxValue, tick = ulong.MaxValue;
        private bool logged;
        private readonly string follow;
        private readonly bool followHead;
        private readonly float followYaw = 45;
        private const float ReviewFieldOfView = 48;
        private readonly ModernCharacterGameplayCapture capture;
        private bool reviewPositioned;
        private Vector3 lastReviewPosition;
        private Quaternion lastReviewRotation;
        private Camera renderedCamera;
        private float reviewDistance;
        private Actor followedLifetime;
        private readonly Plane[] voxelViewPlanes=new Plane[6];

        internal ModernCharacterActors(Transform parent, ModernTerrainMesh terrain)
        {
            this.parent = parent; this.terrain = terrain;
            var args = Environment.GetCommandLineArgs();
            followHead = args.Contains("-mc-character-follow-head");
            var option = Array.IndexOf(args, "-mc-character-follow");
            if (option >= 0)
            {
                if (option + 1 >= args.Length) throw new ArgumentException("Character follow requires Goat, Archer, a settlement character resource, Villager or a source slot");
                follow = args[option + 1];
            }
            var yawOption = Array.IndexOf(args, "-mc-character-follow-yaw");
            if (yawOption >= 0 && (yawOption + 1 >= args.Length ||
                !float.TryParse(args[yawOption + 1], System.Globalization.NumberStyles.Float,
                    System.Globalization.CultureInfo.InvariantCulture, out followYaw) || !float.IsFinite(followYaw)))
                throw new ArgumentException("Character follow yaw requires a finite angle in degrees");
            if (args.Contains("-mc-character-gameplay-capture"))
            {
                capture = parent.gameObject.AddComponent<ModernCharacterGameplayCapture>();
                capture.Bind(() => tick != ulong.MaxValue && (follow == null || Followed() != null), Report);
            }
        }

        private Actor Followed()
        {
            if (follow == null) return null;
            // A reclaimed numeric slot can immediately belong to another
            // creature. Keep this review on the selected incarnation through
            // its corpse/breakup, then hold the camera until an explicit reset.
            if (followedLifetime != null)
                return !followedLifetime.VoxelDeath.Exploded &&
                    (actors.TryGetValue(followedLifetime.State.entityIndex, out var current) && ReferenceEquals(current, followedLifetime)
                     || dying.Contains(followedLifetime)) ? followedLifetime : null;
            if (ushort.TryParse(follow, out var slot))
                return followedLifetime = actors.TryGetValue(slot, out var value) && !value.VoxelDeath.Exploded ? value
                    : dying.FirstOrDefault(a => a.State.entityIndex == slot && !a.VoxelDeath.Exploded);
            return followedLifetime = actors.OrderBy(p => p.Key).Select(p => p.Value).FirstOrDefault(a => !a.VoxelDeath.Exploded &&
                (follow == "Goat" ? a.Goat != null : follow == "Villager" ? a.State.model == 13 :
                a.Villager != null ? ModernVillagerAppearances.Get(a.State.renderParameter).Resource == follow :
                follow == "Firefly" ? a.Firefly != null : follow == "Worm" ? a.Worm != null : follow == "Archer" && a.Archer != null));
        }

        internal bool ReviewCamera(Camera camera)
        {
            var actor = Followed();
            if (actor == null)
            {
                if (follow == null || !reviewPositioned) return false;
                camera.fieldOfView = ReviewFieldOfView;
                camera.transform.SetPositionAndRotation(lastReviewPosition, lastReviewRotation);
                return true;
            }
            // Preserve the last rendered live pose for the imminent breakup.
            if (Voxels.VoxelModernWorld.Requested && actor.VoxelIsDying)
            {
                if (!reviewPositioned) return false;
                camera.fieldOfView = ReviewFieldOfView;
                camera.transform.SetPositionAndRotation(lastReviewPosition, lastReviewRotation);
                return true;
            }
            var position = Interpolated(actor);
            if (actor.Firefly == null && actor.Worm == null) position.y = terrain.SampleVisualHeight(position.x, position.z);
            var heading = InterpolatedYaw(actor);
            // Fit the actual terrain-oriented root, including the first frame.
            // Fitting its spawn-upright bounds cropped the later tilted pose.
            Place(actor, position, heading, camera);
            var sampled = actor.RenderedFrame == Time.frameCount;
            actor.Render(Time.deltaTime);
            actor.Villager?.GroundCorpseToTerrain(terrain.SampleVisualHeight, terrain.Revision);
            if (!sampled && actor.Goat != null)
                actor.TerrainContactComplete = actor.Goat.GroundToTerrain(terrain.SampleVisualHeight);
            // Fit this frame's pose. A previously hidden nocking arrow can
            // extend beyond the preceding frame's body/prop bounds.
            var offset = Quaternion.Euler(0, heading + followYaw, 0) * new Vector3(0, actor.Goat != null ? 1.4f : 1.8f, actor.Goat != null ? 6.5f : 9);
            var bounds = CharacterBounds(actor, followHead);
            if (actor.Worm == null && actor.Firefly == null) bounds.center += Wrap(position, camera.transform.position) - actor.Root.transform.position;
            var rotation = Quaternion.LookRotation(-offset);
            camera.fieldOfView = ReviewFieldOfView;
            var vertical = Mathf.Tan(camera.fieldOfView * Mathf.Deg2Rad * .5f);
            var horizontal = vertical * camera.aspect;
            var distance = actor.Goat != null ? 6.5f : 9;
            var cameraPosition = bounds.center + offset;
            for (var pass = 0; pass < 4; pass++)
            {
                var required = 0f;
                for (var i = 0; i < 8; i++)
                {
                    var view = Quaternion.Inverse(rotation) * Corner(bounds.extents, i);
                    required = Mathf.Max(required, Mathf.Max(Mathf.Abs(view.x) / horizontal, Mathf.Abs(view.y) / vertical) - view.z);
                }
                distance = Mathf.Max(distance, required * 1.25f + .15f);
                cameraPosition = bounds.center - rotation * Vector3.forward * distance;
                var floor = terrain.SampleVisualHeight(cameraPosition.x, cameraPosition.z) + .8f;
                if (cameraPosition.y >= floor) break;
                cameraPosition.y = floor;
                rotation = Quaternion.LookRotation(bounds.center - cameraPosition);
                distance = Vector3.Distance(bounds.center, cameraPosition);
            }
            reviewDistance = Mathf.Max(distance, Mathf.Lerp(reviewDistance, distance, 1 - Mathf.Exp(-Time.deltaTime * 8)));
            cameraPosition = bounds.center - rotation * Vector3.forward * reviewDistance;
            cameraPosition.y = Mathf.Max(cameraPosition.y, terrain.SampleVisualHeight(cameraPosition.x, cameraPosition.z) + .8f);
            camera.transform.SetPositionAndRotation(cameraPosition, Quaternion.LookRotation(bounds.center - cameraPosition));
            lastReviewPosition = camera.transform.position; lastReviewRotation = camera.transform.rotation; reviewPositioned = true;
            return true;
        }

        private static Vector3 Corner(Vector3 extents, int index) => Vector3.Scale(extents,
            new Vector3((index & 1) == 0 ? -1 : 1, (index & 2) == 0 ? -1 : 1, (index & 4) == 0 ? -1 : 1));

        private static Bounds CharacterBounds(Actor actor, bool headOnly = false)
        {
            if (actor.Worm != null && !headOnly && actor.Worm.TryGetFamilyBounds(out var family)) return family;
            actor.Renderers ??= actor.Root.GetComponentsInChildren<Renderer>(true);
            var bounds = new Bounds(); var found = false;
            foreach (var renderer in actor.Renderers)
            {
                if (!renderer.enabled || renderer.transform.lossyScale.sqrMagnitude < .0001f) continue;
                var value = renderer.bounds;
                if (value.size.sqrMagnitude < .000001f) continue;
                if (!float.IsFinite(value.center.x) || !float.IsFinite(value.center.y) || !float.IsFinite(value.center.z) ||
                    !float.IsFinite(value.extents.sqrMagnitude)) throw new InvalidOperationException("Non-finite live character bounds");
                if (found) bounds.Encapsulate(value); else { bounds = value; found = true; }
            }
            if (!found) throw new InvalidOperationException("Live character has no framing geometry");
            return bounds;
        }

        private bool FollowedFramingValid()
        {
            var actor = Followed();
            // Native camera observation writes its projection every turn.
            // Retaining only the review transform still zooms away from the
            // fracture when the followed actor disappears.
            if (follow != null && reviewPositioned && renderedCamera != null &&
                Mathf.Abs(renderedCamera.fieldOfView - ReviewFieldOfView) > .0001f) return false;
            if (actor == null || renderedCamera == null) return true;
            var bounds = CharacterBounds(actor, followHead);
            for (var i = 0; i < 8; i++)
            {
                var point = renderedCamera.WorldToViewportPoint(bounds.center + Corner(bounds.extents, i));
                if (!float.IsFinite(point.x) || !float.IsFinite(point.y) || !float.IsFinite(point.z) ||
                    point.x < 0 || point.x > 1 || point.y < 0 || point.y > 1 ||
                    point.z < renderedCamera.nearClipPlane || point.z > renderedCamera.farClipPlane) return false;
            }
            return true;
        }

        internal static bool IsArcherArrow(NativeSim.CampaignCharacterState state) => state.classId == 9 && state.model == 13;
        internal static bool IsSupportedActor(NativeSim.CampaignCharacterState state) =>
            state.classId == 5 && (state.model == 1 || state.model == 4 ||
                state.model == 3 && state.renderParameter == 88 && NativeCampaignSession.HasSupportedCharacterIdentity(state) ||
                state.model == 19 && NativeCampaignSession.HasSupportedCharacterIdentity(state) || ModernVillagerPresenter.Supports(state));
        internal bool Replaces(ushort sourceSlot)
        {
            if (!observed.TryGetValue(sourceSlot, out var state)) return false;
            if (IsSupportedActor(state) || IsArcherArrow(state)) return true;
            if (state.classId != 5 || state.model != 3 || state.renderParameter < 89 || state.renderParameter > 104) return false;
            foreach (var actor in actors.Values.Concat(dying))
                if (actor.Worm != null && actor.Parts.TryGetValue(sourceSlot, out var part) &&
                    part.State.slotGeneration == state.slotGeneration && part.State.sourceId == state.sourceId &&
                    actor.Worm.SegmentRoot(sourceSlot, state.slotGeneration) != null) return true;
            return false;
        }

        internal static bool SourceVoxelDeath(byte model, byte action, int life, bool livingEntry = false) => life < 0 ||
            (model == 1 && (action == 12 || action == 13)) || (model == 4 && action == 37) ||
            (model == 19 && ModernFireflyPresenter.SourceDeath(action, life)) ||
            (model == 3 && ModernWormPresenter.SourceDeath(action, life)) ||
            ((model == 12 || model == 13 || model == 14) && ModernVillagerPresenter.SourceDeath(action, life, model, livingEntry));

        // The integration delivers lifetime-qualified native death evidence
        // before observing removal or reuse. A slot alone is never sufficient.
        internal bool ApplyDeath(ulong worldEpoch, ushort slot, uint slotGeneration, ushort sourceId)
        {
            if (worldEpoch != generation) return false;
            Actor actor = null;
            if (actors.TryGetValue(slot, out var current) && current.State.slotGeneration == slotGeneration && current.State.sourceId == sourceId)
                actor = current;
            else
                actor = dying.FirstOrDefault(a => a.State.entityIndex == slot && a.State.slotGeneration == slotGeneration && a.State.sourceId == sourceId);
            if (actor == null) return false;
            actor.AuthoritativeDeath = true;
            actor.Villager?.ConfirmDeath();
            actor.Firefly?.ConfirmDeath();
            actor.Worm?.ConfirmDeath();
            return true;
        }

        internal static bool MustClearActors(ulong previousEpoch,ulong previousTick,ulong epoch,ulong currentTick)
            => previousEpoch!=epoch || currentTick<previousTick;

        internal static void ApplyActorVisibility(GameObject root, NativeSim.CampaignCharacterState state,
            bool eligible, bool voxelMode)
        {
            // Spatial admission and spectator follow cannot override native
            // house-entry/drawing hide flags in either presentation style.
            // A confirmed fall continues through the hidden reclaim tail. A
            // living entry still disappears immediately, without a death pose.
            if (!SourceVoxelDeath(state.model, state.action, state.life, (state.lifecycleFlags & 1) != 0) &&
                (state.active == 0 || (state.controlFlags & 0x421u) != 0)) eligible = false;
            root.SetActive(eligible);
        }

        internal void Observe(NativeCampaignSession session, NativeSim.CampaignEntityPresentation[] visible)
        {
            var epoch=Voxels.VoxelModernWorld.Requested?session.VoxelWorldEpoch:session.Generation;
            if (generation == epoch && tick == session.Tick) return;
            if (MustClearActors(generation,tick,epoch,session.Tick)) Clear();
            generation = epoch; tick = session.Tick;
            observed.Clear();
            foreach (var row in session.CharacterStates()) observed.Add(row.entityIndex, row);
            var presentations = visible.ToDictionary(s => s.entityIndex);
            var births = new Dictionary<ushort, NativeSim.CampaignCharacterState>();
            var confirmedBirths = new HashSet<ushort>();
            foreach (var state in observed.Values.Where(s => IsArcherArrow(s) && s.ageTicks <= 1 && s.hasParent != 0))
            {
                if (!arrows.TryGetValue(state.entityIndex, out var prior) || prior.State.slotGeneration != state.slotGeneration)
                    births[state.parentIndex] = state;
            }
            foreach (var state in observed.Values.Where(IsSupportedActor))
            {
                if (actors.TryGetValue(state.entityIndex, out var prior) &&
                    (prior.State.sourceId != state.sourceId || prior.State.slotGeneration != state.slotGeneration || prior.State.model != state.model || prior.State.renderParameter != state.renderParameter))
                { Retire(prior); actors.Remove(state.entityIndex); }
                var position = Position(state);
                var yaw = 180f - state.yaw * 360f / 2048f;
                var hadPriorObservation = actors.ContainsKey(state.entityIndex);
                if (!actors.TryGetValue(state.entityIndex, out var actor))
                {
                    actor = new Actor { From = position, To = position, FromYaw = yaw, ToYaw = yaw,
                        FromRotation = Rotation(state), ToRotation = Rotation(state) };
                    if (state.model == 1)
                    {
                        goatAsset ??= Load("Goat");
                        actor.Goat = new ModernGoatPresenter(goatAsset, parent, 150f / 64);
                        if(Voxels.VoxelModernWorld.Requested){PrepareVoxelDetails();actor.Goat.AttachVoxelSkin(goatFine);}
                    }
                    else if (ModernVillagerPresenter.Supports(state))
                    {
                        var binding = ModernVillagerAppearances.Get(state.renderParameter);
                        var art = PrepareVillagerArt(binding);
                        actor.Villager = new ModernVillagerPresenter(art.Asset, parent, binding.SourceHeight, binding.Model);
                        if(Voxels.VoxelModernWorld.Requested) actor.Villager.AttachVoxelSkin(art.Fine);
                    }
                    else if (state.model == 19)
                    {
                        fireflyArt ??= new VillagerArt { Asset = Load("Firefly") };
                        actor.Firefly = new ModernFireflyPresenter(fireflyArt.Asset, parent, state.renderHeight / 64f);
                        if (Voxels.VoxelModernWorld.Requested)
                        {
                            fireflyArt.Fine ??= LoadDetail("Firefly", 96);
                            fireflyArt.Far ??= LoadDetail("Firefly", 64);
                            actor.Firefly.AttachVoxelSkin(fireflyArt.Fine);
                        }
                    }
                    else if (state.model == 3)
                    {
                        wormHeadAsset ??= Load("WormHead"); wormSegmentAsset ??= Load("WormSegment");
                        actor.Worm = new ModernWormPresenter(wormHeadAsset, wormSegmentAsset, parent,
                            state.entityIndex, state.sourceId, state.slotGeneration);
                        if (Voxels.VoxelModernWorld.Requested)
                        {
                            wormHeadVoxels ??= LoadDetail("WormHead", 64);
                            wormSegmentVoxels ??= LoadDetail("WormSegment", 64);
                            actor.Worm.AttachVoxelSkins(wormHeadVoxels, wormSegmentVoxels);
                        }
                    }
                    else
                    {
                        archerAsset ??= Load("Archer");
                        actor.Archer = new ModernArcherPresenter(archerAsset, parent, 250f / 64);
                        if(Voxels.VoxelModernWorld.Requested){PrepareVoxelDetails();actor.Archer.AttachVoxelSkin(archerFine);}
                    }
                    actors.Add(state.entityIndex, actor);
                    actor.Root.transform.SetPositionAndRotation(
                        new Vector3(position.x, actor.Firefly != null ? position.y : terrain.SampleVisualHeight(position.x, position.z), position.z),
                        actor.Firefly != null ? Rotation(state) : Quaternion.Euler(0, yaw, 0));
                }
                else
                {
                    actor.From = Interpolated(actor);
                    actor.FromYaw = InterpolatedYaw(actor);
                    actor.FromRotation = InterpolatedRotation(actor);
                    actor.Interval = Mathf.Max(1f / 30, (state.tick - actor.State.tick) / 30f);
                }
                actor.To = position; actor.ToYaw = yaw; actor.ObservedAt = Time.timeAsDouble;
                actor.ToRotation = Rotation(state);
                actor.State = state;
                actor.Visible = presentations.TryGetValue(state.entityIndex, out var presentation);
                actor.WasVisible |= actor.Visible;
                if (actor.Goat != null) actor.Goat.Observe(position, yaw, state.life, state.action, state.tick);
                else if (actor.Villager != null) actor.Villager.Observe(position, yaw, state.life, state.action, state.tick, (state.lifecycleFlags & 1) != 0);
                else if (actor.Firefly != null) actor.Firefly.Observe(position, yaw, state.pitch * 360f / 2048f,
                    state.roll * 360f / 2048f, state.life, state.action, state.phase, state.tick);
                else if (actor.Worm != null) ObserveWorm(actor, state);
                else
                {
                    var aim = state.hasTarget != 0 ? Delta(position, new Vector3(state.targetX / 64f, state.targetZ / 64f, state.targetY / 64f)) : Direction(state.yaw, state.pitch);
                    var child = default(NativeSim.CampaignCharacterState);
                    var release = hadPriorObservation && births.TryGetValue(state.entityIndex, out child) && child.ageTicks <= 1 &&
                        child.hasParent != 0 && child.parentId == state.sourceId && child.parentGeneration == state.slotGeneration;
                    if (release) { aim = Direction(child.yaw, child.pitch); confirmedBirths.Add(child.entityIndex); }
                    var kneeling = state.stance == 1;
                    actor.Archer.Observe(position, yaw, state.life, state.action, state.phase, state.tick, aim, release, kneeling);
                }
            }
            foreach (var key in actors.Keys.Where(key => !observed.TryGetValue(key, out var s) || !IsSupportedActor(s)).ToArray())
            {
                var actor = actors[key]; actors.Remove(key);
                Retire(actor);
            }
            foreach (var state in observed.Values.Where(IsArcherArrow))
            {
                var position = Position(state);
                if (!arrows.TryGetValue(state.entityIndex, out var arrow) || arrow.State.slotGeneration != state.slotGeneration)
                {
                    if (arrow != null) UnityEngine.Object.Destroy(arrow.Root);
                    archerAsset ??= Load("Archer");
                    arrowTemplate ??= ModernArcherPresenter.CreateArrow(archerAsset, parent, (250f / 64) / archerAsset.height);
                    arrowTemplate.SetActive(false);
                    arrow = new Arrow { Root = UnityEngine.Object.Instantiate(arrowTemplate, parent), BornAt = Time.timeAsDouble };
                    arrow.Root.SetActive(true);
                    arrow.From = state.ageTicks == 0 ? position : position - Direction(state.yaw, state.pitch) * (state.speed / 64f);
                    if (actors.TryGetValue(state.parentIndex, out var actor) && actor.Archer != null &&
                        confirmedBirths.Contains(state.entityIndex) && state.hasParent != 0 &&
                        actor.State.sourceId == state.parentId && actor.State.slotGeneration == state.parentGeneration)
                    {
                        // Presentation interpolation starts at the actual held
                        // nock. Native flight/collision coordinates are untouched.
                        arrow.AlignAtBirth = true;
                    }
                    arrows[state.entityIndex] = arrow;
                }
                else arrow.From = ArrowPosition(arrow);
                arrow.State = state; arrow.To = position;
                arrow.Interval = 1f / 30;
                arrow.Rotation = Quaternion.LookRotation(Direction(state.yaw, state.pitch));
                arrow.ObservedAt = Time.timeAsDouble;
                arrow.Root.SetActive(state.active != 0);
            }
            foreach (var key in arrows.Keys.Where(key => !observed.TryGetValue(key, out var s) || !IsArcherArrow(s)).ToArray())
            { UnityEngine.Object.Destroy(arrows[key].Root); arrows.Remove(key); }
            if (!logged)
            {
                logged = true;
                Debug.Log($"MC_MODERN_CHARACTERS_LIVE goats={actors.Values.Count(s => s.Goat != null)} archers={actors.Values.Count(s => s.Archer != null)} villagers={actors.Values.Count(s => s.Villager != null)} native_tick={tick} generation={generation}");
            }
        }

        private void ObserveWorm(Actor actor, NativeSim.CampaignCharacterState head)
        {
            var rows = observed.Values.Where(part => part.classId == 5 && part.model == 3 &&
                part.renderParameter >= 89 && part.renderParameter <= 104 && part.hasParent != 0 &&
                part.parentIndex == head.entityIndex && part.parentId == head.sourceId &&
                part.parentGeneration == head.slotGeneration).ToArray();
            var poses = new ModernWormPresenter.SegmentPose[rows.Length];
            for (var i = 0; i < rows.Length; i++)
            {
                var state = rows[i]; var position = Position(state); var rotation = Rotation(state);
                if (!actor.Parts.TryGetValue(state.entityIndex, out var part) ||
                    part.State.slotGeneration != state.slotGeneration || part.State.sourceId != state.sourceId)
                    actor.Parts[state.entityIndex] = part = new Part { From = position, To = position,
                        FromRotation = rotation, ToRotation = rotation };
                else
                {
                    var weight = Mathf.Clamp01((float)(Time.timeAsDouble - part.ObservedAt) / part.Interval);
                    part.From += Delta(part.From, part.To) * weight;
                    part.FromRotation = Quaternion.Slerp(part.FromRotation, part.ToRotation, weight);
                    part.Interval = Mathf.Max(1f / 30, (state.tick - part.State.tick) / 30f);
                }
                part.State = state; part.To = position; part.ToRotation = rotation; part.ObservedAt = Time.timeAsDouble;
                poses[i] = new ModernWormPresenter.SegmentPose(state.entityIndex, state.sourceId, state.slotGeneration,
                    state.parentIndex, state.parentGeneration, checked((byte)state.partIndex), position,
                    180f - state.yaw * 360f / 2048f, state.pitch * 360f / 2048f, -state.roll * 360f / 2048f,
                    state.renderHeight / 64f * actor.Worm.SegmentWidthToHeight, state.renderHeight / 64f, true,
                    state.active != 0 && (state.controlFlags & 0x421u) == 0, state.life, state.action);
            }
            actor.Worm.Observe(head.life, head.action, head.tick, poses);
            if (!actor.Worm.IsDying)
            {
                var current = rows.Select(row => row.entityIndex).ToHashSet();
                foreach (var slot in actor.Parts.Keys.Where(slot => !current.Contains(slot)).ToArray()) actor.Parts.Remove(slot);
                // Retain actual living native placement even offscreen. This
                // does not advance animation or invent a first-dead pose.
                PlaceWorm(actor, actor.To, renderedCamera != null ? renderedCamera.transform.position : actor.To);
            }
        }

        internal void Render(Camera camera)
        {
            renderedCamera = camera;
            var dt = Time.deltaTime;
            var followed = Followed();
            GeometryUtility.CalculateFrustumPlanes(camera,voxelViewPlanes);
            foreach (var actor in actors.Values)
            {
                if (Voxels.VoxelModernWorld.Requested && actor.VoxelDeath.Exploded)
                { actor.SetVisible(false); continue; }
                var sampled = actor.RenderedFrame == Time.frameCount;
                var position = Interpolated(actor);
                // Keep observing every occupant. Manual PlayableGraph updates
                // bypass Animator visibility culling, so don't evaluate rigs
                // that cannot contribute to this view. Modern's large margin
                // retains nearby offscreen shadow casters and has no distance
                // cutoff. Deaths continue even when the camera turns away.
                var voxelMode = Voxels.VoxelModernWorld.Requested;
                var viewBounds = new Bounds(Wrap(position,camera.transform.position)+Vector3.up*3,
                    voxelMode ? new Vector3(12,16,12) : new Vector3(64,24,64));
                if (actor.Worm != null)
                {
                    Place(actor, position, InterpolatedYaw(actor), camera);
                    if (actor.Worm.TryGetFamilyBounds(out var wholeFamily)) viewBounds = wholeFamily;
                }
                var distanceSquared = actor.Worm != null ? viewBounds.SqrDistance(camera.transform.position)
                    : Delta(camera.transform.position,position).sqrMagnitude;
                if(!actor.VoxelIsDying && actor!=followed &&
                    (voxelMode && distanceSquared>14400 ||
                     (voxelMode || actor.Villager != null || actor.Worm != null) && !GeometryUtility.TestPlanesAABB(voxelViewPlanes,viewBounds)))
                { actor.SetVisible(false); actor.WasRendered = false; continue; }
                if(Voxels.VoxelModernWorld.Requested && !actor.VoxelIsDying)SetVoxelDetail(actor,actor==followed?0:Delta(camera.transform.position,position).magnitude);
                if (actor.Worm == null) Place(actor, position, InterpolatedYaw(actor), camera);
                if (!actor.WasRendered && actor.Villager != null) actor.Villager.RebaseTravel();
                actor.WasRendered = true;
                // Capture the last live pose before advancing a death animation.
                if (Voxels.VoxelModernWorld.Requested && actor.VoxelIsDying)
                { FractureIfDead(actor); actor.SetVisible(false); continue; }
                // A voxel observer uses its own spatial admission. Native
                // liveness and hide flags still control the actual renderer.
                ApplyActorVisibility(actor.Root,actor.State,
                    (actor.Visible || actor == followed || actor.IsDying && actor.WasVisible || Voxels.VoxelModernWorld.Requested&&actor.State.active!=0) && ModernCapture.EntityVisibility != 1,
                    Voxels.VoxelModernWorld.Requested);
                if (actor.Worm != null) actor.SetVisible(actor.Root.activeSelf);
                actor.Render(dt);
                actor.Villager?.GroundCorpseToTerrain(terrain.SampleVisualHeight, terrain.Revision);
                if (!sampled && actor.Goat != null && actor.Root.activeSelf)
                    actor.TerrainContactComplete = actor.Goat.GroundToTerrain(terrain.SampleVisualHeight);
                FractureIfDead(actor);
            }
            for (var i = dying.Count - 1; i >= 0; i--)
            {
                var actor = dying[i]; Place(actor, actor.To, actor.ToYaw, camera);
                if (Voxels.VoxelModernWorld.Requested)
                {
                    if (actor.Worm != null && !actor.Worm.HasRetainedLivePose)
                    {
                        // First observed after death: there is no live surface
                        // to fracture. Bound cleanup rather than retaining an
                        // unfulfillable debris request indefinitely.
                        actor.SetVisible(false); actor.Render(dt);
                        if (actor.Finished) { actor.Dispose(); dying.RemoveAt(i); }
                        continue;
                    }
                    FractureIfDead(actor); actor.SetVisible(false);
                    if (actor.VoxelDeath.Exploded) { actor.Dispose(); dying.RemoveAt(i); }
                    continue;
                }
                actor.SetVisible((actor.WasVisible || actor == followed) && ModernCapture.EntityVisibility != 1); actor.Render(dt);
                actor.Villager?.GroundCorpseToTerrain(terrain.SampleVisualHeight, terrain.Revision);
                FractureIfDead(actor);
                if (actor.Finished) { actor.Dispose(); dying.RemoveAt(i); }
            }
            foreach (var arrow in arrows.Values)
            {
                if (arrow.AlignAtBirth)
                {
                    arrow.AlignAtBirth = false;
                    if (actors.TryGetValue(arrow.State.parentIndex, out var actor) && actor.Archer != null &&
                        actor.State.sourceId == arrow.State.parentId && actor.State.slotGeneration == arrow.State.parentGeneration)
                        arrow.BirthOffset = Delta(arrow.From, actor.Archer.HeldNockPosition);
                    arrow.BornAt = Time.timeAsDouble;
                }
                var position = ArrowPosition(arrow);
                var offsetWeight = 1 - Mathf.Clamp01((float)(Time.timeAsDouble - arrow.BornAt) * 30);
                position += arrow.BirthOffset * offsetWeight;
                arrow.Root.transform.SetPositionAndRotation(Wrap(position, camera.transform.position), arrow.Rotation);
            }
        }

        private static void FractureIfDead(Actor actor)
        {
            if(!Voxels.VoxelModernWorld.Requested)return;
            actor.VoxelDeath.TryExplode(actor.VoxelIsDying, () => {
                if (Voxels.VoxelModernWorld.Active == null) return false;
                if (actor.Worm != null && !actor.Worm.HasRetainedLivePose) return false;
                if (actor.DeathFragments == null)
                {
                    actor.DeathFragments = actor.Goat != null ? actor.Goat.SnapshotVoxelFragments() : actor.Villager != null ? actor.Villager.SnapshotVoxelFragments() :
                        actor.Firefly != null ? actor.Firefly.SnapshotVoxelFragments() :
                        actor.Worm != null ? actor.Worm.SnapshotVoxelFragments(8192) : actor.Archer.SnapshotVoxelFragments();
                    actor.DeathVelocity = Delta(actor.From, actor.To) / Mathf.Max(actor.Interval, .0333f);
                    actor.DeathOrigin = actor.Root.transform.position;
                }
                var emitted = Voxels.VoxelModernWorld.Active.Explode(actor.DeathFragments, actor.DeathVelocity, actor.DeathOrigin,
                    actor.State.entityIndex, actor.State.model, actor.State.slotGeneration, actor.State.sourceId);
                if (emitted) actor.DeathFragments = null;
                return emitted;
            });
        }

        private void Retire(Actor actor)
        {
            if (Voxels.VoxelModernWorld.Requested)
            {
                // Disappearance alone is not an authoritative death signal.
                if (actor.VoxelDeath.RetainAfterRemoval(actor.VoxelIsDying)) dying.Add(actor);
                else actor.Dispose();
                return;
            }
            if (actor.IsDying) { actor.Remove(); dying.Add(actor); }
            else actor.Dispose();
        }

        private void PrepareVoxelDetails()
        {
            if(goatFine!=null)return;
            goatFine=LoadDetail("Goat",96);goatMiddle=LoadDetail("Goat",64);goatFar=LoadDetail("Goat",40);
            archerFine=LoadDetail("Archer",96);archerFar=LoadDetail("Archer",64);
        }

        private static Voxels.VoxelGoatAsset LoadDetail(string name, int resolution)
        {
            var asset = Resources.Load<Voxels.VoxelGoatAsset>($"ModernCharacters/{name}/Voxels/{name}{resolution}");
            Voxels.VoxelCharacterSkin.PrepareDetail(asset);
            return asset;
        }

        private VillagerArt PrepareVillagerArt(ModernVillagerAppearances.Binding binding)
        {
            if (!villagerArt.TryGetValue(binding.Parameter, out var art))
            {
                art = new VillagerArt { Asset = Load(binding.Resource) };
                if (Voxels.VoxelModernWorld.Requested)
                {
                    art.Fine = LoadDetail(binding.Resource, 96);
                    art.Far = LoadDetail(binding.Resource, 64);
                }
                villagerArt.Add(binding.Parameter, art);
            }
            return art;
        }
        private void SetVoxelDetail(Actor actor,float distance)
        {
            if (actor.Worm != null) return; // Shared64-resolution body meshes; no sixteen-way LOD churn.
            int current=actor.Goat!=null?actor.Goat.VoxelResolution:actor.Villager!=null?actor.Villager.VoxelResolution:actor.Firefly!=null?actor.Firefly.VoxelResolution:actor.Archer.VoxelResolution;
            bool fine=distance<(current==96?16:12);
            if(actor.Goat!=null)actor.Goat.SetVoxelDetail(fine?goatFine:distance<(current==40?32:40)?goatMiddle:goatFar);
            else if(actor.Villager!=null)
            {
                var art = villagerArt[actor.State.renderParameter];
                actor.Villager.SetVoxelDetail(fine ? art.Fine : art.Far);
            }
            else if (actor.Firefly != null) actor.Firefly.SetVoxelDetail(fine ? fireflyArt.Fine : fireflyArt.Far);
            else actor.Archer.SetVoxelDetail(fine?archerFine:archerFar);
        }

        private void Place(Actor actor, Vector3 position, float yaw, Camera camera)
        {
            if (actor.Worm != null)
            {
                PlaceWorm(actor, position, camera.transform.position);
                return;
            }
            if (actor.Firefly != null)
            {
                position.y += actor.Firefly.PivotAboveBase;
                actor.Root.transform.SetPositionAndRotation(Wrap(position, camera.transform.position), InterpolatedRotation(actor));
                return;
            }
            position.y = terrain.SampleVisualHeight(position.x, position.z);
            var e = .65f;
            var normal = new Vector3(terrain.SampleHeight(position.x - e, position.z) - terrain.SampleHeight(position.x + e, position.z),
                2 * e, terrain.SampleHeight(position.x, position.z - e) - terrain.SampleHeight(position.x, position.z + e)).normalized;
            actor.TerrainSlope = Vector3.Angle(Vector3.up, normal);
            // Goats traverse the original steep hills. The former shared35°
            // cap left their hind legs unable to reach a ~51° ground plane.
            var maximumTilt = actor.Goat != null ? 65f : 35f;
            if (actor.TerrainSlope > maximumTilt) normal = Vector3.RotateTowards(Vector3.up, normal, maximumTilt * Mathf.Deg2Rad, 0);
            var rotation = Quaternion.FromToRotation(Vector3.up, normal) * Quaternion.Euler(0, yaw, 0);
            actor.Root.transform.SetPositionAndRotation(Wrap(position, camera.transform.position), rotation);
        }
        private static void PlaceWorm(Actor actor, Vector3 position, Vector3 reference)
        {
            var height = actor.State.renderHeight / 64f;
            position.y -= height * actor.Worm.HeadBottomToHeight;
            actor.Worm.PlaceHead(Wrap(position, reference), InterpolatedRotation(actor),
                height * actor.Worm.HeadWidthToHeight, height, height * actor.Worm.HeadDepthToHeight);
            foreach (var part in actor.Parts.Values)
            {
                var weight = Mathf.Clamp01((float)(Time.timeAsDouble - part.ObservedAt) / part.Interval);
                var center = part.From + Delta(part.From, part.To) * weight;
                var bodyHeight = part.State.renderHeight / 64f;
                center.y -= bodyHeight * actor.Worm.SegmentBottomToHeight;
                actor.Worm.PlaceSegment(part.State.entityIndex, part.State.slotGeneration,
                    Wrap(center, reference), Quaternion.Slerp(part.FromRotation, part.ToRotation, weight),
                    bodyHeight * actor.Worm.SegmentWidthToHeight, bodyHeight, bodyHeight * actor.Worm.SegmentDepthToHeight);
            }
        }
        private static Vector3 Interpolated(Actor actor) => actor.From + Delta(actor.From, actor.To) *
            Mathf.Clamp01((float)(Time.timeAsDouble - actor.ObservedAt) / actor.Interval);
        private static float InterpolatedYaw(Actor actor) => Mathf.LerpAngle(actor.FromYaw, actor.ToYaw,
            Mathf.Clamp01((float)(Time.timeAsDouble - actor.ObservedAt) / actor.Interval));
        private static Quaternion Rotation(NativeSim.CampaignCharacterState state) => state.model == 3
            ? ModernWormPresenter.SourceRotation(state.yaw)
            : Quaternion.Euler(0, 180f - state.yaw * 360f / 2048f, 0);
        private static Quaternion InterpolatedRotation(Actor actor) => Quaternion.Slerp(actor.FromRotation, actor.ToRotation,
            Mathf.Clamp01((float)(Time.timeAsDouble - actor.ObservedAt) / actor.Interval));
        private static Vector3 ArrowPosition(Arrow arrow) => arrow.From + Delta(arrow.From, arrow.To) *
            Mathf.Clamp01((float)(Time.timeAsDouble - arrow.ObservedAt) / arrow.Interval);
        internal static Vector3 Direction(short yaw, short pitch)
        {
            var rotation = Quaternion.Euler(pitch * 360f / 2048, 180f - yaw * 360f / 2048, 0);
            return rotation * Vector3.forward;
        }
        private static Vector3 Position(NativeSim.CampaignCharacterState state) => new Vector3(state.x / 64f, state.z / 64f, state.y / 64f);
        private static Vector3 Delta(Vector3 from, Vector3 to) => new Vector3(Mathf.Repeat(to.x - from.x + 512, 1024) - 512,
            to.y - from.y, Mathf.Repeat(to.z - from.z + 512, 1024) - 512);
        private static Vector3 Wrap(Vector3 position, Vector3 camera) => camera + Delta(camera, position);
        private static ModernCharacterAsset Load(string name) => Resources.Load<ModernCharacterAsset>("ModernCharacters/" + name + "/Character")
            ?? throw new InvalidOperationException("Missing imported live character " + name);

        [Serializable] private sealed class FrameReport {
            public ulong generation, tick;
            public bool spectatorVisibilityOverride;
            public bool framingChecked, framingValid;
            public Vector3 cameraPosition;
            public Quaternion cameraRotation;
            public float cameraFieldOfView;
            public ActorReport[] actors;
            public ArrowReport[] arrows;
        }
        [Serializable] private sealed class ActorReport {
            public ushort slot; public uint epoch; public byte model, action; public int life;
            public ushort appearance; public string asset; public int voxelResolution;
            public string clip; public float clipTime; public Vector3 source, rendered; public bool nativePresent;
            public bool visible; public Vector3 up, heldNock, heldDirection;
            public float[] hoofClearances; public int hoofStanceMask;
            public bool terrainContactComplete;
            public float terrainSlope;
            public bool surfaceClearanceChecked;
            public float surfaceGroundClearance;
            public bool movingHit;
            public int contactBoundMask, contactReachMask; public float[] contactBefore, contactRequested;
        }
        [Serializable] private sealed class ArrowReport {
            public ushort slot, parent; public uint epoch; public ushort age;
            public short yaw, pitch; public Vector3 source, rendered, forward;
            public Vector3 viewportNock, viewportTip;
        }
        private string Report() => JsonUtility.ToJson(new FrameReport {
            generation = generation, tick = tick, spectatorVisibilityOverride = follow != null,
            framingChecked=follow != null, framingValid=FollowedFramingValid(),
            cameraPosition=renderedCamera != null ? renderedCamera.transform.position : Vector3.zero,
            cameraRotation=renderedCamera != null ? renderedCamera.transform.rotation : Quaternion.identity,
            cameraFieldOfView=renderedCamera != null ? renderedCamera.fieldOfView : 0,
            actors = actors.Values.Concat(dying).Select(a => new ActorReport { slot=a.State.entityIndex,
                epoch=a.State.slotGeneration, model=a.State.model, action=a.State.action, life=a.State.life,
                appearance=a.State.renderParameter,
                asset=a.Villager != null ? ModernVillagerAppearances.Get(a.State.renderParameter).Resource : a.Goat != null ? "Goat" : a.Firefly != null ? "Firefly" : a.Worm != null ? "WormHead" : "Archer",
                voxelResolution=a.Villager != null ? a.Villager.VoxelResolution : a.Goat != null ? a.Goat.VoxelResolution : a.Firefly != null ? a.Firefly.VoxelResolution : a.Worm != null ? (Voxels.VoxelModernWorld.Requested ? 64 : 0) : a.Archer.VoxelResolution,
                clip=a.Clip,
                clipTime=a.ClipTime,
                source=Position(a.State), rendered=a.Root.transform.position,
                nativePresent=actors.TryGetValue(a.State.entityIndex, out var occupant) && occupant == a,
                visible=a.Root.activeSelf, up=a.Root.transform.up,
                hoofClearances=a.Goat != null && a == Followed() ? a.Goat.HoofClearances(terrain.SampleVisualHeight) : null,
                hoofStanceMask=a.Goat != null && a == Followed() ? a.Goat.HoofStanceMask() : 0,
                terrainContactComplete=a.TerrainContactComplete,
                terrainSlope=a.TerrainSlope,
                surfaceClearanceChecked=capture != null && a == Followed() && a.Villager != null && Time.frameCount % 15 == 0,
                surfaceGroundClearance=capture != null && a == Followed() && a.Villager != null && Time.frameCount % 15 == 0
                    ? a.Villager.SurfaceGroundClearance(terrain.SampleVisualHeight) : 0,
                movingHit=a.Goat != null ? a.Goat.MovingHitActive : a.Villager != null && a.Villager.MovingHit,
                contactBoundMask=a.Goat != null && a == Followed() ? a.Goat.ContactBoundMask : 0,
                contactReachMask=a.Goat != null && a == Followed() ? a.Goat.ContactReachMask : 0,
                contactBefore=a.Goat != null && a == Followed() ? a.Goat.ContactBefore : null,
                contactRequested=a.Goat != null && a == Followed() ? a.Goat.ContactRequested : null,
                heldNock=a.Archer != null ? a.Archer.HeldNockPosition : Vector3.zero,
                heldDirection=a.Archer != null ? a.Archer.HeldArrowDirection : Vector3.zero }).ToArray(),
            arrows=arrows.Values.Select(a=>new ArrowReport { slot=a.State.entityIndex, parent=a.State.parentIndex,
                epoch=a.State.slotGeneration, age=a.State.ageTicks, yaw=a.State.yaw, pitch=a.State.pitch,
                source=Position(a.State), rendered=a.Root.transform.position, forward=a.Root.transform.forward,
                viewportNock=renderedCamera.WorldToViewportPoint(a.Root.transform.position),
                viewportTip=renderedCamera.WorldToViewportPoint(a.Root.transform.position+a.Root.transform.forward*2) }).ToArray()
        });
        private void Clear()
        {
            foreach (var actor in actors.Values) actor.Dispose();
            foreach (var actor in dying) actor.Dispose();
            foreach (var arrow in arrows.Values) UnityEngine.Object.Destroy(arrow.Root);
            actors.Clear(); dying.Clear(); arrows.Clear(); observed.Clear();
            reviewPositioned = false;
            reviewDistance = 0;
            followedLifetime = null;
        }
        public void Dispose()
        {
            Clear();
            if (arrowTemplate != null) UnityEngine.Object.Destroy(arrowTemplate);
            if (capture != null) UnityEngine.Object.Destroy(capture);
        }
    }
}
