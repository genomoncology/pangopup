use flate2::{Compression, GzBuilder, read::GzDecoder};
use pangopup_build::build_bundle;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    error::Error,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

const HEADER: &str = "chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos";
const DOI: &str = "10.5281/zenodo.15649338";
const ARCHIVE_MD5: &str = "679ef0b50e511b6102b4b88fbf811108";

#[derive(Clone, Debug)]
struct Row {
    gene: String,
    fields: [String; 8],
}

impl Row {
    fn contig(&self) -> &str {
        &self.fields[0]
    }
    fn position(&self) -> u32 {
        self.fields[1].parse().expect("validated position")
    }
    fn reference(&self) -> &str {
        &self.fields[2]
    }
    fn alternate(&self) -> &str {
        &self.fields[3]
    }
    fn line(&self) -> String {
        self.fields.join("\t")
    }
}

#[derive(Clone, Debug)]
struct Request {
    order: usize,
    group_order: usize,
    group: String,
    contig: String,
    position: u32,
    reference: String,
    alternate: String,
    gene: Option<String>,
}

impl Request {
    fn variant(&self) -> String {
        format!(
            "GRCh38:{}:{}:{}:{}",
            self.contig, self.position, self.reference, self.alternate
        )
    }
}

#[derive(Serialize)]
struct JsonResult<'a> {
    assembly: &'static str,
    contig: &'a str,
    position: u32,
    #[serde(rename = "ref")]
    reference: &'a str,
    #[serde(rename = "alt")]
    alternate: &'a str,
    status: &'static str,
    records: Vec<JsonRecord>,
    source_reference_ambiguities: Vec<JsonAmbiguity>,
    provenance: JsonProvenance<'a>,
}

#[derive(Serialize)]
struct JsonRecord {
    gene: String,
    gain_score: String,
    gain_position: i8,
    loss_score: String,
    loss_position: i8,
}

#[derive(Serialize)]
struct JsonAmbiguity {
    gene: String,
    source_ref: &'static str,
    published_alts: Vec<String>,
    omitted_alt: String,
}

#[derive(Serialize)]
struct JsonProvenance<'a> {
    kind: &'static str,
    bundle_id: &'a str,
    source_doi: &'static str,
    source_archive_md5: &'static str,
    masked: bool,
    window: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let source = PathBuf::from(arguments.next().ok_or("missing source fixture directory")?);
    let output = PathBuf::from(arguments.next().ok_or("missing absent output directory")?);
    if arguments.next().is_some() || output.exists() {
        return Err("usage: pangopup-regression-fixture <SOURCE_EXCERPTS> <ABSENT_OUTPUT>".into());
    }
    generate(&source, &output)
}

fn generate(source: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let mut members = Vec::new();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF-8 member")?;
        if let Some(gene) = name.strip_suffix(".tsv.gz") {
            members.push((gene.to_owned(), entry.path()));
        }
    }
    members.sort_by(|left, right| left.0.cmp(&right.0));
    if members.len() != 6 {
        return Err("regression source must contain exactly six gzip excerpts".into());
    }
    let rows: Vec<Vec<Row>> = members
        .iter()
        .map(|(gene, path)| read_rows(gene, path))
        .collect::<Result<_, _>>()?;
    let requests = select_requests(&rows)?;
    if requests.len() != 1_000 {
        return Err("selection did not produce exactly 1,000 requests".into());
    }
    fs::create_dir(output)?;
    let selected_source = output.join("source");
    fs::create_dir(&selected_source)?;
    write_selected_source(&rows, &requests, &selected_source)?;
    let reference = output.join("reference.fa.gz");
    write_reference(&rows, &requests, &reference)?;
    let bundle = output.join("bundle");
    let outcome = build_bundle(&selected_source, &reference, &bundle)?;
    write_requests(output, &requests)?;
    write_expected(output, &rows, &requests, &outcome.bundle_id)?;
    fs::write(
        output.join("README.md"),
        format!(
            "# Source-derived SNV regression fixture\n\nThis fixture contains exactly 1,000 deterministic requests selected from the six attributed Pangolin precomputed-score excerpts in `../pangolin-precompute/` under the contract in Ticket 006. The source is Pangolin precomputed scores by Nils Wagner and Aleksandr Neverov, Zenodo DOI <https://doi.org/{DOI}>, archive `Pangolin_hg38_snvs_masked.zip` (MD5 `{ARCHIVE_MD5}`), CC BY 4.0.\n\n`source/` is the closure of every source gene/locus needed by those requests, `reference.fa.gz` is a deterministic fixture-only reference, and `bundle/` is fixed-v1. `requests.tsv` defines original and seven-batch order. `expected.jsonl` and `expected/*.jsonl` come from this tool's direct strict TSV join and centi-score formatter; they do not call `BundleOpen`, `ScoreProvider`, or the CLI renderer.\n\nRegenerate into an absent directory with:\n\n```bash skip\ncargo run --locked --package pangopup-build --bin pangopup-regression-fixture -- tests/fixtures/pangolin-precompute <ABSENT_OUTPUT>\n```\n\nBundle identity: `{}`.\n",
            outcome.bundle_id
        ),
    )?;
    Ok(())
}

