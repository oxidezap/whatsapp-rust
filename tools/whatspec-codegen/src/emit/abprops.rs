//! `wacore/src/iq/abprops.rs`: the A/B-props (feature-flag) registry.

use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::ir::{AbPropConfig, AbPropType, AbPropsIr, Scalar};
use crate::naming::{float_lit, rust_lit, snake_case, unique_ident};

const HEADER: &str = "\
//! One `pub mod` per WA Web registry, one `pub const` per flag (screaming-snake of
//! its key) with the numeric `code` sent in the `<props>` IQ, value type, and default;
//! reference only what you use. Each module's `ALL` lists its flags; the top-level
//! `ALL` lists every module's slice.

#![allow(clippy::all)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbPropType {
    Bool,
    Int,
    Float,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct AbProp {
    pub name: &'static str,
    pub code: u32,
    pub value_type: AbPropType,
    pub default: AbDefault,
}

";

pub fn generate(ir: &AbPropsIr) -> Result<String> {
    for (module, retained) in [
        ("WAWebABPropsConfigs", RETAINED_WEB_PROPS),
        ("WAWebHybridABPropsConfigs", RETAINED_HYBRID_PROPS),
    ] {
        for name in retained {
            ensure!(
                !ir.configs
                    .iter()
                    .any(|c| c.module == module && c.name == *name),
                "upstream reclaimed {name}; reconcile props::stale before regenerating"
            );
        }
    }
    let mut out = super::header("A/B-props registry", &ir.wa_version);
    out.push_str(HEADER);

    // `configs` arrives sorted by (module, name); one Rust module per run.
    let mut module_idents = HashSet::new();
    let mut emitted: Vec<String> = Vec::new();
    // Interleaved modules would visit the same registry twice and split it
    // across `web` and `web_2` — a silent API rename that `--check` would then
    // bless. Fail on the IR instead.
    let mut seen_modules: HashSet<&str> = HashSet::new();
    let mut i = 0;
    while i < ir.configs.len() {
        let wa_module = ir.configs[i].module.as_str();
        ensure!(
            seen_modules.insert(wa_module),
            "abprops IR is not grouped by module: {wa_module} appears in two runs"
        );
        let mut j = i;
        while j < ir.configs.len() && ir.configs[j].module == wa_module {
            j += 1;
        }
        let ident = unique_ident(&module_ident(wa_module), &mut module_idents, "m");
        out.push_str(&render_module(&ident, wa_module, &ir.configs[i..j]));
        emitted.push(ident);
        i = j;
    }

    out.push_str(
        "/// Every registry's `ALL`, for whole-catalog iteration:\n\
         /// `ALL.iter().flat_map(|r| r.iter())`.\npub const ALL: &[&[AbProp]] = &[\n",
    );
    for m in &emitted {
        out.push_str(&format!("    {m}::ALL,\n"));
    }
    out.push_str("];\n");
    Ok(out)
}

fn render_module(ident: &str, wa_module: &str, group: &[AbPropConfig]) -> String {
    let mut out = format!(
        "/// `{wa_module}` — {} flags.\npub mod {ident} {{\n    use super::{{AbDefault, AbProp, AbPropType}};\n\n",
        group.len()
    );
    // Ident scope is per module, so a flag carried by two registries keeps its
    // name in each.
    let mut used = HashSet::new();
    if let Some(retained) = match wa_module {
        "WAWebABPropsConfigs" => Some(RETAINED_WEB_PROPS),
        "WAWebHybridABPropsConfigs" => Some(RETAINED_HYBRID_PROPS),
        _ => None,
    } {
        out.push_str(
            "    // Retained API constants, deliberately outside the current catalog's ALL.\n",
        );
        for name in retained {
            out.push_str(&format!(
                "    pub use crate::iq::props::stale::{};\n",
                name.to_uppercase()
            ));
        }
    }
    let names: Vec<String> = group
        .iter()
        .map(|c| unique_ident(&snake_case(&c.name).to_uppercase(), &mut used, "F"))
        .collect();
    for (c, const_name) in group.iter().zip(&names) {
        out.push_str(&format!(
            "    pub const {const_name}: AbProp = AbProp {{ name: {}, code: {}, value_type: AbPropType::{}, default: {} }};\n",
            rust_lit(&c.name),
            c.code,
            type_variant(c.value_type),
            default_lit(&c.default),
        ));
    }
    out.push_str(&format!(
        "\n    /// All {} flags in this registry, sorted by name.\n    pub const ALL: &[AbProp] = &[\n",
        group.len()
    ));
    for const_name in &names {
        out.push_str(&format!("        {const_name},\n"));
    }
    out.push_str("    ];\n}\n\n");
    out
}

const RETAINED_HYBRID_PROPS: &[&str] = &["group_call_max_participants"];

