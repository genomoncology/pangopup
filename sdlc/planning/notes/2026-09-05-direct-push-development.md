# Direct pushes during maintainer-only development

Decision: PangoPup uses direct pushes to `main` during the current maintainer-only development phase. Pull requests are not part of this workflow.

The alternatives were pull requests with a remote required check, direct pushes with the repository's local gate ladder, or continued direct pushes without stating the policy. Pull requests add a server-enforced merge boundary but add ceremony for a repository with one maintainer-led development stream. An unstated bypass leaves the configured rule and the actual workflow in conflict. Direct pushes match the current workflow and keep delivery simple.

The accepted cost is that the hosting service does not independently enforce review or tests before each push. The working process supplies independent ticket and code review when the subagent SDLC is used, and the committer runs the repository-required gates before landing behavior changes. Revisit this decision before granting write access to outside contributors or running concurrent development streams that can overwrite each other's assumptions.
