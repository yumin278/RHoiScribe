#[test]
fn docs_do_not_assume_a_local_workspace_path() {
    let readme = include_str!("../README.md");
    let client_setup = include_str!("../docs/client-setup.md");

    for document in [readme, client_setup] {
        assert!(!document.contains("D:\\GitHubProjects"));
        assert!(!document.contains("RHoiScribe\\target"));
        assert!(document.contains("<ABSOLUTE_PATH_TO_RHOISCRIBE"));
    }
}

#[test]
fn readme_explains_actual_mcp_usage() {
    let readme = include_str!("../README.md");

    assert!(readme.contains("prompts/list"));
    assert!(readme.contains("resources/read"));
    assert!(readme.contains("tools/call"));
    assert!(readme.contains("generate_localisation_batch"));
    assert!(readme.contains("rhoiscribe://hoi4/knowledge/catalog"));
}
