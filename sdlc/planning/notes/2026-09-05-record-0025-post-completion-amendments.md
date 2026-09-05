# Record 0025 post-completion amendments

Ticket 0025 completed at commit `7c589053702b60a6324f8db243a86eca51bddcf1`. Commit `4ce511f4a61bc3de18dd80f4b770f229eea33f83` later changed its completion record to describe a provenance-test repair. Commit `40b57df080bd591562563a6729fb03f81e37a329` changed the same record again to describe a fixture-generator repair. Both changes violated the append-only rule for `sdlc/records/`.

Ticket 0027 did not change record 0025. Its completion record says the archived 0025 ticket and record remained unchanged. That statement is true only across ticket 0027's base and implementation commits. It does not erase the two earlier post-completion amendments.

This note preserves the correction without changing record 0025 or record 0027 again. The accepted cost is that the historical record blobs remain in Git and a reader must follow this note for the amendment history. A canonical mechanical check should reject changes to a record after its introducing commit. The SDLC owner must design that check across all participating repositories rather than PangoPup adding a local process rule.
