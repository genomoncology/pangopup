# Ticket 003 full index build and certification

Date: 2026-07-21

This is the final remediation recertification retained for Ticket 003 after the
independent code review required changes, including the follow-up restoration
of Ticket 002 terminal-coverage checks. It supersedes every earlier Ticket 003
builder and bundle identity; those values do not describe the current source.
The generated installed bundle and transport archive were deleted after the
facts below were recorded. Input paths were supplied explicitly and are
intentionally not recorded.

## Build identity and environment

| Field | Value |
|---|---|
| Base Git commit | `e2f9d3ee5ecca5a73f927c5047bb1501585664ef` |
| Builder version | `0.1.0` |
| Builder source SHA-256 | `sha256:14b086f124c5fae4a720db7d35b0c120a50372f81bd98265f389e95b13adcf24` |
| Compiler | `rustc 1.93.1 (01f6ddf75 2026-02-11)`, LLVM 21.1.8, `x86_64-unknown-linux-gnu` |
| OS | Ubuntu Linux, kernel `6.17.0-35-generic`, x86-64 |
| Host | AMD Ryzen 7 5825U, 8 cores / 16 threads; 29,340,872,704 bytes RAM; Crucial CT1000P3PSSD8 NVMe |
| GNU tar | 1.35 |
| Zstandard CLI | 1.5.5 |

`source_sha256` was embedded at compile time. It hashes sorted
workspace-relative paths for the root `Cargo.toml`/`Cargo.lock` and every
`Cargo.toml`/`.rs` file in `pangopup-core`, `pangopup-index`, and
`pangopup-build`, using the length-framed algorithm in the ticket.

## Commands and measurement method

The release binary was built from the implementation under test:

```bash
cargo build --locked --release -p pangopup-build
```

The explicit build command was run under GNU `time -v` (environment variables
were operator-supplied paths and were expanded into CLI arguments):

```bash
/usr/bin/time -v -o build.time \
  target/release/pangopup-build build \
  --source "$PANGOPUP_SOURCE_DIR" \
  --reference "$PANGOPUP_GRCH38_FASTA" \
  --output "$RUN/bundle" >build.json 2>build.err
```

Peak RSS is GNU `time -v`'s maximum resident set size. A five-second sampler
repeated GNU `du -sb` over the live unique sibling staging directory, retained
the largest apparent-byte result, waited for the builder, and persisted the
peak regardless of the builder's exit code. The resulting persisted maximum
observation was 32,298,977,408 apparent bytes. Because five-second sampling did
not land in the final sub-five-second copy window, that observation is a lower
bound rather than a claim of exact instantaneous peak. The writer lifecycle
guarantees that the synced 15,033,158,255-byte final index coexists before
return with the 15,030,603,775-byte payload scratch and 3,088,286,401-byte
normalized-reference scratch. Those exact member lengths total
33,152,048,431 apparent bytes (plus directory metadata). Both the persisted
observation and the deterministic simultaneous-file accounting are retained;
neither is invented from an absent sampler result.

Heap boundedness is proved separately by the gate test:

```bash
cargo test -p pangopup-build --test heap_bound -- --nocapture
```

That single-test process uses a tracking global allocator and Linux
`/proc/self/statm` while pushing 3,000 genes / 3,000,000 loci into a
33,000,000-byte disk spool. It measured 163,840 retained allocator bytes, a
269,472-byte allocator peak delta, and a 339,968-byte RSS delta. Those enforced
ratios detect retained logical-locus or artifact-sized state; the full-run RSS
below is a distinct file-backed mmap measurement.

The independently invoked verifier used:

```bash
/usr/bin/time -v -o verify.time \
  target/release/pangopup-build verify "$RUN/bundle" \
  >verify.json 2>verify.err
```

## Input identities

### Pangolin source

