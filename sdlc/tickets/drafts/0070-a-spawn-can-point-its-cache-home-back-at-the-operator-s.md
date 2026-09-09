---
priority: 5
---
# A spawn can point its cache home back at the operator's

Ticket 0069 routes every spawn of the shipped executable through one helper and makes that helper give each command a private cache home. The helper hands the caller the `Command` it built, so a caller can call `.env("XDG_CACHE_HOME", ...)` after the helper set it and replace the private path with any other.

Every caller that does this today points at a directory the test owns, which is why 0069 left it alone. Nothing holds them there. A caller that reads `std::env::var("HOME")` and passes it back would undo the redirect, and both of 0069's assertions would stay green: the static gate reads only which file names the executable, and the runtime assertion reads only what the helper itself set.

Done, observably:

- A spawn cannot resolve the model cache of whoever runs the suite, whatever the caller does to the command after the helper builds it.
- A test that needs a cache directory of its own still gets one, named rather than assembled from an environment variable.
- The refusal names the spawn.

Boundary: no product behavior, cache location, default, or scoring assertion changes. The obvious shape is a typed cache-home argument on the helper with the `Command`'s environment no longer reachable, which 0069's design measured and judged more machinery than the need then justified.