const RETAINED_WEB_PROPS: &[&str] = &[
    "ai_3p_agent_link_enabled",
    "lists_smb_web_enabled",
    "scheduled_companion_contact_refresh_days",
    "scheduled_companion_contact_refresh_hours",
    "smoothie_performance_msg_send",
    "updated_harmful_document_dialog",
    "ai_chat_threads_infra_web_enabled",
    "ai_chat_threads_web_killswitch_enabled",
    "ai_chat_threads_web_msgs_load_limit",
    "ai_meta_ai_thread_creation_enabled",
    "auth_agent_soft_offboarding_enabled",
    "bug_reporting_not_shipped_yet_enabled",
    "channels_pulse_on_unread_badge_enabled",
    "channels_stickers_qp_enabled",
    "ctwa_lead_taxonomy",
    "ctwa_native_web_draft_ad_enabled",
    "enable_fmx_redesigned_bcc_for_metaverified_biz",
    "enable_fmx_redesigned_bcc_for_suspicious_biz",
    "enable_fmx_redesigned_bcc_for_tier0_biz",
    "enable_fmx_redesigned_bcc_for_tier1_biz",
    "enable_fmx_redesigned_bcc_for_tier2_biz",
    "enable_fmx_redesigned_bcc_for_tier3_biz",
    "enable_fmx_redesigned_bcc_for_unknown_tiered_biz",
    "enable_fmx_redesigned_bcc_for_untiered_biz",
    "enable_web_voip_video_resolution_cap",
    "enable_web_voip_virtual_audio_capture_driver",
    "expand_fmx_mex_should_use_fmx_use_case",
    "group_from_group_max_missing_privacy_tokens",
    "info_drawer_refresh",
    "integrity_checkpoints_default_enabled",
    "interactive_bloks_widget_web_enabled",
    "interactive_response_message_native_flow_killswitch",
    "kmp_syncd_engine_crypto_enabled",
    "kmp_syncd_engine_outgoing_processor_enabled",
    "large_screens_new_chat_button_variants",
    "newsletter_status_creation_enabled",
    "rnr_min_days_user_active",
    "rt_receiver_v3_validation_enabled",
    "smb_biz_ai_suggested_reply_prefetch",
    "smb_web_enable_fb_linking",
    "status_web_ranking",
    "wa_smb_web_lists_quick_replies_enabled",
    "wa_web_attach_icon_variant",
    "wa_web_biz_profile_graphql_migration_bypass_lid_check_dogfooding",
    "wa_web_blocked_participant_call_warning",
    "wa_web_broadcast_disappearing_messages_fix",
    "wa_web_canonical_reg_reload_enabled",
    "wa_web_change_list_wds_submenu",
    "wa_web_context_card_vertical_buttons",
    "wa_web_favicon_badging_enabled",
    "wa_web_focus_management_for_status_audience",
    "wa_web_frequent_reactions_store_enabled",
    "wa_web_frequent_reactions_weight_reducer",
    "wa_web_invite_link_page_enhancements",
    "wa_web_notify_for",
    "wa_web_scrollable_reaction_tray_enabled",
    "wa_web_search_emoji_picker",
    "wa_web_select_all_chats_enabled",
    "wa_web_starred_msgs_search",
    "wa_web_status_reaction_sticker_reply_enabled",
    "wa_web_status_reshare_attribution_enabled",
    "wa_webtp_preload_thumbnail_renderer_no_exposure",
    "wa_webtp_thumbnail_renderer_mode",
    "wamo_agm_enabled",
    "wamo_privacy_tos_show_channels_nux_enabled",
    "wds_web_badge",
    "wds_web_menu_reaction_detail_panel_v2",
    "web_attach_menu_add_drawing_enabled",
    "web_biz_simple_signal_group_enabled",
    "web_bot_profile_pic_gql_migration_enabled",
    "web_bulk_add_contacts_enabled",
    "web_calling_enable_on_windows",
    "web_calling_full_screen_toggle_enabled",
    "web_chat_theme_drawer_title",
    "web_enable_camera_capture_refresh",
    "web_logout_unmigrated_companion",
    "web_media_worker_split_enabled",
    "web_memory_reduction",
    "web_menu_share_group",
    "web_msg_infra_remove_devices_on_406_error_enabled",
    "web_navigation_bar_updates_tab",
    "web_new_chat_flow_refresh_variant",
    "web_otp_copy_code_disabled",
    "web_read_self_watermark_processing",
    "web_test_abprop_delete_me",
    "web_wam_max_buffer_upload_size_bytes",
    "web_whats_new_banner_short_cooldown",
];

/// `WAWebHybridABPropsConfigs` into `hybrid`; the bare registry into `web`.
fn module_ident(wa_module: &str) -> String {
    let s = wa_module.strip_prefix("WAWeb").unwrap_or(wa_module);
    let s = s.strip_suffix("ABPropsConfigs").unwrap_or(s);
    if s.is_empty() {
        "web".to_string()
    } else {
        snake_case(s)
    }
}

