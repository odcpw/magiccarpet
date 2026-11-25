//! Source-qualified Level 1 sprite dispositions and concrete spell rows.
//!
//! This module is deliberately renderer-independent. It freezes the source
//! tuple selected before a later compositor turns the submission into indexed
//! pixels. Unknown reachable tuples are errors rather than implicit skips.

use crate::{
    FlightState, Position,
    level1_mission::{
        CASTLE_ASSOCIATION_ACTION, CASTLE_ASSOCIATION_CLASS, CASTLE_ASSOCIATION_MODEL,
        CASTLE_DEFENSE_ACTION, CASTLE_DEFENSE_CLASS, CASTLE_DEFENSE_MODEL,
        CASTLE_DEFENSE_RENDER_INDEX, CastleAssociationEffect, CastleConstruction,
        CastleDefenseController, CastleDefenseShot, DEATH_BURST_ACTION, DEATH_BURST_CLASS,
        DEATH_BURST_MODEL, DeathBurst, DynamicResidentKind, DynamicResidentState, Level1SpeedTrail,
        MANA_SPHERE_ACTION, MANA_SPHERE_CLASS, MANA_SPHERE_MODEL, MANA_SPHERE_NEUTRAL_BASE_TMAP,
        MANA_SPHERE_PLAYER_ONE_BASE_TMAP, MANA_SPHERE_PLAYER_ONE_RENDER_BASE,
        MANA_SPHERE_RENDER_MODE, ManaSphere, PLAYER_DEATH_MARKER_ACTION, PLAYER_DEATH_MARKER_CLASS,
        PLAYER_DEATH_MARKER_MODEL, PLAYER_DEATH_MARKER_PARTICLE_PARAMETER, PlayerDeathMarker,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteAnimationOwner {
    SourceEntity,
    SharedTmap,
    Static,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpritePaletteOwner {
    NightLevel,
    PlayerColour,
    SourceEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpriteSubmission {
    pub source_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub control_flags: u32,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub xclass: u8,
    pub xmodel: u8,
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub selected_frame: u16,
    pub render_mode: u8,
    pub logical_height: u16,
    pub animation_owner: SourceSpriteAnimationOwner,
    pub palette_owner: SourceSpritePaletteOwner,
    pub shadow_eligible: bool,
    pub reflection_eligible: bool,
}

impl SourceSpriteSubmission {
    #[must_use]
    pub const fn from_template(
        source_entity_index: u16,
        position: Position,
        yaw: i16,
        pitch: i16,
        roll: i16,
        control_flags: u32,
        class: u8,
        model: u8,
        action: u8,
        xclass: u8,
        xmodel: u8,
        template: SourceSpriteTemplate,
        selected_frame: u16,
    ) -> Self {
        Self {
            source_entity_index,
            position,
            yaw,
            pitch,
            roll,
            control_flags,
            class,
            model,
            action,
            xclass,
            xmodel,
            render_parameter_index: template.render_parameter_index,
            base_tmap: template.base_tmap,
            selected_frame,
            render_mode: template.render_mode,
            logical_height: template.logical_height,
            animation_owner: template.animation_owner,
            palette_owner: template.palette_owner,
            shadow_eligible: template.shadow_eligible && control_flags & 0x20 == 0,
            reflection_eligible: template.reflection_eligible && control_flags & 0x20 == 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteExclusionReason {
    SourceNoDrawFlags,
    UnlinkedSpeedTrail,
    CastleConstruction,
    HiddenSourceModel,
    SpriteLessSwitch,
    UnreachableHostileOrRemote,
    FixedProfileStereoOrAlternateResolution,
}

/// Existing source-qualified renderer selected only after the raw tuple has
/// passed the CP1 census. This keeps legacy family-specific clocks and
/// selectors without allowing an untyped fallthrough.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteLegacyFamily {
    ImportedReset,
    AuthoredActor,
    Ambient,
    Projectile,
    Impact,
    Economy,
    Multipart,
    Goat,
    Resident,
    SpellPickup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteDisposition {
    Render(SourceSpriteSubmission),
    Legacy {
        source_entity_index: u16,
        family: SourceSpriteLegacyFamily,
    },
    Excluded {
        source_entity_index: u16,
        reason: SourceSpriteExclusionReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteContractRoute {
    Render {
        template: SourceSpriteTemplate,
        selected_frame: u16,
    },
    Legacy(SourceSpriteLegacyFamily),
    Excluded(SourceSpriteExclusionReason),
}

/// Select the already-qualified legacy compositor family for one exhaustive
/// Level 1 allocation kind.
///
/// This function is intentionally data-only: the mission owns live payload
/// lookup, while the source contract owns the tuple disposition. Hidden and
/// unlinked constructors become explicit typed exclusions here.
pub fn legacy_route_for_spawn_kind(
    kind: Level1SourceSpawnKind,
) -> Result<SourceSpriteContractRoute, String> {
    use Level1SourceSpawnKind as Kind;
    let route = match kind {
        Kind::Reserved => {
            return Err("reserved source slot has no sprite disposition".to_owned());
        }
        Kind::ImportedResetTuple => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::ImportedReset)
        }
        Kind::AuthoredActivationTuple
        | Kind::AuthoredTerrainFeature
        | Kind::AuthoredPlayerMarker
        | Kind::AuthoredSwitch => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::AuthoredActor)
        }
        Kind::PlayerFireball
        | Kind::ArcherArrow
        | Kind::FireflyFireball
        | Kind::CastleBuild
        | Kind::CastleDefenseShot
        | Kind::Possession => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Projectile)
        }
        Kind::CastleRoot | Kind::CastleDefenseController | Kind::ManaSphere | Kind::ManaCarrier => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Economy)
        }
        Kind::CastleAssociation
        | Kind::GroundFire
        | Kind::FireballImpact
        | Kind::WaterSplash
        | Kind::DeathBurst
        | Kind::PlayerDeathMarker
        | Kind::CheckpointSmoke => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Impact)
        }
        Kind::PossessionImpact | Kind::PossessionAttraction | Kind::StrongPossessionImpact => {
            SourceSpriteContractRoute::Excluded(SourceSpriteExclusionReason::HiddenSourceModel)
        }
        Kind::CastleConstruction => {
            SourceSpriteContractRoute::Excluded(SourceSpriteExclusionReason::CastleConstruction)
        }
        Kind::SpeedTrail => {
            SourceSpriteContractRoute::Excluded(SourceSpriteExclusionReason::UnlinkedSpeedTrail)
        }
        Kind::SpellPickup => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::SpellPickup)
        }
        Kind::Ambient => SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Ambient),
        Kind::MultipartSegment => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Multipart)
        }
        Kind::ResidentArcher
        | Kind::ResidentBuilder
        | Kind::ResidentVillager
        | Kind::ResidentTrader => {
            SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Resident)
        }
    };
    Ok(route)
}

