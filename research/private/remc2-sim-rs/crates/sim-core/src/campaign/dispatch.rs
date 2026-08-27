//! Closed, source-site-aware campaign behavior registry.
//!
//! CF-03 records which already-accepted handler family owns each implemented
//! source key.  It deliberately does not invoke those handlers: the existing
//! schedulers retain their current order and calls until CF-10 performs the
//! scheduler cutover.  There is no registration API or mutable registry state.

use super::{
    source_facts::SourceFactKind,
    spec::{BehaviorKey, BehaviorSelector, CampaignCapabilityRegistry},
};
use std::{collections::BTreeSet, sync::LazyLock};

/// Stable package/source dispatch-site identifiers from contract v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u16)]
pub enum DispatchSite {
    ApplyEvents = 1,
    UpdateEntities = 2,
    Constructor = 3,
    TypeSubtype = 4,
    Direct = 5,
}

impl DispatchSite {
    pub const fn id(self) -> u16 {
        self as u16
    }

    pub const fn from_id(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::ApplyEvents),
            2 => Some(Self::UpdateEntities),
            3 => Some(Self::Constructor),
            4 => Some(Self::TypeSubtype),
            5 => Some(Self::Direct),
            _ => None,
        }
    }

    #[cfg(test)]
    const fn name(self) -> &'static str {
        match self {
            Self::ApplyEvents => "ApplyEvents",
            Self::UpdateEntities => "UpdateEntities",
            Self::Constructor => "Constructor",
            Self::TypeSubtype => "TypeSubtype",
            Self::Direct => "Direct",
        }
    }
}

/// Existing implementation family selected by a compiled entry.
///
/// This enum is the CF-03 delegate boundary.  It names accepted handlers
/// without manufacturing a second implementation or moving scheduler calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ExistingHandler {
    TreeLifecycle = 1,
    Stone = 2,
    Dolmen = 3,
    BadStone = 4,
    PlayerFlight = 5,
    NyphurPlayer = 6,
    CastleRoot = 7,
    ManaCarrier = 8,
    GoatFamily = 9,
    MultipartFamily = 10,
    ArcherFamily = 11,
    Model9Family = 12,
    BuilderFamily = 13,
    VillagerFamily = 14,
    BuilderVillagerPostBuild = 15,
    TraderFamily = 16,
    FireflyFamily = 17,
    ProjectileFamily = 18,
    Model6ProjectileChild = 19,
    Class10Effects = 20,
    SwitchProducers = 21,
    ObjectiveSwitch = 22,
    Class14Events = 23,
    SpellHolderAndPickup = 24,
    BeeFamily = 25,
    CastleAuxiliary = 26,
    BallisticScenery = 27,
    CountdownSwitch = 28,
    TerrainStripController = 29,
    DragonFamily = 30,
}

