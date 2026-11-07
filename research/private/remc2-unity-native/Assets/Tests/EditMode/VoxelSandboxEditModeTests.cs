using System;
using System.Linq;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelSandboxEditModeTests
    {
        [Serializable]
        private sealed class Level1Input
        {
            public ModernBuildingMeshes.Profile profile;
        }

        [TestCase(.5f)]
        [TestCase(1f / 3f)]
        [TestCase(.25f)]
        public void SourceHouseStartsFullySupportedAtEveryPreset(float cell)
        {
            var input = Resources.Load<TextAsset>("ModernCharacters/VoxelSandbox/Level1");
            Assert.That(input, Is.Not.Null, "Run Tools/prepare-voxel-inputs.py to generate the owner-local Level 1 input.");
            var profile = JsonUtility.FromJson<Level1Input>(input.text).profile;
            Assert.That(profile, Is.Not.Null);
            Assert.That(profile.build_index, Is.EqualTo(26));
            Assert.That(profile.columns, Is.EqualTo(5));
            Assert.That(profile.rows, Is.EqualTo(7));
            Assert.That(profile.heights.Length, Is.EqualTo(profile.columns * profile.rows));
            var size = new Vector3Int(
                Mathf.RoundToInt((profile.columns - 1) * 2f / cell),
                Mathf.CeilToInt(profile.heights.Max() * .5f / cell),
                Mathf.RoundToInt((profile.rows - 1) * 2f / cell));
            var shader = Shader.Find("MagicCarpet/VoxelSurface") ?? Shader.Find("Hidden/InternalErrorShader");
            Assert.That(shader, Is.Not.Null);
            var material = new Material(shader);
            var parent = new GameObject("Source house support test");
            try
            {
                using (var house = new VoxelVolume(parent.transform, Vector3.zero, size, cell, material))
                {
                    // Use the runtime producer, including its actual door and window cuts.
                    VoxelSandbox.PopulateHouse(house, profile, cell);
                    house.Build();
                    Assert.That(parent.GetComponentsInChildren<MeshCollider>().Length, Is.GreaterThan(0));
                    var detached = house.DetachUnsupported();
                    Assert.That(detached.Sum(group => group.Count), Is.Zero,
                        "The untouched house contains cells disconnected from its bottom layer.");
                }
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(parent);
                UnityEngine.Object.DestroyImmediate(material);
            }
        }
    }
}
