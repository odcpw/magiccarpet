using System;
using System.Collections.Generic;
using System.Linq;

namespace MagicCarpet.Native
{
    // Reviewed source creature packages. Native state still owns their motion,
    // attacks, children and lifetime; this table owns the portable art contract.
    internal static class ModernCreatureAssets
    {
        internal readonly struct Binding
        {
            internal readonly string Resource;
            internal readonly string[] Clips;
            internal Binding(string resource, params string[] clips) { Resource = resource; Clips = clips; }
        }
        internal static readonly IReadOnlyList<Binding> All = Array.AsReadOnly(new[] {
            new Binding("Firefly", "Idle", "Flight", "Attack", "Hit", "Death"),
            new Binding("WormHead", "Idle", "Attack", "Hit", "Death"),
            new Binding("WormSegment", "Idle")
        });
        internal static bool TryGet(string resource, out Binding binding)
        {
            foreach (var row in All) if (row.Resource == resource) { binding = row; return true; }
            binding = default; return false;
        }
        internal static bool ContainsResource(string resource) => All.Any(row => row.Resource == resource);
    }
}