impl ExistingHandler {
    const fn ordinary_producer(self) -> &'static str {
        match self {
            Self::TreeLifecycle => "accepted reset/group tree constructors",
            Self::Stone => "accepted reset/group stone constructor",
            Self::Dolmen => "accepted reset/group dolmen constructor",
            Self::BadStone => "qualified Level 2 bad-stone reset row",
            Self::PlayerFlight => "accepted player reset row and source pass",
            Self::NyphurPlayer => "qualified class-3/model-1 Nyphur reset row",
            Self::CastleRoot => "accepted Castle root constructor",
            Self::ManaCarrier => "accepted mana-carrier constructor",
            Self::GoatFamily => "accepted Goat reset rows and child producers",
            Self::MultipartFamily => "accepted multipart root constructor",
            Self::ArcherFamily => "accepted archer constructor and child packet",
            Self::Model9Family => "accepted class-5/model-9 constructor",
            Self::BuilderFamily => "accepted builder reset/constructor rows",
            Self::VillagerFamily => "accepted villager reset/constructor rows",
            Self::BuilderVillagerPostBuild => "accepted builder/villager action-105 transition",
            Self::TraderFamily => "accepted Trader reset/constructor rows",
            Self::FireflyFamily => "accepted firefly reset/constructor rows",
            Self::ProjectileFamily => "accepted projectile constructors",
            Self::Model6ProjectileChild => "accepted model-6 spawned-child constructor",
            Self::Class10Effects => "accepted class-10 effect constructors",
            Self::SwitchProducers => "qualified Level 2 switch producers",
            Self::ObjectiveSwitch => "qualified Level 2 model-32 objective switch",
            Self::Class14Events => "qualified Level 2 class-14 constructors",
            Self::SpellHolderAndPickup => "accepted spell holder and pickup constructors",
            Self::BeeFamily => "accepted Level 2 Bee reset rows and spawned children",
            Self::CastleAuxiliary => "accepted Level 2 Castle auxiliary constructor",
            Self::BallisticScenery => "authored class-2 ballistic-scenery constructor",
            Self::CountdownSwitch => "authored absence-countdown switch constructor",
            Self::TerrainStripController => {
                "authored terrain-strip controller and open/close triggers"
            }
            Self::DragonFamily => {
                "accepted Dragon constructor, ordinary roam, and player-target fireball"
            }
        }
    }

    #[cfg(test)]
    const fn registry_name(self) -> &'static str {
        match self {
            Self::TreeLifecycle => "level1-tree-lifecycle",
            Self::Stone => "level1-stone",
            Self::Dolmen => "level1-dolmen",
            Self::BadStone => "level2-bad-stone",
            Self::PlayerFlight => "player-flight",
            Self::NyphurPlayer => "level2-nyphur-player",
            Self::CastleRoot => "castle-root",
            Self::ManaCarrier => "level1-mana-carrier",
            Self::GoatFamily => "goat-family",
            Self::MultipartFamily => "multipart-family",
            Self::ArcherFamily => "archer-family",
            Self::Model9Family => "level2-model9-family",
            Self::BuilderFamily => "level1-builder-family",
            Self::VillagerFamily => "villager-family",
            Self::BuilderVillagerPostBuild => "builder-villager-post-build",
            Self::TraderFamily => "trader-family",
            Self::FireflyFamily => "level1-firefly-family",
            Self::ProjectileFamily => "projectile-family",
            Self::Model6ProjectileChild => "level2-model6-projectile-child",
            Self::Class10Effects => "class10-effects",
            Self::SwitchProducers => "level2-switch-producers",
            Self::ObjectiveSwitch => "level2-objective-switch",
            Self::Class14Events => "level2-class14-events",
            Self::SpellHolderAndPickup => "spell-holder-and-pickup-family",
            Self::BeeFamily => "level2-bee-family",
            Self::CastleAuxiliary => "level2-castle-auxiliary",
            Self::BallisticScenery => "ballistic-scenery",
            Self::CountdownSwitch => "absence-countdown-switch",
            Self::TerrainStripController => "terrain-strip-controller",
            Self::DragonFamily => "dragon-family",
        }
    }
}

/// One immutable compiled behavior declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorEntry {
    pub key: BehaviorKey,
    pub handler: ExistingHandler,
    pub reachable_successors: &'static [BehaviorKey],
    pub required_source_fact_kinds: &'static [SourceFactKind],
    pub ordinary_producer: &'static str,
    pub same_pass_epilogue: &'static str,
    pub following_pass_consumer: &'static str,
}

const AUTHORED_LEVEL_FACTS: &[SourceFactKind] = &[SourceFactKind::AuthoredLevelRecords];
const CONSTRUCTION_FACTS: &[SourceFactKind] = &[
    SourceFactKind::AuthoredLevelRecords,
    SourceFactKind::FeatureConstructionInputs,
];
const ACTION_EPILOGUE: &str = "existing source-site epilogue remains scheduler-owned";
const CONSTRUCTOR_EPILOGUE: &str = "existing constructor caller tail remains scheduler-owned";
const FOLLOWING_CONSUMER: &str =
    "existing following UpdateEntities cleanup/reclaim and reset/hash consumers";