fn read_rows(gene: &str, path: &Path) -> Result<Vec<Row>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(GzDecoder::new(file));
    let mut header = String::new();
    reader.read_line(&mut header)?;
    if header.trim_end() != HEADER {
        return Err(format!("{gene}: invalid source header").into());
    }
    let mut rows = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').map(str::to_owned).collect();
        let fields: [String; 8] = fields.try_into().map_err(|_| "source row width")?;
        let position = fields[1].parse::<u32>()?;
        if position == 0
            || !matches!(fields[2].as_str(), "A" | "C" | "G" | "T" | "N")
            || !matches!(fields[3].as_str(), "A" | "C" | "G" | "T")
        {
            return Err(format!("{gene}: invalid direct TSV row").into());
        }
        exact_centi(&fields[4], false)?;
        exact_centi(&fields[6], true)?;
        fields[5].parse::<i8>()?;
        fields[7].parse::<i8>()?;
        rows.push(Row {
            gene: gene.to_owned(),
            fields,
        });
    }
    Ok(rows)
}

fn select_requests(rows: &[Vec<Row>]) -> Result<Vec<Request>, Box<dyn Error>> {
    let excluded: BTreeSet<_> = [
        ("ENSG00000141499", 7_686_072_u32),
        ("ENSG00000141510", 7_686_073_u32),
    ]
    .into_iter()
    .collect();
    let filtered: Vec<Vec<&Row>> = rows
        .iter()
        .map(|member| {
            member
                .iter()
                .filter(|row| {
                    row.reference() != "N"
                        && !excluded.contains(&(row.gene.as_str(), row.position()))
                })
                .collect()
        })
        .collect();
    let mut cursors = vec![0_usize; filtered.len()];
    let mut requests = Vec::new();
    while requests.len() < 970 {
        let before = requests.len();
        for (member, cursor) in filtered.iter().zip(&mut cursors) {
            if requests.len() == 970 {
                break;
            }
            if let Some(row) = member.get(*cursor) {
                *cursor += 1;
                push_request(&mut requests, row, Some(row.gene.clone()));
            }
        }
        if requests.len() == before {
            return Err("source excerpts exhausted before 970 requests".into());
        }
    }

    for position in 7_686_072..=7_686_075 {
        let member = rows
            .iter()
            .flatten()
            .filter(|row| {
                row.contig() == "chr17" && row.position() == position && row.reference() != "N"
            })
            .collect::<Vec<_>>();
        let reference = member
            .first()
            .ok_or("missing overlap edge locus")?
            .reference();
        let mut seen = BTreeSet::new();
        for row in member {
            if row.reference() != reference {
                return Err("overlap reference disagreement".into());
            }
            if seen.insert(row.alternate()) {
                push_variant(
                    &mut requests,
                    "chr17",
                    position,
                    reference,
                    row.alternate(),
                    None,
                );
            }
        }
        if seen.len() != 3 {
            return Err("overlap edge lacks three published ALTs".into());
        }
    }
    for (gene, position) in [
        ("ENSG00000141499", 7_686_072_u32),
        ("ENSG00000141510", 7_686_073_u32),
    ] {
        let selected = rows
            .iter()
            .flatten()
            .filter(|row| row.gene == gene && row.position() == position)
            .collect::<Vec<_>>();
        if selected.len() != 3 {
            return Err("fixed filtered edge lacks three rows".into());
        }
        for row in selected {
            push_request(&mut requests, row, Some(gene.to_owned()));
        }
    }
    for alternate in ["C", "G", "T"] {
        push_variant(&mut requests, "chr10", 114_306_066, "A", alternate, None);
    }
    for alternate in ["A", "C", "G"] {
        push_variant(&mut requests, "chr12", 122_093_260, "T", alternate, None);
    }
    for (contig, alternate) in [
        ("chr10", "C"),
        ("chr10", "G"),
        ("chr12", "C"),
        ("chr12", "G"),
        ("chr13", "C"),
        ("chr17", "C"),
    ] {
        push_variant(&mut requests, contig, 1, "A", alternate, None);
    }
    Ok(requests)
}

