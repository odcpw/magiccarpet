//! Reward policy carried from the actual campaign entry into gameplay.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Completed portal reentry suppresses earned experience until the source
/// secret-exit completion clears that context. An unfinished-secret
/// continuation inherits the current policy from its parent session.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionRewards {
    #[default]
    Enabled,
    Suppressed,
}

impl MissionRewards {
    #[must_use]
    pub const fn allows_experience(self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub(crate) fn append_hash(self, hasher: &mut Sha256) {
        if self == Self::Suppressed {
            hasher.update(b"mission-rewards-v1");
            hasher.update([1]);
        }
    }
}