/// State-independent tuple consumed by the CP1 typed contract.
///
/// `Level1MissionState` remains responsible only for aggregating current
/// state into these rows. The tuple-to-disposition construction, source
/// no-draw exclusion, and stable ordering live here beside the source census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpriteContractRow {
    pub source_entity_index: u16,
    pub position: Position,
    pub yaw: i16,
    pub pitch: i16,
    pub roll: i16,
    pub control_flags: u32,
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub xclass: u8,
    pub xmodel: u8,
    pub route: SourceSpriteContractRoute,
}

/// Convert source-qualified rows into the only three compositor outcomes.
///
/// The low-byte `0x21` source draw gate is applied only after a caller has
/// identified the tuple and supplied its typed route. It is therefore a
/// recognized exclusion rather than an untyped renderer shortcut.
pub fn source_sprite_dispositions(
    rows: impl IntoIterator<Item = SourceSpriteContractRow>,
) -> Result<Vec<SourceSpriteDisposition>, String> {
    let mut result = Vec::new();
    for row in rows {
        if row.source_entity_index == 0 {
            return Err(unknown_source_sprite_tuple(
                row.source_entity_index,
                row.class,
                row.model,
                row.action,
                0,
            ));
        }
        let disposition = if row.control_flags & 0x21 != 0 {
            SourceSpriteDisposition::Excluded {
                source_entity_index: row.source_entity_index,
                reason: SourceSpriteExclusionReason::SourceNoDrawFlags,
            }
        } else {
            match row.route {
                SourceSpriteContractRoute::Render {
                    template,
                    selected_frame,
                } => SourceSpriteDisposition::Render(SourceSpriteSubmission::from_template(
                    row.source_entity_index,
                    row.position,
                    row.yaw,
                    row.pitch,
                    row.roll,
                    row.control_flags,
                    row.class,
                    row.model,
                    row.action,
                    row.xclass,
                    row.xmodel,
                    template,
                    selected_frame,
                )),
                SourceSpriteContractRoute::Legacy(family) => SourceSpriteDisposition::Legacy {
                    source_entity_index: row.source_entity_index,
                    family,
                },
                SourceSpriteContractRoute::Excluded(reason) => SourceSpriteDisposition::Excluded {
                    source_entity_index: row.source_entity_index,
                    reason,
                },
            }
        };
        result.push(disposition);
    }
    result.sort_unstable_by_key(|disposition| match disposition {
        SourceSpriteDisposition::Render(submission) => submission.source_entity_index,
        SourceSpriteDisposition::Legacy {
            source_entity_index,
            ..
        }
        | SourceSpriteDisposition::Excluded {
            source_entity_index,
            ..
        } => *source_entity_index,
    });
    for pair in result.windows(2) {
        let source_entity_index = |disposition: &SourceSpriteDisposition| match disposition {
            SourceSpriteDisposition::Render(submission) => submission.source_entity_index,
            SourceSpriteDisposition::Legacy {
                source_entity_index,
                ..
            }
            | SourceSpriteDisposition::Excluded {
                source_entity_index,
                ..
            } => *source_entity_index,
        };
        if source_entity_index(&pair[0]) == source_entity_index(&pair[1]) {
            return Err(format!(
                "Level 1 source sprite contract duplicates slot {}",
                source_entity_index(&pair[0])
            ));
        }
    }
    Ok(result)
}

pub struct Level1SourceSpriteState<'a> {
    pub player: &'a FlightState,
    pub player_action: u8,
    pub player_control_flags: Option<u32>,
    pub neutral_owner_entity_indices: &'a [u16],
    pub residents: &'a [DynamicResidentState],
    pub resident_control_flags: &'a [(u16, u32)],
    pub death_bursts: &'a [DeathBurst],
    pub player_death_marker: Option<PlayerDeathMarker>,
    pub player_death_marker_control_flags: u32,
    pub castle_associations: &'a [CastleAssociationEffect],
    pub castle_association_control_flags: &'a [(u16, u32)],
    pub castle_defense_controllers: &'a [CastleDefenseController],
    pub castle_defense_control_flags: &'a [(u16, u32)],
    pub castle_defense_shots: &'a [CastleDefenseShot],
    pub castle_defense_shot_control_flags: &'a [(u16, u32)],
    pub mana_spheres: &'a [ManaSphere],
    pub mana_sphere_control_flags: &'a [(u16, u32)],
    pub castle_construction: Option<CastleConstruction>,
    pub speed_trails: &'a [Level1SpeedTrail],
}

const fn runtime_template(
    render_parameter_index: u16,
    base_tmap: u16,
    render_mode: u8,
    logical_height: u16,
    palette_owner: SourceSpritePaletteOwner,
) -> SourceSpriteTemplate {
    SourceSpriteTemplate {
        render_parameter_index,
        base_tmap,
        render_mode,
        logical_height,
        animation_owner: SourceSpriteAnimationOwner::SourceEntity,
        palette_owner,
        shadow_eligible: true,
        reflection_eligible: true,
    }
}

