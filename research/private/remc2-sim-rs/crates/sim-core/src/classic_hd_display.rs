//! Compatibility facade for the package-neutral Classic HD display compositor.

pub use sim_classic_hd_display::*;

#[cfg(test)]
mod compatibility_tests {
    use super::{ClassicHdDisplayComposition, ClassicHdHudSpace, ClassicHdWindowFilter};
    use crate::classic_hd::ClassicHdRenderProfile;

    #[test]
    fn classic_hd_display_facade_preserves_composition() {
        let profile = ClassicHdRenderProfile::hd_640x480();
        let composition =
            ClassicHdDisplayComposition::new(&profile, 1920, 1080, ClassicHdWindowFilter::Point)
                .expect("facade-backed display composition");

        assert_eq!(composition.profile_revision(), profile.profile_revision);
        assert_eq!(composition.hud_space(), ClassicHdHudSpace::ClassicHd640x480);
        assert_eq!(composition.layout().content.width, 1440);
    }
}
