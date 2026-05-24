use roxmltree::{Document, Node};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) channel: String,
    pub(crate) url: String,
    pub(crate) artifact_location: Option<String>,
    pub(crate) artifact_checksum: Option<String>,
    pub(crate) artifact_checksum_type: Option<String>,
    pub(crate) description: Option<String>,
}

const RELEASES: &str = "https://chewing.im/releases/im.chewing.windows_chewing_tsf.releases.xml";

pub(crate) fn fetch_releases() -> Result<Vec<Release>, FetchReleasesError> {
    let releases_xml = ureq::get(RELEASES).call()?.body_mut().read_to_string()?;
    Ok(parse_releases(&releases_xml)?)
}

fn parse_releases(releases_xml: &str) -> Result<Vec<Release>, roxmltree::Error> {
    let doc = Document::parse(&releases_xml)?;
    let mut ret = vec![];
    for rel in doc.root_element().children() {
        if rel.has_tag_name("release") && rel.has_attribute("version") && rel.has_attribute("type")
        {
            let url = child_text(rel, "url").unwrap_or_default();
            let artifact = rel.descendants().find(|n| n.has_tag_name("artifact"));
            let artifact_location = artifact.and_then(|node| child_text(node, "location"));
            let checksum_node =
                artifact.and_then(|node| node.children().find(|n| n.has_tag_name("checksum")));
            let artifact_checksum = checksum_node.and_then(normalized_text);
            let artifact_checksum_type =
                checksum_node.and_then(|node| node.attribute("type").map(str::to_string));
            ret.push(Release {
                version: rel.attribute("version").unwrap().to_string(),
                channel: rel.attribute("type").unwrap().to_string(),
                url,
                artifact_location,
                artifact_checksum,
                artifact_checksum_type,
                description: rel
                    .children()
                    .find(|n| n.has_tag_name("description"))
                    .and_then(normalized_text),
            })
        }
    }
    Ok(ret)
}

fn child_text(node: Node<'_, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(normalized_text)
}

fn normalized_text(node: Node<'_, '_>) -> Option<String> {
    let text = node
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

#[derive(Debug, thiserror::Error)]
#[error("Failed to download release metadata")]
pub(crate) enum FetchReleasesError {
    Network(#[from] ureq::Error),
    ParseXml(#[from] roxmltree::Error),
}

#[cfg(test)]
mod tests {
    use super::{Release, parse_releases};

    #[test]
    fn parse_release_artifact_checksum_and_description() {
        let releases = parse_releases(
            r#"
            <releases>
              <release version="26.7.1.0" type="stable">
                <url>https://example.test/releases/v26.7.1.0</url>
                <description>
                  <p>修正候選字視窗。</p>
                  <p>改善更新檢查。</p>
                </description>
                <artifacts>
                  <artifact type="binary" platform="x86_64-windows-msvc">
                    <location>https://example.test/elven-ime.msi</location>
                    <checksum type="sha256">abc123</checksum>
                  </artifact>
                </artifacts>
              </release>
            </releases>
            "#,
        )
        .unwrap();

        assert_eq!(
            releases,
            vec![Release {
                version: "26.7.1.0".to_string(),
                channel: "stable".to_string(),
                url: "https://example.test/releases/v26.7.1.0".to_string(),
                artifact_location: Some("https://example.test/elven-ime.msi".to_string()),
                artifact_checksum: Some("abc123".to_string()),
                artifact_checksum_type: Some("sha256".to_string()),
                description: Some("修正候選字視窗。\n改善更新檢查。".to_string()),
            }]
        );
    }
}