fn control_flags(rows: &[(u16, u32)], source_entity_index: u16) -> u32 {
    rows.iter()
        .find_map(|(index, flags)| (*index == source_entity_index).then_some(*flags))
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
const fn render_contract_row(
    source_entity_index: u16,
    position: Position,
    yaw: i16,
    pitch: i16,
    roll: i16,
    control_flags: u32,
    class: u8,
    model: u8,
    action: u8,
    xclass: u8,
    xmodel: u8,
    template: SourceSpriteTemplate,
    selected_frame: u16,
) -> SourceSpriteContractRow {
    SourceSpriteContractRow {
        source_entity_index,
        position,
        yaw,
        pitch,
        roll,
        control_flags,
        class,
        model,
        action,
        xclass,
        xmodel,
        route: SourceSpriteContractRoute::Render {
            template,
            selected_frame,
        },
    }
}

/// Pure construction of the CP2-owned source sprite rows.
///
/// The mission supplies current typed payloads and source-slot flags; this
/// module owns every class/model/action-to-template decision and the final
/// disposition conversion.
pub fn level1_source_sprite_dispositions(
    state: Level1SourceSpriteState<'_>,
) -> Result<Vec<SourceSpriteDisposition>, String> {
    let mut rows = Vec::new();
    if let Some(player_control_flags) = state.player_control_flags {
        rows.push(render_contract_row(
            state.player.entity_index,
            state.player.position,
            state.player.yaw,
            state.player.effective_pitch,
            state.player.roll_accumulator,
            player_control_flags,
            3,
            0,
            state.player_action,
            0,
            0,
            runtime_template(66, 80, 0, 200, SourceSpritePaletteOwner::PlayerColour),
            80,
        ));
    }
    rows.extend(state.residents.iter().filter_map(|resident| {
        let control_flags =
            control_flags(state.resident_control_flags, resident.source_entity_index);
        let row = match resident.kind {
            DynamicResidentKind::Builder | DynamicResidentKind::Trader
                if resident.active && resident.visible =>
            {
                render_contract_row(
                    resident.source_entity_index,
                    resident.position,
                    resident.yaw,
                    0,
                    0,
                    control_flags,
                    resident.class,
                    resident.model,
                    resident.action,
                    0,
                    0,
                    runtime_template(
                        resident.render_parameter_index,
                        resident.base_tmap,
                        resident.render_mode,
                        resident.logical_height,
                        SourceSpritePaletteOwner::NightLevel,
                    ),
                    resident.base_tmap,
                )
            }
            DynamicResidentKind::Archer
            | DynamicResidentKind::Villager
            | DynamicResidentKind::Builder
            | DynamicResidentKind::Trader => SourceSpriteContractRow {
                source_entity_index: resident.source_entity_index,
                position: resident.position,
                yaw: resident.yaw,
                pitch: 0,
                roll: 0,
                control_flags,
                class: resident.class,
                model: resident.model,
                action: resident.action,
                xclass: 0,
                xmodel: 0,
                route: SourceSpriteContractRoute::Legacy(SourceSpriteLegacyFamily::Resident),
            },
        };
        Some(row)
    }));
    rows.extend(
        state
            .death_bursts
            .iter()
            .filter(|burst| burst.active)
            .map(|burst| {
                render_contract_row(
                    burst.source_entity_index,
                    burst.position,
                    0,
                    0,
                    0,
                    burst.control_flags,
                    DEATH_BURST_CLASS,
                    DEATH_BURST_MODEL,
                    DEATH_BURST_ACTION,
                    0,
                    0,
                    runtime_template(41, 54, 21, 250, SourceSpritePaletteOwner::SourceEffect),
                    54,
                )
            }),
    );
    if let Some(marker) = state.player_death_marker.filter(|marker| marker.active) {
        rows.push(render_contract_row(
            marker.source_entity_index,
            marker.position,
            0,
            0,
            0,
            state.player_death_marker_control_flags,
            PLAYER_DEATH_MARKER_CLASS,
            PLAYER_DEATH_MARKER_MODEL,
            PLAYER_DEATH_MARKER_ACTION,
            0,
            0,
            runtime_template(
                PLAYER_DEATH_MARKER_PARTICLE_PARAMETER,
                81,
                0,
                200,
                SourceSpritePaletteOwner::SourceEffect,
            ),
            81,
        ));
    }
    rows.extend(
        state
            .castle_associations
            .iter()
            .filter(|association| association.active)
            .map(|association| {
                render_contract_row(
                    association.source_entity_index,
                    association.position,
                    0,
                    0,
                    0,
                    control_flags(
                        state.castle_association_control_flags,
                        association.source_entity_index,
                    ),
                    CASTLE_ASSOCIATION_CLASS,
                    CASTLE_ASSOCIATION_MODEL,
                    CASTLE_ASSOCIATION_ACTION,
                    0,
                    0,
                    runtime_template(41, 54, 21, 250, SourceSpritePaletteOwner::SourceEffect),
                    54,
                )
            }),
    );
    rows.extend(
        state
            .castle_defense_controllers
            .iter()
            .filter(|controller| controller.active)
            .map(|controller| {
                render_contract_row(
                    controller.source_entity_index,
                    controller.position,
                    controller.yaw,
                    controller.pitch,
                    0,
                    control_flags(
                        state.castle_defense_control_flags,
                        controller.source_entity_index,
                    ),
                    CASTLE_DEFENSE_CLASS,
                    CASTLE_DEFENSE_MODEL,
                    CASTLE_DEFENSE_ACTION,
                    0,
                    0,
                    runtime_template(
                        CASTLE_DEFENSE_RENDER_INDEX,
                        80,
                        0,
                        200,
                        SourceSpritePaletteOwner::PlayerColour,
                    ),
                    80,
                )
            }),
    );
    rows.extend(
        state
            .castle_defense_shots
            .iter()
            .filter(|shot| shot.active)
            .map(|shot| {
                render_contract_row(
                    shot.source_entity_index,
                    shot.position,
                    shot.yaw,
                    shot.pitch,
                    0,
                    control_flags(
                        state.castle_defense_shot_control_flags,
                        shot.source_entity_index,
                    ),
                    shot.entity_class,
                    shot.entity_model,
                    shot.entity_action,
                    shot.xclass,
                    shot.xmodel,
                    runtime_template(
                        shot.particle_parameter_index,
                        shot.base_tmap,
                        shot.render_mode,
                        shot.logical_height,
                        SourceSpritePaletteOwner::SourceEffect,
                    ),
                    shot.base_tmap,
                )
            }),
    );
    rows.extend(
        state
            .mana_spheres
            .iter()
            .filter(|sphere| sphere.active && !sphere.pending_delete)
            .map(|sphere| {
                let ordinal = u16::from(sphere.particle_size_ordinal());
                let (base_tmap, palette_owner) =
                    if sphere.particle_index >= MANA_SPHERE_PLAYER_ONE_RENDER_BASE {
                        (
                            MANA_SPHERE_PLAYER_ONE_BASE_TMAP
                                + (sphere.particle_index - MANA_SPHERE_PLAYER_ONE_RENDER_BASE) / 8,
                            SourceSpritePaletteOwner::PlayerColour,
                        )
                    } else {
                        (
                            MANA_SPHERE_NEUTRAL_BASE_TMAP,
                            SourceSpritePaletteOwner::NightLevel,
                        )
                    };
                render_contract_row(
                    sphere.source_entity_index,
                    sphere.position,
                    sphere.yaw,
                    0,
                    0,
                    control_flags(state.mana_sphere_control_flags, sphere.source_entity_index),
                    MANA_SPHERE_CLASS,
                    MANA_SPHERE_MODEL,
                    MANA_SPHERE_ACTION,
                    0,
                    0,
                    runtime_template(
                        sphere.particle_index,
                        base_tmap,
                        MANA_SPHERE_RENDER_MODE,
                        (ordinal + 1) * 50,
                        palette_owner,
                    ),
                    base_tmap,
                )
            }),
    );
    if let Some(construction) = state
        .castle_construction
        .filter(|construction| construction.active)
    {
        rows.push(SourceSpriteContractRow {
            source_entity_index: construction.source_entity_index,
            position: construction.position,
            yaw: 0,
            pitch: 0,
            roll: 0,
            control_flags: 0,
            class: 10,
            model: 42,
            action: 44,
            xclass: 0,
            xmodel: 0,
            route: SourceSpriteContractRoute::Excluded(
                SourceSpriteExclusionReason::CastleConstruction,
            ),
        });
    }
    rows.extend(
        state
            .speed_trails
            .iter()
            .filter(|trail| trail.active)
            .map(|trail| SourceSpriteContractRow {
                source_entity_index: trail.source_entity_index,
                position: trail.position,
                yaw: 0,
                pitch: 0,
                roll: 0,
                control_flags: 0,
                class: trail.class,
                model: trail.model,
                action: trail.action,
                xclass: 0,
                xmodel: 0,
                route: SourceSpriteContractRoute::Excluded(
                    SourceSpriteExclusionReason::UnlinkedSpeedTrail,
                ),
            }),
    );
    source_sprite_dispositions(rows)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpriteTmapFrame {
    pub selector: u16,
    pub width: u16,
    pub height: u16,
    pub pixels: Box<[u8]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpriteTmapBank {
    frames: Vec<SourceSpriteTmapFrame>,
    catalog: Option<crate::source_animation::SourceTmapCatalog>,
    decoded_payloads: std::sync::Arc<[Box<[u8]>]>,
}

impl SourceSpriteTmapBank {
    pub fn new(mut frames: Vec<SourceSpriteTmapFrame>) -> Result<Self, String> {
        frames.sort_unstable_by_key(|frame| frame.selector);
        for (index, frame) in frames.iter().enumerate() {
            let expected = usize::from(frame.width) * usize::from(frame.height);
            if frame.width == 0 || frame.height == 0 || frame.pixels.len() != expected {
                return Err(format!(
                    "source TMAP {} extent {}x{} requires {expected} pixels, got {}",
                    frame.selector,
                    frame.width,
                    frame.height,
                    frame.pixels.len()
                ));
            }
            if index != 0 && frames[index - 1].selector == frame.selector {
                return Err(format!(
                    "source TMAP selector {} appears more than once",
                    frame.selector
                ));
            }
        }
        Ok(Self { frames, catalog: None, decoded_payloads: std::sync::Arc::from([]) })
    }

    pub(crate) fn with_catalog(mut self, catalog: crate::source_animation::SourceTmapCatalog) -> Self {
        self.catalog = Some(catalog);
        self
    }

    pub(crate) fn catalog(&self) -> Option<&crate::source_animation::SourceTmapCatalog> {
        self.catalog.as_ref()
    }

    pub(crate) fn with_decoded_payloads(mut self, payloads: Vec<Box<[u8]>>) -> Self {
        self.decoded_payloads = payloads.into();
        self
    }

    pub(crate) fn decoded_payloads(&self) -> std::sync::Arc<[Box<[u8]>]> {
        std::sync::Arc::clone(&self.decoded_payloads)
    }

    #[must_use]
    pub fn frame(&self, selector: u16) -> Option<&SourceSpriteTmapFrame> {
        self.frames
            .binary_search_by_key(&selector, |frame| frame.selector)
            .ok()
            .map(|index| &self.frames[index])
    }
}

pub struct SourceSpriteWorldResources<'a> {
    pub dispositions: &'a [SourceSpriteDisposition],
    pub tmap_bank: &'a SourceSpriteTmapBank,
}

/// Compiler-exhaustive allocation/spawn family bound to the Level 1 source
/// arena. Open payloads are split into validated data domains below rather
/// than represented by a catch-all tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level1SourceSpawnKind {
    Reserved,
    ImportedResetTuple,
    AuthoredActivationTuple,
    AuthoredTerrainFeature,
    AuthoredPlayerMarker,
    AuthoredSwitch,
    PlayerFireball,
    ArcherArrow,
    FireflyFireball,
    CastleBuild,
    CastleRoot,
    CastleConstruction,
    CastleDefenseController,
    CastleDefenseShot,
    CastleAssociation,
    Possession,
    PossessionImpact,
    GroundFire,
    PossessionAttraction,
    StrongPossessionImpact,
    FireballImpact,
    WaterSplash,
    ManaSphere,
    ManaCarrier,
    DeathBurst,
    PlayerDeathMarker,
    SpeedTrail,
    CheckpointSmoke,
    SpellPickup,
    Ambient,
    MultipartSegment,
    ResidentArcher,
    ResidentBuilder,
    ResidentVillager,
    ResidentTrader,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level1AuthoredCensusRow {
    LoadHouses,
    LoadTerrain29,
    GroupZeroDolmens,
    GroupZeroEmitters,
    GroupZeroFires,
    GroupZeroGoats,
    GroupZeroMarker,
    GroupZeroSwitches,
    GroupZeroVillagers,
    Group1,
    Group2,
    Group3,
    Group4,
    Group5,
    Group6,
    Group8,
    Group11,
    Group12,
    Group33,
    Group34,
    Group13To32Fires,
    Group13To32Houses,
    Group13To32Splash,
}

impl Level1AuthoredCensusRow {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "AUT-G-1-HOUSES" => Ok(Self::LoadHouses),
            "AUT-G-1-TERRAIN29" => Ok(Self::LoadTerrain29),
            "AUT-G0-DOLMENS" => Ok(Self::GroupZeroDolmens),
            "AUT-G0-EMITTERS" => Ok(Self::GroupZeroEmitters),
            "AUT-G0-FIRES" => Ok(Self::GroupZeroFires),
            "AUT-G0-GOATS" => Ok(Self::GroupZeroGoats),
            "AUT-G0-MARKER" => Ok(Self::GroupZeroMarker),
            "AUT-G0-SWITCHES" => Ok(Self::GroupZeroSwitches),
            "AUT-G0-VILLAGERS" => Ok(Self::GroupZeroVillagers),
            "AUT-G1" => Ok(Self::Group1),
            "AUT-G2" => Ok(Self::Group2),
            "AUT-G3" => Ok(Self::Group3),
            "AUT-G4" => Ok(Self::Group4),
            "AUT-G5" => Ok(Self::Group5),
            "AUT-G6" => Ok(Self::Group6),
            "AUT-G8" => Ok(Self::Group8),
            "AUT-G11" => Ok(Self::Group11),
            "AUT-G12" => Ok(Self::Group12),
            "AUT-G33" => Ok(Self::Group33),
            "AUT-G34" => Ok(Self::Group34),
            "AUT-G13-32-FIRES" => Ok(Self::Group13To32Fires),
            "AUT-G13-32-HOUSES" => Ok(Self::Group13To32Houses),
            "AUT-G13-32-SPLASH" => Ok(Self::Group13To32Splash),
            _ => Err(format!("unknown Level 1 authored census row {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level1AuthoredCensusRecord {
    pub authored_index: u16,
    pub kind: u16,
    pub subtype: i16,
    pub activation_group: i16,
    pub row: Level1AuthoredCensusRow,
}

/// Validate a concrete owner-authored record against its canonical activation
/// row. The row-to-index partition is checked by the production census; this
/// function intentionally has no range-only success path.
pub fn authored_activation_spawn_kind(
    record: Level1AuthoredCensusRecord,
) -> Result<Level1SourceSpawnKind, String> {
    use Level1AuthoredCensusRow as Row;

    let tuple = (record.kind, record.subtype, record.activation_group);
    let exact = match record.row {
        Row::LoadHouses => tuple == (10, 45, -1),
        Row::LoadTerrain29 => tuple == (10, 29, -1),
        Row::GroupZeroDolmens => tuple == (2, 2, 0),
        Row::GroupZeroEmitters => tuple == (10, 60, 0),
        Row::GroupZeroFires => tuple == (10, 1, 0),
        Row::GroupZeroGoats => tuple == (5, 1, 0),
        Row::GroupZeroMarker => tuple == (3, 4, 0),
        Row::GroupZeroSwitches => matches!(tuple, (11, 0 | 1 | 4, 0)),
        Row::GroupZeroVillagers => tuple == (5, 13, 0),
        Row::Group1 => matches!(tuple, (2, 0, 1) | (11, 0 | 32, 1) | (15, 2, 1)),
        Row::Group2 => matches!(tuple, (11, 1 | 32, 2)),
        Row::Group3 => matches!(tuple, (5, 4, 3) | (11, 17 | 32, 3)),
        Row::Group4 => matches!(
            tuple,
            (5, 3, 4) | (10, 59, 4) | (11, 0 | 12 | 16, 4) | (14, 3 | 5, 4)
        ),
        Row::Group5 => tuple == (11, 32, 5),
        Row::Group6 => matches!(tuple, (2, 1, 6) | (5, 19, 6)),
        Row::Group8 => tuple == (11, 0, 8),
        Row::Group11 => tuple == (15, 3, 11),
        Row::Group12 => tuple == (10, 59, 12),
        Row::Group33 => tuple == (5, 19, 33),
        Row::Group34 => tuple == (5, 3, 34),
        Row::Group13To32Fires => {
            record.kind == 10 && record.subtype == 1 && (13..=32).contains(&record.activation_group)
        }
        Row::Group13To32Houses => {
            record.kind == 10
                && record.subtype == 45
                && (13..=32).contains(&record.activation_group)
        }
        Row::Group13To32Splash => {
            record.kind == 10 && record.subtype == 5 && (13..=32).contains(&record.activation_group)
        }
    };
    if !exact {
        return Err(format!(
            "unknown Level 1 authored presentation tuple index={} row={:?} kind={} subtype={} group={}",
            record.authored_index, record.row, record.kind, record.subtype, record.activation_group
        ));
    }
    let kind = match record.row {
        Row::LoadTerrain29 => Level1SourceSpawnKind::AuthoredTerrainFeature,
        Row::GroupZeroMarker => Level1SourceSpawnKind::AuthoredPlayerMarker,
        Row::GroupZeroSwitches | Row::Group2 | Row::Group5 | Row::Group8 => {
            Level1SourceSpawnKind::AuthoredSwitch
        }
        Row::Group1 | Row::Group3 | Row::Group4 if record.kind == 11 => {
            Level1SourceSpawnKind::AuthoredSwitch
        }
        _ => Level1SourceSpawnKind::AuthoredActivationTuple,
    };
    Ok(kind)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ImportedResetSpriteTuple {
    pub class: u8,
    pub model: u8,
    pub action: u8,
    pub entity_parameter_index: u16,
    pub particle_parameter_index: u16,
    pub animation_frame: i8,
    pub control_flags: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpriteTemplate {
    pub render_parameter_index: u16,
    pub base_tmap: u16,
    pub render_mode: u8,
    pub logical_height: u16,
    pub animation_owner: SourceSpriteAnimationOwner,
    pub palette_owner: SourceSpritePaletteOwner,
    pub shadow_eligible: bool,
    pub reflection_eligible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSpriteTemplateDisposition {
    Render(SourceSpriteTemplate),
    Excluded(SourceSpriteExclusionReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportedResetSpriteProfile {
    pub tuple: ImportedResetSpriteTuple,
    pub disposition: SourceSpriteTemplateDisposition,
}

const fn template(
    render_parameter_index: u16,
    base_tmap: u16,
    render_mode: u8,
    logical_height: u16,
) -> SourceSpriteTemplateDisposition {
    SourceSpriteTemplateDisposition::Render(SourceSpriteTemplate {
        render_parameter_index,
        base_tmap,
        render_mode,
        logical_height,
        animation_owner: SourceSpriteAnimationOwner::SourceEntity,
        palette_owner: SourceSpritePaletteOwner::NightLevel,
        shadow_eligible: true,
        reflection_eligible: true,
    })
}

/// Exact 17-key reset presentation census from package M. Both source
/// parameter relations are retained: the entity relation identifies the
/// constructor family, while the live particle parameter selects the current
/// row. The local player is handed to its source-live row 66 before drawing.
pub const IMPORTED_RESET_SPRITE_PROFILES: [ImportedResetSpriteProfile; 17] = [
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 10,
            model: 45,
            action: 52,
            entity_parameter_index: 59,
            particle_parameter_index: 177,
            animation_frame: 0,
            control_flags: 13,
        },
        disposition: template(177, 96, 0, 400),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 11,
            model: 1,
            action: 1,
            entity_parameter_index: 59,
            particle_parameter_index: 0,
            animation_frame: 0,
            control_flags: 1,
        },
        disposition: SourceSpriteTemplateDisposition::Excluded(
            SourceSpriteExclusionReason::SpriteLessSwitch,
        ),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 11,
            model: 0,
            action: 0,
            entity_parameter_index: 59,
            particle_parameter_index: 0,
            animation_frame: 0,
            control_flags: 1,
        },
        disposition: SourceSpriteTemplateDisposition::Excluded(
            SourceSpriteExclusionReason::SpriteLessSwitch,
        ),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 5,
            model: 1,
            action: 15,
            entity_parameter_index: 98,
            particle_parameter_index: 238,
            animation_frame: 0,
            control_flags: 12,
        },
        disposition: template(238, 152, 20, 150),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 5,
            model: 13,
            action: 111,
            entity_parameter_index: 100,
            particle_parameter_index: 271,
            animation_frame: 0,
            control_flags: 12,
        },
        disposition: template(271, 199, 20, 300),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 5,
            model: 13,
            action: 111,
            entity_parameter_index: 100,
            particle_parameter_index: 241,
            animation_frame: 0,
            control_flags: 12,
        },
        disposition: template(241, 180, 20, 250),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 5,
            model: 13,
            action: 111,
            entity_parameter_index: 100,
            particle_parameter_index: 239,
            animation_frame: 0,
            control_flags: 12,
        },
        disposition: template(239, 155, 20, 200),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 5,
            model: 13,
            action: 111,
            entity_parameter_index: 100,
            particle_parameter_index: 242,
            animation_frame: 0,
            control_flags: 12,
        },
        disposition: template(242, 183, 20, 250),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 11,
            model: 4,
            action: 4,
            entity_parameter_index: 59,
            particle_parameter_index: 0,
            animation_frame: 0,
            control_flags: 1,
        },
        disposition: SourceSpriteTemplateDisposition::Excluded(
            SourceSpriteExclusionReason::SpriteLessSwitch,
        ),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 10,
            model: 60,
            action: 65,
            entity_parameter_index: 59,
            particle_parameter_index: 0,
            animation_frame: 0,
            control_flags: 1,
        },
        disposition: template(0, 8, 20, 250),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 10,
            model: 1,
            action: 1,
            entity_parameter_index: 59,
            particle_parameter_index: 41,
            animation_frame: 0,
            control_flags: 655_366,
        },
        disposition: template(41, 54, 21, 250),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 2,
            model: 2,
            action: 6,
            entity_parameter_index: 59,
            particle_parameter_index: 39,
            animation_frame: 0,
            control_flags: 4,
        },
        disposition: template(39, 79, 0, 400),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 3,
            model: 0,
            action: 0,
            entity_parameter_index: 66,
            particle_parameter_index: 44,
            animation_frame: 0,
            control_flags: 13,
        },
        disposition: template(66, 80, 0, 200),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 15,
            model: 0,
            action: 0,
            entity_parameter_index: 59,
            particle_parameter_index: 77,
            animation_frame: 0,
            control_flags: 5,
        },
        disposition: template(77, 59, 1, 150),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 15,
            model: 1,
            action: 3,
            entity_parameter_index: 59,
            particle_parameter_index: 77,
            animation_frame: 0,
            control_flags: 5,
        },
        disposition: template(77, 59, 1, 150),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 10,
            model: 14,
            action: 14,
            entity_parameter_index: 59,
            particle_parameter_index: 9,
            animation_frame: 0,
            control_flags: 131_076,
        },
        disposition: template(9, 63, 21, 50),
    },
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 10,
            model: 0,
            action: 0,
            entity_parameter_index: 59,
            particle_parameter_index: 7,
            animation_frame: 1,
            control_flags: 655_494,
        },
        disposition: template(7, 38, 36, 250),
    },
];

