+++
format_version = 1
id = "01989af2-4305-7b19-88b1-e8ae4ea9a02c"
title = "Use one write boundary"
kind = "decision"
scope = "project:stormbuffer"
status = "active"
access = "agent"
created_at = "2026-08-05T20:10:00-05:00"
updated_at = "2026-08-05T20:12:00-05:00"
tags = ["architecture", "storage"]
aliases = ["core owns writes", "唯一の書き込み境界"]
supersedes = []

[[sources]]
kind = "conversation"
reference = "stormbuffer://session/2026-08-05"
actor = "user"
observed_at = "2026-08-05T20:08:00-05:00"
revision = "session-revision-7"
content_hash = "blake3:4d8f1c"

[[sources]]
kind = "document"
reference = "AGENTS.md#architecture-boundaries"
actor = "user"
revision = "git:9f2c11a"
+++

The core owns validation and mutation so adapters cannot diverge.

```rust
fn commit(record: &Record) -> Result<()> {
    record.validate()?;
    Ok(())
}
```
