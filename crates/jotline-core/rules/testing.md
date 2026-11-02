# Testing

- A change is not done until it is verified. Run the narrowest relevant test first, then the full suite.
- A bug fix needs a regression test that fails without the fix and passes with it.
- Test behavior, not implementation details: assert on outputs, errors, and side effects callers can observe.
- Cover the normal case, boundaries (empty, one, many, maximum), and error paths.
- Keep tests deterministic: no real network, no wall-clock sleeps, no dependence on test order. Use temporary directories for files.
- Never weaken, skip, or delete a failing test to make a change pass. Fix the code, or report the failure in the handoff.
- Report exactly which commands ran and their results; do not claim a check you did not execute.
