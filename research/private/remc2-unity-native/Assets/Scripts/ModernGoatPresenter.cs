using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native
{
    // Presentation only: the source entity owns motion, damage and lifetime.
    internal sealed class ModernGoatPresenter : IDisposable
    {
        private const float ModelHeight = .95917f;
        private const float ModelContactTravel = .16f;
        private const float WalkStance = .18f;
        private const float SprintStance = .075f;
        private const float WorldSize = 1024f;
        private const float DeathHoldSeconds = .25f;
        private const float MovingHitGraceSeconds = .08f;
        private readonly ModernCharacterInstance character;
        private VoxelCharacterSkin voxelSkin;
        private readonly float visualScale;
        private Vector3 lastRenderPosition;
        private bool hasRenderPosition, hasObservation, removed;
        private ulong lastTick;
        private int lastLife;
        private byte action;
        private float hitRemaining, deathElapsed, movingHitGrace;
        private bool restartHit;
        private Transform head;
        private Quaternion[] hitHeadDeltas;
        private static readonly Dictionary<ModernCharacterAsset, Quaternion[]> HitHeadCurves = new();
        private SkinnedMeshRenderer contactRenderer;
        private Mesh contactMesh;
        private int[][] hoofVertices;
        private Transform[] upperLegs, lowerLegs, hooves;
        private Vector3[][] hoofLocalPoints;
        private static readonly string[] LegNames = { "hind_L", "hind_R", "front_L", "front_R" };
        private static readonly float[] GaitOffsets = { 0f, .035f, .48f, .515f };
        internal int ContactBoundMask { get; private set; }
        internal int ContactReachMask { get; private set; }
        internal float[] ContactBefore { get; } = new float[4];
        internal float[] ContactRequested { get; } = new float[4];

        internal GameObject Root => character.Root;
        internal bool IsDying { get; private set; }
        internal bool MovingHitActive { get; private set; }
        internal bool DeathFinished => IsDying && deathElapsed >= character.Asset.Clip("Death").length + DeathHoldSeconds;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;

        internal ModernGoatPresenter(ModernCharacterAsset asset, Transform parent, float sourceHeight)
        {
            if (asset == null) throw new ArgumentNullException(nameof(asset));
            if (float.IsNaN(sourceHeight) || float.IsInfinity(sourceHeight) || sourceHeight <= 0)
                throw new ArgumentOutOfRangeException(nameof(sourceHeight));
            // Refuse mismatched revisions instead of silently using the old walk.
            foreach (var clip in new[] { "Idle", "Walk", "Sprint", "Hit", "Death" }) asset.Clip(clip);
            if (Mathf.Abs(asset.height - ModelHeight) > .001f)
                throw new InvalidOperationException("Goat clip geometry does not match the expected model height");
            visualScale = sourceHeight / asset.height;
            character = new ModernCharacterInstance(asset, parent);
            Root.transform.localScale = Vector3.one * visualScale;
        }

        internal void AttachVoxelSkin(VoxelGoatAsset skin)
        {
            if (voxelSkin != null) throw new InvalidOperationException("Voxel skin already attached");
            voxelSkin = new VoxelCharacterSkin(Root, skin);
        }

        internal int VoxelResolution => voxelSkin != null ? voxelSkin.Resolution : 0;

        internal void SetVoxelDetail(VoxelGoatAsset skin)
        {
            if (voxelSkin == null) throw new InvalidOperationException("Voxel skin not attached");
            voxelSkin.SetDetail(skin);
        }

        internal List<VoxelFragment> SnapshotVoxelFragments() =>
            voxelSkin != null ? voxelSkin.SnapshotFragments() : throw new InvalidOperationException("Voxel skin not attached");

        internal void Observe(Vector3 worldPosition, float yawDegrees, int life, byte sourceAction, ulong tick)
        {
            if (removed) throw new InvalidOperationException("A removed goat cannot accept a new source observation");
            if (hasObservation && tick < lastTick) throw new ArgumentOutOfRangeException(nameof(tick));
            if (!Finite(worldPosition.x) || !Finite(worldPosition.y) || !Finite(worldPosition.z) || !Finite(yawDegrees))
                throw new ArgumentOutOfRangeException(nameof(worldPosition));
            if (hasObservation && tick == lastTick) return;
            if (sourceAction == 12 || sourceAction == 13 || life < 0)
                BeginDeath();
            else if (hasObservation && life < lastLife && !IsDying)
            {
                hitRemaining = character.Asset.Clip("Hit").length;
                restartHit = true;
            }
            hasObservation = true;
            lastLife = life;
            lastTick = tick;
            action = sourceAction;
        }

        // The caller must distinguish actual death/removal from visibility culling.
        internal void MarkRemoved()
        {
            removed = true;
            BeginDeath();
        }

        private void BeginDeath()
        {
            if (IsDying) return;
            IsDying = true;
            hitRemaining = 0;
            deathElapsed = 0;
            character.Play("Death", .06f);
        }

        // Called after the integrator places the interpolated, camera-wrapped root.
        // Distance, rather than nominal speed or source action, drives hoof phase.
        internal void Render(float dt)
        {
            if (!Finite(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            MovingHitActive = false;
            var position = Root.transform.position;
            var distance = hasRenderPosition ? HorizontalTravel(lastRenderPosition, position) : 0;
            lastRenderPosition = position;
            hasRenderPosition = true;
            movingHitGrace = distance > .00001f ? MovingHitGraceSeconds : Mathf.Max(0, movingHitGrace - dt);
            if (IsDying)
            {
                deathElapsed += dt;
                character.Tick(dt, dt);
                return;
            }
            if (hitRemaining > 0)
            {
                var length = character.Asset.Clip("Hit").length;
                var elapsed = Mathf.Min(length, length - hitRemaining + dt);
                // A rendered frame can land exactly on an interpolation
                // boundary while the native goat is still travelling. Keep
                // the gait through that brief pause; its phase remains tied
                // to actual distance and does not advance when stationary.
                if (distance > .00001f || movingHitGrace > 0)
                {
                    TickGait(dt, distance);
                    ApplyHeadReaction(elapsed / length);
                    MovingHitActive = true;
                }
                else
                {
                    var restart = restartHit || CurrentClip != "Hit";
                    character.Play("Hit", .06f, restart);
                    character.Tick(dt, restart ? elapsed : dt);
                }
                restartHit = false;
                hitRemaining = Mathf.Max(0, hitRemaining - dt);
                return;
            }
            if (distance <= .00001f)
            {
                character.Play("Idle", .12f, false);
                character.Tick(dt, dt);
                return;
            }
            TickGait(dt, distance);
        }

        private void TickGait(float dt, float distance)
        {
            var sprint = action == 14;
            var clip = sprint ? "Sprint" : "Walk";
            character.Play(clip, .08f, false);
            var cycleTravel = ModelContactTravel / (sprint ? SprintStance : WalkStance) * visualScale;
            character.Tick(dt, distance / cycleTravel * character.Asset.Clip(clip).length);
        }

        private void ApplyHeadReaction(float progress)
        {
            if (head == null)
            {
                head = Root.GetComponentsInChildren<Transform>(true).Single(bone => bone.name == "head");
                if (!HitHeadCurves.TryGetValue(character.Asset, out hitHeadDeltas))
                {
                    hitHeadDeltas = SampleHitHeadCurve(character.Asset);
                    HitHeadCurves.Add(character.Asset, hitHeadDeltas);
                }
            }
            var index = Mathf.Clamp01(progress) * (hitHeadDeltas.Length - 1);
            var first = Mathf.FloorToInt(index);
            var second = Mathf.Min(first + 1, hitHeadDeltas.Length - 1);
            head.localRotation *= Quaternion.Slerp(hitHeadDeltas[first], hitHeadDeltas[second], index - first);
        }

        // Sample the actual imported Hit track once per asset. This retains the
        // authored recoil axis and amplitude after Unity's FBX conversion.
        private static Quaternion[] SampleHitHeadCurve(ModernCharacterAsset asset)
        {
            var sample = UnityEngine.Object.Instantiate(asset.model);
            sample.SetActive(false);
            try
            {
                var bone = sample.GetComponentsInChildren<Transform>(true).Single(value => value.name == "head");
                asset.Clip("Idle").SampleAnimation(sample, 0);
                var idle = bone.localRotation;
                var clip = asset.Clip("Hit");
                const int count = 33;
                var deltas = new Quaternion[count];
                for (var i = 0; i < count; i++)
                {
                    clip.SampleAnimation(sample, clip.length * i / (count - 1));
                    deltas[i] = Quaternion.Inverse(idle) * bone.localRotation;
                }
                return deltas;
            }
            finally
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(sample);
                else UnityEngine.Object.DestroyImmediate(sample);
            }
        }

        internal static float HorizontalTravel(Vector3 from, Vector3 to)
        {
            var dx = Mathf.Repeat(to.x - from.x + WorldSize / 2, WorldSize) - WorldSize / 2;
            var dz = Mathf.Repeat(to.z - from.z + WorldSize / 2, WorldSize) - WorldSize / 2;
            return Mathf.Sqrt(dx * dx + dz * dz);
        }

        // Capture-only measurement of the actual deformed rigid hoof soles.
        // Order: hind L, hind R, front L, front R. Positive means above terrain.
        // Call after Render/graph evaluation and the caller's terrain placement.
        internal float[] HoofClearances(Func<float, float, float> terrainHeight)
        {
            if (terrainHeight == null) throw new ArgumentNullException(nameof(terrainHeight));
            EnsureContactCache();
            contactRenderer.BakeMesh(contactMesh, true);
            var points = contactMesh.vertices;
            var clearances = new float[hoofVertices.Length];
            for (var leg = 0; leg < hoofVertices.Length; leg++)
            {
                var minimum = float.PositiveInfinity;
                foreach (var index in hoofVertices[leg])
                {
                    var world = contactRenderer.transform.TransformPoint(points[index]);
                    var ground = terrainHeight(world.x, world.z);
                    if (!Finite(ground)) throw new InvalidOperationException("Non-finite terrain contact height");
                    minimum = Mathf.Min(minimum, world.y - ground);
                }
                clearances[leg] = minimum;
            }
            return clearances;
        }

        private void EnsureContactCache()
        {
            if (contactRenderer != null) return;
            contactRenderer = voxelSkin != null ? voxelSkin.Source : Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var weights = contactRenderer.sharedMesh.boneWeights;
            var bones = contactRenderer.bones;
            Transform Bone(string name) => bones.FirstOrDefault(bone => bone.name == name)
                ?? throw new InvalidOperationException("Goat rig lacks " + name);
            upperLegs = LegNames.Select(name => Bone(name + "_upper")).ToArray();
            lowerLegs = LegNames.Select(name => Bone(name + "_lower")).ToArray();
            hooves = LegNames.Select(name => Bone(name + "_hoof")).ToArray();
            hoofVertices = hooves.Select(hoof => {
                var index = Array.IndexOf(bones, hoof);
                var vertices = Enumerable.Range(0, weights.Length)
                    .Where(i => WeightOfBone(weights[i], index) > .999f).ToArray();
                if (vertices.Length < 10) throw new InvalidOperationException("Goat rig lacks rigid vertices for " + hoof.name);
                return vertices;
            }).ToArray();
            contactMesh = new Mesh { name = "Goat contact sample" };
            contactRenderer.BakeMesh(contactMesh, true);
            var baked = contactMesh.vertices;
            // Rigid hoof vertices remain fixed in each hoof's local coordinates.
            // Cache once; gameplay then measures the real sole without rebaking
            // all 17,070 skinned vertices for every goat on every frame.
            hoofLocalPoints = hoofVertices.Select((indices, leg) => indices.Select(index =>
                hooves[leg].InverseTransformPoint(contactRenderer.transform.TransformPoint(baked[index]))).ToArray()).ToArray();
        }

        // Call after Render and terrain placement. Correct stance toward the
        // actual surface, and correct swing only when it would intersect it.
        // Never change the source-position root or stretch either bone. A false
        // result means the bounded/reachable chain could not attain a target.
        internal bool GroundToTerrain(Func<float, float, float> terrainHeight)
        {
            if (terrainHeight == null) throw new ArgumentNullException(nameof(terrainHeight));
            if (IsDying) return true; // Preserve the authored whole-body fall.
            EnsureContactCache();
            var complete = true;
            ContactBoundMask = 0;
            ContactReachMask = 0;
            for (var leg = 0; leg < hooves.Length; leg++)
            {
                var minimum = float.PositiveInfinity;
                foreach (var point in hoofLocalPoints[leg])
                {
                    var world = hooves[leg].TransformPoint(point);
                    var ground = terrainHeight(world.x, world.z);
                    if (!Finite(ground)) throw new InvalidOperationException("Non-finite terrain contact height");
                    minimum = Mathf.Min(minimum, world.y - ground);
                }
                var weight = ContactWeight(leg);
                var requested = Mathf.Max(-minimum, -minimum * weight);
                // Scale the bound with the actual two-bone chain, not a fixed
                // world distance. A sharp terrain step can require more than
                // the previous .12 model units while still being reachable.
                var chainLength = Vector3.Distance(upperLegs[leg].position, lowerLegs[leg].position) +
                    Vector3.Distance(lowerLegs[leg].position, hooves[leg].position);
                var maximumAdjustment = .75f * chainLength;
                var bounded = Mathf.Clamp(requested, -maximumAdjustment, maximumAdjustment);
                ContactBefore[leg] = minimum;
                ContactRequested[leg] = requested;
                if (Mathf.Abs(requested - bounded) > .001f) { complete = false; ContactBoundMask |= 1 << leg; }
                if (Mathf.Abs(bounded) > .00001f && !MoveHoofVertically(leg, bounded))
                { complete = false; ContactReachMask |= 1 << leg; }
            }
            return complete;
        }

        private float ContactWeight(int leg)
        {
            if (CurrentClip == "Idle" || CurrentClip == "Hit") return 1;
            if (CurrentClip != "Walk" && CurrentClip != "Sprint") return 0;
            var t = Mathf.Repeat(ClipTime / character.Asset.Clip(CurrentClip).length + GaitOffsets[leg], 1);
            var stance = CurrentClip == "Walk" ? WalkStance : SprintStance;
            if (t < stance) return 1;
            // Ease the correction into/out of stance rather than popping at
            // toe-off or landing; mid-flight retains its authored height.
            const float margin = .08f;
            return Mathf.Max(1 - Mathf.SmoothStep(0, 1, Mathf.Clamp01((t - stance) / margin)),
                Mathf.SmoothStep(0, 1, Mathf.Clamp01((t - (1 - margin)) / margin)));
        }

        private bool MoveHoofVertically(int leg, float amount)
        {
            var upper = upperLegs[leg]; var lower = lowerLegs[leg]; var hoof = hooves[leg];
            var hip = upper.position; var knee = lower.position; var ankle = hoof.position;
            var upperLength = Vector3.Distance(hip, knee); var lowerLength = Vector3.Distance(knee, ankle);
            if (upperLength < .0001f || lowerLength < .0001f) return false;
            var destination = ankle + Vector3.up * amount;
            var vector = destination - hip; var distance = vector.magnitude;
            if (distance < .0001f) return false;
            var direction = vector / distance;
            var reachable = Mathf.Clamp(distance, Mathf.Abs(upperLength - lowerLength) + .00001f,
                upperLength + lowerLength - .00001f);
            var complete = Mathf.Abs(reachable - distance) < .001f;
            destination = hip + direction * reachable;
            var along = (upperLength * upperLength - lowerLength * lowerLength + reachable * reachable) / (2 * reachable);
            var bend = knee - hip - direction * Vector3.Dot(knee - hip, direction);
            if (bend.sqrMagnitude < 1e-10f) return false;
            var joint = hip + direction * along + bend.normalized * Mathf.Sqrt(Mathf.Max(0, upperLength * upperLength - along * along));
            var hoofRotation = hoof.rotation;
            upper.rotation = Quaternion.FromToRotation(knee - hip, joint - hip) * upper.rotation;
            lower.rotation = Quaternion.FromToRotation(hoof.position - lower.position, destination - lower.position) * lower.rotation;
            hoof.rotation = hoofRotation;
            return complete;
        }

        // Bit order matches HoofClearances. During a crossfade this describes
        // the target clip phase; discard the blend interval for gait analysis.
        internal int HoofStanceMask()
        {
            if (CurrentClip == "Idle") return 15;
            if (CurrentClip != "Walk" && CurrentClip != "Sprint") return 0;
            var phase = Mathf.Repeat(ClipTime / character.Asset.Clip(CurrentClip).length, 1);
            var stance = CurrentClip == "Walk" ? WalkStance : SprintStance;
            var mask = 0;
            for (var leg = 0; leg < GaitOffsets.Length; leg++)
                if (Mathf.Repeat(phase + GaitOffsets[leg], 1) < stance) mask |= 1 << leg;
            return mask;
        }

        private static float WeightOfBone(BoneWeight value, int bone) =>
            (value.boneIndex0 == bone ? value.weight0 : 0) +
            (value.boneIndex1 == bone ? value.weight1 : 0) +
            (value.boneIndex2 == bone ? value.weight2 : 0) +
            (value.boneIndex3 == bone ? value.weight3 : 0);

        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
        public void Dispose()
        {
            if (contactMesh != null)
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(contactMesh);
                else UnityEngine.Object.DestroyImmediate(contactMesh);
                contactMesh = null;
            }
            voxelSkin?.Dispose();
            character.Dispose();
        }
    }
}
