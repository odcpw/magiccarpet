using System.Collections.Generic;
using System.IO;
using NUnit.Framework;

namespace MagicCarpet.Native.Tests
{
    public sealed class ModernBuildingLayoutEditModeTests
    {
        // Deliberately synthetic; owner game height samples never enter source control.
        internal static ModernBuildingLayout Layout(out byte[] original)
        {
            original=new byte[65536];
            var profiles=new List<ModernBuildingMeshes.Profile>();
            foreach(var selector in new[] {26,29,30,42})
            {
                var values=new float[25];
                for(var z=1;z<4;z++) for(var x=1;x<4;x++) values[z*5+x]=(x==2&&z==2)?16:8;
                profiles.Add(new ModernBuildingMeshes.Profile {build_index=selector,columns=5,rows=5,heights=values});
            }
            var entries=new List<ModernBuildingLayout.Entry>();
            var ids=new[] {26,29,30,42,26};
            for(var b=0;b<ids.Length;b++)
            {
                var profile=profiles.Find(p=>p.build_index==ids[b]);var cx=25+b*8;const int cz=25;
                var cells=new List<ModernBuildingLayout.Cell>();
                for(var z=0;z<5;z++) for(var x=0;x<5;x++)
                {
                    var value=profile.heights[z*5+x];if(value==0) continue;
                    var index=(cz+z-2)*256+cx+x-2;original[index]=(byte)(20+value*2);
                    cells.Add(new ModernBuildingLayout.Cell {index=index,expected=original[index],replacement=20});
                }
                entries.Add(new ModernBuildingLayout.Entry {source_slot=b,build_index=ids[b],x=cx*4,z=cz*4,
                    base_height=10,width=16,depth=16,height=16,cells=cells.ToArray(),
                    source_entity_position=new ModernBuildingLayout.SourcePosition {x=cx*256,y=cz*256,z=1280}});
            }
            return new ModernBuildingLayout {schema="mc-modern-building-layout/v2",profiles=profiles.ToArray(),
                provenance=new ModernBuildingLayout.Provenance {source_height_plane_sha256=ModernBuildingLayout.Digest(original)},
                buildings=entries.ToArray()};
        }

        [Test] public void VisualReplacementPreservesSourceAndUnlistedSkirtCells()
        {
            var layout=Layout(out var source);var cell=layout.buildings[0].cells[0];var expected=source[cell.index];
            var output=layout.VisualHeights(source);
            Assert.That(output,Is.Not.SameAs(source));Assert.That(output[cell.index],Is.EqualTo(20));
            Assert.That(source[cell.index],Is.EqualTo(expected));Assert.That(output[0],Is.EqualTo(source[0]));
        }

        [Test] public void StaleSnapshotOrLaterBadPatchRejectsTheWholeDescription()
        {
            var layout=Layout(out var source);source[0]=1;
            Assert.Throws<InvalidDataException>(()=>layout.VisualHeights(source));
            source[0]=0;var cell=layout.buildings[1].cells[0];var expected=source[cell.index];cell.expected--;
            Assert.Throws<InvalidDataException>(()=>layout.VisualHeights(source));
            Assert.That(source[cell.index],Is.EqualTo(expected));
        }

        [Test] public void WrongShapeOrIncompleteReplacementIsRejected()
        {
            var layout=Layout(out var source);layout.profiles[0].heights[6]+=.5f;
            Assert.Throws<InvalidDataException>(()=>layout.VisualHeights(source));
            layout=Layout(out source);var cells=new List<ModernBuildingLayout.Cell>(layout.buildings[0].cells);
            cells.RemoveAt(0);layout.buildings[0].cells=cells.ToArray();
            Assert.Throws<InvalidDataException>(()=>layout.VisualHeights(source));
        }
    }
}
