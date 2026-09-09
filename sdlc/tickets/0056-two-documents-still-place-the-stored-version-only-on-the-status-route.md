---
flow: build
priority: 2
---
# Two documents still place the stored version only on the status route

Ticket 0052 put `data_set_version` on every HTTP score item. Two published documents still describe it as a status-route field, so a reader of either one concludes a second request is required.

`architecture/service.md:37` opens "The status route also publishes one `pangopup.scoring-data-set-version.v1` value as `data_set_version`." Line 35 of the same section says of the other value "The status route and every returned score item expose the same full SHA-256 value." The section now tells a reader that one identity rides on the item and the other does not.

`README.md:133` says "Status and score items share the `scoring_identity`. Store `data_set_version` as the data-set version when a system has one version field." It names the value to store and does not say the item carries it.

Both sentences are true. Neither is false. Each is incomplete after 0052, and the incompleteness points a reader at the second request that `architecture/compatibility.md:76` now says is unnecessary.

Nothing fails today. `spec/http-service.md:201` pins one sentence out of the `## Active scoring identity` section and `spec/http-service.md:174-175` pin two README sentences. No gate reads the placement claim in either document, so a stale placement cannot turn a gate red.

Ticket 0052's design named `architecture/compatibility.md` and `spec/http-service.md` as the documents it changes and listed `README.md` under what it does not change. Nothing here is a defect in what 0052 shipped.

Both documents are corrected, not just the contract document. A reader who reaches `architecture/service.md` or `README.md` and stops there is the reader this ticket exists for; sending them to a third document to learn the placement is the same second hop the ticket is trying to remove.

Done, observably:

- `architecture/service.md` tells a reader that every returned score item carries `data_set_version`, in the same section and with the same force as the sentence that already says it about `scoring_identity`.
- `README.md` tells a consumer storing the data-set version that the score item already carries it, so no status request is needed to retain it beside a score.
- Neither correction can go stale unnoticed: a spec gate reads the placement claim out of each document, so deleting or reversing it turns `make spec` red.
- No document acquires a second, differently worded contract. `architecture/compatibility.md` stays the one place the full response shape is enumerated, and the two corrected sentences point there rather than restating it.
- `make spec` and `make lint` pass.

Boundary: this ticket changes documentation and the spec gates that pin it. It does not change any product source under `crates/`, any response shape, any field name, or any score value. It does not revisit what 0052 shipped, does not move `data_set_version` onto or off any route, and does not change the sentences those two spec gates already pin.
