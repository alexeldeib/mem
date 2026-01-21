use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use mem::mem::Mem;
use mem::storage::Storage;
use serde::Serialize;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mem")]
#[command(about = "A markdown-based knowledge tracking CLI for projects")]
#[command(version)]
struct Cli {
    /// Specify .mems/ directories to search (can be repeated)
    #[arg(long = "dir", global = true)]
    dirs: Vec<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new .mems/ directory
    Init,

    /// Add a new mem
    Add {
        /// Path for the mem (e.g., "arch/decisions/adr-001")
        path: String,

        /// Content of the mem
        #[arg(short, long)]
        content: Option<String>,

        /// Title (defaults to last path segment)
        #[arg(short, long)]
        title: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Overwrite if exists
        #[arg(short, long)]
        force: bool,
    },

    /// Show a mem's content
    Show {
        /// Path of the mem
        path: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Edit an existing mem
    Edit {
        /// Path of the mem
        path: String,

        /// New content
        #[arg(short, long)]
        content: Option<String>,

        /// New title
        #[arg(short, long)]
        title: Option<String>,

        /// New tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Remove a mem
    Rm {
        /// Path of the mem
        path: String,
    },

    /// List mems
    Ls {
        /// Path to list under (optional)
        path: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Search mems by content
    Find {
        /// Search query
        query: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show hierarchy as tree
    Tree {
        /// Path to show tree from (optional)
        path: Option<String>,
    },

    /// List stale mems not updated recently
    Stale {
        /// Days threshold (default: 90)
        #[arg(long, default_value = "90")]
        days: u32,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Validate all mems
    Lint,

    /// Archive a mem
    Archive {
        /// Path of the mem
        path: String,
    },

    /// Dump all mems under a path as concatenated markdown
    Dump {
        /// Path prefix to dump (defaults to all mems)
        path: Option<String>,
    },
}

/// JSON representation for mem output.
#[derive(Serialize)]
struct MemJson {
    path: String,
    title: String,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
    content: String,
}

impl From<&Mem> for MemJson {
    fn from(mem: &Mem) -> Self {
        Self {
            path: mem.path.to_string_lossy().to_string(),
            title: mem.title.clone(),
            created_at: mem.created_at.to_rfc3339(),
            updated_at: mem.updated_at.to_rfc3339(),
            tags: mem.tags.clone(),
            content: mem.content.clone(),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Add {
            path,
            content,
            title,
            tags,
            force,
        } => cmd_add(&path, content, title, tags, force)?,
        Commands::Show { path, json } => cmd_show(&path, json)?,
        Commands::Edit {
            path,
            content,
            title,
            tags,
        } => cmd_edit(&path, content, title, tags)?,
        Commands::Rm { path } => cmd_rm(&path)?,
        Commands::Ls { path, json } => cmd_ls(path.as_deref(), json, &cli.dirs)?,
        Commands::Find { query, json } => cmd_find(&query, json, &cli.dirs)?,
        Commands::Tree { path } => cmd_tree(path.as_deref(), &cli.dirs)?,
        Commands::Stale { days, json } => cmd_stale(days, json, &cli.dirs)?,
        Commands::Lint => cmd_lint(&cli.dirs)?,
        Commands::Archive { path } => cmd_archive(&path)?,
        Commands::Dump { path } => cmd_dump(path.as_deref(), &cli.dirs)?,
    }

    Ok(())
}

/// Get storages from explicit dirs or find default .mems/
fn get_storages(dirs: &[PathBuf]) -> Result<Vec<(String, Storage)>> {
    if dirs.is_empty() {
        let storage = Storage::find()?;
        Ok(vec![("".to_string(), storage)])
    } else {
        let mut storages = Vec::new();
        for dir in dirs {
            if !dir.exists() {
                return Err(anyhow!("directory not found: {}", dir.display()));
            }
            let label = dir.to_string_lossy().to_string();
            storages.push((label, Storage::new(dir.clone())));
        }
        Ok(storages)
    }
}

fn cmd_init() -> Result<()> {
    Storage::init()?;
    println!("Initialized .mems/ directory");
    Ok(())
}

fn cmd_add(
    path: &str,
    content: Option<String>,
    title: Option<String>,
    tags: Option<String>,
    force: bool,
) -> Result<()> {
    let storage = Storage::find()?;

    // Check if mem already exists
    if storage.exists(path) && !force {
        return Err(anyhow!(
            "mem already exists: {path} (use --force to overwrite)"
        ));
    }

    // Get content from flag or stdin
    let content = match content {
        Some(c) => c,
        None => {
            // Try reading from stdin
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            if buf.is_empty() {
                return Err(anyhow!("no content provided (use -c or pipe via stdin)"));
            }
            buf
        }
    };

    // Derive title from path if not provided
    let title = title.unwrap_or_else(|| {
        path.rsplit('/')
            .next()
            .unwrap_or(path)
            .replace(['-', '_'], " ")
    });

    // Parse tags
    let tags: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mem = Mem::new(PathBuf::from(path), title, content).with_tags(tags);
    storage.write_mem(&mem)?;

    println!("Created: {path}");
    Ok(())
}

fn cmd_show(path: &str, json: bool) -> Result<()> {
    let storage = Storage::find()?;
    let mem = storage.read_mem(path)?;

    if json {
        let json_output = MemJson::from(&mem);
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        println!("# {}", mem.title);
        println!();
        if !mem.tags.is_empty() {
            println!("Tags: {}", mem.tags.join(", "));
            println!();
        }
        println!("{}", mem.content);
    }

    Ok(())
}

fn cmd_edit(
    path: &str,
    content: Option<String>,
    title: Option<String>,
    tags: Option<String>,
) -> Result<()> {
    let storage = Storage::find()?;
    let mut mem = storage.read_mem(path)?;

    // Update fields if provided
    if let Some(c) = content {
        mem.content = c;
    }
    if let Some(t) = title {
        mem.title = t;
    }
    if let Some(t) = tags {
        mem.tags = t.split(',').map(|s| s.trim().to_string()).collect();
    }

    // Update timestamp
    mem.touch();

    storage.write_mem(&mem)?;
    println!("Updated: {path}");
    Ok(())
}

fn cmd_rm(path: &str) -> Result<()> {
    let storage = Storage::find()?;
    storage.delete_mem(path)?;
    println!("Deleted: {path}");
    Ok(())
}

fn cmd_ls(path: Option<&str>, json: bool, dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let multi = storages.len() > 1;

    let mut all_mems: Vec<(String, Mem)> = Vec::new();
    for (label, storage) in &storages {
        let mems = match path {
            Some(p) => storage.list_mems_under(p)?,
            None => storage.list_mems()?,
        };
        for mem in mems {
            all_mems.push((label.clone(), mem));
        }
    }

    if json {
        let json_output: Vec<MemJson> = all_mems.iter().map(|(_, m)| MemJson::from(m)).collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else if all_mems.is_empty() {
        println!("No mems found");
    } else {
        for (label, mem) in &all_mems {
            let path_str = mem.path.to_string_lossy();
            let tags = if mem.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", mem.tags.join(", "))
            };
            let prefix = if multi {
                format!("[{label}] ")
            } else {
                String::new()
            };
            println!("{prefix}{path_str}: {}{tags}", mem.title);
        }
    }

    Ok(())
}

fn cmd_archive(path: &str) -> Result<()> {
    let storage = Storage::find()?;
    storage.archive_mem(path)?;
    println!("Archived: {path}");
    Ok(())
}

fn cmd_find(query: &str, json: bool, dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let multi = storages.len() > 1;

    // Case-insensitive substring search on title and content
    let query_lower = query.to_lowercase();
    let mut matches: Vec<(String, Mem)> = Vec::new();

    for (label, storage) in &storages {
        let mems = storage.list_mems()?;
        for mem in mems {
            if mem.title.to_lowercase().contains(&query_lower)
                || mem.content.to_lowercase().contains(&query_lower)
            {
                matches.push((label.clone(), mem));
            }
        }
    }

    if json {
        let json_output: Vec<MemJson> = matches.iter().map(|(_, m)| MemJson::from(m)).collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else if matches.is_empty() {
        println!("No matches found for: {query}");
    } else {
        for (label, mem) in &matches {
            let path_str = mem.path.to_string_lossy();
            let prefix = if multi {
                format!("[{label}] ")
            } else {
                String::new()
            };
            println!("{prefix}{path_str}: {}", mem.title);
        }
    }

    Ok(())
}

fn cmd_tree(path: Option<&str>, dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let multi = storages.len() > 1;

    let mut any_found = false;
    for (idx, (label, storage)) in storages.iter().enumerate() {
        let mems = match path {
            Some(p) => storage.list_mems_under(p)?,
            None => storage.list_mems()?,
        };

        if mems.is_empty() {
            continue;
        }
        any_found = true;

        // Add separator between directories
        if multi && idx > 0 {
            println!();
        }

        // Build tree structure: map parent path -> mems at that level
        let mut tree: std::collections::BTreeMap<String, Vec<&Mem>> =
            std::collections::BTreeMap::new();
        // Track all directory paths that exist
        let mut all_dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for mem in &mems {
            let path_str = mem.path.to_string_lossy().to_string();
            let parts: Vec<&str> = path_str.split('/').collect();

            // Add all parent directories to the set
            for i in 1..parts.len() {
                all_dirs.insert(parts[..i].join("/"));
            }

            // Group by parent path
            if parts.len() == 1 {
                tree.entry(String::new()).or_default().push(mem);
            } else {
                let parent = parts[..parts.len() - 1].join("/");
                tree.entry(parent).or_default().push(mem);
            }
        }

        // Print tree with box-drawing characters
        let root_name = if multi {
            label.as_str()
        } else {
            path.unwrap_or(".mems")
        };
        print_tree(&tree, &all_dirs, "", "", root_name);
    }

    if !any_found {
        println!("No mems found");
    }

    Ok(())
}

fn print_tree(
    tree: &std::collections::BTreeMap<String, Vec<&Mem>>,
    all_dirs: &std::collections::BTreeSet<String>,
    parent: &str,
    prefix: &str,
    root_name: &str,
) {
    // Get items at this level
    let items = tree.get(parent).map(|v| v.as_slice()).unwrap_or(&[]);

    // Get subdirectories at this level (direct children only)
    let subdirs: Vec<&String> = all_dirs
        .iter()
        .filter(|d| {
            if parent.is_empty() {
                !d.contains('/')
            } else {
                d.starts_with(&format!("{parent}/"))
                    && d[parent.len() + 1..].split('/').count() == 1
            }
        })
        .collect();

    if prefix.is_empty() {
        println!("{root_name}/");
    }

    let total = items.len() + subdirs.len();
    let mut idx = 0;

    // Print subdirectories first
    for subdir in &subdirs {
        idx += 1;
        let is_last = idx == total;
        let connector = if is_last { "└── " } else { "├── " };
        let dir_name = if parent.is_empty() {
            subdir.as_str()
        } else {
            &subdir[parent.len() + 1..]
        };
        println!("{prefix}{connector}{dir_name}/");

        let new_prefix = if is_last {
            format!("{prefix}    ")
        } else {
            format!("{prefix}│   ")
        };
        print_tree(tree, all_dirs, subdir, &new_prefix, root_name);
    }

    // Print items
    for mem in items {
        idx += 1;
        let is_last = idx == total;
        let connector = if is_last { "└── " } else { "├── " };
        let name = mem
            .path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        println!("{prefix}{connector}{name} - {}", mem.title);
    }
}

fn cmd_stale(days: u32, json: bool, dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let multi = storages.len() > 1;

    let now = chrono::Utc::now();
    let threshold = chrono::Duration::days(i64::from(days));

    let mut stale: Vec<(String, Mem)> = Vec::new();
    for (label, storage) in &storages {
        let mems = storage.list_mems()?;
        for mem in mems {
            if now - mem.updated_at > threshold {
                stale.push((label.clone(), mem));
            }
        }
    }

    if json {
        let json_output: Vec<MemJson> = stale.iter().map(|(_, m)| MemJson::from(m)).collect();
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else if stale.is_empty() {
        println!("No stale mems (threshold: {days} days)");
    } else {
        println!("Stale mems (not updated in {days}+ days):");
        for (label, mem) in &stale {
            let path_str = mem.path.to_string_lossy();
            let days_old = (now - mem.updated_at).num_days();
            let prefix = if multi {
                format!("[{label}] ")
            } else {
                String::new()
            };
            println!("  {prefix}{path_str}: {} ({days_old} days)", mem.title);
        }
    }

    Ok(())
}

fn cmd_lint(dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let multi = storages.len() > 1;

    let mut issues = Vec::new();
    let mut total_mems = 0;

    for (label, storage) in &storages {
        let mems = storage.list_mems()?;
        total_mems += mems.len();

        for mem in &mems {
            let path_str = mem.path.to_string_lossy();
            let prefix = if multi {
                format!("[{label}] ")
            } else {
                String::new()
            };

            // Check for empty title
            if mem.title.trim().is_empty() {
                issues.push(format!("{prefix}{path_str}: empty title"));
            }

            // Check for empty content
            if mem.content.trim().is_empty() {
                issues.push(format!("{prefix}{path_str}: empty content"));
            }

            // Check for broken internal links
            for line in mem.content.lines() {
                // Simple regex-free link extraction: find [text](path.md) patterns
                let mut chars = line.char_indices().peekable();
                while let Some((i, c)) = chars.next() {
                    if c == '[' {
                        // Find closing ]
                        let mut depth = 1;
                        let mut j = i + 1;
                        for (idx, ch) in chars.by_ref() {
                            j = idx;
                            if ch == '[' {
                                depth += 1;
                            } else if ch == ']' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                        }
                        // Check for (
                        if let Some(&(_, '(')) = chars.peek() {
                            chars.next();
                            let start = j + 2;
                            let mut end = start;
                            for (idx, ch) in chars.by_ref() {
                                if ch == ')' {
                                    end = idx;
                                    break;
                                }
                            }
                            let link = &line[start..end];
                            // Check if it's a relative .md link
                            if link.ends_with(".md") && !link.starts_with("http") {
                                // Resolve relative to mem's directory
                                let mem_dir = mem.path.parent().unwrap_or(std::path::Path::new(""));
                                let link_path = mem_dir.join(link.trim_end_matches(".md"));
                                let link_str = link_path.to_string_lossy().to_string();
                                if !storage.exists(&link_str) {
                                    issues
                                        .push(format!("{prefix}{path_str}: broken link to {link}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if issues.is_empty() {
        println!("No issues found ({total_mems} mems checked)");
        Ok(())
    } else {
        println!("Found {} issues:", issues.len());
        for issue in &issues {
            println!("  {issue}");
        }
        Err(anyhow!("lint failed with {} issues", issues.len()))
    }
}

fn cmd_dump(path: Option<&str>, dirs: &[PathBuf]) -> Result<()> {
    let storages = get_storages(dirs)?;
    let mut first = true;

    for (label, storage) in &storages {
        let mems = match path {
            Some(p) => storage.list_mems_under(p)?,
            None => storage.list_mems()?,
        };

        if mems.is_empty() {
            continue;
        }

        // Multi-dir header
        if storages.len() > 1 && !first {
            println!();
        }
        if storages.len() > 1 {
            println!("<!-- ═══ {label} ═══ -->");
            println!();
        }
        first = false;

        for mem in &mems {
            let path_str = mem.path.to_string_lossy();

            // Section divider with path
            println!(
                "<!-- ═══════════════════════════════════════════════════════════════════ -->"
            );
            println!("<!-- {path_str} -->");
            println!(
                "<!-- ═══════════════════════════════════════════════════════════════════ -->"
            );
            println!();

            // Title as H1
            println!("# {}", mem.title);
            println!();

            // Tags if present
            if !mem.tags.is_empty() {
                println!("Tags: {}", mem.tags.join(", "));
                println!();
            }

            // Content
            println!("{}", mem.content);
            println!();
        }
    }

    Ok(())
}