macro_rules! compiled_registry {
    (
        actions {
            $(($handler:ident, $class:literal, [$($action:literal),+]);)+
        }
        update_only_actions {
            $(($update_handler:ident, $update_class:literal, [$($update_action:literal),+]);)+
        }
        constructors {
            $(($constructor_handler:ident, $constructor_class:literal, [$($model:literal),+]);)+
        }
    ) => {
        static COMPILED_BEHAVIOR_ENTRIES: &[BehaviorEntry] = &[
            $(
                $(
                    BehaviorEntry {
                        key: BehaviorKey {
                            dispatch_site: DispatchSite::ApplyEvents.id(),
                            selector: BehaviorSelector::ClassAction {
                                class: $class,
                                action: $action,
                            },
                        },
                        handler: ExistingHandler::$handler,
                        reachable_successors: &[BehaviorKey {
                            dispatch_site: DispatchSite::UpdateEntities.id(),
                            selector: BehaviorSelector::ClassAction {
                                class: $class,
                                action: $action,
                            },
                        }],
                        required_source_fact_kinds: AUTHORED_LEVEL_FACTS,
                        ordinary_producer: ExistingHandler::$handler.ordinary_producer(),
                        same_pass_epilogue: ACTION_EPILOGUE,
                        following_pass_consumer: FOLLOWING_CONSUMER,
                    },
                )+
            )+
            $(
                $(
                    BehaviorEntry {
                        key: BehaviorKey {
                            dispatch_site: DispatchSite::UpdateEntities.id(),
                            selector: BehaviorSelector::ClassAction {
                                class: $update_class,
                                action: $update_action,
                            },
                        },
                        handler: ExistingHandler::$update_handler,
                        reachable_successors: &[],
                        required_source_fact_kinds: AUTHORED_LEVEL_FACTS,
                        ordinary_producer: ExistingHandler::$update_handler.ordinary_producer(),
                        same_pass_epilogue: ACTION_EPILOGUE,
                        following_pass_consumer: FOLLOWING_CONSUMER,
                    },
                )+
            )+
            $(
                $(
                    BehaviorEntry {
                        key: BehaviorKey {
                            dispatch_site: DispatchSite::UpdateEntities.id(),
                            selector: BehaviorSelector::ClassAction {
                                class: $class,
                                action: $action,
                            },
                        },
                        handler: ExistingHandler::$handler,
                        reachable_successors: &[],
                        required_source_fact_kinds: AUTHORED_LEVEL_FACTS,
                        ordinary_producer: ExistingHandler::$handler.ordinary_producer(),
                        same_pass_epilogue: ACTION_EPILOGUE,
                        following_pass_consumer: FOLLOWING_CONSUMER,
                    },
                )+
            )+
            $(
                $(
                    BehaviorEntry {
                        key: BehaviorKey {
                            dispatch_site: DispatchSite::Constructor.id(),
                            selector: BehaviorSelector::ClassModel {
                                class: $constructor_class,
                                model: $model,
                            },
                        },
                        handler: ExistingHandler::$constructor_handler,
                        reachable_successors: &[],
                        required_source_fact_kinds: CONSTRUCTION_FACTS,
                        ordinary_producer: ExistingHandler::$constructor_handler.ordinary_producer(),
                        same_pass_epilogue: CONSTRUCTOR_EPILOGUE,
                        following_pass_consumer: FOLLOWING_CONSUMER,
                    },
                )+
            )+
            BehaviorEntry {
                key: BehaviorKey {
                    dispatch_site: DispatchSite::TypeSubtype.id(),
                    selector: BehaviorSelector::TypeSubtype {
                        type_id: 10,
                        subtype: 45,
                    },
                },
                handler: ExistingHandler::Class10Effects,
                reachable_successors: &[BehaviorKey {
                    dispatch_site: DispatchSite::Constructor.id(),
                    selector: BehaviorSelector::ClassModel {
                        class: 10,
                        model: 45,
                    },
                }],
                required_source_fact_kinds: CONSTRUCTION_FACTS,
                ordinary_producer: "qualified authored class-10/subtype-45 row",
                same_pass_epilogue: CONSTRUCTOR_EPILOGUE,
                following_pass_consumer: FOLLOWING_CONSUMER,
            },
        ];
    };
}

