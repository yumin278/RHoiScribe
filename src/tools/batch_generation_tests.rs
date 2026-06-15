//------------------------------------------------------------------------------------
// batch_generation_tests.rs -- Part of RHoiScribe
//
// Copyright (C) 2026 CzXieDdan. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// https://github.com/czxieddan/RHoiScribe
//------------------------------------------------------------------------------------

use super::{
    DecisionBatchRequest, DecisionEntry, EventBatchRequest, EventEntry, EventOptionEntry,
    FocusBatchRequest, FocusEntry, ScriptAssignment, ToolEngine,
};

#[test]
fn focus_batch_renders_complete_optional_focus_blocks() {
    let result = ToolEngine::generate_focus_batch(FocusBatchRequest {
        country_tag: "GOL".to_string(),
        tree_id: "GOL_ogas_tree".to_string(),
        focuses: vec![FocusEntry {
            id: "GOL_finish_ogas".to_string(),
            icon: Some("GFX_goal_generic_intelligence_exchange".to_string()),
            x: Some(4),
            y: Some(2),
            cost: Some(8),
            prerequisite: vec!["GOL_initial_planning".to_string()],
            mutually_exclusive: vec!["GOL_abandon_ogas".to_string()],
            available: Some("has_government = communism".to_string()),
            bypass: Some("has_country_flag = GOL_ogas_complete".to_string()),
            will_lead_to_war_with: Some("GER".to_string()),
            completion_reward: Some(
                "hidden_effect = { set_country_flag = GOL_ogas_complete }\n\
                 add_political_power = 75"
                    .to_string(),
            ),
            ai_will_do: Some("factor = 5\nmodifier = { factor = 0 has_war = yes }".to_string()),
            extra_blocks: vec![
                "search_filters = { FOCUS_FILTER_POLITICAL FOCUS_FILTER_INDUSTRY }".to_string(),
            ],
            ..Default::default()
        }],
        dry_run: true,
        output_root: None,
    })
    .expect("focus batch should render complex focus blocks");

    let content = &result.files[0].content;
    assert!(content.contains("icon = GFX_goal_generic_intelligence_exchange"));
    assert!(content.contains("x = 4"));
    assert!(content.contains("prerequisite = {\n\t\t\tfocus = GOL_initial_planning\n\t\t}"));
    assert!(content.contains("mutually_exclusive = {\n\t\t\tfocus = GOL_abandon_ogas\n\t\t}"));
    assert!(content.contains("will_lead_to_war_with = GER"));
    assert!(content.contains("hidden_effect = {"));
    assert!(content.contains("set_country_flag = GOL_ogas_complete"));
    assert!(content.contains("ai_will_do = {"));
    assert!(content.contains("search_filters = {"));
    assert!(content.contains("FOCUS_FILTER_POLITICAL"));
    assert!(content.contains("FOCUS_FILTER_INDUSTRY"));
}

#[test]
fn event_batch_renders_news_events_options_and_hidden_effects() {
    let result = ToolEngine::generate_event_batch(EventBatchRequest {
        namespace: "GOL".to_string(),
        events: vec![EventEntry {
            id: Some("GOL.10".to_string()),
            event_type: Some("news_event".to_string()),
            title: Some("GOL.10.t".to_string()),
            desc: Some("GOL.10.d".to_string()),
            picture: Some("GFX_report_event_generic_factory".to_string()),
            major: Some(true),
            is_triggered_only: Some(true),
            trigger: Some("has_country_flag = GOL_ogas_complete".to_string()),
            immediate: Some("hidden_effect = { set_global_flag = GOL_ogas_seen }".to_string()),
            options: vec![EventOptionEntry {
                name: "GOL.10.a".to_string(),
                ai_chance: Some("factor = 1".to_string()),
                effects: Some("add_war_support = 0.05".to_string()),
                hidden_effect: Some("set_country_flag = GOL_news_acknowledged".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        dry_run: true,
        output_root: None,
    })
    .expect("event batch should render complex event blocks");

    let content = &result.files[0].content;
    assert!(content.contains("news_event = {"));
    assert!(content.contains("id = GOL.10"));
    assert!(content.contains("picture = GFX_report_event_generic_factory"));
    assert!(content.contains("major = yes"));
    assert!(content.contains("trigger = {"));
    assert!(content.contains("option = {"));
    assert!(content.contains("ai_chance = {"));
    assert!(content.contains("hidden_effect = {"));
    assert!(content.contains("set_country_flag = GOL_news_acknowledged"));
}

#[test]
fn decision_batch_renders_missions_dynamic_and_custom_effects() {
    let result = ToolEngine::generate_decision_batch(DecisionBatchRequest {
        category_id: "GOL_debug_decisions".to_string(),
        icon: Some("generic_political_discourse".to_string()),
        visible: Some("has_country_flag = GOL_debug_enabled".to_string()),
        allowed: None,
        decisions: vec![DecisionEntry {
            id: "GOL_debug_push_crisis".to_string(),
            icon: Some("decision_generic_prepare_civil_war".to_string()),
            cost: Some(15),
            days_mission_timeout: Some(30),
            fire_only_once: Some(true),
            available: Some("has_political_power > 15".to_string()),
            cancel_trigger: Some("NOT = { has_country_flag = GOL_debug_enabled }".to_string()),
            complete_effect: Some(
                "custom_effect_tooltip = GOL_debug_declare_war_warning\n\
                 hidden_effect = { set_country_flag = GOL_debug_crisis_used }"
                    .to_string(),
            ),
            timeout_effect: Some("clr_country_flag = GOL_debug_crisis_pending".to_string()),
            ai_will_do: Some("factor = 0".to_string()),
            extra_assignments: vec![ScriptAssignment {
                key: "dynamic".to_string(),
                value: "yes".to_string(),
            }],
            ..Default::default()
        }],
        dry_run: true,
        output_root: None,
    })
    .expect("decision batch should render complex decision blocks");

    let content = &result.files[0].content;
    assert!(content.contains("icon = generic_political_discourse"));
    assert!(content.contains("GOL_debug_push_crisis = {"));
    assert!(content.contains("icon = decision_generic_prepare_civil_war"));
    assert!(content.contains("days_mission_timeout = 30"));
    assert!(content.contains("dynamic = yes"));
    assert!(content.contains("available = {"));
    assert!(content.contains("custom_effect_tooltip = GOL_debug_declare_war_warning"));
    assert!(content.contains("hidden_effect = {"));
    assert!(content.contains("set_country_flag = GOL_debug_crisis_used"));
    assert!(content.contains("timeout_effect = {"));
}
