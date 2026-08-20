//! Evidence-only conformance checks for the CF-00 Omega seal.
//!
//! The inventory is intentionally checked as a committed data contract here;
//! this test adds no production observer, setter, route, or ABI surface.

const OMEGA: &str = include_str!("../../../level2-conformance/fold/campaign-fold-omega-v1.json");

fn requires(marker: &str) {
    assert!(
        OMEGA.contains(marker),
        "Omega receipt is missing marker: {marker}"
    );
}

#[test]
fn omega_receipt_binds_exact_revision_and_double_regeneration() {
    requires("\"schema_version\": \"campaign-fold-omega-v1\"");
    requires("\"superproject_revision\": \"462b438d41f1d39ce4b4a1ae24f892dc3e04cafe\"");
    requires("\"remc2_revision\": \"6806c7204d17220ecc0430759f784aea8ee2ddb2\"");
    requires(
        "\"json_sha256\": \"aad97175330bfdef21b09e3e164f054d2b6df59f66bd7c449f02e3f633514684\"",
    );
    requires("\"json_byte_identical\": true");
    requires("\"markdown_byte_identical\": true");
    requires("\"reached_transition_count\": 54");
    requires("\"route_kind_count\": 14");
}

#[test]
fn omega_receipt_covers_state_schedule_rng_effects_reset_abi_and_frame() {
    for marker in [
        "authoritative_campaign_state_and_hash",
        "scheduler_phase_dispatch_slot_scan_and_rescan",
        "rng_old_site_draw_result_new",
        "allocation_reclamation_generation_and_topology",
        "ordered_effect_and_audio_emissions_with_cursor",
        "reset_result_persistent_state_and_replay",
        "package_profile_source_facts_and_preflight",
        "c_abi_return_layout_handle_generation_error_and_mutation_receipt",
        "unity_visible_frame_and_journal_without_unity_authority",
        "ordinary_producer",
        "following_pass_consumer",
    ] {
        requires(marker);
    }
}

#[test]
fn omega_receipt_covers_all_remaining_realms_and_required_diffs() {
    requires("\"level_count\": 22");
    requires("\"byte_identical_twins\": 22");
    requires("\"scope\": \"external Levels 3-24\"");
    for marker in [
        "\"tuple_diff\"",
        "\"objective_form_diff\"",
        "\"map_type_gate\"",
        "\"required_source_fact_diff\"",
        "\"outer_fields\": 151",
        "\"economy_fields\": 57",
        "\"nyphur_fields\": 89",
    ] {
        requires(marker);
    }
}

#[test]
fn omega_receipt_declares_and_rejects_planted_failures() {
    for marker in [
        "missing_state_cell",
        "missing_transition",
        "dropped_or_reordered_ordered_effect",
        "missing_required_capability",
        "changed_evidence_revision",
        "trace_on_production_difference",
        "known_stale_m1_hash",
        "\"equal\": true",
    ] {
        requires(marker);
    }
}
