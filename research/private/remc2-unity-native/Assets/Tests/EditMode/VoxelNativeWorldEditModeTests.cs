using System;
using System.Collections.Generic;
using MagicCarpet.Native.Voxels;
using NUnit.Framework;
using UnityEngine;

namespace MagicCarpet.Native.Tests
{
    public sealed class VoxelNativeWorldEditModeTests
    {
        [Test]
        public void SourceLoweringDistinguishesHistoricalHolesFromCurrentBase()
        {
            var terrain=new NativeCampaignSession.VoxelVolumeProjection {
                kind=0,slot=0,dimensions=new[]{1,1,8},terrain_columns=new[]{4}
            };
            var hole=new NativeCampaignSession.VoxelRemovedCell {kind=0,slot=0,x=0,y=0,z=3};
            Assert.That(VoxelModernWorld.RemovedCellIntersectsBase(terrain,hole),Is.True);
            terrain.terrain_columns[0]=2;
            Assert.That(VoxelModernWorld.RemovedCellIntersectsBase(terrain,hole),Is.False);
            terrain.terrain_columns[0]=4;
            Assert.That(VoxelModernWorld.RemovedCellIntersectsBase(terrain,hole),Is.True,
                "The old hole remains omitted when source terrain rises again");
            hole.x=1;
            Assert.Throws<InvalidOperationException>(()=>VoxelModernWorld.RemovedCellIntersectsBase(terrain,hole));
            hole.x=0;hole.slot=38;
            Assert.Throws<InvalidOperationException>(()=>VoxelModernWorld.RemovedCellIntersectsBase(terrain,hole));
            var house=new NativeCampaignSession.VoxelVolumeProjection {
                kind=1,slot=38,dimensions=new[]{1,1,8},building_bytes=new[]{8}
            };
            hole.kind=1;
            Assert.That(VoxelModernWorld.RemovedCellIntersectsBase(house,hole),Is.True);
            hole.z=2;
            Assert.That(VoxelModernWorld.RemovedCellIntersectsBase(house,hole),Is.False);
        }

        [Test]
        public void SeedSamplesTheUneditedLatticeInNativeRowOrder()
        {
            var patch = new Bounds(new Vector3(100, 0, 200), new Vector3(32, 1, 40));
            var values = VoxelModernWorld.SampleSeedHeights(patch, (x,z) => x + 1000*z);
            Assert.That(values.Length, Is.EqualTo(82277));
            Assert.That(values[0], Is.EqualTo(84 + 1000*180));
            Assert.That(values[16], Is.EqualTo(116 + 1000*180));
            Assert.That(values[17], Is.EqualTo(84 + 1000*182));
            Assert.That(values[356], Is.EqualTo(116 + 1000*220));
            Assert.That(values[357], Is.EqualTo(84.0625f + 1000*180.0625f));
            Assert.That(values[357+256], Is.EqualTo(84.0625f + 1000*180.1875f));
            Assert.That(values[values.Length-1], Is.EqualTo(115.9375f + 1000*219.9375f));
        }

        [Test]
        public void PackedHouseOccupancyUsesNativeXThenYThenZAndLittleEndianBits()
        {
            var row = new NativeCampaignSession.VoxelVolumeProjection { dimensions = new[] { 3, 2, 2 }, building_bytes = new[] { 0x21, 0x08 } };
            Assert.That(VoxelModernWorld.BaseBuildingCell(row,0,0,0), Is.True);
            Assert.That(VoxelModernWorld.BaseBuildingCell(row,2,1,0), Is.True);
            Assert.That(VoxelModernWorld.BaseBuildingCell(row,2,1,1), Is.True);
            Assert.That(VoxelModernWorld.BaseBuildingCell(row,0,0,1), Is.False);
        }

        [Test]
        public void ColumnHeightTracksExactRemovedCellsWithoutCollapsingCavities()
        {
            var removed = new HashSet<(int kind,int slot,int x,int y,int z)> { (0,0,2,4,2), (0,0,2,4,3), (0,0,2,4,0), (1,38,2,4,1) };
            Assert.That(VoxelModernWorld.ColumnTop(4,0,2,4,removed), Is.EqualTo(2));
            removed.Add((0,0,2,4,1));
            Assert.That(VoxelModernWorld.ColumnTop(2,0,2,4,removed), Is.Zero);
            Assert.That(VoxelModernWorld.ColumnTop(4,0,3,4,removed), Is.EqualTo(4));
        }

        [Test]
        public void PauseGenerationRebindKeepsActorsUntilWorldEpochChangesOrTickRewinds()
        {
            Assert.That(ModernCharacterActors.MustClearActors(1,280,1,280),Is.False,"Outer pause/resume generation is not a new voxel world");
            Assert.That(ModernCharacterActors.MustClearActors(1,280,1,281),Is.False);
            Assert.That(ModernCharacterActors.MustClearActors(1,280,2,280),Is.True,"A new voxel world clears prior lifetimes");
            Assert.That(ModernCharacterActors.MustClearActors(1,280,1,279),Is.True,"Rewind clears stale actor lifetimes");
        }

