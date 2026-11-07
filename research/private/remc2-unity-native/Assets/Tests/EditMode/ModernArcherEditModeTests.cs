using System.Linq;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernArcherEditModeTests
    {
        private static ModernCharacterAsset ArcherAsset()
        {
            var asset = Resources.Load<ModernCharacterAsset>("ModernCharacters/Archer/Character");
            Assert.That(asset, Is.Not.Null, "Prepare the owner-local archer asset first");
            return asset;
        }

        [TestCase("StandingShot", 2.35f, 3.20f, 3.35f)]
        [TestCase("KneelingShot", .60f, 1.10f, 1.25f)]
        public void LevelShotKeepsShaftAndFlightHorizontal(string clip, float draw, float launch, float flight)
        {
            var asset = ArcherAsset();
            using var character = new ModernCharacterInstance(asset, null);
            var shaft = character.Root.GetComponentsInChildren<MeshFilter>(true)
                .Single(filter => filter.name == "Arrow shaft");
            var arrow = character.Root.GetComponentsInChildren<Transform>(true)
                .Single(node => node.name == "ArrowControl");
            var size = shaft.sharedMesh.bounds.size;
            var localAxis = size.x > size.y && size.x > size.z ? Vector3.right :
                size.y > size.z ? Vector3.up : Vector3.forward;

            character.Sample(clip, draw);
            var drawnAxis = shaft.transform.TransformVector(localAxis).normalized;
            character.Sample(clip, launch);
            var launchAxis = shaft.transform.TransformVector(localAxis).normalized;
            var start = arrow.position;
            character.Sample(clip, flight);
            var displacement = arrow.position - start;

            TestContext.WriteLine($"{clip}: draw={drawnAxis}, launch={launchAxis}, flight={displacement}");
            Assert.That(Mathf.Abs(drawnAxis.y), Is.LessThan(.003f), "Drawn arrow must aim level");
            Assert.That(Mathf.Abs(launchAxis.y), Is.LessThan(.003f), "Released shaft must aim level");
            Assert.That(displacement.magnitude, Is.GreaterThan(.1f), "Released arrow must actually travel");
            Assert.That(Mathf.Abs(displacement.normalized.y), Is.LessThan(.01f), "Level arrow flight must stay level");
            Assert.That(Mathf.Abs(Vector3.Dot(launchAxis, displacement.normalized)), Is.GreaterThan(.99f),
                "Shaft and flight must agree");
        }

        [TestCase(.5f, 0f, 0f)]
        [TestCase(-.5f, 0f, 0f)]
        [TestCase(0f, 15f, 0f)]
        [TestCase(.5f, 15f, 0f)]
        [TestCase(-.5f, 15f, 0f)]
        [TestCase(.5f, 15f, 37f)]
        [TestCase(-.5f, 15f, -74f)]
        public void TargetElevationRotatesBowAndTorsoTogetherWithoutMovingFeet(float height, float slope, float yaw)
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            presenter.Root.transform.rotation = Quaternion.AngleAxis(slope, new Vector3(1, 0, .5f).normalized);
            var nodes = presenter.Root.GetComponentsInChildren<Transform>(true);
            var bow = nodes.Single(node => node.name == "BowControl");
            var hand = nodes.Single(node => node.name == "RightHand");
            var foot = nodes.Single(node => node.name == "LeftFoot");
            var shaft = presenter.Root.GetComponentsInChildren<MeshFilter>(true).Single(node => node.name == "Arrow shaft");
            var size = shaft.sharedMesh.bounds.size;
            var axis = size.x > size.y && size.x > size.z ? Vector3.right : size.y > size.z ? Vector3.up : Vector3.forward;
            var horizontal = Quaternion.Euler(0, yaw, 0) * Vector3.forward;
            presenter.Observe(Vector3.zero, 0, 100, 34, 24, 1, horizontal, false, false);
            presenter.Render(0);
            var contact = Vector3.Distance(bow.position, hand.position);
            var nockContact = Vector3.Distance(presenter.HeldNockPosition, hand.position);
            var planted = foot.position;
            var aim = horizontal + Vector3.up * height;
            presenter.Observe(Vector3.zero, 0, 100, 34, 24, 2, aim, false, false);
            presenter.Render(0);
            var shaftAxis = shaft.transform.TransformVector(axis).normalized;
            Assert.That(Mathf.Abs(Vector3.Dot(shaftAxis, aim.normalized)), Is.GreaterThan(.998f));
            Assert.That(Vector3.Distance(bow.position, hand.position), Is.EqualTo(contact).Within(.002f));
            Assert.That(Vector3.Distance(presenter.HeldNockPosition, hand.position), Is.EqualTo(nockContact).Within(.002f));
            Assert.That(Vector3.Dot(presenter.HeldArrowDirection, aim.normalized), Is.GreaterThan(.998f));
            Assert.That(Vector3.Distance(foot.position, planted), Is.LessThan(.001f));
        }

        [Test]
        public void EmbeddedArrowReleasesOnlyAfterTheSourceSpawn()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var arrow = presenter.Root.GetComponentsInChildren<Transform>(true).Single(node => node.name == "ArrowControl");
            presenter.Observe(Vector3.zero, 0, 100, 34, 0, 1, Vector3.forward, false, false);
            presenter.Render(0);
            Assert.That(arrow.localScale.magnitude, Is.GreaterThan(.1f), "An unconfirmed attempt must remain nocked");
            presenter.Observe(Vector3.zero, 0, 100, 34, 1, 2, Vector3.forward, true, false);
            presenter.Render(0);
            Assert.That(arrow.localScale, Is.EqualTo(Vector3.zero), "The actual source child takes over after release");
        }

        [Test]
        public void FailedOrUnobservedAllocationKeepsTheArrowDrawn()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var arrow = presenter.Root.GetComponentsInChildren<Transform>(true).Single(node => node.name == "ArrowControl");
            presenter.Observe(Vector3.zero, 0, 1000, 34, 30, 1, Vector3.forward, false, false);
            presenter.Render(0);
            var drawnTime = presenter.ClipTime;
            var drawnPosition = arrow.position;
            presenter.Observe(Vector3.zero, 0, 1000, 34, 31, 2, Vector3.forward, false, false);
            presenter.Render(1f / 30);
            Assert.That(presenter.ClipTime, Is.EqualTo(drawnTime).Within(.001f));
            Assert.That(Vector3.Distance(arrow.position, drawnPosition), Is.LessThan(.01f));
            Assert.That(arrow.localScale.magnitude, Is.GreaterThan(.1f));
        }

        [Test]
        public void FirstObservationInKneelingStanceStartsKneeling()
        {
            var asset = ArcherAsset();
            using var standing = new ModernCharacterInstance(asset, null);
            standing.Sample("Idle", 0);
            var standingHip = standing.Root.GetComponentsInChildren<Transform>(true)
                .Single(node => node.name == "Hips").position.y;
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            presenter.Observe(Vector3.zero, 0, 1000, 34, 24, 1, Vector3.forward, false, true);
            presenter.Render(0);
            var kneelingHip = presenter.Root.GetComponentsInChildren<Transform>(true)
                .Single(node => node.name == "Hips").position.y;
            Assert.That(presenter.CurrentClip, Is.EqualTo("KneelingShot"),
                "An existing kneeler must not enter the stance again on load or camera admission");
            Assert.That(kneelingHip, Is.LessThan(standingHip - .15f));
        }

        [Test]
        public void LeavingKneelingAimJoinsTheRiseWithoutTurningOrMovingTheBowAbruptly()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var nodes = presenter.Root.GetComponentsInChildren<Transform>(true);
            var hand = nodes.Single(node => node.name == "RightHand");
            var bow = nodes.Single(node => node.name == "BowControl");
            var body = presenter.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                presenter.Observe(Vector3.zero, 0, 1000, 34, 240, 1, Vector3.forward, false, true);
                presenter.Render(0);
                var handBefore = hand.position;
                var bowBefore = bow.position;
                var directionBefore = presenter.HeldArrowDirection;
                var handStep = 0f;
                var bowStep = 0f;
                var directionStep = 0f;
                var low = float.PositiveInfinity;
                var high = float.NegativeInfinity;
                for (var i = 1; i <= 22; i++)
                {
                    presenter.Observe(Vector3.zero, 0, 1000, i == 1 ? (byte)33 : (byte)39,
                        (byte)((240 + i) % 256), (ulong)i + 1, Vector3.forward, false, false);
                    presenter.Render(1f / 30);
                    var direction = presenter.HeldArrowDirection;
                    handStep = Mathf.Max(handStep, Vector3.Distance(handBefore, hand.position));
                    bowStep = Mathf.Max(bowStep, Vector3.Distance(bowBefore, bow.position));
                    directionStep = Mathf.Max(directionStep, Vector3.Angle(directionBefore, direction));
                    handBefore = hand.position; bowBefore = bow.position; directionBefore = direction;
                    body.BakeMesh(mesh, true);
                    var floor = mesh.vertices.Min(vertex => body.transform.TransformPoint(vertex).y);
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                }
                TestContext.WriteLine($"Rise hand step={handStep:R}; bow step={bowStep:R}; direction step={directionStep:R}°; floor={low:R}..{high:R}");
                Assert.That(handStep, Is.LessThan(.45f), "Drawing hand jumped at the start of the rise");
                Assert.That(bowStep, Is.LessThan(.45f), "Bow jumped at the start of the rise");
                Assert.That(directionStep, Is.LessThan(50f), "Bow turned abruptly at the start of the rise");
                Assert.That(low, Is.GreaterThan(-.001f), "Rise transition penetrates the floor");
                Assert.That(high, Is.LessThan(.005f), "Rise transition loses supporting contact");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [TestCase(false)]
        [TestCase(true)]
        public void LeavingStandingAimJoinsIdleOrWalkWithoutAPoseJump(bool moving)
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            using var control = new ModernCharacterInstance(asset, null);
            Assert.That(presenter.Root.activeInHierarchy, Is.True);
            var nodes = presenter.Root.GetComponentsInChildren<Transform>(true);
            var hand = nodes.Single(node => node.name == "RightHand");
            var bow = nodes.Single(node => node.name == "BowControl");
            var spine = nodes.Single(node => node.name == "Spine02");
            var arrow = nodes.Single(node => node.name == "ArrowControl");
            var body = presenter.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var controlBody = control.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            var controlMesh = new Mesh();
            try
            {
                presenter.Observe(Vector3.zero, 0, 1000, 34, 24, 1, Vector3.forward, false, false);
                presenter.Render(0);
                control.Sample("Walk", 0);
                var handBefore = hand.position;
                var bowBefore = bow.position;
                var directionBefore = presenter.HeldArrowDirection;
                var nockBefore = presenter.HeldNockPosition;
                var bowRotationBefore = bow.rotation;
                var spineRotationBefore = spine.rotation;
                var arrowVisibleBefore = arrow.lossyScale.sqrMagnitude > .001f;
                var handStep = 0f;
                var bowStep = 0f;
                var directionStep = 0f;
                var nockStep = 0f;
                var bowAngleStep = 0f;
                var spineAngleStep = 0f;
                var low = float.PositiveInfinity;
                var high = float.NegativeInfinity;
                for (var i = 1; i <= 22; i++)
                {
                    var position = moving ? Vector3.forward * (.03f * i) : Vector3.zero;
                    presenter.Root.transform.position = position;
                    presenter.Observe(position, 0, 1000, 39, (byte)(24 + i), (ulong)i + 1,
                        Vector3.forward, false, false);
                    presenter.Render(1f / 30);
                    control.Root.transform.position = position;
                    control.Sample("Walk", presenter.ClipTime);
                    var direction = presenter.HeldArrowDirection;
                    var arrowVisible = arrow.lossyScale.sqrMagnitude > .001f;
                    handStep = Mathf.Max(handStep, Vector3.Distance(handBefore, hand.position));
                    bowStep = Mathf.Max(bowStep, Vector3.Distance(bowBefore, bow.position));
                    if (arrowVisibleBefore && arrowVisible)
                        directionStep = Mathf.Max(directionStep, Vector3.Angle(directionBefore, direction));
                    nockStep = Mathf.Max(nockStep, Vector3.Distance(nockBefore, presenter.HeldNockPosition));
                    bowAngleStep = Mathf.Max(bowAngleStep, Quaternion.Angle(bowRotationBefore, bow.rotation));
                    spineAngleStep = Mathf.Max(spineAngleStep, Quaternion.Angle(spineRotationBefore, spine.rotation));
                    handBefore = hand.position; bowBefore = bow.position; directionBefore = direction;
                    nockBefore = presenter.HeldNockPosition;
                    bowRotationBefore = bow.rotation; spineRotationBefore = spine.rotation; arrowVisibleBefore = arrowVisible;
                    body.BakeMesh(mesh, true);
                    var floor = mesh.vertices.Min(vertex => body.transform.TransformPoint(vertex).y);
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                    if (floor < -.001f || floor > .005f)
                    {
                        controlBody.BakeMesh(controlMesh, true);
                        var controlFloor = controlMesh.vertices.Min(vertex => controlBody.transform.TransformPoint(vertex).y);
                        TestContext.WriteLine($"Standing exit moving={moving} frame={i} clip={presenter.CurrentClip} time={presenter.ClipTime:R} floor={floor:R}; ordinary walk time={control.ClipTime:R} floor={controlFloor:R}");
                        if (i == 1)
                        {
                            foreach (var part in new[] { "Hips", "LeftUpLeg", "LeftLeg", "LeftFoot", "RightUpLeg", "RightLeg", "RightFoot" })
                            {
                                var a = nodes.Single(node => node.name == part);
                                var b = control.Root.GetComponentsInChildren<Transform>(true).Single(node => node.name == part);
                                TestContext.WriteLine($"{part}: parent={a.parent.name}; local position difference={Vector3.Distance(a.localPosition, b.localPosition):R}; local angle={Quaternion.Angle(a.localRotation, b.localRotation):R}");
                            }
                        }
                    }
                }
                TestContext.WriteLine($"Standing exit moving={moving}; hand step={handStep:R}; bow step={bowStep:R}; nock step={nockStep:R}; visible direction step={directionStep:R}°; bow angle step={bowAngleStep:R}°; spine angle step={spineAngleStep:R}°; floor={low:R}..{high:R}");
                Assert.That(handStep, Is.LessThan(.45f), "Drawing hand jumped on leaving standing aim");
                Assert.That(bowStep, Is.LessThan(.45f), "Bow jumped on leaving standing aim");
                Assert.That(nockStep, Is.LessThan(.45f), "Nock jumped on leaving standing aim");
                Assert.That(directionStep, Is.LessThan(50f), "Bow turned abruptly on leaving standing aim");
                Assert.That(bowAngleStep, Is.LessThan(50f), "Bow rotated abruptly on leaving standing aim");
                Assert.That(spineAngleStep, Is.LessThan(50f), "Spine rotated abruptly on leaving standing aim");
                Assert.That(low, Is.GreaterThan(-.001f), "Standing exit penetrates the floor");
                Assert.That(high, Is.LessThan(.005f), "Standing exit loses supporting contact");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); UnityEngine.Object.DestroyImmediate(controlMesh); }
        }

        [Test]
        public void RepeatedDamageRestartsTheHitReaction()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            presenter.Observe(Vector3.zero, 0, 100, 33, 1, 1, Vector3.zero, false, false);
            presenter.Observe(Vector3.zero, 0, 90, 33, 2, 2, Vector3.zero, false, false);
            presenter.Render(.15f);
            Assert.That(presenter.CurrentClip, Is.EqualTo("Hit"));
            Assert.That(presenter.ClipTime, Is.GreaterThan(.1f));
            presenter.Observe(Vector3.zero, 0, 80, 33, 3, 3, Vector3.zero, false, false);
            presenter.Render(.01f);
            Assert.That(presenter.CurrentClip, Is.EqualTo("Hit"));
            Assert.That(presenter.ClipTime, Is.LessThan(.03f));
        }

        [Test]
        public void RecoveryJoinsTheNextDrawWithoutAPoseJump()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var nodes = presenter.Root.GetComponentsInChildren<Transform>(true);
            var hand = nodes.Single(node => node.name == "RightHand");
            var bow = nodes.Single(node => node.name == "BowControl");
            var arrow = nodes.Single(node => node.name == "ArrowControl");
            presenter.Observe(Vector3.zero, 0, 100, 34, 0, 1, Vector3.forward, false, false);
            presenter.Render(0);
            var handBefore = hand.position;
            var bowBefore = bow.position;
            var handStep = 0f;
            var bowStep = 0f;
            for (var i = 1; i <= 20; i++)
            {
                presenter.Observe(Vector3.zero, 0, 100, 34, (byte)i, (ulong)i + 1,
                    Vector3.forward, i == 1, false);
                presenter.Render(1f / 30);
                handStep = Mathf.Max(handStep, Vector3.Distance(handBefore, hand.position));
                bowStep = Mathf.Max(bowStep, Vector3.Distance(bowBefore, bow.position));
                handBefore = hand.position;
                bowBefore = bow.position;
                var scale = arrow.localScale.magnitude;
                Assert.That(scale <= .003f || scale >= 1.7f, Is.True, "The hidden arrow must not grow into view");
            }
            TestContext.WriteLine($"Maximum recovery hand step={handStep:R}; bow step={bowStep:R}");
            Assert.That(handStep, Is.LessThan(.45f), "Drawing hand jumped between recovery and the next shot");
            Assert.That(bowStep, Is.LessThan(.45f), "Bow jumped between recovery and the next shot");
        }

        [Test]
        public void RecoveryAndNextDrawKeepASupportedSole()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var body = presenter.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                presenter.Observe(Vector3.zero, 0, 100, 34, 0, 1, Vector3.forward, false, false);
                presenter.Render(0);
                var floors = new float[21];
                for (var i = 0; i < floors.Length; i++)
                {
                    if (i > 0)
                    {
                        presenter.Observe(Vector3.zero, 0, 100, 34, (byte)i, (ulong)i + 1,
                            Vector3.forward, i == 1, false);
                        presenter.Render(1f / 30);
                    }
                    body.BakeMesh(mesh, true);
                    floors[i] = mesh.vertices.Min(vertex => body.transform.TransformPoint(vertex).y);
                }
                TestContext.WriteLine("Recovery sole heights: " + string.Join(", ", floors.Select(value => value.ToString("R"))));
                Assert.That(floors.Min(), Is.GreaterThan(-.001f), "Recovery penetrates the floor");
                Assert.That(floors.Max(), Is.LessThan(.005f), "Recovery loses supporting-foot contact");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void HitJoinsTheInterruptedShotWithoutAPoseJumpOrGroundLoss()
        {
            var asset = ArcherAsset();
            using var presenter = new ModernArcherPresenter(asset, null, asset.height);
            var nodes = presenter.Root.GetComponentsInChildren<Transform>(true);
            var hand = nodes.Single(node => node.name == "RightHand");
            var bow = nodes.Single(node => node.name == "BowControl");
            var body = presenter.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                presenter.Observe(Vector3.zero, 0, 1000, 34, 10, 1, Vector3.forward, false, false);
                presenter.Render(0);
                var handBefore = hand.position;
                var bowBefore = bow.position;
                var handStep = 0f;
                var bowStep = 0f;
                var low = float.PositiveInfinity;
                var high = float.NegativeInfinity;
                for (var i = 1; i <= 24; i++)
                {
                    presenter.Observe(Vector3.zero, 0, 750, 34, (byte)(10 + i), (ulong)i + 1,
                        Vector3.forward, false, false);
                    presenter.Render(1f / 30);
                    var handDelta = Vector3.Distance(handBefore, hand.position);
                    var bowDelta = Vector3.Distance(bowBefore, bow.position);
                    if (handDelta > .35f || bowDelta > .35f)
                        TestContext.WriteLine($"Hit transition frame={i} clip={presenter.CurrentClip} time={presenter.ClipTime:R} hand={handDelta:R} bow={bowDelta:R}");
                    handStep = Mathf.Max(handStep, handDelta);
                    bowStep = Mathf.Max(bowStep, bowDelta);
                    handBefore = hand.position;
                    bowBefore = bow.position;
                    body.BakeMesh(mesh, true);
                    var floor = mesh.vertices.Min(vertex => body.transform.TransformPoint(vertex).y);
                    low = Mathf.Min(low, floor); high = Mathf.Max(high, floor);
                }
                TestContext.WriteLine($"Hit hand step={handStep:R}; bow step={bowStep:R}; floor={low:R}..{high:R}");
                Assert.That(handStep, Is.LessThan(.45f), "Drawing hand jumped across a hit transition");
                Assert.That(bowStep, Is.LessThan(.45f), "Bow jumped across a hit transition");
                Assert.That(low, Is.GreaterThan(-.001f), "Hit transition penetrates the floor");
                Assert.That(high, Is.LessThan(.005f), "Hit transition loses supporting-foot contact");
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }

        [Test]
        public void SourceArrowTemplatePointsForwardFromItsNock()
        {
            var asset = ArcherAsset();
            var template = ModernArcherPresenter.CreateArrow(asset, null, 1);
            try
            {
                var shaft = template.GetComponentsInChildren<MeshFilter>(true).Single(node => node.name == "Arrow shaft");
                var vertices = shaft.sharedMesh.vertices.Select(shaft.transform.TransformPoint).ToArray();
                var back = vertices.Min(vertex => vertex.z);
                var front = vertices.Max(vertex => vertex.z);
                Assert.That(front - back, Is.GreaterThan(.9f));
                Assert.That(Mathf.Abs(back), Is.LessThan(.03f), "Template pivot must be at the source string nock");
                Assert.That(vertices.Max(vertex => Mathf.Abs(vertex.x)), Is.LessThan(.02f));
                Assert.That(vertices.Max(vertex => Mathf.Abs(vertex.y)), Is.LessThan(.02f));
            }
            finally { UnityEngine.Object.DestroyImmediate(template); }
        }

        [Test]
        public void DeathRemainsOnTheGroundThroughoutTheFall()
        {
            using var character = new ModernCharacterInstance(ArcherAsset(), null);
            var body = character.Root.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .OrderByDescending(renderer => renderer.sharedMesh.vertexCount).First();
            var mesh = new Mesh();
            try
            {
                var length = character.Asset.Clip("Death").length;
                var samples = Mathf.CeilToInt(length * 96);
                for (var i = 0; i <= samples; i++)
                {
                    character.Sample("Death", length * i / samples);
                    body.BakeMesh(mesh, true);
                    var floor = mesh.vertices.Min(vertex => body.transform.TransformPoint(vertex).y);
                    Assert.That(floor, Is.InRange(-.002f, .005f), $"Death sample {i} lost ground contact");
                }
            }
            finally { UnityEngine.Object.DestroyImmediate(mesh); }
        }
    }
}
