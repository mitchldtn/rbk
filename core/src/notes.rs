use std::path::{Path, PathBuf};


/// A parsed note loaded from a .md file.
#[derive(Debug, Clone)]
pub struct Note {
    pub name: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
    pub code_blocks: Vec<CodeBlock>,
    pub path: PathBuf,
}

/// A fenced code block extracted from markdown.
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub content: String,
}

/// Pure function: parse a markdown file's content into a Note.
/// Extracts YAML frontmatter (name, tags), first # heading as title,
/// and all fenced code blocks.
pub fn parse_note(slug: &str, content: &str, path: &Path) -> Note {
    let (frontmatter, body) = split_frontmatter(content);
    let (fm_name, tags) = parse_frontmatter(&frontmatter);
    let name = fm_name.unwrap_or_else(|| slug.to_string());
    let title = extract_title(&body);
    let code_blocks = extract_code_blocks(&body);

    Note {
        name,
        title,
        tags,
        body,
        code_blocks,
        path: path.to_path_buf(),
    }
}

/// Pure function: split content into frontmatter and body.
/// Frontmatter is between opening and closing `---` lines.
fn split_frontmatter(content: &str) -> (String, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), content.to_string());
    }

    let after_first = &trimmed[3..];
    let rest = after_first
        .trim_start_matches(|c: char| c == '-')
        .trim_start_matches('\n');

    match rest.find("\n---") {
        Some(end) => {
            let fm = rest[..end].to_string();
            let body = rest[end + 4..].trim_start_matches('-').trim_start().to_string();
            (fm, body)
        }
        None => (String::new(), content.to_string()),
    }
}

/// Pure function: extract name and tags from frontmatter text.
fn parse_frontmatter(fm: &str) -> (Option<String>, Vec<String>) {
    let mut name = None;
    let mut tags = Vec::new();

    for line in fm.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(unquote(val.trim()));
        } else if let Some(val) = trimmed.strip_prefix("tags:") {
            tags = parse_tag_list(val.trim());
        }
    }

    (name, tags)
}

/// Pure function: parse a YAML-style tag list like `[build, install, run]`.
fn parse_tag_list(s: &str) -> Vec<String> {
    let inner = s.trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|t| unquote(t.trim()))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Pure function: remove surrounding quotes from a string.
fn unquote(s: &str) -> String {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Pure function: extract the first `# heading` from markdown body.
fn extract_title(body: &str) -> Option<String> {
    body.lines()
        .find(|line| line.trim().starts_with("# "))
        .map(|line| line.trim().strip_prefix("# ").unwrap_or("").to_string())
}

/// Pure function: extract all fenced code blocks from markdown.
fn extract_code_blocks(body: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut content_lines: Vec<&str> = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        if !in_block && trimmed.starts_with("```") {
            in_block = true;
            content_lines.clear();
        } else if in_block && trimmed.starts_with("```") {
            blocks.push(CodeBlock {
                content: content_lines.join("\n"),
            });
            in_block = false;
        } else if in_block {
            content_lines.push(line);
        }
    }

    blocks
}

// ── Filesystem operations ──────────────────────────────────────────────────


/// Load all notes from a directory. Returns an empty vec if the dir doesn't exist.
pub fn load_notes_from_dir(dir: &Path) -> Vec<Note> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut notes: Vec<Note> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map_or(false, |ext| ext == "md")
        })
        .filter_map(|e| {
            let path = e.path();
            let slug = path.file_stem()?.to_str()?.to_string();
            let content = std::fs::read_to_string(&path).ok()?;
            Some(parse_note(&slug, &content, &path))
        })
        .collect();

    notes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    notes
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_full() {
        let content = r#"---
name: My Commands
tags: [build, dev]
---

# Build Commands

Some text.

```bash
cargo build
cargo test
```

More text.

```sql
SELECT * FROM events;
```
"#;
        let note = parse_note("my-commands", content, Path::new("/tmp/test.md"));
        assert_eq!(note.name, "My Commands");
        assert_eq!(note.tags, vec!["build", "dev"]);
        assert_eq!(note.title, Some("Build Commands".to_string()));
        assert_eq!(note.code_blocks.len(), 2);
        assert_eq!(note.code_blocks[0].content, "cargo build\ncargo test");
        assert_eq!(note.code_blocks[1].content, "SELECT * FROM events;");
    }

    #[test]
    fn test_parse_note_minimal() {
        let content = "Just some text with no frontmatter or headings.\n";
        let note = parse_note("bare-note", content, Path::new("/tmp/bare.md"));
        assert_eq!(note.name, "bare-note");
        assert_eq!(note.title, None);
        assert!(note.tags.is_empty());
        assert!(note.code_blocks.is_empty());
    }

    #[test]
    fn test_parse_note_no_name() {
        let content = r#"---
tags: [ssh]
---

# SSH Tunnels
"#;
        let note = parse_note("ssh-tunnels", content, Path::new("/tmp/ssh.md"));
        assert_eq!(note.name, "ssh-tunnels");
        assert_eq!(note.title, Some("SSH Tunnels".to_string()));
        assert_eq!(note.tags, vec!["ssh"]);
    }

}
