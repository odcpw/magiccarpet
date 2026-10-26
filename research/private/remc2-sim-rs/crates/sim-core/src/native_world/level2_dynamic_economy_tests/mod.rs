use super::super::tests::level_two_fixture_path;
use super::*;

fn level_two_possession_lifecycle_session() -> Mc2Simulation {
    let package = level_two_fixture_path();
    let gameplay_cache = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated/level2-day-owner-cache/components/gameplay");
    NativeWorld::inspect_level2_with_owner_cache(&package, &gameplay_cache)
        .expect("qualified Level 2 gameplay component")
        .level2_reset_session()
        .expect("Level 2 possession lifecycle session")
}

include!("part_01.rs");
include!("part_02.rs");
include!("part_03.rs");
