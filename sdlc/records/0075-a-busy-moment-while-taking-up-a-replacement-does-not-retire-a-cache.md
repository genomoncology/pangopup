---
base: bcd60db5feb4afa2fec51d4b73692ba2823271fb
head: 2e57d32c0a14dc1fd4b3c1d44c4ee5fe5e88fef6
---
# A busy moment while taking up a replacement does not retire a cache

Contention is no longer a verdict about a file. The probe ticket 0068 put ahead
of every `get` and `put` re-opens the path when the file there is not the one it
judged, and the take-up that follows trims the replacement to the running entry
limit. A peer holding that file's write lock makes the trim fail as `Busy`.
Nothing was destroyed and nothing is wrong with either file, so the cache now
declines the operation in hand -- a get counts a miss, a put stores nothing --
and probes again next time. A moment's contention no longer costs a long-running
service its cache for the rest of the process's life.

Every other cause is unchanged. Another setup, an earlier layout, a file this
build cannot read, and a path naming no file still retire the cache permanently
and destroy nothing, and a file recording the running setup arriving afterwards
is not taken up. The exemption is narrower than the branch it sits in.

Evictions performed by the open that takes up a replacement, or recreates a
default, now cross into the cache's counters with the connection instead of
dying with the short-lived open that ordered the trim.

Exercised beyond the suite under a private `HOME`, `XDG_CACHE_HOME` and
`TMPDIR`, against real `pangopup serve` processes. A service holding a marked
row had its cache file replaced by a second service running the same setup; with
that replacement's write lock held, the service answered every request, served
the recomputed score rather than the marked row from the file that had left the
path, wrote nothing, left the replacement byte-identical by md5 and by size and
mtime, and printed nothing. Once the lock went, the same service took the
replacement up, trimmed it to its own limit and wrote into it, still silently. A
second run replaced the file under a peer that filled it hard from ten parallel
requests a round for twelve rounds: the service answered all 160 of its own
requests and printed nothing. A disposable default cache with no `--model-cache`
behaved the same way on both halves. A service whose file was replaced by a peer
running a different mask said `stopped using model cache <path>: it no longer
holds the file it opened, so restart to use what is there now` exactly once
across six following requests, kept answering, and wrote nothing into the file
that later returned recording its own setup.

One cost is accepted rather than fixed. While a peer holds the lock, each
operation pays one extra `open_inner`, because the cache's judged identity still
names the departed file until the take-up succeeds.

`make lint`, `make test`, `make spec`, `scripts/run-service-fixture-tests.sh`
and `sdlc/scripts/lint` all pass on this candidate. Nothing else moved: no
answer, no score, no row key, no recorded setup, no discard, no report, and no
change to the one-stat-per-operation cost.
