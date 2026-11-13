# Skills

One directory per skill: `.agents/skills/<name>/SKILL.md`. The format is generic
so any coding agent can use it.

```markdown
---
name: example
description: One sentence on when to use this skill.
---

# Title

Instructions for the agent.
```

Rules:

- Name capabilities, not products.
- `.claude/skills` is a symlink to this directory. Do not put files there.