fn type_variant(t: AbPropType) -> &'static str {
    match t {
        AbPropType::Bool => "Bool",
        AbPropType::Int => "Int",
        AbPropType::String => "Str",
        AbPropType::Float => "Float",
    }
}

fn default_lit(s: &Scalar) -> String {
    match s {
        Scalar::Bool(b) => format!("AbDefault::Bool({b})"),
        Scalar::Int(i) => format!("AbDefault::Int({i})"),
        Scalar::Float(f) => format!("AbDefault::Float({})", float_lit(*f)),
        Scalar::Str(v) => format!("AbDefault::Str({})", rust_lit(v)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn historical_names_are_guarded_per_registry() {
        let mut ir: AbPropsIr = serde_json::from_value(serde_json::json!({
            "waVersion":"2.3000.1052559453", "configs":[{
                "module":"WAWebABPropsConfigs", "name":"group_call_max_participants",
                "code":4190, "valueType":"int", "default":32
            }]
        }))
        .unwrap();
        assert!(generate(&ir).is_ok());
        ir.configs[0].module = "WAWebHybridABPropsConfigs".into();
        assert!(
            generate(&ir)
                .unwrap_err()
                .to_string()
                .contains("reconcile props::stale")
        );
    }
    use super::*;

    /// The emitters are fallible now; a test IR that trips a guard should
    /// fail the test, not be silently skipped.
    fn emitted(ir: &AbPropsIr) -> String {
        generate(ir).expect("emitter rejected the test IR")
    }

    fn cfg(module: &str, name: &str, code: u32, ty: AbPropType, default: Scalar) -> AbPropConfig {
        AbPropConfig {
            module: module.into(),
            name: name.into(),
            code,
            value_type: ty,
            default,
        }
    }

    fn ir(configs: Vec<AbPropConfig>) -> AbPropsIr {
        AbPropsIr {
            wa_version: "2.3000.1".into(),
            configs,
        }
    }

    #[test]
    fn emits_one_tree_shakeable_const_per_flag() {
        let code = emitted(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "flag_b",
                10,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "flag_f",
                11,
                AbPropType::Float,
                Scalar::Float(0.5),
            ),
        ]));
        assert!(code.contains("(WhatsApp 2.3000.1). DO NOT EDIT."));
        assert!(code.contains("pub mod web {"));
        assert!(code.contains(
            "pub const FLAG_B: AbProp = AbProp { name: \"flag_b\", code: 10, value_type: AbPropType::Bool, default: AbDefault::Bool(false) };"
        ));
        assert!(code.contains("default: AbDefault::Float(0.5) }"));
        assert!(code.contains("pub const ALL: &[&[AbProp]] = &[\n    web::ALL,\n];"));
    }

    #[test]
    fn keeps_registries_apart_and_disambiguates_names() {
        let code = emitted(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "shared",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "SHARED",
                2,
                AbPropType::Bool,
                Scalar::Bool(true),
            ),
            cfg(
                "WAWebHybridABPropsConfigs",
                "shared",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
        ]));
        assert!(code.contains("pub mod web {"));
        assert!(code.contains("pub mod hybrid {"));
        // Same key in two registries keeps the same const name in each; a
        // within-registry collision gets a suffix instead of a duplicate item.
        assert_eq!(code.matches("pub const SHARED: AbProp").count(), 2);
        assert_eq!(code.matches("pub const SHARED_2: AbProp").count(), 1);
    }

    #[test]
    fn an_ir_not_grouped_by_module_is_rejected() {
        // The run-grouping loop would visit the same registry twice and split
        // it across `web` and `web_2`, renaming half the flags in silence.
        let err = generate(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "a",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebHybridABPropsConfigs",
                "b",
                2,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "c",
                3,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
        ]))
        .expect_err("interleaved modules must not be emitted");
        assert!(err.to_string().contains("two runs"), "{err}");
    }

    #[test]
    fn a_non_finite_float_default_still_renders_a_literal() {
        let code = emitted(&ir(vec![cfg(
            "WAWebABPropsConfigs",
            "nanflag",
            1,
            AbPropType::Float,
            Scalar::Float(f64::NAN),
        )]));
        assert!(code.contains("AbDefault::Float(f64::NAN)"), "{code}");
        assert!(!code.contains("Float(NaN)"));
    }

    #[test]
    fn a_default_holding_quotes_stays_a_valid_literal() {
        let code = emitted(&ir(vec![cfg(
            "WAWebABPropsConfigs",
            "quoted",
            1,
            AbPropType::String,
            Scalar::Str("a\"b\\c".into()),
        )]));
        assert!(code.contains(r#"AbDefault::Str("a\"b\\c")"#), "{code}");
    }
}
