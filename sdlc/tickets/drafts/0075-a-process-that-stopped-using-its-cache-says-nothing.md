---
---
# A process that stopped using its cache says nothing

Ticket 0068 makes a process stop answering from a cache file another setup discarded and replaced underneath it, and stop writing to it for the rest of its life. Stopping is the right answer and it is silent.

Ticket 0067 gave an operator two sentences at open, so a cold cache can be told from a destroyed one. Ticket 0074 asks for the third, when a run destroys a file mid-run. This is a fourth event and it is not a destruction: nothing is deleted, and the file the second process left in place is untouched. A long-running service simply stops finding rows and stops storing them, for the rest of its life, and nothing on any stream says so. The cache counters that would show it are private to the crate.

An operator sees a service that was warm go cold and stay cold, with no upgrade, no restart and no error. The lever is a restart, and nothing tells them that is the lever.

Done, observably:

- A process that stops using a cache file because another setup replaced it, or because the path no longer names a file it can open, says so once, naming the file, on the stream the other cache reports use.
- It says it once for the process, not once per lookup and not once per connection.
- A process that takes up a replacement recording its own setup goes on working and says nothing, because nothing was lost.
- A process whose file is never replaced still says nothing.

Boundary: this changes no answer, no row key, no recorded setup, when a file is discarded, or what a discard report says. It does not widen or narrow which replacements a process takes up; 0068 settles that.
