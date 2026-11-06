using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using UnityEngine;

namespace MagicCarpet.Native
{
    // Owner-local reset BUILD identities, qualified against one frozen sandbox
    // snapshot. This is not a replacement for the live gameplay building feed.
    [Serializable]
    internal sealed class ModernBuildingLayout
    {
        public string schema;
        public Provenance provenance;
        public ModernBuildingMeshes.Profile[] profiles;
        public Entry[] buildings;
        [Serializable] internal sealed class Provenance { public string source_height_plane_sha256; }
        [Serializable] internal sealed class Entry
        {
            public int source_slot,build_index;
            public float x,z,base_height,width,depth,height;
            public Cell[] cells;
        }
        [Serializable] internal sealed class Cell { public int index,expected,replacement; }

        internal static ModernBuildingLayout LoadRequested()
        {
            var path=Environment.GetEnvironmentVariable("MC_SANDBOX_BUILDING_LAYOUT");
            if(string.IsNullOrEmpty(path)) return null;
            var file=new FileInfo(path);
            if(!file.Exists||file.Length>4*1024*1024) throw new InvalidDataException("Missing or oversized sandbox building layout");
            var layout=JsonUtility.FromJson<ModernBuildingLayout>(File.ReadAllText(path));
            if(layout==null||layout.schema!="mc-modern-building-layout/v2"||layout.provenance==null
                ||layout.buildings==null||layout.buildings.Length==0||layout.buildings.Length>128)
                throw new InvalidDataException("Invalid sandbox building layout");
            return layout;
        }

        internal ModernBuildingMeshes.Profile Profile(int selector)
        {
            if(profiles!=null) foreach(var profile in profiles)
                if(profile!=null&&profile.build_index==selector) return profile;
            throw new InvalidDataException("Missing source building profile");
        }

        internal void ValidateProfiles()
        {
            if(profiles==null||profiles.Length==0||profiles.Length>32)
                throw new InvalidDataException("Missing or excessive source building profiles");
            var selectors=new HashSet<int>();
            foreach(var profile in profiles)
            {
                if(profile==null||!selectors.Add(profile.build_index)||profile.diagonal_parity<0||profile.diagonal_parity>1
                    ||profile.columns<3||profile.rows<3
                    ||profile.columns>33||profile.rows>33||profile.heights==null
                    ||profile.heights.Length!=profile.columns*profile.rows)
                    throw new InvalidDataException("Invalid source building profile extent");
                var max=0f;
                foreach(var value in profile.heights)
                {
                    if(!float.IsFinite(value)||value<0||value>127.5f||Mathf.Abs(value*2-Mathf.Round(value*2))>.001f)
                        throw new InvalidDataException("Invalid source building profile sample");
                    max=Mathf.Max(max,value);
                }
                if(max<=0) throw new InvalidDataException("Empty source building profile");
            }
        }

        internal static string Digest(byte[] bytes)
        { using(var sha=SHA256.Create()) return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-","").ToLowerInvariant(); }

        internal byte[] VisualHeights(byte[] original)
        {
            ValidateProfiles();
            if(original==null||original.Length!=65536||provenance==null
                ||Digest(original)!=provenance.source_height_plane_sha256)
                throw new InvalidDataException("Building layout does not match the frozen terrain snapshot");
            var changes=new Dictionary<int,int>();var slots=new HashSet<int>();
            if(buildings==null||buildings.Length==0||buildings.Length>128) throw new InvalidDataException("Invalid building count");
            foreach(var entry in buildings)
            {
                if(entry==null||!slots.Add(entry.source_slot)||entry.cells==null||entry.cells.Length==0||entry.cells.Length>4096
                    ||(entry.build_index!=26&&entry.build_index!=29&&entry.build_index!=30&&entry.build_index!=42)
                    ||!float.IsFinite(entry.x)||!float.IsFinite(entry.z)||entry.x<0||entry.x>=1024||entry.z<0||entry.z>=1024
                    ||!float.IsFinite(entry.base_height)||entry.base_height<0||entry.base_height>127.5f
                    ||!float.IsFinite(entry.width)||!float.IsFinite(entry.depth)||!float.IsFinite(entry.height)
                    ||entry.width<=0||entry.width>128||entry.depth<=0||entry.depth>128||entry.height<=0||entry.height>128)
                    throw new InvalidDataException("Invalid building identity or dimensions");
                var profile=Profile(entry.build_index);
                var footprint=ModernBuildingMeshes.Footprint(profile);var maxHeight=0f;
                foreach(var sample in profile.heights) maxHeight=Mathf.Max(maxHeight,sample);
                if(Mathf.Abs(footprint.x-entry.width)>.001f||Mathf.Abs(footprint.y-entry.depth)>.001f
                    ||Mathf.Abs(maxHeight-entry.height)>.001f)
                    throw new InvalidDataException("Building dimensions disagree with its source profile");
                var positive=0;foreach(var sample in profile.heights) if(sample>0) positive++;
                var ownCells=new HashSet<int>();
                if(entry.cells.Length!=positive) throw new InvalidDataException("Incomplete building height replacement");
                foreach(var cell in entry.cells)
                {
                    if(cell==null||!ownCells.Add(cell.index)||cell.index<0||cell.index>=original.Length||cell.expected>255
                        ||cell.replacement<0||cell.replacement>=cell.expected||original[cell.index]!=cell.expected
                        ||Mathf.Abs(cell.replacement*.5f-entry.base_height)>.001f
                        ||Mathf.Abs(ModernBuildings.WrappedDelta((cell.index%256)*4,entry.x))>entry.width*.5f
                        ||Mathf.Abs(ModernBuildings.WrappedDelta((cell.index/256)*4,entry.z))>entry.depth*.5f
                        ||(changes.TryGetValue(cell.index,out var prior)&&prior!=cell.replacement))
                        throw new InvalidDataException("Building stamp byte mismatch or conflicting replacement");
                    var px=(WrappedSample(cell.index%256,entry.x)+entry.width*.5f)/4;
                    var pz=(WrappedSample(cell.index/256,entry.z)+entry.depth*.5f)/4;
                    var ix=Mathf.RoundToInt(px);var iz=Mathf.RoundToInt(pz);
                    if(Mathf.Abs(px-ix)>.001f||Mathf.Abs(pz-iz)>.001f||ix<0||ix>=profile.columns||iz<0||iz>=profile.rows
                        ||Mathf.Abs(profile.heights[iz*profile.columns+ix]*2-(cell.expected-cell.replacement))>.001f)
                        throw new InvalidDataException("Building mesh sample disagrees with the original terrain stamp");
                    changes[cell.index]=cell.replacement;
                }
            }
            // Validate the entire description before touching even the private
            // copy. Original Rust terrain bytes, topology and water stay intact.
            var visual=(byte[])original.Clone();
            foreach(var pair in changes) visual[pair.Key]=(byte)pair.Value;
            Debug.Log($"MC_BUILDING_VISUAL_TERRAIN buildings={buildings.Length} replaced_roof_cells={changes.Count} original_sha256={provenance.source_height_plane_sha256} simulation_changed=False");
            return visual;
        }
        private static float WrappedSample(int sample,float center) => ModernBuildings.WrappedDelta(sample*4,center);
    }
}