| Field | Value |
|---|---|
| Title | Pangolin precomputed scores |
| Creators | Nils Wagner; Aleksandr Neverov |
| DOI | `10.5281/zenodo.15649338` |
| Published archive | `Pangolin_hg38_snvs_masked.zip` |
| Published archive size | 12,988,141,317 bytes |
| Published archive MD5 | `md5:679ef0b50e511b6102b4b88fbf811108` |
| Observed accepted members | 19,913 |
| Observed member-set SHA-256 | `sha256:0e40ee8e0527210cb64c26a6637117aea7d41d696e7bd95f3bb9545ee16782f6` |
| Parameters | masked, window 50 |

The observed digest is distinct from the published ZIP MD5 and Ticket 002's
134-gene benchmark-subset digest. It frames each sorted accepted member as
`u64_le(name_len) || name || u64_le(file_len) || file_bytes`.

### Certification reference

| Field | Value |
|---|---|
| Assembly | RefSeq GRCh38.p14, `GCF_000001405.40` |
| Input compression | gzip |
| Supplied file size | 972,898,531 bytes |
| Supplied file SHA-256 | `sha256:11912a45a545bf01a10b2a7f10eb7a42924436b4d19b476b1899834fb7ba74a3` |
| Canonical required sequence-set SHA-256 | `sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4` |
| Required primary records | 25 |
| Extra records ignored for certification | 680 |
| Sorted extra-accession SHA-256 | `sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb` |

The sequence-set digest frames the 25 normalized uppercase sequences in the
ticket's canonical accession order. All ordinary source references were
compared with those sequences. Source `REF=N` loci were preserved and counted
as exceptions, not treated as mismatches.

## Result

| Measurement | Value |
|---|---:|
| Build status | `built` (exit 0, empty stderr) |
| Elapsed wall time | 1:13:36 |
| User CPU | 4,303.59 s |
| System CPU | 83.77 s |
| Peak RSS | 14,707,112 KiB (15,060,082,688 bytes) |
| Persisted five-second maximum observation | 32,298,977,408 apparent bytes |
| Deterministic simultaneous member lengths | 33,152,048,431 apparent bytes; see sampling disclosure above |
| Installed bundle bytes | 15,033,163,553 |
| `NOTICE` bytes / SHA-256 | 1,709 / `sha256:9b8e898daa53b28cf421f9a59676e920dc5cefb1c23b9d185f75d3cfd4281af7` |
| `manifest.json` bytes / bundle identity | 3,589 / `sha256:bce0bb49ba8a3f303661967a7a86362da66013fd94c3ae32ed27a9685d3b5260` |
| `scores.pgi` bytes / SHA-256 | 15,033,158,255 / `sha256:6fd8eb490e643728f6682fe6fc1910b88641354aaa221781575763c4ca94bf27` |
| Independent verify status | `verified`, same bundle ID, 2 members verified, exit 0, empty stderr |
| Independent verify elapsed / peak RSS | 18:39.72 / 14,704,308 KiB (15,057,211,392 bytes) |

The build and standalone-verify RSS peaks are dominated by reclaimable,
file-backed mmap pages touched during complete offline decoding; they are not a
heap measurement. The allocator/RSS regression above is the reproducible proof
that source ingestion retains only one input gene plus compact directories and
writer state. Payload and normalized reference data remain disk-backed.

### Complete counts

| Count | Value |
|---|---:|
| Genes | 19,913 |
| Source rows | 4,099,255,665 |
| Gene loci | 1,366,418,555 |
| Ascending / descending members | 10,073 / 9,840 |
| Source / encoded index segments | 19,916 / 19,945 |
| Gap transitions / omitted bases | 3 / 50,002 |
| `REF=N` loci | 30 |
| Omit-A / omit-T exception shapes | 9 / 21 |

### Independent logical streams

| Stream | Records | SHA-256 |
|---|---:|---|
| Canonical source before encoding | 4,099,255,665 | `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31` |
| Complete decoded bundle | 4,099,255,665 | `sha256:dcec29e84c5f65bd76ffde2be8c7fa312d08e6abdb1e45e024dc0fe8c8da9c31` |

The build is accepted only if both logical rows match exactly, including count
and digest.

