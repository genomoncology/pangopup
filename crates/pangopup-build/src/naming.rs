//! Maintainer-only inspection of a gene naming source.
//!
//! The inspector is offline and reads one dated release file. It exists so
//! the withheld-name rules can be pinned: an accession that carries two
//! approved records, and an approved symbol that is a clone-derived
//! placeholder, both appear in the published release and in no score bundle.

use pangopup_assets::{AssetError, GeneNaming, NamingSource};
use std::{io::Write, path::Path};

/// Report what a naming source yields, one line per Ensembl accession, sorted
/// by accession, followed by one total line.
pub fn inspect_naming_source(source: &Path, output: &mut dyn Write) -> Result<(), AssetError> {
    let source = NamingSource::read(source)?;
    for (accession, naming) in source.entries() {
        match naming {
            GeneNaming::Named(names) => writeln!(
                output,
                "gene={accession} hgnc={} symbol={} ncbi={} prev={} alias={}",
                names.hgnc_id,
                names.symbol,
                names
                    .ncbi_gene_id
                    .map_or_else(|| "-".to_owned(), |value| value.to_string()),
                joined(&names.prev_symbols),
                joined(&names.alias_symbols),
            ),
            GeneNaming::Unnamed(reason) => {
                writeln!(output, "gene={accession} unnamed={}", reason.as_str())
            }
        }
        .map_err(write_failed)?;
    }
    writeln!(
        output,
        "total rows={} accessions={} named={} unnamed={}",
        source.rows(),
        source.accessions(),
        source.named(),
        source.unnamed(),
    )
    .map_err(write_failed)
}

/// Every value of a multi-valued field, in source order. A field the source
/// does not supply reports `-`.
fn joined(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_owned()
    } else {
        values.join("|")
    }
}

fn write_failed(error: std::io::Error) -> AssetError {
    AssetError::new(pangopup_assets::AssetErrorKind::OutputIo, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_accession_reports_one_sorted_line_and_one_total() {
        let mut output = Vec::new();
        inspect_naming_source(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/gene-naming-mini/hgnc_complete_set_2026-09-04.tsv"
            )),
            &mut output,
        )
        .expect("inspect the miniature naming source");
        let text = String::from_utf8(output).expect("inspection is UTF-8");
        assert_eq!(
            text,
            concat!(
                "gene=ENSG00000010610 hgnc=HGNC:1678 symbol=CD4 ncbi=920 prev=- alias=T4|Leu-3\n",
                "gene=ENSG00000119888 hgnc=HGNC:11529 symbol=EPCAM ncbi=4072 prev=M4S1|MIC18|TACSTD1 alias=Ly74|TROP1|GA733-2|EGP34|EGP40|EGP-2|KSA|CD326|Ep-CAM|HEA125|KS1/4|MK-1|MH99|MOC31|MOC-31|323/A3|17-1A|TACST-1|CO-17A|ESA|BerEp4|Ber-Ep4\n",
                "gene=ENSG00000141499 hgnc=HGNC:25522 symbol=WRAP53 ncbi=55135 prev=WDR79 alias=FLJ10385|TCAB1\n",
                "gene=ENSG00000141510 hgnc=HGNC:11998 symbol=TP53 ncbi=- prev=- alias=p53|LFS1\n",
                "gene=ENSG00000157764 hgnc=HGNC:1097 symbol=BRAF ncbi=673 prev=- alias=BRAF1|BRAF-1\n",
                "gene=ENSG00000167034 hgnc=HGNC:7838 symbol=NKX3-1 ncbi=4824 prev=NKX3A alias=NKX3.1|BAPX2\n",
                "gene=ENSG00000169129 hgnc=HGNC:25901 symbol=AFAP1L2 ncbi=84632 prev=KIAA1914 alias=FLJ14564|Em:AC005383.4|XB130\n",
                "gene=ENSG00000175658 unnamed=conflicting_records\n",
                "gene=ENSG00000185974 unnamed=placeholder_symbol\n",
                "total rows=10 accessions=9 named=7 unnamed=2\n",
            )
        );
    }
}
