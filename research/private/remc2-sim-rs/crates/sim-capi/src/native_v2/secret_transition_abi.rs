//! Tagged JSON envelope for one accepted runtime boundary transition.
//!
//! The ordinary completion receipt and the source-owned secret transition
//! receipt remain distinct payload types. The tag is authoritative; consumers
//! must not infer the transition kind from the resulting runtime mode.

use serde::Serialize;
use sim_core::game_runtime::{CompletionTransitionReceipt, SecretTransitionReceipt};

/// The sole stored accepted runtime boundary receipt.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum BoundaryTransitionReceipt {
    MissionRetry,
    OrdinaryCompletion {
        ordinary_completion: CompletionTransitionReceipt,
    },
    SecretTransition {
        secret_transition: SecretTransitionReceipt,
    },
}

impl BoundaryTransitionReceipt {
    pub(super) fn ordinary_completion(&self) -> Option<&CompletionTransitionReceipt> {
        match self {
            Self::OrdinaryCompletion {
                ordinary_completion,
            } => Some(ordinary_completion),
            Self::SecretTransition { .. } | Self::MissionRetry => None,
        }
    }

    pub(super) fn secret_transition(&self) -> Option<&SecretTransitionReceipt> {
        match self {
            Self::OrdinaryCompletion { .. } | Self::MissionRetry => None,
            Self::SecretTransition { secret_transition } => Some(secret_transition),
        }
    }

    /// Encode through the existing bounded CAPI JSON writer.
    pub(super) fn write_json(
        &self,
        buffer: *mut u8,
        capacity: u32,
        out_length: *mut u32,
    ) -> super::AbiResult<()> {
        super::game_session_abi::write_json(self, buffer, capacity, out_length)
    }
}
