//------------------------------------------------------------------------------------
// project_validation_tests.rs -- Part of RHoiScribe
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

use std::fs;

use super::{ProjectValidationCheck, ProjectValidationRequest, validate_hoi4_project};
use crate::tools::{ScanRoot, test_support::unique_test_dir};

#[test]
fn validation_reports_red_yellow_and_green_checks() {
    let root = unique_test_dir("project-validation");
    write_file(
        &root,
        "common/national_focus/sample_tree.txt",
        "focus_tree = {\n\tid = sample_tree\n\tfocus = { id = sample_rebuild title = sample_rebuild desc = sample_rebuild_desc }\n\tfocus = { id = sample_rebuild }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_panel\" texturefile = \"gfx/interface/sample/missing_panel.png\" } }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel\" background = { quadTextureSprite = \"GFX_sample_missing\" } } }\n",
    );
    write_file(
        &root,
        "localisation/simp_chinese/validation_fixture_l_simp_chinese.yml",
        "\u{feff}l_simp_chinese:\n sample_rebuild:0 \"重建\"\n",
    );

    let result = validate_hoi4_project(ProjectValidationRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        include_game_roots: Some(true),
    })
    .expect("validation should complete");

    assert_eq!(result.status, "red");
    assert!(result.index_summary.contains("file"));
    assert_red_yellow_green_checks(&result.checks);

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn validation_avoids_gui_name_and_vanilla_texture_false_positives() {
    let mod_root = unique_test_dir("project-validation-mod");
    let game_root = unique_test_dir("project-validation-game");
    write_file(
        &mod_root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_panel\" texturefile = \"gfx/interface/vanilla/panel.dds\" } }\n",
    );
    write_file(
        &mod_root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel\" background = { quadTextureSprite = \"GFX_sample_panel\" } } }\n",
    );
    write_file(
        &game_root,
        "gfx/interface/vanilla/panel.dds",
        "fake texture",
    );

    let result = validate_hoi4_project(ProjectValidationRequest {
        roots: vec![
            ScanRoot {
                path: mod_root.to_string_lossy().to_string(),
                role: Some("mod".to_string()),
            },
            ScanRoot {
                path: game_root.to_string_lossy().to_string(),
                role: Some("game".to_string()),
            },
        ],
        include_game_roots: Some(false),
    })
    .expect("validation should complete");

    assert!(
        !result
            .checks
            .iter()
            .any(|check| check.id == "missing_gfx_texture" && check.status != "green")
    );
    assert!(
        !result
            .checks
            .iter()
            .any(|check| check.id == "missing_localisation"
                && check.message.contains("sample_panel"))
    );

    fs::remove_dir_all(mod_root).expect("temp output should clean up");
    fs::remove_dir_all(game_root).expect("temp output should clean up");
}

#[test]
fn validation_reports_green_checks_for_clean_project_categories() {
    let root = unique_test_dir("project-validation-clean");
    write_file(
        &root,
        "descriptor.mod",
        "name=\"Clean Fixture\"\nsupported_version=\"1.19.*\"\n",
    );
    write_file(
        &root,
        "common/national_focus/sample_tree.txt",
        "focus_tree = {\n\tid = sample_tree\n\tfocus = { id = sample_clean_focus title = sample_clean_focus desc = sample_clean_focus_desc }\n}\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gfx",
        "spriteTypes = { spriteType = { name = \"GFX_sample_clean_panel\" texturefile = \"gfx/interface/sample/clean_panel.png\" } }\n",
    );
    write_file(
        &root,
        "interface/sample_interface.gui",
        "guiTypes = { containerWindowType = { name = \"sample_panel\" background = { quadTextureSprite = \"GFX_sample_clean_panel\" } } }\n",
    );
    write_file(&root, "gfx/interface/sample/clean_panel.png", "fake png");
    write_file(
        &root,
        "localisation/simp_chinese/clean_fixture_l_simp_chinese.yml",
        "\u{feff}l_simp_chinese:\n sample_clean_focus:0 \"清晰目标\"\n sample_clean_focus_desc:0 \"全部引用均已落地。\"\n",
    );

    let result = validate_hoi4_project(ProjectValidationRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        include_game_roots: Some(true),
    })
    .expect("validation should complete");

    assert_eq!(result.status, "green", "{:#?}", result.checks);
    for id in [
        "duplicate_definition",
        "brace_balance",
        "replace_path",
        "missing_gfx_texture",
        "missing_gfx_sprite",
        "missing_localisation",
    ] {
        assert_check(&result.checks, id, "green", "");
    }

    fs::remove_dir_all(root).expect("temp output should clean up");
}

fn write_file(root: &std::path::Path, relative_path: &str, content: &str) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent should be created");
    }
    fs::write(path, content).expect("fixture file should be written");
}

fn assert_check(checks: &[ProjectValidationCheck], id: &str, status: &str, text: &str) {
    assert!(checks.iter().any(|check| {
        check.id == id
            && check.status == status
            && (text.is_empty() || check.message.contains(text))
    }));
}

fn assert_red_yellow_green_checks(checks: &[ProjectValidationCheck]) {
    assert_check(checks, "duplicate_definition", "red", "sample_rebuild");
    assert_check(checks, "brace_balance", "red", "");
    assert_check(checks, "missing_gfx_texture", "red", "missing_panel");
    assert_check(checks, "missing_gfx_sprite", "yellow", "GFX_sample_missing");
    assert_check(
        checks,
        "missing_localisation",
        "yellow",
        "sample_rebuild_desc",
    );
    assert_check(checks, "index_completed", "green", "");
}
