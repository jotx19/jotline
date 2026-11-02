# Examples

Starting points for configuring jotline in your own projects. jotline does not load anything from this directory; copy what you need.

| File | Copy to | Purpose |
|---|---|---|
| [JOTLINE.md](JOTLINE.md) | `<project>/JOTLINE.md` | Main project rules: language, verification command, layout, conventions. |
| [rules/rust.md](rules/rust.md) | `<project>/.jotline/rules/rust.md` | Extra rules for Rust projects. |
| [rules/typescript.md](rules/typescript.md) | `<project>/.jotline/rules/typescript.md` | Extra rules for TypeScript projects. |
| [plugins/word_count.json](plugins/word_count.json) | `<project>/.jotline/plugins/word_count.json` | A plugin tool, `plugin.word_count`, that runs a command and returns its output. |

After copying, run `/rules reload` or `/plugins` in a running session, or restart jotline. See [rules](../docs/RULES.md) and [tools](../docs/TOOLS.md#plugins).