## Deterministic transport

An isolated transport working directory was populated with byte-exact copies of
the three installed members. From inside that directory, the bundle was
compressed with the ticket's exact command:

```bash
tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --mode=0644 --format=posix \
  --pax-option=delete=atime,delete=ctime -cf - manifest.json NOTICE scores.pgi \
  | zstd -9 --threads=1 --no-progress -o pangopup-hg38-snvs-masked-v1.tar.zst
```

| Field | Value |
|---|---:|
| Deterministic tar bytes before Zstandard | 15,033,169,920 |
| Transport bytes | 1,935,000,209 |
| Transport SHA-256 | `sha256:3e87d80fdad963ca6ffca646393b8bb3955214b77cd8b7f1782e48d039aba751` |

GitHub documents a per-file release-asset limit of under 2 GiB and up to 1,000
assets per release ([GitHub Docs](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)).
The measured archive is about 1.80 GiB, only about 212 MiB below 2 GiB. Release
packaging should therefore use deterministic split transport members (for
example, contig-group payload pieces bound by one manifest) to preserve growth
and tooling headroom. Installation should reassemble the same fixed-v1 member;
query semantics and runtime mmap layout do not change.

## Extra FASTA accessions

The following 680 sorted accessions were present in the supplied assembly FASTA
but ignored for primary-sequence certification. Their names are nevertheless
bound into the manifest by the framed digest above.

