# Subagents

One Markdown file per subagent: `.agents/agents/<name>.md`. The format is generic
so any coding agent can use it.

```markdown
---
name: example
description: One sentence on when to use this subagent.
# Optional, vendor-specific, ignored by other agents:
# model: ...
# tools: ...
---

# Role
What this subagent is and what it is not.

# Inputs
What the caller must provide.

# Outputs
What it returns, and in what shape.

# Allowed actions
What it may read, run and write. Say what it must never do.
```

Rules:

- Name capabilities, not products ("a browser automation tool", "the project's
  test command").
- Anything the subagent runs is a `mise` task or a `recon` subcommand.
- `.claude/agents` is a symlink to this directory. Do not put files there.
