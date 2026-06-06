use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use rhoiscribe::tools::{
    BatchEntry, FormatParadoxScriptRequest, LocalisationBatchRequest, ToolCatalog, ToolEngine,
    ValidateHoi4PathsRequest,
};

#[test]
fn tool_catalog_lists_planned_tools() {
    let names = ToolCatalog::builtin().names();

    assert_eq!(
        names,
        vec![
            "generate_localisation_batch",
            "generate_focus_batch",
            "generate_event_batch",
            "generate_decision_batch",
            "validate_hoi4_paths",
            "format_paradox_script"
        ]
    );
}

#[test]
fn localisation_batch_dry_run_generates_game_readable_file() {
    let result = ToolEngine::generate_localisation_batch(LocalisationBatchRequest {
        language: "l_simp_chinese".to_string(),
        file_stem: "economic_recovery".to_string(),
        key_prefix: Some("BTA_01".to_string()),
        entries: vec![BatchEntry {
            id: "FS_01".to_string(),
            title: "经济复苏".to_string(),
            description: Some("启动国家经济复苏计划。".to_string()),
        }],
        dry_run: true,
        output_root: None,
    })
    .expect("localisation dry-run should succeed");

    assert!(result.dry_run);
    assert_eq!(
        result.files[0].path,
        "localisation/simp_chinese/economic_recovery_l_simp_chinese.yml"
    );
    assert_eq!(result.files[0].encoding.as_deref(), Some("utf-8-bom"));
    assert!(result.files[0].content.contains("l_simp_chinese:"));
    assert!(result.files[0].content.contains("BTA_01_FS_01:0"));
}

#[test]
fn localisation_batch_write_mode_writes_utf8_bom_file() {
    let output_root = unique_temp_dir();
    let result = ToolEngine::generate_localisation_batch(LocalisationBatchRequest {
        language: "l_english".to_string(),
        file_stem: "events".to_string(),
        key_prefix: None,
        entries: vec![BatchEntry {
            id: "rhoiscribe_event_1".to_string(),
            title: "Recovery Begins".to_string(),
            description: None,
        }],
        dry_run: false,
        output_root: Some(output_root.to_string_lossy().to_string()),
    })
    .expect("localisation write should succeed");

    let file_path = output_root.join(&result.files[0].path);
    let bytes = fs::read(&file_path).expect("written localisation should be readable");
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert!(String::from_utf8_lossy(&bytes).contains("rhoiscribe_event_1:0"));

    fs::remove_dir_all(output_root).expect("temp output should clean up");
}

#[test]
fn path_validation_rejects_unsafe_or_non_mod_paths() {
    let result = ToolEngine::validate_hoi4_paths(ValidateHoi4PathsRequest {
        paths: vec![
            "common/national_focus/BTA_01.txt".to_string(),
            "../outside.txt".to_string(),
            "random/file.txt".to_string(),
        ],
    });

    assert_eq!(result.valid_paths, vec!["common/national_focus/BTA_01.txt"]);
    assert_eq!(result.invalid_paths.len(), 2);
}

#[test]
fn paradox_formatter_balances_readable_indentation() {
    let result = ToolEngine::format_paradox_script(FormatParadoxScriptRequest {
        script: "focus={id=BTA_01 cost=10 completion_reward={add_political_power=50}}".to_string(),
    });

    assert!(result.formatted.contains("focus = {"));
    assert!(result.formatted.contains("\n\tid = BTA_01"));
    assert!(result.formatted.contains("\n}"));
}

fn unique_temp_dir() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rhoiscribe-tool-test-{}", suffix))
}