fn push_request(requests: &mut Vec<Request>, row: &Row, gene: Option<String>) {
    push_variant(
        requests,
        row.contig(),
        row.position(),
        row.reference(),
        row.alternate(),
        gene,
    );
}

fn push_variant(
    requests: &mut Vec<Request>,
    contig: &str,
    position: u32,
    reference: &str,
    alternate: &str,
    gene: Option<String>,
) {
    let group = gene.clone().unwrap_or_else(|| "unfiltered".to_owned());
    let group_order = requests
        .iter()
        .filter(|request| request.group == group)
        .count();
    requests.push(Request {
        order: requests.len(),
        group_order,
        group,
        contig: contig.to_owned(),
        position,
        reference: reference.to_owned(),
        alternate: alternate.to_owned(),
        gene,
    });
}

fn write_selected_source(
    rows: &[Vec<Row>],
    requests: &[Request],
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let loci: BTreeSet<_> = requests
        .iter()
        .map(|request| (request.contig.as_str(), request.position))
        .collect();
    for member in rows {
        let gene = &member[0].gene;
        let file = File::create(output.join(format!("{gene}.tsv.gz")))?;
        let mut gzip = GzBuilder::new().mtime(0).write(file, Compression::best());
        writeln!(gzip, "{HEADER}")?;
        for row in member {
            if loci.contains(&(row.contig(), row.position())) {
                writeln!(gzip, "{}", row.line())?;
            }
        }
        gzip.finish()?;
    }
    Ok(())
}

fn write_reference(
    rows: &[Vec<Row>],
    requests: &[Request],
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let accessions = [
        "NC_000001.11",
        "NC_000002.12",
        "NC_000003.12",
        "NC_000004.12",
        "NC_000005.10",
        "NC_000006.12",
        "NC_000007.14",
        "NC_000008.11",
        "NC_000009.12",
        "NC_000010.11",
        "NC_000011.10",
        "NC_000012.12",
        "NC_000013.11",
        "NC_000014.9",
        "NC_000015.10",
        "NC_000016.10",
        "NC_000017.11",
        "NC_000018.10",
        "NC_000019.10",
        "NC_000020.11",
        "NC_000021.9",
        "NC_000022.11",
        "NC_000023.11",
        "NC_000024.10",
        "NC_012920.1",
    ];
    let contigs = (1..=22)
        .map(|value| format!("chr{value}"))
        .chain(["chrX".to_owned(), "chrY".to_owned(), "chrM".to_owned()])
        .collect::<Vec<_>>();
    let mut lengths: BTreeMap<String, u32> =
        contigs.iter().cloned().map(|name| (name, 1)).collect();
    for request in requests {
        lengths
            .entry(request.contig.clone())
            .and_modify(|length| *length = (*length).max(request.position));
    }
    let selected_loci: BTreeSet<_> = requests
        .iter()
        .map(|request| (request.contig.as_str(), request.position))
        .collect();
    let mut bases = BTreeMap::new();
    for row in rows.iter().flatten().filter(|row| row.reference() != "N") {
        if selected_loci.contains(&(row.contig(), row.position())) {
            let key = (row.contig().to_owned(), row.position());
            if bases
                .insert(key.clone(), row.reference().as_bytes()[0])
                .is_some_and(|old| old != row.reference().as_bytes()[0])
            {
                return Err("source reference disagreement".into());
            }
        }
    }
    let file = File::create(output)?;
    let mut gzip = GzBuilder::new().mtime(0).write(file, Compression::best());
    for ((contig, accession), length) in contigs
        .iter()
        .zip(accessions)
        .zip(contigs.iter().map(|name| lengths[name]))
    {
        writeln!(gzip, ">{accession} regression fixture")?;
        let mut first = 1_u32;
        while first <= length {
            let count = usize::try_from((length - first + 1).min(1_048_576))?;
            let mut chunk = vec![b'A'; count];
            for ((_, position), base) in
                bases.range((contig.clone(), first)..=(contig.clone(), first + count as u32 - 1))
            {
                chunk[usize::try_from(*position - first)?] = *base;
            }
            gzip.write_all(&chunk)?;
            gzip.write_all(b"\n")?;
            first += count as u32;
        }
    }
    gzip.finish()?;
    Ok(())
}

