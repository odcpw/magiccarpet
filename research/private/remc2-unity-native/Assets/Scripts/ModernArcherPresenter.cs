using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;
using MagicCarpet.Native.Voxels;

namespace MagicCarpet.Native
{
    // Presentation follows source observations; it never creates gameplay shots.
    internal sealed class ModernArcherPresenter : IDisposable
    {
        private const float SimulationHz = 30f;
        private const float ModelWalkTravel = 1.65f;
        private const float DeathHoldSeconds = .25f;
        private readonly ModernCharacterInstance character;
        private VoxelCharacterSkin voxelSkin;
        private readonly Transform visual, spine, bow, arrow;
        private readonly SkinnedMeshRenderer bowstring;
        private readonly Transform[] animatedNodes;
        private readonly Vector3[] recoveryPositions, recoveryScales;
        private readonly Quaternion[] recoveryRotations;
        private readonly SkinnedMeshRenderer[] shapedRenderers;
        private readonly float[][] recoveryShapes;
        private readonly Quaternion visualBindRotation;
        private readonly float visualScale, attackYaw;
        private readonly Vector3 arrowLocalForward;
        private bool observed, removed, wasAttacking, kneeling, awaitingConfirmedShot;
        private ulong lastTick;
        private int lastLife;
        private byte action, phase;
        private Vector3 aimDirection, lastPosition;
        private Vector3 releasedNock, releasedDirection;
        private bool hasPosition;
        private float withinTick, recovery = -1, hitRemaining, deathElapsed, transitionElapsed = -1;
        private float recoveryBlend = -1;
        private float recoveryBlendDuration = .18f;
        private bool recoveryPreservesWalkingLegs;
        private bool transitionToKneel;

        internal GameObject Root => character.Root;
        internal bool IsDying { get; private set; }
        internal bool DeathFinished => IsDying && deathElapsed >= character.Asset.Clip("Death").length + DeathHoldSeconds;
        internal string CurrentClip => character.CurrentClip;
        internal float ClipTime => character.ClipTime;
        internal Vector3 HeldNockPosition => recovery >= 0 ? releasedNock : NockPosition(bowstring);
        internal Vector3 HeldArrowDirection => recovery >= 0 ? releasedDirection : arrow.TransformDirection(arrowLocalForward).normalized;

