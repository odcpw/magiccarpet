use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::campaign::{CampaignSession, spec::CampaignSelection};
use crate::day_indexed_frame::{DayIndexedFrameAssets, DayIndexedFrameState};

fn generated(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../generated")
        .join(name)
}

fn is_family(campaign: &CampaignSession, slot: u16, class: u8, model: u8) -> bool {
    let sim = campaign.simulation();
    if let Some(row) = sim
        .economy()
        .house_residents()
        .iter()
        .find(|row| row.source_entity_index == slot)
    {
        return row.active && class == 5 && row.model == model;
    }
    if let Some(row) = sim
        .economy()
        .authored_class5()
        .iter()
        .find(|row| row.source_entity_index == slot)
    {
        return row.active && class == 5 && row.model == model;
    }
    let entity = &sim.entities()[usize::from(slot)];
    entity.active && (entity.class, entity.model) == (class, model)
}

// This exercises actual authored actors, owner pixels and the ordinary frame
// feedback/turn path. Camera placement is a test view, not actor injection.
#[test]
fn real_night_goats_and_day_villager_consume_shared_live_pixels() {
    for (package, cache, selection, class, model, required, selector) in [
        (
            "mc-bhq8-level1-collision-v5-night",
            "mc-oi62-environment-level1-v2",
            CampaignSelection::LEVEL1,
            crate::goat::GOAT_CLASS,
            crate::goat::GOAT_MODEL,
            2,
            Some(152),
        ),
        (
            "level2-day-block32-campaign-v2",
            "mc-oi62-environment-level2-v2",
            CampaignSelection::LEVEL2,
            crate::villager::VILLAGER_CLASS,
            crate::villager::VILLAGER_MODEL,
            1,
            None,
        ),
    ] {
        let cache = generated(cache);
        let mut campaign =
            CampaignSession::open_with_owner_cache(generated(package), &cache, selection).unwrap();
        let assets = DayIndexedFrameAssets::load(&cache).unwrap();
        if selection == CampaignSelection::LEVEL1 {
            let night = DayIndexedFrameAssets::load_night(&cache).unwrap();
            assert_eq!(night.cache_content_sha256(), assets.cache_content_sha256());
        }
        let rows = campaign
            .simulation()
            .campaign_entity_presentations(0)
            .unwrap();
        let candidates: Vec<_> = rows
            .into_iter()
            .filter(|row| is_family(&campaign, row.entity_index, class, model))
            .collect();
        assert!(
            !candidates.is_empty(),
            "authored family must be present: class={class} model={model}; residents={} class5={}",
            campaign.simulation().economy().house_residents().len(),
            campaign.simulation().economy().authored_class5().len()
        );
        let generation = campaign.generation();
        let mut found = None;
        'views: for row in &candidates {
            let ground = crate::legacy_terrain::interpolate_ground(
                &campaign
                    .simulation()
                    .canonical_terrain()
                    .planes()
                    .ground_heights,
                row.position.x,
                row.position.y,
            );
            for depth in [1024_u16, 2048, 4096] {
                campaign
                    .simulation_mut()
                    .session
                    .state
                    .scratch
                    .source_camera_pose = Some(sim_session::CameraPose {
                    x: row.position.x,
                    y: row.position.y.wrapping_add(depth),
                    z: (ground + 128) as i16,
                    yaw: 0,
                    pitch: 0,
                    roll: 0,
                });
                let mut carrier = DayIndexedFrameState::new(generation);
                let frame = carrier
                    .render_current(
                        campaign.inspection(),
                        campaign.simulation(),
                        &assets,
                        generation,
                    )
                    .unwrap();
                let mut by_selector = std::collections::BTreeMap::<u16, BTreeSet<u16>>::new();
                for source in frame.animation_sources() {
                    if is_family(&campaign, source.entity_index, class, model)
                        && (selector.is_none() || selector == Some(source.selected_tmap))
                        && (selector.is_some()
                            || crate::frozen_actor_tmap::VILLAGER_ANIMATED_SELECTORS
                                .contains(&source.selected_tmap))
                        && (selector.is_some() || matches!(source.render_mode, 17 | 19 | 20))
                    {
                        by_selector
                            .entry(source.selected_tmap)
                            .or_default()
                            .insert(source.entity_index);
                    }
                }
                if let Some((&selected, _)) = by_selector
                    .iter()
                    .find(|(_, actors)| actors.len() >= required)
                {
                    found = Some((frame, selected));
                    break 'views;
                }
            }
        }
        let (frame, selected) =
            found.expect("ordinary camera admits requested authored family and shared selector");
        assert!(
            assets
                .environment_resources()
                .source_tmaps()
                .unwrap()
                .catalog()
                .unwrap()
                .record(selected)
                .unwrap()
                .encoded_frame_count
                .is_some(),
            "the selected actor uses an animated owner stream"
        );
        if selector.is_none() {
            let source = frame
                .animation_sources()
                .iter()
                .find(|source| {
                    is_family(&campaign, source.entity_index, class, model)
                        && source.selected_tmap == selected
                })
                .unwrap();
            let turned = campaign
                .simulation()
                .campaign_entity_presentations(512)
                .unwrap();
            let other = turned
                .iter()
                .find(|row| row.entity_index == source.entity_index)
                .unwrap();
            assert_ne!(
                (source.selected_tmap, source.mirrored),
                (other.selected_tmap, other.mirrored),
                "the authored villager changes directional binding as the view turns"
            );
        }
        let before_hash = campaign.state_hash();
        let mut repeat_carrier = DayIndexedFrameState::new(generation);
        let repeated = repeat_carrier
            .render_current(
                campaign.inspection(),
                campaign.simulation(),
                &assets,
                generation,
            )
            .unwrap();
        assert!(
            frame.scene_indices == repeated.scene_indices,
            "repeated frame pixels differ"
        );
        assert_eq!(campaign.state_hash(), before_hash, "frame reads are pure");
        frame
            .apply_primary_visibility(campaign.simulation_mut(), generation)
            .unwrap();
        let before = campaign
            .simulation()
            .source_animation_clocks
            .pixels(selected)
            .unwrap()
            .to_vec();
        campaign.paused_frame();
        assert_eq!(
            campaign
                .simulation()
                .source_animation_clocks
                .pixels(selected)
                .unwrap(),
            before,
            "a paused outer frame leaves the selected image unchanged"
        );
        campaign.step(crate::PlayerCommand::default(), 0);
        let after = campaign
            .simulation()
            .source_animation_clocks
            .pixels(selected)
            .unwrap()
            .to_vec();
        assert_ne!(
            before, after,
            "selected owner stream must advance after its draw"
        );
        let mut base = campaign.clone();
        base.simulation_mut()
            .source_animation_clocks
            .discard_for_test(selected);
        let rows = campaign
            .simulation()
            .campaign_entity_presentations(0)
            .unwrap();
        let mut visible = false;
        'camera_search: for row in rows
            .iter()
            .filter(|row| {
                is_family(&campaign, row.entity_index, class, model)
                    && row.selected_tmap == selected
            })
            .copied()
            .take(12)
            .collect::<Vec<_>>()
        {
            let ground = crate::legacy_terrain::interpolate_ground(
                &campaign
                    .simulation()
                    .canonical_terrain()
                    .planes()
                    .ground_heights,
                row.position.x,
                row.position.y,
            );
            for depth in [384_u16, 512, 768, 1024, 1536] {
                for altitude in [64_i16, 128, 256] {
                    for pitch in [0_i16, -128, 128] {
                        let camera = Some(sim_session::CameraPose {
                            x: row.position.x,
                            y: row.position.y.wrapping_add(depth),
                            z: (ground as i16).saturating_add(altitude),
                            yaw: 0,
                            pitch,
                            roll: 0,
                        });
                        campaign
                            .simulation_mut()
                            .session
                            .state
                            .scratch
                            .source_camera_pose = camera;
                        base.simulation_mut()
                            .session
                            .state
                            .scratch
                            .source_camera_pose = camera;
                        let live = DayIndexedFrameState::new(generation)
                            .render_current(
                                campaign.inspection(),
                                campaign.simulation(),
                                &assets,
                                generation,
                            )
                            .unwrap();
                        if !live.animation_sources().iter().any(|source| {
                            source.entity_index == row.entity_index
                                && source.selected_tmap == selected
                        }) {
                            continue;
                        }
                        let base_frame = DayIndexedFrameState::new(generation)
                            .render_current(
                                base.inspection(),
                                base.simulation(),
                                &assets,
                                generation,
                            )
                            .unwrap();
                        if live.scene_indices != base_frame.scene_indices {
                            visible = true;
                            break 'camera_search;
                        }
                    }
                }
            }
        }
        assert!(
            visible,
            "changed owner pixels must reach the real raster for class={class} model={model} selector={selected}"
        );
        campaign.reset();
        assert!(
            campaign
                .simulation()
                .source_animation_clocks
                .pixels(selected)
                .is_none(),
            "reset retires the old mutable image"
        );
    }
}