fn write_requests(output: &Path, requests: &[Request]) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(output.join("requests.tsv"))?;
    writeln!(file, "order\tgroup\tgroup_order\tvariant\tgene")?;
    for request in requests {
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}",
            request.order,
            request.group,
            request.group_order,
            request.variant(),
            request.gene.as_deref().unwrap_or(".")
        )?;
    }
    Ok(())
}

fn write_expected(
    output: &Path,
    rows: &[Vec<Row>],
    requests: &[Request],
    bundle_id: &str,
) -> Result<(), Box<dyn Error>> {
    let expected_dir = output.join("expected");
    fs::create_dir(&expected_dir)?;
    let mut all = File::create(output.join("expected.jsonl"))?;
    let mut groups = BTreeMap::new();
    for request in requests {
        let bytes = direct_expected(rows, request, bundle_id)?;
        all.write_all(&bytes)?;
        groups
            .entry(request.group.clone())
            .or_insert_with(Vec::new)
            .extend_from_slice(&bytes);
    }
    for (group, bytes) in groups {
        fs::write(expected_dir.join(format!("{group}.jsonl")), bytes)?;
    }
    Ok(())
}

fn direct_expected(
    rows: &[Vec<Row>],
    request: &Request,
    bundle_id: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let accepts = |row: &&Row| request.gene.as_ref().is_none_or(|gene| gene == &row.gene);
    let mut records = rows
        .iter()
        .flatten()
        .filter(|row| {
            row.contig() == request.contig
                && row.position() == request.position
                && row.reference() == request.reference
                && row.alternate() == request.alternate
        })
        .filter(accepts)
        .map(|row| {
            Ok(JsonRecord {
                gene: row.gene.clone(),
                gain_score: format_score(exact_centi(&row.fields[4], false)?, false),
                gain_position: row.fields[5].parse()?,
                loss_score: format_score(exact_centi(&row.fields[6], true)?, true),
                loss_position: row.fields[7].parse()?,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    records.sort_by(|left, right| left.gene.cmp(&right.gene));
    let mut ambiguities = Vec::new();
    for member in rows {
        let locus = member
            .iter()
            .filter(|row| {
                row.contig() == request.contig
                    && row.position() == request.position
                    && row.reference() == "N"
            })
            .collect::<Vec<_>>();
        if locus.is_empty()
            || request
                .gene
                .as_ref()
                .is_some_and(|gene| gene != &locus[0].gene)
        {
            continue;
        }
        let published: BTreeSet<_> = locus.iter().map(|row| row.alternate().to_owned()).collect();
        if published.len() != 3 {
            return Err("ambiguous locus lacks three published alternatives".into());
        }
        let omitted = ["A", "C", "G", "T"]
            .into_iter()
            .find(|base| !published.contains(*base))
            .ok_or("ambiguous omitted base")?;
        ambiguities.push(JsonAmbiguity {
            gene: locus[0].gene.clone(),
            source_ref: "N",
            published_alts: published.into_iter().collect(),
            omitted_alt: omitted.to_owned(),
        });
    }
    ambiguities.sort_by(|left, right| left.gene.cmp(&right.gene));
    let status = match (records.is_empty(), ambiguities.is_empty()) {
        (false, true) => "found",
        (true, false) => "ambiguous_source_reference",
        (false, false) => "mixed",
        (true, true) => "not_found",
    };
    let value = JsonResult {
        assembly: "GRCh38",
        contig: &request.contig,
        position: request.position,
        reference: &request.reference,
        alternate: &request.alternate,
        status,
        records,
        source_reference_ambiguities: ambiguities,
        provenance: JsonProvenance {
            kind: "precomputed",
            bundle_id,
            source_doi: DOI,
            source_archive_md5: ARCHIVE_MD5,
            masked: true,
            window: 50,
        },
    };
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn exact_centi(text: &str, loss: bool) -> Result<u16, Box<dyn Error>> {
    let negative = text.starts_with('-');
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let mut parts = unsigned.split('.');
    let whole = parts.next().ok_or("score whole")?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some()
        || whole.len() != 1
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("score is not an exact hundredth".into());
    }
    let mut value = whole.parse::<u16>()? * 100;
    value += match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u16>()? * 10,
        _ => fraction.parse::<u16>()?,
    };
    if value > 100 || (!loss && negative && value != 0) || (loss && !negative && value != 0) {
        return Err("score sign or range is invalid".into());
    }
    Ok(value)
}

fn format_score(value: u16, loss: bool) -> String {
    let sign = if loss && value != 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", value / 100, value % 100)
}
