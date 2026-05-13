use std::path::Path;

use sha2::{Digest, Sha256};

use crate::models::RawItem;
use crate::TianJiError;

const ATOM_NS: &str = "http://www.w3.org/2005/Atom";

pub fn parse_feed(feed_text: &str, source: &str) -> Result<Vec<RawItem>, TianJiError> {
    let document = roxmltree::Document::parse(feed_text).map_err(|error| {
        TianJiError::Input(format!(
            "Failed to parse feed for source: {source}: {error}"
        ))
    })?;
    let root = document.root_element();

    if let Some(channel) = root.children().find(|node| node.has_tag_name("channel")) {
        return Ok(parse_rss(channel, source));
    }

    if root.tag_name().name() == "feed" {
        return Ok(parse_atom(root, source));
    }

    Err(TianJiError::Input(format!(
        "Unsupported feed format for source {source}: expected RSS or Atom"
    )))
}

pub fn assign_canonical_hashes(items: &mut [RawItem]) {
    for item in items {
        item.entry_identity_hash = derive_canonical_entry_identity_hash(item);
        item.content_hash = derive_canonical_content_hash(item);
    }
}

pub fn derive_canonical_entry_identity_hash(item: &RawItem) -> String {
    sha256_hex(&format!(
        "{}|{}",
        clean_text(&item.link),
        clean_text(item.published_at.as_deref().unwrap_or(""))
    ))
}

pub fn derive_canonical_content_hash(item: &RawItem) -> String {
    sha256_hex(&format!(
        "{}|{}|{}",
        clean_text(&item.title),
        clean_text(&item.summary),
        clean_text(item.published_at.as_deref().unwrap_or(""))
    ))
}

pub fn fixture_source_name(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("fixture");
    format!("fixture:{name}")
}

fn parse_rss(channel: roxmltree::Node<'_, '_>, source: &str) -> Vec<RawItem> {
    let mut items = Vec::new();
    for item in channel.children().filter(|node| node.has_tag_name("item")) {
        let title = child_direct_text(item, "title").trim().to_string();
        if title.is_empty() {
            continue;
        }
        let published_at = child_direct_text(item, "pubDate").trim().to_string();
        items.push(RawItem {
            source: source.to_string(),
            title,
            summary: child_direct_text(item, "description").trim().to_string(),
            link: child_direct_text(item, "link").trim().to_string(),
            published_at: none_if_empty(published_at),
            entry_identity_hash: String::new(),
            content_hash: String::new(),
        });
    }
    items
}

fn parse_atom(root: roxmltree::Node<'_, '_>, source: &str) -> Vec<RawItem> {
    let mut items = Vec::new();
    for entry in root.children().filter(|node| has_atom_tag(*node, "entry")) {
        let title = child_descendant_text(entry, "title").trim().to_string();
        if title.is_empty() {
            continue;
        }
        let summary = first_non_empty(&[
            child_descendant_text(entry, "summary"),
            child_descendant_text(entry, "content"),
        ]);
        let published_at = first_non_empty(&[
            child_descendant_text(entry, "published"),
            child_descendant_text(entry, "updated"),
        ]);
        let link = entry
            .children()
            .find(|node| has_atom_tag(*node, "link"))
            .and_then(|node| node.attribute("href"))
            .unwrap_or("")
            .to_string();

        items.push(RawItem {
            source: source.to_string(),
            title,
            summary,
            link,
            published_at: none_if_empty(published_at),
            entry_identity_hash: String::new(),
            content_hash: String::new(),
        });
    }
    items
}

fn child_direct_text(node: roxmltree::Node<'_, '_>, tag_name: &str) -> String {
    node.children()
        .find(|child| child.has_tag_name(tag_name))
        .and_then(|child| child.text())
        .unwrap_or_default()
        .to_string()
}

fn child_descendant_text(node: roxmltree::Node<'_, '_>, tag_name: &str) -> String {
    node.children()
        .find(|child| has_atom_tag(*child, tag_name))
        .map(|child| {
            child
                .descendants()
                .filter(|descendant| descendant.is_text())
                .filter_map(|descendant| descendant.text())
                .collect()
        })
        .unwrap_or_default()
}

fn has_atom_tag(node: roxmltree::Node<'_, '_>, tag_name: &str) -> bool {
    node.tag_name().namespace() == Some(ATOM_NS) && node.tag_name().name() == tag_name
}

fn first_non_empty(values: &[String]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("")
        .to_string()
}

fn none_if_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