/// Level 2 class-2/model-3/action-9 bad-stone render tuple. Kept separate
/// from the sealed Level 1 imported-reset table so Level 1 profile cardinality
/// and behavior remain unchanged.
pub const LEVEL2_BAD_STONE_SPRITE_PROFILE: ImportedResetSpriteProfile =
    ImportedResetSpriteProfile {
        tuple: ImportedResetSpriteTuple {
            class: 2,
            model: 3,
            action: 9,
            entity_parameter_index: 59,
            particle_parameter_index: 270,
            animation_frame: 0,
            control_flags: 131_076,
        },
        disposition: template(270, 198, 0, 42),
    };

pub fn level2_bad_stone_sprite_disposition(
    tuple: ImportedResetSpriteTuple,
) -> Result<SourceSpriteTemplateDisposition, String> {
    (tuple == LEVEL2_BAD_STONE_SPRITE_PROFILE.tuple)
        .then_some(LEVEL2_BAD_STONE_SPRITE_PROFILE.disposition)
        .ok_or_else(|| {
            format!(
                "unknown Level 2 bad-stone sprite tuple class={} model={} action={} entity_parameter={} particle_parameter={} frame={} control={:#010x}",
                tuple.class,
                tuple.model,
                tuple.action,
                tuple.entity_parameter_index,
                tuple.particle_parameter_index,
                tuple.animation_frame,
                tuple.control_flags
            )
        })
}