        [Test]
        public void ImpactChildSuppressionUsesOnlyCurrentTickProjection()
        {
            var slots=new HashSet<ushort>();
            ModernEntityPresenter.RefreshVoxelImpactChildren(slots,new[]{new NativeSim.VoxelImpactChild { slot=42,slotGeneration=7 }});
            Assert.That(slots.Contains(42),Is.True);
            Assert.That(slots.Contains(43),Is.False,"Unrelated model-zero effects remain visible");
            ModernEntityPresenter.RefreshVoxelImpactChildren(slots,Array.Empty<NativeSim.VoxelImpactChild>());
            Assert.That(slots.Contains(42),Is.False,"Reclaimed or reused slots are not retained");
        }

        [Test]
        public void ImpactUsesNativeVelocityAndContactNormalWithoutInventingActorNormals()
        {
            var impact=new NativeCampaignSession.VoxelImpact { velocity_x=64,velocity_y=-32,velocity_z=16,normal_x=0,normal_y=-1,normal_z=0 };
            Assert.That(VoxelModernWorld.ImpactVelocity(impact),Is.EqualTo(new Vector3(30,7.5f,-15)));
            Assert.That(VoxelModernWorld.ImpactNormal(impact),Is.EqualTo(Vector3.back));
            impact.normal_y=0;
            Assert.That(VoxelModernWorld.ImpactNormal(impact),Is.EqualTo(Vector3.zero));
        }

        [Test]
        public void RebuildUsesWorldEpochRatherThanPauseGeneration()
        {
            var state=new NativeSim.VoxelState { enabled=1,generation=3,worldGeneration=1,revision=4,contentHash=15,impactHead=7 };
            Assert.That(VoxelModernWorld.NeedsRebuild(state,50,1,50,4,15,7),Is.False,"Outer generation changed but the voxel world did not");
            state.generation=4;
            Assert.That(VoxelModernWorld.NeedsRebuild(state,51,1,50,4,15,7),Is.False);
            state.worldGeneration=2;
            Assert.That(VoxelModernWorld.NeedsRebuild(state,51,1,50,4,15,7),Is.True,"New world requires reconstruction");
            state.worldGeneration=1;
            Assert.That(VoxelModernWorld.NeedsRebuild(state,49,1,50,4,15,7),Is.True,"Rewind discards stale geometry");
            Assert.That(VoxelModernWorld.NeedsRebuild(state,50,1,50,4,14,7),Is.True);
            Assert.That(VoxelModernWorld.NeedsRebuild(state,50,1,50,5,15,7),Is.True);
            Assert.That(VoxelModernWorld.NeedsRebuild(state,50,1,50,4,15,8),Is.True);
            state.revision=5;state.contentHash=16;
            Assert.That(VoxelModernWorld.NeedsRebuild(state,51,1,50,4,15,7),Is.False,"A forward native edit is incremental");
            state.enabled=0;
            Assert.That(VoxelModernWorld.NeedsRebuild(state,51,1,50,4,15,7),Is.True);
        }

        [Test]
        public void HouseAppearanceUsesExistingTriangularProfileAtTheOriginalScale()
        {
            var p=new ModernBuildingMeshes.Profile { columns=2,rows=2,heights=new[]{0f,4f,8f,20f},diagonal_parity=0 };
            Assert.That(VoxelModernWorld.HouseAppearanceTop(p,1,3),Is.EqualTo(9f).Within(.0001f));
            Assert.That(VoxelModernWorld.HouseAppearanceTop(p,3,1),Is.EqualTo(7f).Within(.0001f));
            p.diagonal_parity=1;
            Assert.That(VoxelModernWorld.HouseAppearanceTop(p,1,1),Is.EqualTo(3f).Within(.0001f));
        }

        [Test]
        public void NativeCellEditRemovesOnlyListedCellsAndDoesNotRepeatThem()
        {
            var parent=new GameObject("Native cell edit");
            var shader=Shader.Find("Universal Render Pipeline/Lit") ?? Shader.Find("Standard") ?? Shader.Find("Hidden/InternalErrorShader");
            var material=new Material(shader);
            try
            {
                using var volume=new VoxelVolume(parent.transform,Vector3.zero,new Vector3Int(3,3,3),1,material);
                for(int z=0;z<3;z++)for(int y=0;y<3;y++)for(int x=0;x<3;x++)volume.SetCell(x,y,z,Color.white);
                volume.Build();
                var cells=new[]{new Vector3Int(0,0,0),new Vector3Int(2,2,2),new Vector3Int(0,0,0)};
                var edit=volume.BeginRemoveCells(cells);
                while(!edit.Complete)edit.Step(1);
                Assert.That(edit.Removed.Count,Is.EqualTo(2));
                Assert.That(edit.Removed.ConvertAll(cell=>cell.Position),Does.Contain(new Vector3(.5f,.5f,.5f)));
                Assert.That(edit.Removed.ConvertAll(cell=>cell.Position),Does.Contain(new Vector3(2.5f,2.5f,2.5f)));
                var repeated=volume.BeginRemoveCells(cells);while(!repeated.Complete)repeated.Step(1);
                Assert.That(repeated.Removed,Is.Empty);
                var middle=volume.BeginRemoveCells(new[]{new Vector3Int(1,1,1)});while(!middle.Complete)middle.Step(1);
                Assert.That(middle.Removed.Count,Is.EqualTo(1),"Cells inside the list's bounding box remain occupied");
            }
            finally { UnityEngine.Object.DestroyImmediate(parent);UnityEngine.Object.DestroyImmediate(material); }
        }

