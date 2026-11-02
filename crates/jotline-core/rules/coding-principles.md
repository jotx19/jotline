# Coding principles

- Read before you write. Inspect the files you will change and the code that calls them; follow the conventions already in the project (naming, layout, error handling, formatting) over your own preferences.
- Make the smallest change that fully solves the task. Do not refactor, rename, or reformat code the task did not ask about.
- Keep functions small and single-purpose. Prefer clear names over comments; comment the *why*, never restate the *what*.
- Handle errors explicitly. Never swallow an error silently; propagate it with context or report it. Validate input at boundaries.
- Prefer simple, boring solutions. Avoid premature abstraction, global mutable state, and clever tricks. Duplicate twice before abstracting.
- Keep types precise: use enums and structs for known shapes instead of loose strings, maps, or booleans.
- Do not add a dependency when the standard library or an existing dependency does the job.
- Leave the code buildable after every step. If you break something, fix it before moving on.