pub fn imported_reset_sprite_disposition(
    tuple: ImportedResetSpriteTuple,
) -> Result<SourceSpriteTemplateDisposition, String> {
    IMPORTED_RESET_SPRITE_PROFILES
        .iter()
        .find(|profile| profile.tuple == tuple)
        .map(|profile| profile.disposition)
        .ok_or_else(|| {
            format!(
                "unknown Level 1 imported reset sprite tuple class={} model={} action={} entity_parameter={} particle_parameter={} frame={} control={:#010x}",
                tuple.class,
                tuple.model,
                tuple.action,
                tuple.entity_parameter_index,
                tuple.particle_parameter_index,
                tuple.animation_frame,
                tuple.control_flags
            )
        })
}

/// Classify one of the only four resident constructor pairs reachable from
/// the Level 1 house-removal allocator.
pub fn resident_spawn_kind(model: u8, action: u8) -> Result<Level1SourceSpawnKind, String> {
    match (model, action) {
        (4, 33) => Ok(Level1SourceSpawnKind::ResidentArcher),
        (12, 97) => Ok(Level1SourceSpawnKind::ResidentBuilder),
        (13, 105) => Ok(Level1SourceSpawnKind::ResidentVillager),
        (14, 113) => Ok(Level1SourceSpawnKind::ResidentTrader),
        _ => Err(format!(
            "unknown Level 1 resident spawn tuple class=5 model={model} action={action}"
        )),
    }
}

