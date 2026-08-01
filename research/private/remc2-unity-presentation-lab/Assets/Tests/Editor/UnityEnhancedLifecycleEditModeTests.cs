#if UNITY_EDITOR
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.PresentationLab.Tests
{
    public sealed class UnityEnhancedLifecycleEditModeTests
    {
        [Test]
        public void OnlyEnhancedModeCanOpenTheRoot()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();

            Assert.Throws<System.InvalidOperationException>(() => root.OpenDiagnostic(PresentationModeId.ClassicHd));
            Assert.AreEqual(UnityEnhancedLifecycleState.Closed, root.State);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void OpeningBuildsOneExclusiveHierarchy()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();

            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.AreEqual(UnityEnhancedLifecycleState.Open, root.State);
            Assert.AreEqual(1, gameObject.GetComponents<UnityEnhancedInputAdapter>().Length);
            Assert.AreEqual(1, gameObject.GetComponents<UnityEnhancedPresentationAdapters>().Length);
            Assert.AreEqual(1, gameObject.GetComponentsInChildren<UnityEnhancedCameraRig>().Length);
            Assert.IsNotNull(gameObject.transform.Find("EnhancedWorldObjects"));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void SnapshotLifecycleRejectsStaleAndResetsGeneration()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot first = new UnityEnhancedWorldSnapshot
            {
                Generation = 4,
                Tick = 10,
                CameraPose = new UnityEnhancedCameraPose(Vector3.zero, Quaternion.identity, 70f),
                Entities = new[]
                {
                    Entity(4, 11, new Vector3(1f, 2f, 3f)),
                    Entity(4, 12, new Vector3(4f, 5f, 6f))
                }
            };

            Assert.IsTrue(root.ApplySnapshot(first));
            Assert.AreEqual(2, root.ActiveEntityCount);

            UnityEnhancedWorldSnapshot stale = new UnityEnhancedWorldSnapshot
            {
                Generation = 4,
                Tick = 9,
                Entities = new[] { Entity(4, 11, Vector3.one) }
            };
            Assert.IsFalse(root.ApplySnapshot(stale));
            Assert.AreEqual(2, root.ActiveEntityCount);

            UnityEnhancedWorldSnapshot reset = new UnityEnhancedWorldSnapshot
            {
                Generation = 5,
                Tick = 1,
                Reset = true,
                Entities = new[] { Entity(5, 11, Vector3.one) }
            };
            Assert.IsTrue(root.ApplySnapshot(reset));
            Assert.AreEqual(1, root.ActiveEntityCount);
            Assert.AreEqual(5UL, root.Generation);
            Assert.AreEqual(1UL, root.LastTick);
            Assert.GreaterOrEqual(root.PooledEntityCount, 1);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void PresentationTransformsCanInterpolateWithoutChangingSnapshotTick()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 1,
                Entities = new[] { Entity(1, 5, Vector3.zero) }
            }));
            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 2,
                Entities = new[] { Entity(1, 5, new Vector3(10f, 0f, 0f)) }
            }));

            Transform entity = gameObject.transform.Find("EnhancedWorldObjects/EnhancedEntity_0000000000000005");
            Assert.IsNotNull(entity);
            root.InterpolatePresentation(0.5f);
            Assert.That(entity.position.x, Is.EqualTo(5f).Within(0.001f));
            Assert.AreEqual(2UL, root.LastTick);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void DuplicateOrWrongGenerationRecordsFailClosed()
        {
            GameObject gameObject = new GameObject("enhanced-root-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            UnityEnhancedWorldSnapshot duplicate = new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 1,
                Entities = new[] { Entity(2, 7, Vector3.zero), Entity(2, 7, Vector3.one) }
            };
            Assert.IsFalse(root.ApplySnapshot(duplicate));
            Assert.AreEqual(UnityEnhancedLifecycleState.Faulted, root.State);

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void InputIsSequenceBoundAndConsumedOnce()
        {
            GameObject gameObject = new GameObject("enhanced-input-test");
            UnityEnhancedInputAdapter adapter = gameObject.AddComponent<UnityEnhancedInputAdapter>();
            UnityEnhancedInputFrame frame = new UnityEnhancedInputFrame(3, Vector2.right, Vector2.up, false, true, 2);
            UnityEnhancedInputFrame consumed;

            Assert.IsTrue(adapter.Publish(frame));
            Assert.IsFalse(adapter.Publish(frame));
            Assert.IsTrue(adapter.TryConsume(out consumed));
            Assert.AreEqual(3UL, consumed.Sequence);
            Assert.IsTrue(consumed.Cast);
            Assert.IsFalse(adapter.TryConsume(out consumed));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ClosingAndReopeningStartsAResetInputSession()
        {
            GameObject gameObject = new GameObject("enhanced-input-session-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.InputAdapter.Publish(new UnityEnhancedInputFrame(3, Vector2.zero, Vector2.zero, false, false, 0)));
            root.Close();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.InputAdapter.Publish(new UnityEnhancedInputFrame(1, Vector2.zero, Vector2.zero, false, false, 0)));
            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void ReusedPoolObjectsStartAtTheNewGenerationPose()
        {
            GameObject gameObject = new GameObject("enhanced-pool-session-test");
            UnityEnhancedWorldRoot root = gameObject.AddComponent<UnityEnhancedWorldRoot>();
            root.OpenDiagnostic(PresentationModeId.UnityEnhanced);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 1,
                Tick = 1,
                Entities = new[] { Entity(1, 20, new Vector3(100f, 0f, 0f)) }
            }));
            root.InterpolatePresentation(1f);

            Assert.IsTrue(root.ApplySnapshot(new UnityEnhancedWorldSnapshot
            {
                Generation = 2,
                Tick = 1,
                Reset = true,
                Entities = new[] { Entity(2, 21, Vector3.zero) }
            }));
            Transform entity = gameObject.transform.Find("EnhancedWorldObjects/EnhancedEntity_0000000000000015");
            Assert.IsNotNull(entity);
            root.InterpolatePresentation(0f);
            Assert.That(entity.position, Is.EqualTo(Vector3.zero));

            Object.DestroyImmediate(gameObject);
        }

        [Test]
        public void PresentationAdaptersRejectDuplicateEvents()
        {
            GameObject gameObject = new GameObject("enhanced-adapter-test");
            UnityEnhancedPresentationAdapters adapters = gameObject.AddComponent<UnityEnhancedPresentationAdapters>();
            UnityEnhancedPresentationEvent presentationEvent = new UnityEnhancedPresentationEvent(9, 2, Vector3.zero);

            Assert.IsTrue(adapters.AcceptEvent(presentationEvent));
            Assert.IsFalse(adapters.AcceptEvent(presentationEvent));
            adapters.Reset(8);
            adapters.ApplyTick(8, 12);
            Assert.AreEqual(8UL, adapters.CurrentGeneration);
            Assert.AreEqual(12UL, adapters.LastTick);

            Object.DestroyImmediate(gameObject);
        }

        private static UnityEnhancedEntitySnapshot Entity(ulong generation, ulong identity, Vector3 position)
        {
            return new UnityEnhancedEntitySnapshot
            {
                Generation = generation,
                StableIdentity = identity,
                Position = position,
                Rotation = Quaternion.identity,
                Visible = true
            };
        }
    }
}
#endif
