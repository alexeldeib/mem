use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Frontmatter fields for YAML serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Frontmatter {
    title: String,

    #[serde(rename = "created-at")]
    created_at: DateTime<Utc>,

    #[serde(rename = "updated-at")]
    updated_at: DateTime<Utc>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

/// A memory document with YAML frontmatter and markdown content.
#[derive(Debug, Clone)]
pub struct Mem {
    /// Relative path within .mems/ (without .md extension)
    pub path: PathBuf,

    /// Title (required)
    pub title: String,

    /// Creation timestamp (auto-managed)
    pub created_at: DateTime<Utc>,

    /// Last update timestamp (auto-managed)
    pub updated_at: DateTime<Utc>,

    /// Optional tags
    pub tags: Vec<String>,

    /// Markdown content (not in frontmatter)
    pub content: String,
}

impl Mem {
    /// Create a new Mem with current timestamp.
    pub fn new(path: PathBuf, title: String, content: String) -> Self {
        let now = Utc::now();
        Self {
            path,
            title,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
            content,
        }
    }

    /// Create a new Mem with tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Update the updated_at timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Parse a Mem from file content.
    ///
    /// Expected format:
    /// ```text
    /// ---
    /// title: Document Title
    /// created-at: 2025-01-19T12:00:00Z
    /// updated-at: 2025-01-19T12:00:00Z
    /// tags:
    ///   - tag1
    ///   - tag2
    /// ---
    /// Markdown content here
    /// ```
    pub fn parse(path: PathBuf, content: &str) -> Result<Self> {
        // Find frontmatter delimiters
        if !content.starts_with("---") {
            return Err(anyhow!("missing frontmatter: file must start with ---"));
        }

        // Find the closing delimiter
        let rest = &content[3..];
        let end_pos = rest
            .find("\n---")
            .ok_or_else(|| anyhow!("missing frontmatter: no closing --- found"))?;

        let yaml_content = rest[..end_pos].trim_start_matches('\n');
        let markdown_content = rest[end_pos + 4..].trim_start_matches('\n');

        // Parse YAML frontmatter
        let frontmatter: Frontmatter = serde_yaml::from_str(yaml_content)
            .map_err(|e| anyhow!("invalid frontmatter YAML: {e}"))?;

        Ok(Self {
            path,
            title: frontmatter.title,
            created_at: frontmatter.created_at,
            updated_at: frontmatter.updated_at,
            tags: frontmatter.tags,
            content: markdown_content.to_string(),
        })
    }

    /// Serialize the Mem to file content.
    pub fn serialize(&self) -> Result<String> {
        let frontmatter = Frontmatter {
            title: self.title.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            tags: self.tags.clone(),
        };

        let yaml = serde_yaml::to_string(&frontmatter)
            .map_err(|e| anyhow!("failed to serialize frontmatter: {e}"))?;

        Ok(format!("---\n{yaml}---\n{}", self.content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = r#"---
title: Test Document
created-at: 2025-01-19T12:00:00Z
updated-at: 2025-01-19T12:00:00Z
---
Hello, world!"#;

        let mem = Mem::parse(PathBuf::from("test"), content).unwrap();
        assert_eq!(mem.title, "Test Document");
        assert_eq!(mem.content, "Hello, world!");
        assert!(mem.tags.is_empty());
    }

    #[test]
    fn test_parse_with_tags() {
        let content = r#"---
title: Tagged Document
created-at: 2025-01-19T12:00:00Z
updated-at: 2025-01-19T12:00:00Z
tags:
  - rust
  - cli
---
Content with tags."#;

        let mem = Mem::parse(PathBuf::from("test"), content).unwrap();
        assert_eq!(mem.title, "Tagged Document");
        assert_eq!(mem.tags, vec!["rust", "cli"]);
        assert_eq!(mem.content, "Content with tags.");
    }

    #[test]
    fn test_parse_multiline_content() {
        let content = r#"---
title: Multiline
created-at: 2025-01-19T12:00:00Z
updated-at: 2025-01-19T12:00:00Z
---
First paragraph.

Second paragraph.

## Heading

More content."#;

        let mem = Mem::parse(PathBuf::from("test"), content).unwrap();
        assert!(mem.content.contains("First paragraph."));
        assert!(mem.content.contains("## Heading"));
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let content = "Just some text without frontmatter.";
        let result = Mem::parse(PathBuf::from("test"), content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unclosed_frontmatter() {
        let content = "---\ntitle: Test\nNo closing delimiter";
        let result = Mem::parse(PathBuf::from("test"), content);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let original = Mem::new(
            PathBuf::from("test/doc"),
            "Roundtrip Test".to_string(),
            "Test content here.".to_string(),
        )
        .with_tags(vec!["tag1".to_string(), "tag2".to_string()]);

        let serialized = original.serialize().unwrap();
        let parsed = Mem::parse(PathBuf::from("test/doc"), &serialized).unwrap();

        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.tags, original.tags);
        assert_eq!(parsed.content, original.content);
        // Timestamps may have slight precision differences, so check they're close
        assert_eq!(
            parsed.created_at.timestamp(),
            original.created_at.timestamp()
        );
    }

    #[test]
    fn test_new_sets_timestamps() {
        let mem = Mem::new(
            PathBuf::from("test"),
            "Title".to_string(),
            "Content".to_string(),
        );

        // Timestamps should be recent (within last second)
        let now = Utc::now();
        assert!((now - mem.created_at).num_seconds() < 1);
        assert!((now - mem.updated_at).num_seconds() < 1);
    }

    #[test]
    fn test_touch_updates_timestamp() {
        let mut mem = Mem::new(
            PathBuf::from("test"),
            "Title".to_string(),
            "Content".to_string(),
        );

        let original_updated = mem.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        mem.touch();

        assert!(mem.updated_at > original_updated);
        assert_eq!(mem.created_at.timestamp(), original_updated.timestamp());
    }
}
