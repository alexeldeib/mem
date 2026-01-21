# mem

A fast, minimal CLI for markdown-based knowledge tracking. Store project knowledge as plain markdown files with YAML frontmatter in `.mems/`. Zero dependencies beyond Rust, git-friendly, designed for AI agent workflows.

## Installation

```bash
cargo install --path .
```

## Quick Start

```bash
# Initialize in your project
mem init

# Add knowledge
mem add arch/decisions/adr-001 -c "Use PostgreSQL for persistence" -t "ADR-001 Database" --tags arch,database

# View it
mem show arch/decisions/adr-001

# List all mems
mem ls

# Search
mem find "database"

# Tree view
mem tree

# Dump for LLM context
mem dump arch
```

## Commands

| Command | Purpose |
|---------|---------|
| `mem init` | Initialize `.mems/` directory |
| `mem add <path>` | Create new mem |
| `mem show <path>` | Display mem content |
| `mem edit <path>` | Update a mem |
| `mem ls [path]` | List mems |
| `mem find <query>` | Search by content |
| `mem tree [path]` | Show hierarchy |
| `mem dump [path]` | Concatenate as markdown |
| `mem rm <path>` | Delete a mem |
| `mem archive <path>` | Soft delete |
| `mem lint` | Validate mems |
| `mem stale` | Find outdated mems |

## Storage Format

Mems are stored as markdown with YAML frontmatter:

```
.mems/
  arch/
    decisions/
      adr-001.md
  guides/
    setup.md
  archive/
```

Each file:

```markdown
---
title: ADR-001 Database Choice
created-at: 2025-01-20T12:00:00Z
updated-at: 2025-01-20T14:30:00Z
tags:
  - architecture
  - database
---

Use PostgreSQL for persistence.
```

## Multi-Directory Support

Query across multiple `.mems/` directories:

```bash
mem ls --dir ./project-a/.mems --dir ./project-b/.mems
mem find "api" --dir ./frontend/.mems --dir ./backend/.mems
```

## LLM Context Export

Dump mems as concatenated markdown for LLM context windows:

```bash
mem dump arch > context.md
```

Output uses HTML comment dividers with paths:

```markdown
<!-- ═══════════════════════════════════════════════════════════════════ -->
<!-- arch/decisions/adr-001 -->
<!-- ═══════════════════════════════════════════════════════════════════ -->

# ADR-001 Database

Tags: arch, database

Use PostgreSQL for persistence.
```

## License

MIT
