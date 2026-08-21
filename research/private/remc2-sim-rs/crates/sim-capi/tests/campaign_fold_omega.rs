//! Evidence-only conformance checks for the CF-00 Omega seal.
//!
//! The inventory is intentionally checked as a committed data contract here;
//! this test adds no production observer, setter, route, or ABI surface.

const OMEGA_V1: &str = include_str!("../../../level2-conformance/fold/campaign-fold-omega-v1.json");
const OMEGA_V1_1: &str =
    include_str!("../../../level2-conformance/fold/campaign-fold-omega-v1.1.json");

fn requires(omega: &str, marker: &str) {
    assert!(
        omega.contains(marker),
        "Omega receipt is missing marker: {marker}"
    );
}

#[test]
fn omega_v1_audit_receipt_remains_immutable() {
    requires(OMEGA_V1, "\"schema_version\": \"campaign-fold-omega-v1\"");
    requires(
        OMEGA_V1,
        "\"superproject_revision\": \"462b438d41f1d39ce4b4a1ae24f892dc3e04cafe\"",
    );
    requires(
        OMEGA_V1,
        "\"remc2_revision\": \"6806c7204d17220ecc0430759f784aea8ee2ddb2\"",
    );
    requires(
        OMEGA_V1,
        "\"json_sha256\": \"aad97175330bfdef21b09e3e164f054d2b6df59f66bd7c449f02e3f633514684\"",
    );
    requires(OMEGA_V1, "\"json_byte_identical\": true");
    requires(OMEGA_V1, "\"markdown_byte_identical\": true");
    requires(OMEGA_V1, "\"reached_transition_count\": 54");
    requires(OMEGA_V1, "\"route_kind_count\": 14");
}

#[test]
fn omega_v1_1_binds_exact_revision_double_regeneration_and_mapped_delta() {
    requires(
        OMEGA_V1_1,
        "\"schema_version\": \"campaign-fold-omega-v1.1\"",
    );
    requires(OMEGA_V1_1, "\"bead_id\": \"mc-cf00-omega-reseal-3dxle\"");
    requires(
        OMEGA_V1_1,
        "\"superproject_revision\": \"656416b62fb6c859e842c3cb81cccaa38ac2df7f\"",
    );
    requires(
        OMEGA_V1_1,
        "\"remc2_revision\": \"6806c7204d17220ecc0430759f784aea8ee2ddb2\"",
    );
    requires(
        OMEGA_V1_1,
        "\"json_sha256\": \"7986b9a2935d160f40c22fae1250f7bfa9abe9ede81529832ed35ce148470383\"",
    );
    requires(
        OMEGA_V1_1,
        "\"markdown_sha256\": \"91b385359a70c78ab72ad011e0f6f5de827f334e1dcbe50283c1deb8565fc6ce\"",
    );
    requires(OMEGA_V1_1, "\"json_byte_identical\": true");
    requires(OMEGA_V1_1, "\"markdown_byte_identical\": true");
    requires(OMEGA_V1_1, "\"reached_transition_count\": 54");
    requires(OMEGA_V1_1, "\"route_kind_count\": 14");
    requires(OMEGA_V1_1, "\"baseline_open_question_count\": 224");
    requires(OMEGA_V1_1, "\"current_open_question_count\": 229");
    requires(OMEGA_V1_1, "\"added_open_question_count\": 5");
    requires(OMEGA_V1_1, "\"unmapped_open_question_count\": 0");
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
        requires(OMEGA_V1_1, marker);
    }
}

#[test]
fn omega_receipt_covers_all_remaining_realms_and_required_diffs() {
    requires(OMEGA_V1_1, "\"level_count\": 22");
    requires(OMEGA_V1_1, "\"byte_identical_twins\": 22");
    requires(OMEGA_V1_1, "\"scope\": \"external Levels 3-24\"");
    for marker in [
        "\"tuple_diff\"",
        "\"objective_form_diff\"",
        "\"map_type_gate\"",
        "\"required_source_fact_diff\"",
        "\"outer_fields\": 151",
        "\"economy_fields\": 57",
        "\"nyphur_fields\": 89",
    ] {
        requires(OMEGA_V1_1, marker);
    }
}

#[test]
fn omega_receipt_declares_mapped_delta_and_rejects_planted_failures() {
    for marker in [
        "v1_to_v1_1_delta",
        "nyphur-possession-holder",
        "nyphur-fireball-holder",
        "group31-tree-source",
        "tree-callback-source",
        "tree-callback-row",
        "mc-1g06.1",
        "mc-l2-33a6-tree-producer-7pclk",
        "mc-cf01-pure-campaign-module-extraction-19t39",
        "missing_state_cell",
        "missing_transition",
        "dropped_or_reordered_ordered_effect",
        "missing_required_capability",
        "changed_evidence_revision",
        "trace_on_production_difference",
        "known_stale_m1_hash",
        "unmapped_open_question_count",
        "\"equal\": true",
    ] {
        requires(OMEGA_V1_1, marker);
    }
}