compiled_registry! {
    actions {
        (TreeLifecycle, 2, [0, 1, 2]);
        (Stone, 2, [3]);
        (Dolmen, 2, [6]);
        (BadStone, 2, [9]);
        (PlayerFlight, 3, [0]);
        (NyphurPlayer, 3, [1]);
        (PlayerFlight, 3, [2, 3, 12]);
        (CastleRoot, 3, [4, 5, 6]);
        (ManaCarrier, 3, [9]);
        (DragonFamily, 5, [1, 2, 4, 5]);
        (GoatFamily, 5, [9, 10, 11, 12, 13, 14, 15]);
        (BeeFamily, 5, [16, 17, 18, 19, 20, 21, 22, 23]);
        (MultipartFamily, 5, [25, 26, 27, 28, 29, 30, 31]);
        (ArcherFamily, 5, [33, 34, 35, 36, 37, 38, 39]);
        (Model9Family, 5, [72, 73, 74, 76, 77]);
        (BuilderFamily, 5, [96, 97, 98, 99, 100, 101, 102, 103]);
        (VillagerFamily, 5, [104]);
        (BuilderVillagerPostBuild, 5, [105]);
        (VillagerFamily, 5, [106, 107, 108, 109, 110, 111]);
        (TraderFamily, 5, [112, 113, 114, 115, 116, 117, 118, 119]);
        (CastleAuxiliary, 5, [121, 122, 124, 125]);
        (FireflyFamily, 5, [153, 154, 155, 156, 157, 159]);
        (MultipartFamily, 5, [232]);
        (ProjectileFamily, 9, [0, 1, 10, 13]);
        (Model6ProjectileChild, 9, [15]);
        (ProjectileFamily, 9, [18]);
        (Class10Effects, 10, [0, 1, 2, 5, 6, 12, 13, 14, 17, 19, 22, 41, 42, 43, 44, 45, 48, 51, 52, 53, 58, 59, 64, 65, 76, 77, 83, 86, 98]);
        (SwitchProducers, 11, [0, 1, 4, 12]);
        (ObjectiveSwitch, 11, [32]);
        (Class14Events, 14, [8, 10]);
        (SpellHolderAndPickup, 15, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 18, 19, 20, 33, 34, 35, 36, 37, 38, 78]);
    }
    update_only_actions {
        (BallisticScenery, 2, [21]);
        (Model9Family, 5, [79]);
        (TerrainStripController, 10, [68, 69]);
        (CountdownSwitch, 11, [16, 17]);
        (TerrainStripController, 14, [6]);
        (SpellHolderAndPickup, 15, [12, 13]);
    }
    constructors {
        (TreeLifecycle, 2, [0]);
        (Stone, 2, [1]);
        (Dolmen, 2, [2]);
        (BadStone, 2, [3]);
        (BallisticScenery, 2, [8]);
        (PlayerFlight, 3, [0]);
        (NyphurPlayer, 3, [1]);
        (CastleRoot, 3, [2]);
        (ManaCarrier, 3, [3]);
        (DragonFamily, 5, [0]);
        (GoatFamily, 5, [1]);
        (BeeFamily, 5, [2]);
        (MultipartFamily, 5, [3]);
        (ArcherFamily, 5, [4]);
        (Model9Family, 5, [9]);
        (BuilderFamily, 5, [12]);
        (VillagerFamily, 5, [13]);
        (TraderFamily, 5, [14]);
        (CastleAuxiliary, 5, [15]);
        (FireflyFamily, 5, [19]);
        (ProjectileFamily, 9, [0, 1, 10, 13]);
        (Model6ProjectileChild, 9, [14]);
        (ProjectileFamily, 9, [17]);
        (Class10Effects, 10, [0, 1, 2, 5, 6, 12, 13, 14, 17, 19, 22, 39, 40, 41, 42, 43, 45, 53, 54, 59, 60, 70, 76, 79, 91]);
        (TerrainStripController, 10, [63, 64]);
        (SwitchProducers, 11, [0, 1, 4, 12]);
        (CountdownSwitch, 11, [16, 17]);
        (ObjectiveSwitch, 11, [32]);
        (TerrainStripController, 14, [1]);
        (Class14Events, 14, [3, 5]);
        (SpellHolderAndPickup, 15, [0, 1, 2, 3, 6, 11, 12]);
    }
}

// The declaration macro intentionally keeps the long-standing two-site
// compatibility set separate from source-proven UpdateEntities-only entries.
// Sorting once preserves the immutable binary-search contract without forcing
// the declarations back into a false ApplyEvents/UpdateEntities cross-product.
static SORTED_COMPILED_BEHAVIOR_ENTRIES: LazyLock<Vec<BehaviorEntry>> = LazyLock::new(|| {
    let mut entries = COMPILED_BEHAVIOR_ENTRIES.to_vec();
    entries.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    entries
});

/// Zero-sized access point for the immutable compiled registry.
#[derive(Debug, Clone, Copy, Default)]
pub struct StaticBehaviorRegistry;

pub static CAMPAIGN_BEHAVIOR_REGISTRY: StaticBehaviorRegistry = StaticBehaviorRegistry;

