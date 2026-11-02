# TypeScript rules

Copy to `.jotline/rules/typescript.md` in a TypeScript project.

- Verify with `npx tsc --noEmit && npm test -- --run`.
- `strict` mode is on; never use `any`. Use `unknown` and narrow it.
- Prefer named exports. One React component per file, named like the file.
- Use the existing test framework and file naming (`*.test.ts`).
