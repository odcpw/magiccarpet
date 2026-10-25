//! Recovery of a pending level open without retaining a gameplay session.

use super::*;

/// The menu boundary that owns a pending load. Failed opens retain this value;
/// publishing a gameplay session consumes it with the Loading state.
#[derive(Clone, Debug)]
pub(super) enum LoadingOrigin {
    Title {
        presentation_cue: SemanticPresentationCueId,
    },
    Campaign {
        definition: Arc<GameDefinition>,
        selected_definition: Option<Arc<GameDefinition>>,
        phase: CampaignMenuPhase,
        completed_level: Option<SemanticLevelId>,
    },
}

#[cfg(test)]
impl Default for LoadingOrigin {
    fn default() -> Self {
        Self::Title {
            presentation_cue: SemanticPresentationCueId::parse(TITLE_PRESENTATION_CUE_ID).unwrap(),
        }
    }
}

impl<B: GameSessionBackend> GameRuntime<B> {
    /// Abandon a pending load and restore its exact menu origin.
    ///
    /// No gameplay session is created or retained. The installed-package owner
    /// must use this boundary to discard an abandoned new-game carry while
    /// preserving campaign carry for a later mission selection.
    ///
    /// # Errors
    ///
    /// Stale guards, non-Loading modes and generation overflow leave the owner
    /// unchanged. A failed open can instead be retried with the current guard.
    pub fn cancel_loading(
        &mut self,
        guard: RuntimeGuard,
    ) -> Result<RuntimeIdentity, RuntimeTransitionError> {
        self.validate_guard(guard)
            .map_err(Box::new)
            .map_err(RuntimeTransitionError::Guard)?;
        let RuntimeState::Loading {
            session_generation,
            origin,
            ..
        } = &self.state
        else {
            return Err(RuntimeTransitionError::InvalidMode {
                expected: RuntimeMode::Loading,
                actual: self.identity().mode,
            });
        };
        let next_generation = self.next_generation()?;
        let session_generation = *session_generation;
        let state = match origin.clone() {
            LoadingOrigin::Title { presentation_cue } => RuntimeState::Title {
                session_generation,
                presentation_cue,
            },
            LoadingOrigin::Campaign {
                definition,
                selected_definition,
                phase,
                completed_level,
            } => RuntimeState::Campaign {
                definition,
                selected_definition,
                session_generation,
                phase,
                completed_level,
            },
        };
        let returns_to_title = matches!(state, RuntimeState::Title { .. });
        self.state = state;
        self.generation = next_generation;
        if returns_to_title {
            self.frontend_media.return_to_title();
        }
        Ok(self.identity())
    }
}
