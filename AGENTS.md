when asked to review big files: review the files with high line counts. Don't refactor just for the sake of refactoring but look for real areas where we could move things around and get big line counts down, make new files, and organize better

## Canonical cross-repository documentation

If you change a public or cross-repository contract, update `cubacadabra/docs`
in the same piece of work. Do not create a competing repo-local `docs/`
specification.
