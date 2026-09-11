# Why this project exists

The best-known splice predictor carries licence terms of its own, and the
public service that answers for it answers a few queries at a time. This page
records what each party says about its own software or service, with the date
that source was read. It draws no conclusion of its own. None of this page is
legal advice.

## What the upstream predictor's licence says

The better-known predictor is SpliceAI, from Illumina, Inc.

- The LICENSE file in the SpliceAI repository states: "SpliceAI source code is
  provided under the PolyForm Strict License 1.0.0. SpliceAI models are
  provided under CC BY NC 4.0 license for academic and non-commercial use."
  Read at https://github.com/Illumina/SpliceAI on 2026-08-28.

- That LICENSE file states that the PolyForm Strict License 1.0.0 clause covers
  "any permitted purpose, other than distributing the software or making
  changes or new works based on the software". Read at
  https://github.com/Illumina/SpliceAI on 2026-08-28.

- The SpliceAI README states: "The trained models used by SpliceAI (located in
  this package at spliceai/models) are provided under the CC BY NC 4.0 license
  for academic and non-commercial use; other use requires a commercial license
  from Illumina, Inc." Read at https://github.com/Illumina/SpliceAI on
  2026-08-28.

- GitHub reports the repository's licence as "Other/NOASSERTION", and records
  the repository as archived by its owner on 2026-04-20 and read-only. Read at
  https://github.com/Illumina/SpliceAI on 2026-08-28.

## What the public lookup service says

- The public SpliceAI lookup service states: "This service supports no more
  than a handful of queries per-user per-minute." It tells a user who needs to
  batch-process many variants to set up their own instance of the API server,
  or to run the models directly. Read at
  https://spliceailookup.broadinstitute.org on 2026-08-28.

- The same page states that it no longer uses Illumina's precomputed SpliceAI
  score tables, because those tables are based on an old version of GENCODE. It
  names the TGG at the Broad Institute as its maintainer. Read at
  https://spliceailookup.broadinstitute.org on 2026-08-28.

## What this repository runs instead

This repository runs Pangolin, and publishes a precomputed index of Pangolin
scores for GRCh38 single-nucleotide variants. A query reads that index from
local disk, so it sends nothing over a network and meets no per-minute limit. A
supported variant the index does not cover runs through the Pangolin model on
the CPU, and the result is kept in a local SQLite cache.

- This repository's NOTICE file records the Pangolin project as licensed under
  the GNU General Public License version 3.

- Pangolin was published as "Predicting RNA splicing from DNA sequence using
  Pangolin", Zeng T and Li YI, Genome Biology 2022, PMID 35449021,
  DOI 10.1186/s13059-022-02664-4. That record was read on 2026-08-28.

- SpliceAI was published as "Predicting Splicing from Primary Sequence with
  Deep Learning", Jaganathan K and co-authors, Cell 2019, PMID 30661751. That
  record was read on 2026-08-28.

This repository is GPL-3.0-only. Its LICENSE file is the statement of that, and
its NOTICE file names the upstream software and data it builds on.

## What calling this software means for the caller

Software reaches this project in three ways. It runs the executable and reads
the JSON Lines written to standard output. It calls the HTTP service over a
socket. Or it links the crates in this repository into its own binary.

The licence text answers what each of those arrangements means for a calling
program's own licence, and this repository does not answer it. The LICENSE file
in this repository holds the GPL-3.0-only text, and a reader goes there. This
repository states no conclusion about any other program's licence. None of this
page is legal advice.
