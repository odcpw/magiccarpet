//! Neutral definition-domain facade plus the private MC2 owner adapter.
//!
//! Immutable package, catalog, level-composition, and transaction authority
//! live in `sim-definition`. The owner-data loader remains here because it
//! consumes the live MC2 world adapter.

mod mc2_owned;
mod runtime;

pub use mc2_owned::{Mc2OwnedInputs, load_mc2_owned};
pub use runtime::{
    DefinitionFireballBinding, DefinitionFlightBinding, DefinitionGoatPolicyBinding,
    ENEMY_DEFAULT_POLICY_ID, FIREBALL_MANA_COST_ID, PLAYER_FORWARD_ACCELERATION_ID,
    PLAYER_MAX_FORWARD_SPEED_ID,
};
pub use sim_definition::runtime_flow::{
    RuntimeFlowDefinition, RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
    RuntimeFlowPresentationDefinition, RuntimeFlowPresentationEntry, SemanticGameId,
    SemanticLevelId, SemanticPresentationCueId, TITLE_PRESENTATION_CUE_ID,
};
pub use sim_definition::{
    APPLY_RECEIPT_SCHEMA, ARCHER_MAX_LIFE_CAPABILITY_ID, AREA_ENTRY_OVERLAP_EXTENT_CAPABILITY_ID,
    AREA_EXIT_OVERLAP_EXTENT_CAPABILITY_ID, Access, ApplyBinding, ApplyReceipt, ApplyStatus,
    ArtifactId, ArtifactKind, ArtifactNode, BEE_MAX_LIFE_CAPABILITY_ID, BoundPresentationAsset,
    CHANGE_SET_SCHEMA, CREATURE_POLICY_CAPABILITY_ID, ChangeSet, ChangeSetBuilder,
    CompiledCapability, CompiledCapabilityRegistry, DRAGON_MAX_LIFE_CAPABILITY_ID, DefinitionDiff,
    DefinitionError, DefinitionErrorCode, DefinitionOperation, DescriptorKind, Diagnostic,
    EXIT_SEQUENCE_OVERLAP_EXTENT_CAPABILITY_ID, EXPERIENCE_SCROLL_AWARD_CAPABILITY_ID,
    FIREBALL_LAUNCH_SOUND_ASSET_SLOT_ID, FIREBALL_MANA_COST_CAPABILITY_ID,
    FIREFLY_MAX_LIFE_CAPABILITY_ID, GAME_DEFINITION_SCHEMA, GAME_PACKAGE_SCHEMA,
    GOAT_MAX_LIFE_CAPABILITY_ID, GameDefinition, GamePackage,
    HOUSE_CONSTRUCTION_VISITS_CAPABILITY_ID, HOUSE_LIFE_MANA_SCALAR_CAPABILITY_ID,
    INDEXED_SPRITE_SCHEMA, IndexedPresentationAsset, IntegerParameter, MutationTiming,
    OptionParameter, PackageProvenance, PackageProvenanceKind, PresentationAssetBudgets,
    PresentationAssetSlot, PublicationFault, ReadBackBinding, ReadBinding, RequestedTiming,
    RequiredCapability, Result, SEMANTIC_CONTRACT_VERSION, SKELETON_MAX_LIFE_CAPABILITY_ID,
    SemanticBinding, SemanticCatalog, SemanticDescriptor, SemanticId, SemanticValue,
    TRADER_MAX_LIFE_CAPABILITY_ID, TemporalContract, TransactionError, TransactionErrorCode,
    VALIDATION_RECEIPT_SCHEMA, VILLAGER_MAX_LIFE_CAPABILITY_ID, ValidateBinding, ValidationReceipt,
    ValueType, WORKSPACE_SCHEMA, WORM_MAX_LIFE_CAPABILITY_ID, WorkspaceInspection, WorkspaceStore,
    creature_max_life_capability_id, default_catalog, engine_capabilities,
};
