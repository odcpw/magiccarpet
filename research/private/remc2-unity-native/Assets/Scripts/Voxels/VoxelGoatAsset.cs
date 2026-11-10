using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // Generated from the approved character's source mesh; one entry per cell.
    public sealed class VoxelGoatAsset : ScriptableObject
    {
        public Mesh sourceMesh;
        public string sourceFingerprint;
        public Mesh mesh;
        public Color32[] cellColors;
        public Vector3[] cellCenters;
        public BoneWeight[] cellWeights;
        // 0 = legacy 24-vertex cubes; 1 = faces culled only across
        // neighboring cells with exactly identical skin weights.
        public int meshFormat;
        public byte[] cellFaceMasks;
        public float cellSize;
        public int resolution;
    }
}