/// Validate the four pickup constructors reachable from the Level 1 mission
/// projection. The 26-family input/menu domain does not imply 26 source
/// pickup constructors.
pub fn spell_pickup_spawn_kind(family: u8) -> Result<Level1SourceSpawnKind, String> {
    if family < 4 {
        Ok(Level1SourceSpawnKind::SpellPickup)
    } else {
        Err(format!(
            "unknown Level 1 spell-pickup spawn tuple class=15 family={family}"
        ))
    }
}

/// Concrete constructor and post-`sub_6DCA0` state for one castle-defense
/// choice reachable from action 86 in Level 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CastleDefenseShotProfile {
    pub(crate) spell_family: u8,
    pub(crate) subspell: u8,
    pub(crate) spell_life: u8,
    pub(crate) spell_subspell_index: u16,
    pub(crate) entity_class: u8,
    pub(crate) entity_model: u8,
    pub(crate) entity_action: u8,
    pub(crate) xclass: u8,
    pub(crate) xmodel: u8,
    pub(crate) source_life_ticks: i16,
    pub(crate) source_variant_byte: u8,
    pub(crate) source_subspell_index: u16,
    pub(crate) entity_parameter_index: u16,
    pub(crate) particle_parameter_index: u16,
    pub(crate) base_tmap: u16,
    pub(crate) render_mode: u8,
    pub(crate) logical_height: u16,
    pub(crate) movement_speed: i16,
    pub(crate) sound_id: u16,
}