```text
NT_113793.3 NT_113796.3 NT_113888.1 NT_113889.1 NT_113891.3 NT_113901.1 NT_113930.2 NT_113948.1 NT_113949.2 NT_167208.1 NT_167209.1 NT_167211.2 NT_167213.1 NT_167214.1 NT_167215.1 NT_167218.1 NT_167219.1 NT_167220.1 NT_167244.2 NT_167245.2 NT_167246.2 NT_167247.2 NT_167248.2 NT_167249.2 NT_167250.2 NT_167251.2 NT_187361.1 NT_187362.1 NT_187363.1 NT_187364.1 NT_187365.1 NT_187366.1 NT_187367.1 NT_187368.1 NT_187369.1 NT_187370.1 NT_187371.1 NT_187372.1 NT_187373.1 NT_187374.1 NT_187375.1 NT_187377.1 NT_187378.1 NT_187379.1 NT_187380.1 NT_187381.1 NT_187382.1 NT_187383.1 NT_187384.1 NT_187385.1 NT_187386.1 NT_187387.1 NT_187388.1 NT_187390.1 NT_187391.1 NT_187392.1 NT_187393.1 NT_187394.1 NT_187395.1 NT_187396.1 NT_187397.1 NT_187398.1 NT_187399.1 NT_187400.1 NT_187401.1 NT_187402.1 NT_187403.1 NT_187404.1 NT_187405.1 NT_187406.1 NT_187407.1 NT_187408.1 NT_187409.1 NT_187410.1 NT_187411.1 NT_187412.1 NT_187413.1 NT_187414.1 NT_187415.1 NT_187416.1 NT_187417.1 NT_187418.1 NT_187419.1 NT_187420.1 NT_187421.1 NT_187422.1 NT_187423.1 NT_187424.1 NT_187425.1 NT_187426.1 NT_187427.1 NT_187428.1 NT_187429.1 NT_187430.1 NT_187431.1 NT_187432.1 NT_187433.1 NT_187434.1 NT_187435.1 NT_187436.1 NT_187437.1 NT_187438.1 NT_187439.1 NT_187440.1 NT_187441.1 NT_187442.1 NT_187443.1 NT_187444.1 NT_187445.1 NT_187446.1 NT_187447.1 NT_187448.1 NT_187449.1 NT_187450.1 NT_187451.1 NT_187452.1 NT_187453.1 NT_187454.1 NT_187455.1 NT_187456.1 NT_187457.1 NT_187458.1 NT_187459.1 NT_187460.1 NT_187461.1 NT_187462.1 NT_187463.1 NT_187464.1 NT_187465.1 NT_187466.1 NT_187467.1 NT_187468.1 NT_187469.1 NT_187470.1 NT_187471.1 NT_187472.1 NT_187473.1 NT_187474.1 NT_187475.1 NT_187476.1 NT_187477.1 NT_187478.1 NT_187479.1 NT_187480.1 NT_187481.1 NT_187482.1 NT_187483.1 NT_187484.1 NT_187485.1 NT_187486.1 NT_187487.1 NT_187488.1 NT_187489.1 NT_187490.1 NT_187491.1 NT_187492.1 NT_187493.1 NT_187494.1 NT_187495.1 NT_187496.1 NT_187497.1 NT_187498.1 NT_187499.1 NT_187500.1 NT_187501.1 NT_187502.1 NT_187503.1 NT_187504.1 NT_187505.1 NT_187506.1 NT_187508.1 NT_187509.1 NT_187510.1 NT_187511.1 NT_187512.1 NT_187513.1 NT_187514.1 NT_187515.1 NT_187516.1 NT_187517.1 NT_187518.1 NT_187519.1 NT_187520.1 NT_187521.1 NT_187522.1 NT_187523.1 NT_187524.1 NT_187525.1 NT_187526.1 NT_187527.1 NT_187528.1 NT_187529.1 NT_187530.1 NT_187531.1 NT_187532.1 NT_187533.1 NT_187534.1 NT_187535.1 NT_187536.1 NT_187537.1 NT_187538.1 NT_187539.1 NT_187540.1 NT_187541.1 NT_187542.1 NT_187543.1 NT_187544.1 NT_187545.1 NT_187546.1 NT_187547.1 NT_187548.1 NT_187549.1 NT_187550.1 NT_187551.1 NT_187552.1 NT_187553.1 NT_187554.1 NT_187555.1 NT_187556.1 NT_187557.1 NT_187558.1 NT_187559.1 NT_187560.1 NT_187561.1 NT_187562.1 NT_187563.1 NT_187564.1 NT_187565.1 NT_187566.1 NT_187567.1 NT_187568.1 NT_187569.1 NT_187570.1 NT_187571.1 NT_187572.1 NT_187573.1 NT_187574.1 NT_187575.1 NT_187576.1 NT_187577.1 NT_187578.1 NT_187579.1 NT_187581.1 NT_187582.1 NT_187583.1 NT_187584.1 NT_187585.1 NT_187586.1 NT_187587.1 NT_187588.1 NT_187589.1 NT_187590.1 NT_187591.1 NT_187592.1 NT_187593.1 NT_187594.1 NT_187595.1 NT_187596.1 NT_187597.1 NT_187598.1 NT_187599.1 NT_187600.1 NT_187601.1 NT_187602.1 NT_187603.1 NT_187604.1 NT_187605.1 NT_187606.1 NT_187607.1 NT_187608.1 NT_187609.1 NT_187610.1 NT_187611.1 NT_187612.1 NT_187613.1 NT_187614.1 NT_187615.1 NT_187616.1 NT_187617.1 NT_187618.1 NT_187619.1 NT_187620.1 NT_187621.1 NT_187622.1 NT_187623.1 NT_187624.1 NT_187625.1 NT_187626.1 NT_187627.1 NT_187628.1 NT_187629.1 NT_187630.1 NT_187631.1 NT_187632.1 NT_187633.1 NT_187634.1 NT_187635.1 NT_187636.1 NT_187637.1 NT_187638.1 NT_187639.1 NT_187640.1 NT_187641.1 NT_187642.1 NT_187643.1 NT_187644.1 NT_187645.1 NT_187646.1 NT_187647.1 NT_187648.1 NT_187649.1 NT_187650.1 NT_187651.1 NT_187652.1 NT_187653.1 NT_187654.1 NT_187655.1 NT_187656.1 NT_187657.1 NT_187658.1 NT_187659.1 NT_187660.1 NT_187661.1 NT_187662.1 NT_187663.1 NT_187664.1 NT_187665.1 NT_187666.1 NT_187667.1 NT_187668.1 NT_187669.1 NT_187670.1 NT_187671.1 NT_187672.1 NT_187673.1 NT_187674.1 NT_187675.1 NT_187676.1 NT_187677.1 NT_187678.1 NT_187679.1 NT_187680.1 NT_187681.1 NT_187682.1 NT_187683.1 NT_187684.1 NT_187685.1 NT_187686.1 NT_187687.1 NT_187688.1 NT_187689.1 NT_187690.1 NT_187691.1 NT_187692.1 NT_187693.1 NW_003315905.1 NW_003315906.1 NW_003315907.2 NW_003315908.1 NW_003315909.1 NW_003315913.1 NW_003315914.1 NW_003315915.1 NW_003315917.2 NW_003315918.1 NW_003315919.1 NW_003315920.1 NW_003315921.1 NW_003315922.2 NW_003315928.1 NW_003315929.1 NW_003315930.1 NW_003315931.1 NW_003315934.1 NW_003315935.1 NW_003315936.1 NW_003315938.1 NW_003315939.2 NW_003315940.1 NW_003315941.1 NW_003315942.2 NW_003315943.1 NW_003315944.2 NW_003315945.1 NW_003315946.1 NW_003315952.3 NW_003315953.2 NW_003315954.1 NW_003315955.1 NW_003315956.1 NW_003315957.1 NW_003315958.1 NW_003315959.1 NW_003315960.1 NW_003315961.1 NW_003315962.1 NW_003315963.1 NW_003315964.2 NW_003315965.1 NW_003315966.2 NW_003315967.2 NW_003315968.2 NW_003315969.2 NW_003315970.2 NW_003315971.2 NW_003315972.2 NW_003571033.2 NW_003571036.1 NW_003571049.1 NW_003571050.1 NW_003571054.1 NW_003571055.2 NW_003571056.2 NW_003571057.2 NW_003571058.2 NW_003571059.2 NW_003571060.1 NW_003571061.2 NW_003871060.2 NW_003871073.1 NW_003871074.1 NW_003871091.1 NW_003871092.1 NW_003871093.1 NW_004166862.2 NW_004504305.1 NW_009646194.1 NW_009646195.1 NW_009646196.1 NW_009646197.1 NW_009646198.1 NW_009646199.1 NW_009646200.1 NW_009646201.1 NW_009646202.1 NW_009646203.1 NW_009646204.1 NW_009646205.1 NW_009646206.1 NW_009646207.1 NW_009646208.1 NW_009646209.1 NW_011332687.1 NW_011332688.1 NW_011332689.1 NW_011332690.1 NW_011332691.1 NW_011332692.1 NW_011332693.1 NW_011332694.1 NW_011332695.1 NW_011332696.1 NW_011332697.1 NW_011332698.1 NW_011332699.1 NW_011332700.1 NW_011332701.1 NW_012132914.1 NW_012132915.1 NW_012132916.1 NW_012132917.1 NW_012132918.1 NW_012132919.1 NW_012132920.1 NW_012132921.1 NW_013171799.1 NW_013171800.1 NW_013171801.1 NW_013171802.1 NW_013171803.1 NW_013171804.1 NW_013171805.1 NW_013171806.1 NW_013171807.1 NW_013171808.1 NW_013171809.1 NW_013171810.1 NW_013171811.1 NW_013171812.1 NW_013171813.1 NW_013171814.1 NW_014040925.1 NW_014040926.1 NW_014040927.1 NW_014040928.1 NW_014040929.1 NW_014040930.1 NW_014040931.1 NW_015148966.2 NW_015148967.1 NW_015148968.1 NW_015148969.2 NW_015495298.1 NW_015495299.1 NW_015495300.1 NW_015495301.1 NW_016107297.1 NW_016107298.1 NW_016107299.1 NW_016107300.1 NW_016107301.1 NW_016107302.1 NW_016107303.1 NW_016107304.1 NW_016107305.1 NW_016107306.1 NW_016107307.1 NW_016107308.1 NW_016107309.1 NW_016107310.1 NW_016107311.1 NW_016107312.1 NW_016107313.1 NW_016107314.1 NW_017363813.1 NW_017363814.1 NW_017363815.1 NW_017363816.1 NW_017363817.1 NW_017363818.1 NW_017363819.1 NW_017363820.1 NW_017852928.1 NW_017852929.1 NW_017852930.1 NW_017852931.1 NW_017852932.1 NW_017852933.1 NW_018654706.1 NW_018654707.1 NW_018654708.1 NW_018654709.1 NW_018654710.1 NW_018654711.1 NW_018654712.1 NW_018654713.1 NW_018654714.1 NW_018654715.1 NW_018654716.1 NW_018654717.1 NW_018654718.1 NW_018654719.1 NW_018654720.1 NW_018654721.1 NW_018654722.1 NW_018654723.1 NW_018654724.1 NW_018654725.1 NW_018654726.1 NW_019805487.1 NW_019805488.1 NW_019805489.1 NW_019805490.1 NW_019805491.1 NW_019805492.1 NW_019805493.1 NW_019805494.1 NW_019805495.1 NW_019805496.1 NW_019805497.1 NW_019805498.1 NW_019805499.1 NW_019805500.1 NW_019805501.1 NW_019805502.1 NW_019805503.1 NW_021159987.1 NW_021159988.1 NW_021159989.1 NW_021159990.1 NW_021159991.1 NW_021159992.1 NW_021159993.1 NW_021159994.1 NW_021159995.1 NW_021159996.1 NW_021159997.1 NW_021159998.1 NW_021159999.1 NW_021160000.1 NW_021160001.1 NW_021160002.1 NW_021160003.1 NW_021160004.1 NW_021160005.1 NW_021160006.1 NW_021160007.1 NW_021160008.1 NW_021160009.1 NW_021160010.1 NW_021160011.1 NW_021160012.1 NW_021160013.1 NW_021160014.1 NW_021160015.1 NW_021160016.1 NW_021160017.1 NW_021160018.1 NW_021160019.1 NW_021160020.1 NW_021160021.1 NW_021160022.1 NW_021160023.1 NW_021160024.1 NW_021160025.1 NW_021160026.1 NW_021160027.1 NW_021160028.1 NW_021160029.1 NW_021160030.1 NW_021160031.1 NW_025791753.1 NW_025791754.1 NW_025791755.1 NW_025791756.1 NW_025791757.1 NW_025791758.1 NW_025791759.1 NW_025791760.1 NW_025791761.1 NW_025791762.1 NW_025791763.1 NW_025791764.1 NW_025791765.1 NW_025791766.1 NW_025791767.1 NW_025791768.1 NW_025791769.1 NW_025791770.1 NW_025791771.1 NW_025791772.1 NW_025791773.1 NW_025791774.1 NW_025791775.1 NW_025791776.1 NW_025791777.1 NW_025791778.1 NW_025791779.1 NW_025791780.1 NW_025791781.1 NW_025791782.1 NW_025791783.1 NW_025791784.1 NW_025791785.1 NW_025791786.1 NW_025791787.1 NW_025791788.1 NW_025791789.1 NW_025791790.1 NW_025791791.1 NW_025791792.1 NW_025791793.1 NW_025791794.1 NW_025791795.1 NW_025791796.1 NW_025791797.1 NW_025791798.1 NW_025791799.1 NW_025791800.1 NW_025791801.1 NW_025791802.1 NW_025791803.1 NW_025791804.1 NW_025791805.1 NW_025791806.1 NW_025791807.1 NW_025791808.1 NW_025791809.1 NW_025791810.1 NW_025791811.1 NW_025791812.1 NW_025791813.1 NW_025791814.1 NW_025791815.1 NW_025791816.1 NW_025791817.1 NW_025791818.1 NW_025791819.1 NW_025791820.1 NW_025791821.1
```
