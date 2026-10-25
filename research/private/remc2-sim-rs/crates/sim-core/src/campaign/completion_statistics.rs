//! Source `CollectLevelStats_5C530` arithmetic. The live adapter supplies every
//! counter explicitly; unavailable producer state must not become a default
//! successful result. `sub_82AB0` retains the resulting five fields per level.

use sim_session::GameCompletionStatistics;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceCompletionCounters {
    /// Nonzero `str_0x364D2.dword_0x364D6[family]`.
    pub available_spell_types: [bool; 26],
    /// Current-mission `array_0x403_1027x`, not the carried inventory union.
    pub found_spell_types: [bool; 26],
    pub creatures_killed: i32,
    pub total_creatures: i32,
    pub projectile_attempts: i32,
    pub projectile_hits: i32,
    /// Source A4 `dword_0x13C_316` (claimed sphere aggregate).
    pub owned_mana: i32,
    /// The player's actual Castle relation; absence yields zero percent.
    pub castle_mana: Option<i32>,
    /// Source `str_index_242ar.dword_4`; the source subtracts one below.
    pub global_mana_counter: i32,
    /// Source `time_393` before collection and `j___clock()` at collection.
    pub start_clock: u32,
    pub completion_clock: u32,
}

fn percent(numerator: i32, denominator: i32) -> i32 {
    if denominator == 0 {
        100
    } else {
        numerator
            .wrapping_mul(100)
            .wrapping_div(denominator)
            .clamp(0, 100)
    }
}

pub(crate) fn collect(counters: SourceCompletionCounters) -> GameCompletionStatistics {
    let available = counters
        .available_spell_types
        .iter()
        .filter(|&&value| value)
        .count() as i32;
    let found = counters
        .available_spell_types
        .iter()
        .zip(counters.found_spell_types)
        .filter(|(available, found)| **available && *found)
        .count() as i32;
    let mana_collected_percent = counters.castle_mana.map_or(0, |castle_mana| {
        let denominator = counters.global_mana_counter.wrapping_sub(1);
        if denominator <= 0 {
            100
        } else {
            percent(counters.owned_mana.wrapping_add(castle_mana), denominator)
        }
    });
    GameCompletionStatistics {
        spells_found_percent: percent(found, available),
        hit_accuracy_percent: percent(counters.projectile_hits, counters.projectile_attempts),
        creatures_killed_percent: percent(counters.creatures_killed, counters.total_creatures),
        mana_collected_percent,
        time_taken_seconds: counters.completion_clock.wrapping_sub(counters.start_clock) / 100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counters() -> SourceCompletionCounters {
        SourceCompletionCounters {
            available_spell_types: [false; 26],
            found_spell_types: [false; 26],
            creatures_killed: 0,
            total_creatures: 0,
            projectile_attempts: 0,
            projectile_hits: 0,
            owned_mana: 0,
            castle_mana: None,
            global_mana_counter: 1,
            start_clock: 0,
            completion_clock: 0,
        }
    }

    #[test]
    fn source_empty_totals_and_absent_castle_have_distinct_defaults() {
        let result = collect(counters());
        assert_eq!(result.spells_found_percent, 100);
        assert_eq!(result.hit_accuracy_percent, 100);
        assert_eq!(result.creatures_killed_percent, 100);
        assert_eq!(result.mana_collected_percent, 0);
        let mut input = counters();
        input.castle_mana = Some(0);
        assert_eq!(collect(input).mana_collected_percent, 100);
    }

    #[test]
    fn source_truncates_clamps_counts_only_available_spells_and_wraps_clock() {
        let mut input = counters();
        input.available_spell_types[..3].fill(true);
        input.found_spell_types[0] = true;
        input.found_spell_types[25] = true; // carried/unavailable type is not a found level spell
        input.creatures_killed = 3;
        input.total_creatures = 7;
        input.projectile_attempts = 3;
        input.projectile_hits = 4;
        input.owned_mana = 250;
        input.castle_mana = Some(500);
        input.global_mana_counter = 1_001;
        input.start_clock = u32::MAX - 49;
        input.completion_clock = 250;
        let result = collect(input);
        assert_eq!(result.spells_found_percent, 33);
        assert_eq!(result.creatures_killed_percent, 42);
        assert_eq!(result.hit_accuracy_percent, 100);
        assert_eq!(result.mana_collected_percent, 75);
        assert_eq!(result.time_taken_seconds, 3);
    }
}