impl CastleDefenseShotProfile {
    #[cfg(test)]
    #[must_use]
    pub const fn submission(
        self,
        source_entity_index: u16,
        position: Position,
        yaw: i16,
        pitch: i16,
        roll: i16,
        control_flags: u32,
    ) -> SourceSpriteSubmission {
        SourceSpriteSubmission {
            source_entity_index,
            position,
            yaw,
            pitch,
            roll,
            control_flags,
            class: self.entity_class,
            model: self.entity_model,
            action: self.entity_action,
            xclass: self.xclass,
            xmodel: self.xmodel,
            render_parameter_index: self.particle_parameter_index,
            base_tmap: self.base_tmap,
            selected_frame: self.base_tmap,
            render_mode: self.render_mode,
            logical_height: self.logical_height,
            animation_owner: SourceSpriteAnimationOwner::SourceEntity,
            palette_owner: SourceSpritePaletteOwner::NightLevel,
            shadow_eligible: control_flags & 0x20 == 0,
            reflection_eligible: control_flags & 0x20 == 0,
        }
    }
}

/// Exact five-row Level 1 action-86 data contract from owned `SPELLS.DAT`,
/// pinned `sub_6DCA0`, and its class-9 constructors.
pub(crate) const CASTLE_DEFENSE_SHOT_PROFILES: [CastleDefenseShotProfile; 5] = [
    CastleDefenseShotProfile {
        spell_family: 0,
        subspell: 1,
        spell_life: 1,
        spell_subspell_index: 160,
        entity_class: 9,
        entity_model: 0,
        entity_action: 0,
        xclass: 10,
        xmodel: 0,
        source_life_ticks: 21,
        source_variant_byte: 0,
        source_subspell_index: 100,
        entity_parameter_index: 64,
        particle_parameter_index: 340,
        base_tmap: 463,
        render_mode: 21,
        logical_height: 100,
        movement_speed: 384,
        sound_id: 9,
    },
    CastleDefenseShotProfile {
        spell_family: 7,
        subspell: 0,
        spell_life: 0,
        spell_subspell_index: 200,
        entity_class: 9,
        entity_model: 9,
        entity_action: 9,
        xclass: 10,
        xmodel: 23,
        source_life_ticks: 9,
        source_variant_byte: 0,
        source_subspell_index: 200,
        entity_parameter_index: 63,
        particle_parameter_index: 216,
        base_tmap: 151,
        render_mode: 21,
        logical_height: 50,
        movement_speed: 384,
        sound_id: 23,
    },
    CastleDefenseShotProfile {
        spell_family: 7,
        subspell: 1,
        spell_life: 1,
        spell_subspell_index: 250,
        entity_class: 9,
        entity_model: 12,
        entity_action: 12,
        xclass: 9,
        xmodel: 9,
        source_life_ticks: 5,
        source_variant_byte: 0,
        source_subspell_index: 250,
        entity_parameter_index: 60,
        particle_parameter_index: 216,
        base_tmap: 151,
        render_mode: 21,
        logical_height: 50,
        movement_speed: 384,
        sound_id: 9,
    },
    CastleDefenseShotProfile {
        spell_family: 0,
        subspell: 2,
        spell_life: 2,
        spell_subspell_index: 80,
        entity_class: 9,
        entity_model: 28,
        entity_action: 29,
        xclass: 10,
        xmodel: 76,
        source_life_ticks: 21,
        source_variant_byte: 0,
        source_subspell_index: 100,
        entity_parameter_index: 64,
        particle_parameter_index: 340,
        base_tmap: 463,
        render_mode: 21,
        logical_height: 100,
        movement_speed: 384,
        sound_id: 9,
    },
    CastleDefenseShotProfile {
        spell_family: 9,
        subspell: 0,
        spell_life: 2,
        spell_subspell_index: 1_750,
        entity_class: 9,
        entity_model: 3,
        entity_action: 3,
        xclass: 10,
        xmodel: 17,
        source_life_ticks: 21,
        source_variant_byte: 2,
        source_subspell_index: 1_750,
        entity_parameter_index: 60,
        particle_parameter_index: 76,
        base_tmap: 64,
        render_mode: 21,
        logical_height: 350,
        movement_speed: 384,
        sound_id: 15,
    },
];

