//! Compatibility facade for the package-neutral economy TMAP owner.

pub use sim_economy_tmap::*;

#[cfg(test)]
mod tests {
    use crate::level1_mission::mana_sphere_size_ordinal;

    #[test]
    fn mana_size_buckets_use_the_source_strict_greater_than_boundaries() {
        for (mana, expected) in [
            (0, 0),
            (256, 0),
            (257, 1),
            (512, 1),
            (513, 2),
            (1_024, 2),
            (1_025, 3),
            (2_048, 3),
            (2_049, 4),
            (4_096, 4),
            (4_097, 5),
            (9_192, 5),
            (9_193, 6),
            (18_384, 6),
            (18_385, 7),
        ] {
            assert_eq!(mana_sphere_size_ordinal(mana), expected, "mana={mana}");
        }
    }
}
