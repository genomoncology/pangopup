---
---
# Two documents still place the stored version only on the status route

Ticket 0052 put `data_set_version` on every HTTP score item. Two published documents still describe it as a status-route field, so a reader of either one concludes a second request is required.

`architecture/service.md:37` opens "The status route also publishes one `pangopup.scoring-data-set-version.v1` value as `data_set_version`." Line 35 of the same section says of the other value "The status route and every returned score item expose the same full SHA-256 value." The section now tells a reader that one identity rides on the item and the other does not.

`README.md:133` says "Status and score items share the `scoring_identity`. Store `data_set_version` as the data-set version when a system has one version field." It names the value to store and does not say the item carries it.

Both sentences are true. Neither is false. Each is incomplete after 0052, and the incompleteness points a reader at the second request that `architecture/compatibility.md:76` now says is unnecessary.

Nothing fails today. `spec/http-service.md:201` pins one sentence out of the `## Active scoring identity` section and `spec/http-service.md:174-175` pin two README sentences. No gate reads the placement claim in either document, so a stale placement cannot turn a gate red.

Ticket 0052's design named `architecture/compatibility.md` and `spec/http-service.md` as the documents it changes and listed `README.md` under what it does not change. Nothing here is a defect in what 0052 shipped.

A successor decides whether the placement belongs in all three documents or only in `architecture/compatibility.md`, which already carries the full contract, and whether whatever it writes gets pinned by the same spec gate that pins the rest.
