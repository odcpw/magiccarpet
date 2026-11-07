using UnityEngine;

namespace MagicCarpet.Native.Voxels
{
    // Generated from the approved character's source mesh; one entry per cell.
    public sealed class VoxelGoatAsset : ScriptableObject
    {
        public Mesh sourceMesh;
        public Mesh mesh;
        public Color32[] cellColors;
        public float cellSize;
        public int resolution;
    }
}