        [Test]
        public void NativeActorHideFlagsControlRealRendererAcrossSlotReuse()
        {
            var asset=Resources.Load<ModernCharacterAsset>("ModernCharacters/Goat/Character");
            Assert.That(asset,Is.Not.Null,"The owner-local goat model is required");
            using var first=new ModernCharacterInstance(asset,null);
            var renderer=first.Root.GetComponentInChildren<Renderer>(true);
            Assert.That(renderer,Is.Not.Null);
            var state=new NativeSim.CampaignCharacterState { entityIndex=93,slotGeneration=1,active=1 };
            ModernCharacterActors.ApplyActorVisibility(first.Root,state,true,true);
            Assert.That(renderer.enabled && renderer.gameObject.activeInHierarchy,Is.True);
            foreach(uint hide in new uint[]{1,0x20,0x400})
            {
                state.controlFlags=hide;
                ModernCharacterActors.ApplyActorVisibility(first.Root,state,true,true);
                Assert.That(renderer.gameObject.activeInHierarchy,Is.False,$"Native hide bit {hide:x} must suppress the actual renderer");
                state.controlFlags=0;
                ModernCharacterActors.ApplyActorVisibility(first.Root,state,true,true);
                Assert.That(renderer.enabled && renderer.gameObject.activeInHierarchy,Is.True);
            }
            state.active=0;
            ModernCharacterActors.ApplyActorVisibility(first.Root,state,true,true);
            Assert.That(renderer.gameObject.activeInHierarchy,Is.False);

            using var reused=new ModernCharacterInstance(asset,null);
            var reusedRenderer=reused.Root.GetComponentInChildren<Renderer>(true);
            state.slotGeneration=2;state.active=1;state.controlFlags=0;
            ModernCharacterActors.ApplyActorVisibility(reused.Root,state,true,true);
            Assert.That(reusedRenderer.enabled && reusedRenderer.gameObject.activeInHierarchy,Is.True,
                "A new lifetime in the same slot does not inherit the retired actor's hidden state");
        }

        [Test]
        public void FireballVisualUsesNativeVisibilityAndNeverClaimsArrowSlots()
        {
            var parent=new GameObject("Native projectile test");var cameraNode=new GameObject("Camera");
            try
            {
                var camera=cameraNode.AddComponent<Camera>();camera.transform.position=new Vector3(1023,0,1023);
                using var presenter=new VoxelNativeProjectiles(parent.transform);
                var fire=new NativeSim.CampaignCharacterState { entityIndex=7,active=1,classId=9,model=0,x=64,y=128,z=192,slotGeneration=1 };
                var arrow=new NativeSim.CampaignCharacterState { entityIndex=8,active=1,classId=9,model=13 };
                var visible=new[]{new NativeSim.CampaignEntityPresentation { entityIndex=7,x=64,y=128,z=192,logicalHeight=128 },new NativeSim.CampaignEntityPresentation { entityIndex=8 }};
                presenter.Present(new[]{fire,arrow},visible,1);presenter.Render(camera);
                Assert.That(presenter.Replaces(7),Is.True);Assert.That(presenter.Replaces(8),Is.False);
                var node=parent.transform.GetChild(0).GetChild(0);
                Assert.That(node.position,Is.EqualTo(new Vector3(1025,4,1026)));
                Assert.That(node.GetComponent<Collider>(),Is.Null);
                fire.slotGeneration=2;fire.x=128;
                visible[0].x=128;
                presenter.Present(new[]{fire},visible,1);presenter.Render(camera);
                Assert.That(node.position,Is.EqualTo(new Vector3(1026,4,1026)),"A recycled slot must not interpolate from its prior lifetime");
                presenter.Present(new[]{fire},Array.Empty<NativeSim.CampaignEntityPresentation>(),1);
                Assert.That(presenter.Replaces(7),Is.False);Assert.That(node.gameObject.activeSelf,Is.False);
                fire.model=28;presenter.Present(new[]{fire},visible,2);
                Assert.That(presenter.Replaces(7),Is.True);
                presenter.Present(Array.Empty<NativeSim.CampaignCharacterState>(),visible,3);
                Assert.That(presenter.Replaces(7),Is.False);
            }
            finally { UnityEngine.Object.DestroyImmediate(cameraNode);UnityEngine.Object.DestroyImmediate(parent); }
        }
    }
}
