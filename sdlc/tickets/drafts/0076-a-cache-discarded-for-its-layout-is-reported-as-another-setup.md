---
---
# A cache discarded for its layout is reported as another setup

Ticket 0067 split the report for a destroyed cache in two, because an operator told that another setup filled a corrupt file goes looking for an upgrade nobody made. The same confusion is still shipped for the third cause.

A cache file this build reads as an earlier layout is discarded whole and reported as `discarded model cache <path>: another setup filled it`. Measured on 2026-09-10 in this checkout: a `lookup` run over a cache file carrying an earlier `user_version` printed exactly that line, then restamped the file to the current layout. Nothing about the assets changed. What changed was the release.

`ModelResultCache::replace` raises one flag for both causes, so `discarded_earlier_setup` is true whether the file recorded another setup or an earlier layout, and both the service and the command line print the setup sentence. The operator is sent to look for an asset or model change that never happened, and away from the release note that would explain it.

Done, observably:

- A cache discarded because this build reads its layout as an earlier one says so, naming the layout as the cause, and does not say another setup filled it.
- A cache discarded because another setup filled it still says exactly what it says today.
- A cache destroyed because this build could not read it still says exactly what it says today.
- The service and the command line say the same thing for the same cause, once per open, as they do now.

Boundary: this changes no answer, no row key, no recorded setup, and not when a file is discarded. It adds a third sentence to the report; it does not remove or reword the two ticket 0067 settled.