        internal ModernArcherPresenter(ModernCharacterAsset asset, Transform parent, float sourceHeight)
        {
            if (asset == null) throw new ArgumentNullException(nameof(asset));
            if (!Finite(sourceHeight) || sourceHeight <= 0) throw new ArgumentOutOfRangeException(nameof(sourceHeight));
            if (!Finite(asset.height) || asset.height <= 0) throw new InvalidOperationException("Archer asset has no valid height");
            foreach (var clip in new[] { "Idle", "Walk", "StandingShot", "KneelEnter", "KneelingShot", "KneelExit", "Hit", "Death" }) asset.Clip(clip);
            visualScale = sourceHeight / asset.height;
            character = new ModernCharacterInstance(asset, parent);
            Root.transform.localScale = Vector3.one * visualScale;
            visual = Root.transform.GetChild(0);
            visualBindRotation = visual.localRotation;
            var transforms = Root.GetComponentsInChildren<Transform>(true);
            spine = transforms.Single(node => node.name == "Spine02");
            bow = transforms.Single(node => node.name == "BowControl");
            arrow = transforms.Single(node => node.name == "ArrowControl");
            animatedNodes = transforms.Where(node => node != Root.transform).ToArray();
            recoveryPositions = new Vector3[animatedNodes.Length];
            recoveryRotations = new Quaternion[animatedNodes.Length];
            recoveryScales = new Vector3[animatedNodes.Length];
            shapedRenderers = Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .Where(node => node.sharedMesh != null && node.sharedMesh.blendShapeCount > 0).ToArray();
            recoveryShapes = shapedRenderers.Select(node => new float[node.sharedMesh.blendShapeCount]).ToArray();
            bowstring = Root.GetComponentsInChildren<SkinnedMeshRenderer>(true).Single(node => node.name == "Bowstring");
            // The authored archer stands side-on to its shot. Measure the
            // imported geometry rather than assuming either FBX axis sign.
            character.Sample("StandingShot", 3.20f);
            var start = arrow.position;
            character.Sample("StandingShot", 3.35f);
            var flight = Root.transform.InverseTransformVector(arrow.position - start);
            if (new Vector2(flight.x, flight.z).sqrMagnitude < .001f)
                throw new InvalidOperationException("Archer shot has no measurable flight direction");
            attackYaw = Mathf.Atan2(flight.x, flight.z) * Mathf.Rad2Deg;
            arrowLocalForward = arrow.InverseTransformDirection(Root.transform.TransformDirection(flight.normalized));
            character.Sample("Idle", 0);
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

        // Called on source ticks. aim points from the source archer to its
        // current target; confirmedArrowSpawn is an actual allocation event.
        internal void Observe(Vector3 worldPosition, float yawDegrees, int life, byte sourceAction,
            byte sourcePhase, ulong tick, Vector3 aim, bool confirmedArrowSpawn, bool sourceKneeling)
        {
            if (removed) throw new InvalidOperationException("A removed archer cannot accept observations");
            if (observed && tick < lastTick) throw new ArgumentOutOfRangeException(nameof(tick));
            if (!Finite(worldPosition.x) || !Finite(worldPosition.y) || !Finite(worldPosition.z) ||
                !Finite(yawDegrees) || !Finite(aim.x) || !Finite(aim.y) || !Finite(aim.z))
                throw new ArgumentOutOfRangeException(nameof(worldPosition));
            if (observed && tick == lastTick) return;
            // The phase byte is observed after the source pass.  If a firing
            // cadence elapsed without an observed child allocation, retain the
            // drawn arrow; wrapping to the next reach would imply a release
            // that never happened. A later confirmed child resumes recovery.
            if (observed && tick == lastTick + 1 && action == 34 && sourceAction == 34 &&
                phase % 30 == 0 && sourcePhase == phase + 1 && !confirmedArrowSpawn)
                awaitingConfirmedShot = true;
            if (sourceAction != 34 || confirmedArrowSpawn) awaitingConfirmedShot = false;
            if (sourceAction == 36 || sourceAction == 37 || life < 0) BeginDeath();
            else if (observed && life < lastLife && !IsDying)
            {
                RememberRecoveryPose();
                recovery = -1;
                recoveryBlend = 0;
                hitRemaining = character.Asset.Clip("Hit").length;
                character.Play("Hit", .06f, true);
            }
            if (confirmedArrowSpawn && !IsDying) { recovery = 0; recoveryBlend = -1; hitRemaining = 0; }
            var attacking = sourceAction == 34;
            // A newly observed source may already be kneeling.  Entering the
            // stance is animated only when a previous observation established
            // a standing pose; loading/camera admission must not invent one.
            if (observed && attacking && (!wasAttacking || sourceKneeling != kneeling) && sourceKneeling)
            {
                RememberRecoveryPose();
                recoveryBlend = 0;
                transitionToKneel = true;
                transitionElapsed = 0;
            }
            else if (wasAttacking && kneeling && (!attacking || !sourceKneeling))
            {
                RememberRecoveryPose();
                recoveryBlend = 0;
                transitionToKneel = false;
                transitionElapsed = 0;
            }
            else if (wasAttacking && !kneeling && !attacking && !IsDying &&
                hitRemaining <= 0 && recovery < 0 && !confirmedArrowSpawn)
            {
                // Aim is applied after clip sampling, so the animator's own
                // crossfade cannot preserve the previous aimed upper body.
                RememberRecoveryPose(.28f, true);
                recoveryBlend = 0;
            }
            observed = true;
            lastTick = tick;
            lastLife = life;
            action = sourceAction;
            phase = sourcePhase;
            aimDirection = aim;
            kneeling = sourceKneeling;
            wasAttacking = attacking;
            withinTick = 0;
        }

        internal void MarkRemoved() { removed = true; BeginDeath(); }

        private void BeginDeath()
        {
            if (IsDying) return;
            IsDying = true;
            hitRemaining = 0;
            if (observed)
            {
                RememberRecoveryPose();
                recoveryBlend = 0;
            }
            else recoveryBlend = -1;
            deathElapsed = 0;
            character.Play("Death", .06f);
        }

        // The integrator places the camera-wrapped root before calling Render.
        internal void Render(float dt)
        {
            if (!Finite(dt) || dt < 0) throw new ArgumentOutOfRangeException(nameof(dt));
            var position = Root.transform.position;
            var distance = hasPosition ? HorizontalTravel(lastPosition, position) : 0;
            lastPosition = position;
            hasPosition = true;
            visual.localRotation = visualBindRotation;
            withinTick = Mathf.Min(1, withinTick + dt * SimulationHz);
            if (IsDying)
            {
                deathElapsed += dt;
                character.Tick(dt, dt);
                BlendRecoveryPose(dt);
                return;
            }
            if (hitRemaining > 0 && recovery < 0)
            {
                character.Play("Hit", .06f, false);
                hitRemaining = Mathf.Max(0, hitRemaining - dt);
                character.Tick(dt, dt);
                BlendRecoveryPose(dt);
                if (hitRemaining == 0)
                {
                    RememberRecoveryPose();
                    recoveryBlend = 0;
                }
                return;
            }
            if (recovery >= 0)
            {
                var clip = kneeling ? "KneelingShot" : "StandingShot";
                var release = kneeling ? 1.05f : 3.15f;
                if (recovery == 0)
                {
                    // The exported curves include quarter-frame contact keys.
                    // Sample one such interval before the release key so the
                    // nock is still on the drawing hand, not interpolating
                    // toward the relaxed string.
                    character.Sample(clip, release - 1f / 96f);
                    Aim(1);
                    releasedNock = NockPosition(bowstring);
                    releasedDirection = arrow.TransformDirection(arrowLocalForward).normalized;
                }
                character.Sample(clip, release + recovery * 3f);
                Aim(1);
                arrow.localScale = Vector3.zero;
                recovery += dt;
                if (recovery >= .20f)
                {
                    RememberRecoveryPose();
                    recovery = -1;
                    recoveryBlend = 0;
                }
                return;
            }
            if (transitionElapsed >= 0)
            {
                const float transitionDuration = .5f;
                var clip = transitionToKneel ? "KneelEnter" : "KneelExit";
                character.Sample(clip, character.Asset.Clip(clip).length * Mathf.Clamp01(transitionElapsed / transitionDuration));
                if (transitionToKneel) Aim(Mathf.Clamp01(transitionElapsed / transitionDuration));
                BlendRecoveryPose(dt);
                transitionElapsed += dt;
                if (transitionElapsed >= transitionDuration) transitionElapsed = -1;
                return;
            }
            if (action == 34 && aimDirection.sqrMagnitude > .000001f)
            {
                var remaining = phase <= 240 ? (30 - phase % 30) % 30 : 256 - phase;
                var interval = phase > 240 || phase == 0 ? 16f : 30f;
                var progress = awaitingConfirmedShot ? 1f : Mathf.Clamp01(1f - (remaining - withinTick) / interval);
                var clip = kneeling ? "KneelingShot" : "StandingShot";
                var release = kneeling ? 1.05f : 3.15f;
                var time = Mathf.Min(release - 1f / 96f, release * progress);
                character.Sample(clip, time);
                Aim(kneeling ? 1 : Mathf.SmoothStep(0, 1, Mathf.Clamp01((time - .7f) / .95f)));
                BlendRecoveryPose(dt);
                return;
            }
            if (distance <= .00001f)
            {
                character.Play("Idle", .12f, false);
                character.Tick(dt, dt);
            }
            else
            {
                character.Play("Walk", recoveryPreservesWalkingLegs && recoveryBlend >= 0 ? 0 : .08f, false);
                character.Tick(dt, distance / (ModelWalkTravel * visualScale));
            }
            BlendRecoveryPose(dt);
        }

        private void RememberRecoveryPose(float duration = .18f, bool preserveWalkingLegs = false)
        {
            recoveryBlendDuration = duration;
            recoveryPreservesWalkingLegs = preserveWalkingLegs;
            for (var i = 0; i < animatedNodes.Length; i++)
            {
                recoveryPositions[i] = animatedNodes[i].localPosition;
                recoveryRotations[i] = animatedNodes[i].localRotation;
                recoveryScales[i] = animatedNodes[i].localScale;
            }
            for (var i = 0; i < shapedRenderers.Length; i++)
                for (var shape = 0; shape < recoveryShapes[i].Length; shape++)
                    recoveryShapes[i][shape] = shapedRenderers[i].GetBlendShapeWeight(shape);
        }

        private void BlendRecoveryPose(float dt)
        {
            if (recoveryBlend < 0) return;
            recoveryBlend += dt;
            var weight = Mathf.SmoothStep(0, 1, Mathf.Clamp01(recoveryBlend / recoveryBlendDuration));
            for (var i = 0; i < animatedNodes.Length; i++)
            {
                var node = animatedNodes[i];
                // Blending a planted shot's leg bones with the walk pose
                // pushes the supporting sole below the floor. Keep the
                // authored walk lower body while joining the aimed torso.
                if (recoveryPreservesWalkingLegs && character.CurrentClip == "Walk" && IsWalkingLeg(node.name))
                    continue;
                node.localPosition = Vector3.Lerp(recoveryPositions[i], node.localPosition, weight);
                node.localRotation = Quaternion.Slerp(recoveryRotations[i], node.localRotation, weight);
                // The extracted arrow appears discretely, never as a growing
                // miniature between the release and quiver poses.
                node.localScale = node == arrow && weight < 1 ? Vector3.zero :
                    Vector3.Lerp(recoveryScales[i], node.localScale, weight);
            }
            for (var i = 0; i < shapedRenderers.Length; i++)
                for (var shape = 0; shape < recoveryShapes[i].Length; shape++)
                    shapedRenderers[i].SetBlendShapeWeight(shape,
                        Mathf.Lerp(recoveryShapes[i][shape], shapedRenderers[i].GetBlendShapeWeight(shape), weight));
            if (weight >= 1) recoveryBlend = -1;
        }

        private static bool IsWalkingLeg(string name) => name == "Armature" || name == "Hips" || name == "LeftUpLeg" ||
            name == "LeftLeg" || name == "LeftFoot" || name == "LeftToeBase" || name == "RightUpLeg" ||
            name == "RightLeg" || name == "RightFoot" || name == "RightToeBase";

        private void Aim(float weight)
        {
            if (aimDirection.sqrMagnitude < .000001f) return;
            var horizontal = new Vector3(aimDirection.x, 0, aimDirection.z);
            if (horizontal.sqrMagnitude >= .000001f)
            {
                var targetYaw = Mathf.Atan2(horizontal.x, horizontal.z) * Mathf.Rad2Deg;
                var correction = Mathf.DeltaAngle(Root.transform.eulerAngles.y, targetYaw) - attackYaw;
                visual.localRotation = Quaternion.Euler(0, correction * weight, 0) * visualBindRotation;
            }
            // The root may already be tilted to the terrain.  Correct the
            // imported shaft's actual world direction to the source direction,
            // rather than adding a pitch measured from world-horizontal to an
            // already pitched rig. Rotate the torso and the independent bow
            // rigidly about one pivot; the legs remain on the terrain.
            var shaft = arrow.TransformDirection(arrowLocalForward).normalized;
            var rotation = Quaternion.Slerp(Quaternion.identity,
                Quaternion.FromToRotation(shaft, aimDirection.normalized), weight);
            var pivot = spine.position;
            spine.rotation = rotation * spine.rotation;
            bow.SetPositionAndRotation(pivot + rotation * (bow.position - pivot), rotation * bow.rotation);
        }

        // Geometry template for the authoritative source child. Its origin is
        // the string nock and +Z is the direction of flight. Call once and
        // instantiate the result for live child rows; it has no gameplay logic.
        internal static GameObject CreateArrow(ModernCharacterAsset asset, Transform parent, float scale)
        {
            if (asset == null) throw new ArgumentNullException(nameof(asset));
            if (!Finite(scale) || scale <= 0) throw new ArgumentOutOfRangeException(nameof(scale));
            using var source = new ModernCharacterInstance(asset, null);
            var nodes = source.Root.GetComponentsInChildren<Transform>(true);
            var control = nodes.Single(node => node.name == "ArrowControl");
            var stringRenderer = source.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .Single(renderer => renderer.name == "Bowstring");
            source.Sample("StandingShot", 3.20f);
            var first = control.position;
            source.Sample("StandingShot", 3.35f);
            var direction = (control.position - first).normalized;
            if (direction.sqrMagnitude < .99f) throw new InvalidOperationException("Archer arrow has no flight direction");
            source.Sample("StandingShot", 2.35f);
            var nock = NockPosition(stringRenderer);
            var align = Quaternion.FromToRotation(direction, Vector3.forward);
            var result = new GameObject("Source archer arrow");
            result.transform.SetParent(parent, false);
            result.transform.localScale = Vector3.one * scale;
            var geometry = UnityEngine.Object.Instantiate(control.gameObject, result.transform);
            geometry.name = "Arrow geometry";
            geometry.transform.localPosition = align * (control.position - nock);
            geometry.transform.localRotation = align * control.rotation;
            geometry.transform.localScale = control.lossyScale;
            return result;
        }

        private static Vector3 NockPosition(SkinnedMeshRenderer renderer)
        {
            var bind = renderer.sharedMesh.vertices;
            var extent = renderer.sharedMesh.bounds.size;
            var axis = extent.x > extent.y && extent.x > extent.z ? 0 : extent.y > extent.z ? 1 : 2;
            var center = (bind.Max(v => v[axis]) + bind.Min(v => v[axis])) * .5f;
            var nearest = bind.Min(v => Mathf.Abs(v[axis] - center));
            var indices = Enumerable.Range(0, bind.Length)
                .Where(i => Mathf.Abs(bind[i][axis] - center) < nearest + .0001f).ToArray();
            var mesh = new Mesh();
            try
            {
                renderer.BakeMesh(mesh, true);
                var vertices = mesh.vertices;
                return indices.Aggregate(Vector3.zero,
                    (sum, i) => sum + renderer.transform.TransformPoint(vertices[i])) / indices.Length;
            }
            finally
            {
                if (Application.isPlaying) UnityEngine.Object.Destroy(mesh);
                else UnityEngine.Object.DestroyImmediate(mesh);
            }
        }

        private static float HorizontalTravel(Vector3 from, Vector3 to)
        {
            var dx = Mathf.Repeat(to.x - from.x + 512, 1024) - 512;
            var dz = Mathf.Repeat(to.z - from.z + 512, 1024) - 512;
            return Mathf.Sqrt(dx * dx + dz * dz);
        }

        private static bool Finite(float value) => !float.IsNaN(value) && !float.IsInfinity(value);
        public void Dispose() { voxelSkin?.Dispose(); character.Dispose(); }
    }
}