/// Resolve a reachable castle-defense choice. Unknown data is a hard error,
/// carrying the complete source selector pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CastleDefenseShotError {
    UnknownChoice { spell_family: u8, subspell: u8 },
}

impl std::fmt::Display for CastleDefenseShotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownChoice {
                spell_family,
                subspell,
            } => write!(
                formatter,
                "unknown Level 1 castle-defense shot family {spell_family}, subspell {subspell}"
            ),
        }
    }
}

pub(crate) fn castle_defense_shot_profile(
    spell_family: u8,
    subspell: u8,
) -> Result<CastleDefenseShotProfile, CastleDefenseShotError> {
    CASTLE_DEFENSE_SHOT_PROFILES
        .iter()
        .copied()
        .find(|profile| profile.spell_family == spell_family && profile.subspell == subspell)
        .ok_or(CastleDefenseShotError::UnknownChoice {
            spell_family,
            subspell,
        })
}

/// Produce the mandatory diagnostic for an unknown reachable tuple.
pub fn unknown_source_sprite_tuple(
    source_entity_index: u16,
    class: u8,
    model: u8,
    action: u8,
    render_parameter_index: u16,
) -> String {
    format!(
        "unknown Level 1 source sprite tuple: slot={source_entity_index} class={class} model={model} action={action} parameter={render_parameter_index}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn castle_defense_shot_profiles_are_exact_and_unknown_choices_fail_closed() {
        assert_eq!(
            castle_defense_shot_profile(7, 1).unwrap(),
            CASTLE_DEFENSE_SHOT_PROFILES[2]
        );
        let submission = castle_defense_shot_profile(7, 0).unwrap().submission(
            900,
            Position { x: 1, y: 2, z: 3 },
            4,
            5,
            6,
            8,
        );
        assert_eq!(
            (
                submission.class,
                submission.model,
                submission.action,
                submission.xclass,
                submission.xmodel,
                submission.control_flags,
                submission.render_parameter_index,
            ),
            (9, 9, 9, 10, 23, 8, 216),
            "the source entity tuple and xclass/xmodel must remain distinct submission fields"
        );
        let fireball = castle_defense_shot_profile(0, 2).unwrap();
        assert_eq!(
            (
                fireball.entity_model,
                fireball.entity_action,
                fireball.xclass,
                fireball.xmodel,
            ),
            (28, 29, 10, 76),
            "the alternate fireball constructor keeps its distinct action and xmodel"
        );
        let lightning = castle_defense_shot_profile(9, 0).unwrap();
        assert_eq!(
            (
                lightning.source_variant_byte,
                lightning.source_subspell_index,
                lightning.base_tmap,
                lightning.logical_height,
                lightning.sound_id,
            ),
            (2, 1_750, 64, 350, 15),
            "the lightning discriminator remains source-shaped"
        );
        assert_eq!(
            castle_defense_shot_profile(7, 2).unwrap_err(),
            CastleDefenseShotError::UnknownChoice {
                spell_family: 7,
                subspell: 2,
            }
        );
        assert_eq!(
            castle_defense_shot_profile(7, 2).unwrap_err().to_string(),
            "unknown Level 1 castle-defense shot family 7, subspell 2"
        );
        assert_eq!(
            unknown_source_sprite_tuple(845, 10, 79, 86, 66),
            "unknown Level 1 source sprite tuple: slot=845 class=10 model=79 action=86 parameter=66"
        );
    }
}
