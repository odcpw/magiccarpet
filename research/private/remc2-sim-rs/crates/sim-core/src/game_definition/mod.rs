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
    ENEMY_DEFAULT_POLICY_ID, FIREBALL_MANA_COST_ID, PLAYER_MAX_FORWARD_SPEED_ID,
};
pub use sim_definition::runtime_flow::{
    RuntimeFlowDefinition, RuntimeFlowDestination, RuntimeFlowEntry, RuntimeFlowNodeId,
    SemanticGameId, SemanticLevelId,
};
pub use sim_definition::{
    APPLY_RECEIPT_SCHEMA, ARCHER_MAX_LIFE_CAPABILITY_ID, Access, ApplyBinding, ApplyReceipt,
    ApplyStatus, ArtifactId, ArtifactKind, ArtifactNode, BEE_MAX_LIFE_CAPABILITY_ID,
    BoundPresentationAsset, CHANGE_SET_SCHEMA, CREATURE_POLICY_CAPABILITY_ID, ChangeSet,
    ChangeSetBuilder, CompiledCapability, CompiledCapabilityRegistry,
    DRAGON_MAX_LIFE_CAPABILITY_ID, DefinitionDiff, DefinitionError, DefinitionErrorCode,
    DefinitionOperation, DescriptorKind, Diagnostic, FIREBALL_MANA_COST_CAPABILITY_ID,
    FIREFLY_MAX_LIFE_CAPABILITY_ID, GAME_DEFINITION_SCHEMA, GAME_PACKAGE_SCHEMA,
    GOAT_MAX_LIFE_CAPABILITY_ID, GameDefinition, GamePackage, INDEXED_SPRITE_SCHEMA,
    IndexedPresentationAsset, IntegerParameter, MutationTiming, OptionParameter, PackageProvenance,
    PackageProvenanceKind, PresentationAssetBudgets, PresentationAssetSlot, PublicationFault,
    ReadBackBinding, ReadBinding, RequestedTiming, RequiredCapability, Result,
    SEMANTIC_CONTRACT_VERSION, SKELETON_MAX_LIFE_CAPABILITY_ID, SemanticBinding, SemanticCatalog,
    SemanticDescriptor, SemanticId, SemanticValue, TRADER_MAX_LIFE_CAPABILITY_ID, TemporalContract,
    TransactionError, TransactionErrorCode, VALIDATION_RECEIPT_SCHEMA, ValidateBinding,
    ValidationReceipt, ValueType, WORKSPACE_SCHEMA, WorkspaceInspection, WorkspaceStore,
    creature_max_life_capability_id, default_catalog, engine_capabilities,
};
