//! Neutral definition-domain facade plus the private MC2 owner adapter.
//!
//! Immutable package, catalog, level-composition, and transaction authority
//! live in `sim-definition`. The owner-data loader remains here because it
//! consumes the live MC2 world adapter.

mod mc2_owned;

pub use mc2_owned::{Mc2OwnedInputs, load_mc2_owned};
pub use sim_definition::{
    APPLY_RECEIPT_SCHEMA, Access, ApplyBinding, ApplyReceipt, ApplyStatus, ArtifactKind,
    ArtifactNode, BoundPresentationAsset, CHANGE_SET_SCHEMA, ChangeSet, ChangeSetBuilder,
    CompiledCapability, CompiledCapabilityRegistry, DefinitionDiff, DefinitionError,
    DefinitionErrorCode, DescriptorKind, Diagnostic, GAME_DEFINITION_SCHEMA, GAME_PACKAGE_SCHEMA,
    GameDefinition, GamePackage, INDEXED_SPRITE_SCHEMA, IndexedPresentationAsset, MutationTiming,
    PackageProvenance, PackageProvenanceKind, PresentationAssetBudgets, PublicationFault,
    ReadBackBinding, ReadBinding, Replacement, RequestedTiming, RequiredCapability, Result,
    SEMANTIC_CONTRACT_VERSION, SemanticBinding, SemanticCatalog, SemanticDescriptor, SemanticId,
    SemanticValue, TemporalContract, TransactionError, TransactionErrorCode,
    VALIDATION_RECEIPT_SCHEMA, ValidateBinding, ValidationReceipt, ValueType, WORKSPACE_SCHEMA,
    WorkspaceInspection, WorkspaceStore, default_catalog, engine_capabilities,
};
