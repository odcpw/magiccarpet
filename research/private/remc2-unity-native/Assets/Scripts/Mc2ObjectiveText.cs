namespace MagicCarpet.Native
{
    // Presentation copy for MC2-owned semantic objectives. Rust selects the
    // current objective and owns every completion and exit condition.
    internal static class Mc2ObjectiveText
    {
        internal static string Resolve(EngineSessionTransport.ObjectiveProgress progress)
        {
            if (progress == null) return null;
            if (progress.all_objectives_satisfied) return "Enter the level exit.";
            return progress.current_objective switch
            {
                "objective.reach.sight_beacon" => "Fly to the beacon.",
                "objective.defeat.nyphur" => "Defeat Nyphur.",
                "objective.clear.plague_bees" => "Destroy the plague Bees.",
                "objective.reach.payahandra_ruins" => "Fly to Payahandra's ruins.",
                "objective.clear.payahandra_vaults" => "Destroy Payahandra's vaults.",
                "objective.clear.undead_horde" => "Destroy the undead horde.",
                "objective.clear.guardian_dragons" => "Defeat the guardian Dragons.",
                "objective.charge.reveal_entrance" => "Charge your Castle with mana to reveal the entrance.",
                "objective.reach.revealed_way" => "Fly to the revealed entrance.",
                "objective.clear.final_bees" => "Destroy the remaining Bees.",
                _ => null,
            };
        }
    }
}
