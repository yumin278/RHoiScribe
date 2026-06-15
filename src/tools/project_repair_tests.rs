//------------------------------------------------------------------------------------
// project_repair_tests.rs -- Part of RHoiScribe
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

use encoding_rs::GBK;

use super::{
    RepairCheck, RepairHoi4ProjectRequest, RepairHoi4ProjectResult, detect_ffmpeg_with_installer,
    ffprobe_command, repair_hoi4_project,
};
use crate::tools::{ScanRoot, test_support::unique_test_dir};

#[test]
fn dry_run_reports_encoding_and_media_repairs_without_writing() {
    let root = unique_test_dir("project-repair");
    write_bytes(
        &root,
        "localisation/simp_chinese/repair_fixture_l_simp_chinese.yml",
        b"l_simp_chinese:\n sample_key:0 \"text\"\n",
    );
    write_bytes(
        &root,
        "common/national_focus/sample_focus.txt",
        &[0xEF, 0xBB, 0xBF, b'f', b'o', b'c', b'u', b's'],
    );
    write_bytes(&root, "sound/effect.ogg", b"not real audio");
    write_bytes(&root, "music/theme.ogg", b"not real audio");

    let result = repair_hoi4_project(RepairHoi4ProjectRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        dry_run: true,
        apply: Some(false),
        install_ffmpeg: Some(false),
        format_scripts: Some(false),
        check_media: Some(true),
        ffmpeg_path: Some(
            root.join("missing-ffmpeg.exe")
                .to_string_lossy()
                .to_string(),
        ),
    })
    .expect("repair dry-run should complete");

    assert!(result.dry_run);
    assert!(!result.applied);
    assert!(!result.ffmpeg.available);
    assert_repair_check(&result.checks, "localisation_bom", "yellow");
    assert_repair_check(&result.checks, "script_no_bom", "yellow");
    assert_repair_check(&result.checks, "sound_wav_only", "red");
    assert_repair_check(&result.checks, "music_ogg_probe", "yellow");
    assert_change_planned(&result, "add_utf8_bom");
    assert!(
        !fs::read(root.join("localisation/simp_chinese/repair_fixture_l_simp_chinese.yml"))
            .expect("localisation should remain readable")
            .starts_with(&[0xEF, 0xBB, 0xBF])
    );

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn apply_repairs_bom_rules_and_formats_scripts() {
    let root = unique_test_dir("project-repair");
    write_bytes(
        &root,
        "localisation/english/repair_fixture_l_english.yml",
        b"l_english:\n sample_key:0 \"Text\"\n",
    );
    write_bytes(
        &root,
        "interface/credits.txt",
        b"credits = { name = Test }\n",
    );
    write_bytes(
        &root,
        "common/scripted_effects/sample_effects.txt",
        b"\xEF\xBB\xBFsample_effect={add_political_power=1}",
    );

    let result = repair_hoi4_project(RepairHoi4ProjectRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        dry_run: false,
        apply: Some(true),
        install_ffmpeg: Some(false),
        format_scripts: Some(true),
        check_media: Some(false),
        ffmpeg_path: None,
    })
    .expect("repair apply should complete");

    assert!(result.applied);
    assert!(
        fs::read(root.join("localisation/english/repair_fixture_l_english.yml"))
            .expect("localisation should read")
            .starts_with(&[0xEF, 0xBB, 0xBF])
    );
    assert!(
        fs::read(root.join("interface/credits.txt"))
            .expect("credits should read")
            .starts_with(&[0xEF, 0xBB, 0xBF])
    );
    let script = fs::read(root.join("common/scripted_effects/sample_effects.txt"))
        .expect("script should read");
    assert!(!script.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert!(String::from_utf8_lossy(&script).contains("sample_effect = {"));

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn apply_repairs_legacy_encoded_localisation_to_utf8_bom() {
    let root = unique_test_dir("project-repair");
    let (gbk_bytes, _, _) = GBK.encode("l_simp_chinese:\n sample_key:0 \"测试文本\"\n");
    write_bytes(
        &root,
        "localisation/simp_chinese/legacy_l_simp_chinese.yml",
        gbk_bytes.as_ref(),
    );

    let result = repair_hoi4_project(RepairHoi4ProjectRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        dry_run: false,
        apply: Some(true),
        install_ffmpeg: Some(false),
        format_scripts: Some(false),
        check_media: Some(false),
        ffmpeg_path: None,
    })
    .expect("repair apply should convert legacy localisation");

    assert!(result.applied);
    assert_repair_check(&result.checks, "text_encoding", "yellow");
    assert_change_applied(&result, "convert_to_utf8");
    let repaired = fs::read(root.join("localisation/simp_chinese/legacy_l_simp_chinese.yml"))
        .expect("localisation should read");
    assert!(repaired.starts_with(&[0xEF, 0xBB, 0xBF]));
    let text = String::from_utf8(repaired[3..].to_vec()).expect("output should be utf-8");
    assert!(text.contains("测试文本"));

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn format_repair_skips_comments_and_quoted_strings() {
    let root = unique_test_dir("project-repair");
    let original =
        "sample_effect={ log=\"hello world\" # keep this comment\n add_political_power=1 }\n";
    write_bytes(
        &root,
        "common/scripted_effects/sample_effects.txt",
        original.as_bytes(),
    );

    let result = repair_hoi4_project(RepairHoi4ProjectRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        dry_run: false,
        apply: Some(true),
        install_ffmpeg: Some(false),
        format_scripts: Some(true),
        check_media: Some(false),
        ffmpeg_path: None,
    })
    .expect("repair apply should complete");

    assert!(
        result
            .checks
            .iter()
            .any(|check| { check.id == "script_format_skipped" && check.status == "yellow" })
    );
    assert_eq!(
        fs::read_to_string(root.join("common/scripted_effects/sample_effects.txt"))
            .expect("script should read"),
        original
    );

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn ffmpeg_install_request_returns_script_when_missing_in_dry_run() {
    let root = unique_test_dir("project-repair");
    write_bytes(&root, "music/theme.ogg", b"not real audio");

    let result = repair_hoi4_project(RepairHoi4ProjectRequest {
        roots: vec![ScanRoot {
            path: root.to_string_lossy().to_string(),
            role: Some("mod".to_string()),
        }],
        dry_run: true,
        apply: Some(false),
        install_ffmpeg: Some(true),
        format_scripts: Some(false),
        check_media: Some(true),
        ffmpeg_path: Some(
            root.join("missing-ffmpeg.exe")
                .to_string_lossy()
                .to_string(),
        ),
    })
    .expect("repair should return ffmpeg status");

    assert!(result.ffmpeg.install_required);
    assert!(result.ffmpeg.install_script.contains("ffmpeg"));
    assert!(!result.ffmpeg.install_attempted);

    fs::remove_dir_all(root).expect("temp output should clean up");
}

#[test]
fn approved_non_dry_run_attempts_silent_ffmpeg_install_when_missing() {
    let result =
        detect_ffmpeg_with_installer(Some("Z:/missing/ffmpeg.exe"), true, true, false, || {
            Err("installer unavailable in test".to_string())
        });

    assert!(result.install_required);
    assert!(result.install_attempted);
    assert!(!result.install_succeeded);
    assert_eq!(
        result.install_error.as_deref(),
        Some("installer unavailable in test")
    );
    assert!(result.message.contains("attempted"));
}

#[test]
fn dry_run_does_not_attempt_silent_ffmpeg_install_even_when_approved() {
    let result =
        detect_ffmpeg_with_installer(Some("Z:/missing/ffmpeg.exe"), true, true, true, || {
            panic!("dry-run must not run installer")
        });

    assert!(result.install_required);
    assert!(!result.install_attempted);
    assert!(!result.install_succeeded);
    assert!(result.message.contains("Dry-run mode"));
}

#[test]
fn ffprobe_preserves_windows_executable_extension() {
    assert_eq!(
        ffprobe_command(r"C:\tools\ffmpeg\bin\ffmpeg.exe"),
        r"C:\tools\ffmpeg\bin\ffprobe.exe"
    );
    assert!(ffprobe_command("/usr/local/bin/ffmpeg").ends_with("ffprobe"));
}

fn write_bytes(root: &std::path::Path, relative_path: &str, bytes: &[u8]) {
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent should be created");
    }
    fs::write(path, bytes).expect("fixture file should be written");
}

fn assert_repair_check(checks: &[RepairCheck], id: &str, status: &str) {
    assert!(
        checks
            .iter()
            .any(|check| check.id == id && check.status == status)
    );
}

fn assert_change_planned(result: &RepairHoi4ProjectResult, action: &str) {
    assert!(
        result
            .changes
            .iter()
            .any(|change| change.action == action && !change.applied)
    );
}

fn assert_change_applied(result: &RepairHoi4ProjectResult, action: &str) {
    assert!(
        result
            .changes
            .iter()
            .any(|change| change.action == action && change.applied)
    );
}
