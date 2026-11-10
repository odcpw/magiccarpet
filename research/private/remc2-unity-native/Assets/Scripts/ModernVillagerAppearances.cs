using System;
using System.Collections.Generic;
using System.Linq;

namespace MagicCarpet.Native
{
    // MC2 compatibility bindings, shared by runtime admission and art import.
    // Heights match sim-definition/adapter.rs::VILLAGER_APPEARANCES, including
    // the carrier's jar. The six-resident pilot retains its resource identity.
    internal static class ModernVillagerAppearances
    {
        internal readonly struct Binding
        {
            internal readonly ushort Parameter;
            internal readonly string Resource;
            internal readonly float SourceHeight;

            internal Binding(ushort parameter, string resource, ushort logicalHeight)
            {
                Parameter = parameter;
                Resource = resource;
                SourceHeight = logicalHeight / 64f;
            }
        }

        internal static readonly IReadOnlyList<Binding> All = Array.AsReadOnly(new[] {
            new Binding(239, "VillagerBrown", 200),
            new Binding(241, "Villager", 250),
            new Binding(242, "VillagerStriped", 250),
            new Binding(271, "VillagerCarrier", 300)
        });

        internal static bool TryGet(ushort parameter, out Binding binding)
        {
            foreach (var candidate in All)
                if (candidate.Parameter == parameter) { binding = candidate; return true; }
            binding = default;
            return false;
        }

        internal static Binding Get(ushort parameter) => TryGet(parameter, out var binding) ? binding :
            throw new ArgumentException("No reviewed villager appearance: " + parameter);

        internal static bool ContainsResource(string resource) => All.Any(b => b.Resource == resource);
    }
}
