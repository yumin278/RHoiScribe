use rhoiscribe::resources::{LATEST_UPDATE_URI, ResourceCatalog};
use rmcp::model::ResourceContents;

#[test]
fn resource_catalog_lists_latest_update_and_knowledge_topics() {
    let catalog = ResourceCatalog::load_embedded().expect("resources should load");
    let resources = catalog.to_mcp_resources();

    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == LATEST_UPDATE_URI)
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "rhoiscribe://hoi4/knowledge/catalog")
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "rhoiscribe://hoi4/knowledge/localisation.encoding")
    );
}

#[test]
fn latest_update_resource_is_a_local_snapshot_with_source() {
    let catalog = ResourceCatalog::load_embedded().expect("resources should load");
    let result = catalog
        .read_text(LATEST_UPDATE_URI)
        .expect("latest update should be readable");

    assert!(result.contains("Snapshot date: 2026-06-06"));
    assert!(result.contains("1.18.3.0.7709"));
    assert!(result.contains("Steam News"));
}

#[test]
fn knowledge_topic_resource_returns_text_content() {
    let catalog = ResourceCatalog::load_embedded().expect("resources should load");
    let result = catalog
        .read_mcp_resource("rhoiscribe://hoi4/knowledge/localisation.encoding")
        .expect("topic resource should be readable");

    assert_eq!(result.contents.len(), 1);
    let ResourceContents::TextResourceContents {
        text, mime_type, ..
    } = &result.contents[0]
    else {
        panic!("topic should be returned as text");
    };

    assert_eq!(mime_type.as_deref(), Some("text/markdown"));
    assert!(text.contains("UTF-8 BOM"));
}
