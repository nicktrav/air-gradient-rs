# Walkthrough logbook

A chaptered, learner-oriented tour of this firmware as it grows. Each chapter is a
**snapshot**, not living documentation: it explains the repo as it stood at a specific
commit and is left alone afterwards. Read in order, the chapters double as a logbook of
how the project evolved, wedge by wedge.

Every chapter pins a **high-watermark SHA** in its frontmatter. Check that SHA out and
the tree matches the prose:

```sh
git checkout <high_watermark>
```

Frontmatter fields:

- `high_watermark` — the commit the chapter describes up to. The prose is accurate at
  this SHA.
- `covers` — the commit range the chapter narrates, from the previous chapter's
  watermark to this one (`<root>` for the first).
- `encoded` — the date the chapter was written.

See the `## Walkthrough logbook` section in [`CLAUDE.md`](../../CLAUDE.md) for when and
how new chapters are cut.

## Chapters

| Chapter | Title | High watermark | Covers |
|---|---|---|---|
| [01](01-heartbeat.md) | The heartbeat wedge | `0b89a5e` | `<root>..0b89a5e` |
