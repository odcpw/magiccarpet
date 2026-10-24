//! Immutable content shared by environmental effects and their real children.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Impact,
    RadialExplosion,
    Expiry,
    ShortVisual,
    DormantGroundMarker,
    Ash,
    GroundFire,
    ImmediateVisual,
    Smoke,
    DarkSmoke,
    BurningDebris,
    Eruption,
    EruptionSmoke,
    AmbientSmoke,
    PossessionContact,
    FireSpheres,
    FadingSmoke,
}

impl EffectKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Impact => "impact",
            Self::RadialExplosion => "radial_explosion",
            Self::Expiry => "expiry",
            Self::ShortVisual => "short_visual",
            Self::DormantGroundMarker => "dormant_ground_marker",
            Self::Ash => "ash",
            Self::GroundFire => "ground_fire",
            Self::ImmediateVisual => "immediate_visual",
            Self::Smoke => "smoke",
            Self::DarkSmoke => "dark_smoke",
            Self::BurningDebris => "burning_debris",
            Self::Eruption => "eruption",
            Self::EruptionSmoke => "eruption_smoke",
            Self::AmbientSmoke => "ambient_smoke",
            Self::PossessionContact => "possession_contact",
            Self::FireSpheres => "fire_spheres",
            Self::FadingSmoke => "fading_smoke",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectDefinition {
    kind: EffectKind,
    lifetime_base: i32,
    damage: u16,
}

impl EffectDefinition {
    pub(crate) const fn new(kind: EffectKind, lifetime_base: i32, damage: u16) -> Self {
        Self {
            kind,
            lifetime_base,
            damage,
        }
    }

    #[must_use]
    pub const fn kind(self) -> EffectKind {
        self.kind
    }

    #[must_use]
    pub const fn lifetime_base(self) -> i32 {
        self.lifetime_base
    }

    #[must_use]
    pub const fn damage(self) -> u16 {
        self.damage
    }

    pub(super) fn validate(self) -> Result<(), String> {
        if self.lifetime_base < 0 {
            return Err("effect construction lifetime cannot be negative".to_owned());
        }
        if self.kind == EffectKind::FireSpheres && self.lifetime_base > i32::from(i16::MAX) {
            return Err("Fire Spheres lifetime exceeds its retained carrier".to_owned());
        }
        Ok(())
    }
}