impl StaticBehaviorRegistry {
    pub fn entries(self) -> &'static [BehaviorEntry] {
        &SORTED_COMPILED_BEHAVIOR_ENTRIES
    }

    pub fn entry(self, key: &BehaviorKey) -> Option<&'static BehaviorEntry> {
        SORTED_COMPILED_BEHAVIOR_ENTRIES
            .binary_search_by(|candidate| candidate.key.cmp(key))
            .ok()
            .map(|index| &SORTED_COMPILED_BEHAVIOR_ENTRIES[index])
    }

    /// Require a declared package set to include every compiled successor.
    /// This validates metadata only and never constructs a session.
    pub fn validate_declared_closure(
        self,
        declared: &BTreeSet<BehaviorKey>,
    ) -> Result<(), RegistryValidationError> {
        for key in declared {
            let entry = self
                .entry(key)
                .ok_or_else(|| RegistryValidationError::UnknownImplementedKey(key.clone()))?;
            for successor in entry.reachable_successors {
                if !declared.contains(successor) {
                    return Err(RegistryValidationError::MissingTransitiveRequirement {
                        predecessor: key.clone(),
                        missing: successor.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

impl CampaignCapabilityRegistry for StaticBehaviorRegistry {
    fn contains(&self, key: &BehaviorKey) -> bool {
        self.entry(key).is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryValidationError {
    UnknownDispatchSite(u16),
    SelectorSiteMismatch(BehaviorKey),
    DeclarationOrder,
    DuplicateKey(BehaviorKey),
    MissingSuccessor {
        predecessor: BehaviorKey,
        missing: BehaviorKey,
    },
    MissingFromSource(BehaviorKey),
    UnknownImplementedKey(BehaviorKey),
    MissingTransitiveRequirement {
        predecessor: BehaviorKey,
        missing: BehaviorKey,
    },
}

#[cfg(test)]
fn validate_key_shape(key: &BehaviorKey) -> Result<(), RegistryValidationError> {
    let site = DispatchSite::from_id(key.dispatch_site).ok_or(
        RegistryValidationError::UnknownDispatchSite(key.dispatch_site),
    )?;
    let matches = matches!(
        (site, &key.selector),
        (
            DispatchSite::ApplyEvents | DispatchSite::UpdateEntities,
            BehaviorSelector::ClassAction { .. }
        ) | (
            DispatchSite::Constructor,
            BehaviorSelector::ClassModel { .. }
        ) | (
            DispatchSite::TypeSubtype,
            BehaviorSelector::TypeSubtype { .. }
        ) | (
            DispatchSite::Direct,
            BehaviorSelector::DirectCallback { .. }
        )
    );
    if matches {
        Ok(())
    } else {
        Err(RegistryValidationError::SelectorSiteMismatch(key.clone()))
    }
}

#[cfg(test)]
fn validate_entries(entries: &[BehaviorEntry]) -> Result<(), RegistryValidationError> {
    let mut previous: Option<&BehaviorKey> = None;
    let all_keys = entries
        .iter()
        .map(|entry| entry.key.clone())
        .collect::<BTreeSet<_>>();
    if all_keys.len() != entries.len() {
        let mut seen = BTreeSet::new();
        let duplicate = entries
            .iter()
            .find(|entry| !seen.insert(entry.key.clone()))
            .expect("different lengths prove a duplicate");
        return Err(RegistryValidationError::DuplicateKey(duplicate.key.clone()));
    }
    for entry in entries {
        validate_key_shape(&entry.key)?;
        if previous.is_some_and(|key| key >= &entry.key) {
            return Err(RegistryValidationError::DeclarationOrder);
        }
        previous = Some(&entry.key);
        if entry.ordinary_producer.is_empty()
            || entry.same_pass_epilogue.is_empty()
            || entry.following_pass_consumer.is_empty()
        {
            return Err(RegistryValidationError::SelectorSiteMismatch(
                entry.key.clone(),
            ));
        }
        if entry
            .required_source_fact_kinds
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(RegistryValidationError::DeclarationOrder);
        }
        for successor in entry.reachable_successors {
            validate_key_shape(successor)?;
            if !all_keys.contains(successor) {
                return Err(RegistryValidationError::MissingSuccessor {
                    predecessor: entry.key.clone(),
                    missing: successor.clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn source_key(key: &BehaviorKey) -> String {
    let site = DispatchSite::from_id(key.dispatch_site)
        .expect("compiled entries use validated dispatch sites")
        .name();
    match key.selector {
        BehaviorSelector::ClassAction { class, action } => {
            format!("{site}:ClassAction:{class}:{action}")
        }
        BehaviorSelector::ClassModel { class, model } => {
            format!("{site}:ClassModel:{class}:{model}")
        }
        BehaviorSelector::TypeSubtype { type_id, subtype } => {
            format!("{site}:TypeSubtype:{type_id}:{subtype}")
        }
        BehaviorSelector::DirectCallback { stable_callback_id } => {
            format!("{site}:DirectCallback:{stable_callback_id}")
        }
    }
}

#[cfg(test)]
fn validate_against_source(
    entries: &[BehaviorEntry],
    source_keys: &BTreeSet<String>,
) -> Result<(), RegistryValidationError> {
    validate_entries(entries)?;
    for entry in entries {
        if !source_keys.contains(&source_key(&entry.key)) {
            return Err(RegistryValidationError::MissingFromSource(
                entry.key.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlayerCommand,
        native_world::{Level2ResetSession, NativeWorld},
    };
    use serde_json::Value;
    use std::{collections::BTreeMap, path::PathBuf};

    const INVENTORY: &str = include_str!(
        "../../../../../remc2-atlas/campaign_registry_probe/str-d4c48ar-inventory.json"
    );
    const REGISTRY: &str = include_str!(
        "../../../../../remc2-atlas/campaign_registry_probe/rust_behavior_registry.json"
    );
    const DIFF: &str =
        include_str!("../../../../../remc2-atlas/campaign_registry_probe/rust-diff.json");
    const OMEGA: &str =
        include_str!("../../../../level2-conformance/fold/campaign-fold-omega-v1.1.json");

    fn level_two_fixture_path() -> PathBuf {
        std::env::var_os("MC_LEVEL2_PACKAGE_V5").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/l2-debug3.fJ3PaM/level2")
            },
            PathBuf::from,
        )
    }

    fn source_site_keys(inventory: &Value) -> BTreeSet<String> {
        inventory["source_site_keys"]
            .as_array()
            .expect("source_site_keys array")
            .iter()
            .map(|item| item["key"].as_str().expect("source site key").to_owned())
            .collect()
    }

    fn cf10_supplemental_handlers() -> BTreeMap<String, &'static str> {
        let mut expected = BTreeMap::new();
        for (class, actions, handler) in [
            (3_u16, &[2_u16, 3, 12][..], "player-flight"),
            (3, &[4, 6][..], "castle-root"),
            (5, &[1, 2, 4, 5][..], "dragon-family"),
            (
                5,
                &[16, 17, 18, 19, 20, 21, 22, 23][..],
                "level2-bee-family",
            ),
            (5, &[74, 76, 77][..], "level2-model9-family"),
            (5, &[121, 122, 124, 125][..], "level2-castle-auxiliary"),
            (11, &[4][..], "level2-switch-producers"),
        ] {
            for action in actions {
                for site in ["ApplyEvents", "UpdateEntities"] {
                    expected.insert(format!("{site}:ClassAction:{class}:{action}"), handler);
                }
            }
        }
        expected.insert("Constructor:ClassModel:5:2".to_owned(), "level2-bee-family");
        expected.insert("Constructor:ClassModel:5:0".to_owned(), "dragon-family");
        expected.insert(
            "Constructor:ClassModel:5:15".to_owned(),
            "level2-castle-auxiliary",
        );
        expected.insert(
            "Constructor:ClassModel:11:4".to_owned(),
            "level2-switch-producers",
        );
        for (class, actions, handler) in [
            (2_u16, &[21_u16][..], "ballistic-scenery"),
            (5, &[79][..], "level2-model9-family"),
            (10, &[68, 69][..], "terrain-strip-controller"),
            (11, &[16, 17][..], "absence-countdown-switch"),
            (14, &[6][..], "terrain-strip-controller"),
            (15, &[12, 13][..], "spell-holder-and-pickup-family"),
        ] {
            for action in actions {
                expected.insert(
                    format!("UpdateEntities:ClassAction:{class}:{action}"),
                    handler,
                );
            }
        }
        for (class, models, handler) in [
            (2_u16, &[8_u16][..], "ballistic-scenery"),
            (10, &[63, 64][..], "terrain-strip-controller"),
            (11, &[16, 17][..], "absence-countdown-switch"),
            (14, &[1][..], "terrain-strip-controller"),
        ] {
            for model in models {
                expected.insert(format!("Constructor:ClassModel:{class}:{model}"), handler);
            }
        }
        expected
    }

    #[test]
    fn cf03_compiled_registry_matches_source_sites() {
        let inventory: Value = serde_json::from_str(INVENTORY).expect("checked inventory JSON");
        let registry: Value = serde_json::from_str(REGISTRY).expect("checked registry JSON");
        let diff: Value = serde_json::from_str(DIFF).expect("checked diff JSON");
        let source_keys = source_site_keys(&inventory);

        assert_eq!(inventory["class_count"], 17);
        assert_eq!(inventory["summary"]["source_behavior_key_count"], 486);
        assert_eq!(diff["summary"]["matching_key_count"], 127);
        assert_eq!(diff["summary"]["matching_site_key_count"], 315);
        assert_eq!(
            diff["summary"]["implemented_site_key_missing_from_source_count"],
            0
        );
        assert_eq!(registry["action_dispatch_sites"][0], "ApplyEvents");
        assert_eq!(registry["action_dispatch_sites"][1], "UpdateEntities");
        assert_eq!(registry["constructor_dispatch_site"], "Constructor");
        assert_eq!(CAMPAIGN_BEHAVIOR_REGISTRY.entries().len(), 384);
        validate_against_source(CAMPAIGN_BEHAVIOR_REGISTRY.entries(), &source_keys)
            .expect("compiled entries retain exact pinned sites and selector shapes");

        let report_keys = diff["matching_site_keys"]
            .as_array()
            .expect("matching_site_keys array")
            .iter()
            .map(|item| item.as_str().expect("report key").to_owned())
            .collect::<BTreeSet<_>>();
        let compiled_keys = CAMPAIGN_BEHAVIOR_REGISTRY
            .entries()
            .iter()
            .map(|entry| source_key(&entry.key))
            .collect::<BTreeSet<_>>();
        assert!(
            report_keys.is_subset(&compiled_keys),
            "the sealed CF03 archaeology receipt remains a compiled subset"
        );
        let supplemental_keys = compiled_keys
            .difference(&report_keys)
            .cloned()
            .collect::<BTreeSet<_>>();
        let supplemental_handlers = cf10_supplemental_handlers();
        assert_eq!(
            supplemental_keys,
            supplemental_handlers.keys().cloned().collect(),
            "only source-backed handlers accepted after the CF03 receipt may extend the registry"
        );

        let mut expected_handlers = BTreeMap::new();
        for group in registry["entries"].as_array().expect("registry entries") {
            let name = group["name"].as_str().expect("registry family name");
            let class = group["class"].as_u64().expect("registry class");
            let actions = group["level_scopes"]
                .as_object()
                .expect("level scopes")
                .values()
                .flat_map(|items| items.as_array().expect("level actions"))
                .map(|action| action.as_u64().expect("action"))
                .collect::<BTreeSet<_>>();
            for action in actions {
                for site in ["ApplyEvents", "UpdateEntities"] {
                    expected_handlers.insert(format!("{site}:ClassAction:{class}:{action}"), name);
                }
            }
            for model in group["constructor_models"]
                .as_array()
                .expect("constructor models")
            {
                expected_handlers.insert(
                    format!(
                        "Constructor:ClassModel:{class}:{}",
                        model.as_u64().expect("constructor model")
                    ),
                    name,
                );
            }
        }
        for entry in registry["type_subtype_entries"]
            .as_array()
            .expect("type-subtype entries")
        {
            expected_handlers.insert(
                format!(
                    "TypeSubtype:TypeSubtype:{}:{}",
                    entry["type_id"].as_u64().expect("type id"),
                    entry["subtype"].as_u64().expect("subtype")
                ),
                entry["handler"].as_str().expect("subtype handler"),
            );
        }
        assert_eq!(expected_handlers.len(), 315);
        expected_handlers.extend(supplemental_handlers);
        assert_eq!(expected_handlers.len(), 384);

        for entry in CAMPAIGN_BEHAVIOR_REGISTRY.entries() {
            assert_eq!(
                expected_handlers.get(&source_key(&entry.key)).copied(),
                Some(entry.handler.registry_name())
            );
            assert!(!entry.ordinary_producer.is_empty());
            assert!(!entry.same_pass_epilogue.is_empty());
            assert!(!entry.following_pass_consumer.is_empty());
        }

        // Missing source key.
        let mut missing_source = source_keys.clone();
        missing_source.remove(&source_key(&CAMPAIGN_BEHAVIOR_REGISTRY.entries()[0].key));
        assert!(matches!(
            validate_against_source(CAMPAIGN_BEHAVIOR_REGISTRY.entries(), &missing_source),
            Err(RegistryValidationError::MissingFromSource(_))
        ));

        // Extra Rust key.
        let mut extra_rust = CAMPAIGN_BEHAVIOR_REGISTRY.entries().to_vec();
        let mut extra = extra_rust.last().expect("registry is non-empty").clone();
        extra.key = BehaviorKey {
            dispatch_site: DispatchSite::Direct.id(),
            selector: BehaviorSelector::DirectCallback {
                stable_callback_id: u16::MAX,
            },
        };
        extra.reachable_successors = &[];
        extra_rust.push(extra);
        assert!(matches!(
            validate_against_source(&extra_rust, &source_keys),
            Err(RegistryValidationError::MissingFromSource(_))
        ));

        // Wrong site for an otherwise unchanged class/action selector.
        let mut wrong_site = CAMPAIGN_BEHAVIOR_REGISTRY.entries().to_vec();
        wrong_site[0].key.dispatch_site = DispatchSite::Constructor.id();
        assert!(matches!(
            validate_entries(&wrong_site),
            Err(RegistryValidationError::SelectorSiteMismatch(_))
        ));

        // A constructor cannot be flattened into the action selector shape.
        let mut flattened = CAMPAIGN_BEHAVIOR_REGISTRY.entries().to_vec();
        let constructor = flattened
            .iter_mut()
            .find(|entry| entry.key.dispatch_site == DispatchSite::Constructor.id())
            .expect("compiled constructor entry");
        let constructor_selector = match constructor.key.selector {
            BehaviorSelector::ClassModel { class, model } => Some((class, model)),
            _ => None,
        };
        let (class, model) = constructor_selector.expect("constructor entry uses ClassModel");
        constructor.key.selector = BehaviorSelector::ClassAction {
            class,
            action: model,
        };
        assert!(matches!(
            validate_entries(&flattened),
            Err(RegistryValidationError::SelectorSiteMismatch(_))
        ));

        // Declaration order is part of the compiled contract.
        let mut permuted = CAMPAIGN_BEHAVIOR_REGISTRY.entries().to_vec();
        permuted.swap(0, 1);
        assert_eq!(
            validate_entries(&permuted),
            Err(RegistryValidationError::DeclarationOrder)
        );

        // An ApplyEvents root requires the matching UpdateEntities key.
        let root = CAMPAIGN_BEHAVIOR_REGISTRY.entries()[0].key.clone();
        let declared = BTreeSet::from([root.clone()]);
        assert!(matches!(
            CAMPAIGN_BEHAVIOR_REGISTRY.validate_declared_closure(&declared),
            Err(RegistryValidationError::MissingTransitiveRequirement {
                predecessor,
                ..
            }) if predecessor == root
        ));
    }

    #[test]
    fn cf03_existing_handlers_preserve_omega() {
        assert!(OMEGA.contains("\"schema_version\": \"campaign-fold-omega-v1.1\""));
        assert!(OMEGA.contains("\"json_byte_identical\": true"));

        let inspection = NativeWorld::inspect_level2(level_two_fixture_path())
            .expect("qualified Level 2 Omega fixture loads");
        let mut session: Level2ResetSession = inspection
            .level2_reset_session()
            .expect("qualified Level 2 Omega session opens");
        let initial_hash = session.campaign_state_hash();
        let initial_terrain_hash = session.terrain_hash();

        let first_evidence = session.step_outer_turn(PlayerCommand::default(), 0);
        let first_events = session.dispatch_events().to_vec();
        let stepped_hash = session.campaign_state_hash();
        let stepped_terrain_hash = session.terrain_hash();
        assert!(!first_evidence.is_empty());
        assert!(!first_events.is_empty());

        session.reset();
        assert_eq!(session.campaign_state_hash(), initial_hash);
        assert_eq!(session.terrain_hash(), initial_terrain_hash);
        assert!(session.dispatch_events().is_empty());

        assert_eq!(
            session.step_outer_turn(PlayerCommand::default(), 0),
            first_evidence
        );
        assert_eq!(session.dispatch_events(), first_events.as_slice());
        assert_eq!(session.campaign_state_hash(), stepped_hash);
        assert_eq!(session.terrain_hash(), stepped_terrain_hash);
    }
}
