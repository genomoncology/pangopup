---
---
# The score-evidence rule is held against one page and not the one read first

`spec/repository-motivation.md` requires that the material behind the README's
motivation link "does not say what a score is evidence for", and
`tests/repository-sourcing.sh` reads that requirement out of
`architecture/motivation.md` and refuses it there. Measured on this tree, that
page holds: it states every third-party claim as a dated quotation, denies
being legal advice, and draws no inference from a score.

`README.md:7` is outside that gate and states one:

    The scores help identify variants that may alter RNA splicing.

That sentence says what a score is evidence for. It is the seventh line of the
document a stranger reads first, and nothing counts it. The two literal bans
that `spec/readme-first-use.md` does hold over the README are
`pathogenicity classification` and `clinical diagnosis`; this sentence carries
neither and passes.

The asymmetry is the finding, not the wording. The repository built a rule
about evidentiary claims, gated it, and pointed the gate at the page that was
already careful while leaving the page with the widest readership uncovered.
Either the rule covers both pages or it covers the one that is read.

Nothing in the product is wrong: `pangopup lookup` and the served item report a
gain, a loss, their offsets and their provenance, and claim nothing about what
they mean. Only the README says more than the score does.
