use pangopup_build::sparse_latency::{SparseLatencyArguments, measure_sparse_latency};
use std::{env, ffi::OsString, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    match parse(env::args_os().skip(1).collect()) {
        Ok(arguments) => match measure_sparse_latency(&arguments) {
            Ok(outcome) => write_json(&outcome, false),
            Err(error) => write_json(&error, true),
        },
        Err(message) => write_json(
            &pangopup_build::CommandError::new("CLI_USAGE", message),
            true,
        ),
    }
}

fn parse(arguments: Vec<OsString>) -> Result<SparseLatencyArguments, &'static str> {
    const FLAGS: [&str; 6] = [
        "--fixed",
        "--candidate",
        "--queries",
        "--selection",
        "--output",
        "--command-commit",
    ];
    if arguments.len() != FLAGS.len() * 2 {
        return Err(
            "requires --fixed, --candidate, --queries, --selection, --output, and --command-commit exactly once",
        );
    }
    let mut values: [Option<OsString>; 6] = std::array::from_fn(|_| None);
    for pair in arguments.chunks_exact(2) {
        let Some(index) = FLAGS.iter().position(|flag| pair[0] == *flag) else {
            return Err("unknown latency benchmark flag");
        };
        if values[index].replace(pair[1].clone()).is_some() {
            return Err("duplicate latency benchmark flag");
        }
    }
    let [fixed, candidate, queries, selection, output, command_commit] =
        values.map(|value| value.expect("all flags present"));
    let command_commit = command_commit
        .into_string()
        .map_err(|_| "--command-commit must be UTF-8")?;
    Ok(SparseLatencyArguments {
        fixed: PathBuf::from(fixed),
        candidate: PathBuf::from(candidate),
        queries: PathBuf::from(queries),
        selection: PathBuf::from(selection),
        output: PathBuf::from(output),
        command_commit,
    })
}

fn write_json(value: &impl serde::Serialize, failed: bool) -> ExitCode {
    use std::io::Write;
    let bytes = match serde_jcs::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => b"{\"code\":\"LATENCY_REPORT\",\"details\":null,\"message\":\"JSON encoding failed\",\"status\":\"error\"}".to_vec(),
    };
    let mut stream: Box<dyn Write> = if failed {
        Box::new(std::io::stderr().lock())
    } else {
        Box::new(std::io::stdout().lock())
    };
    if stream
        .write_all(&bytes)
        .and_then(|()| stream.write_all(b"\n"))
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_requires_each_closed_flag_once() {
        let arguments = [
            "--fixed",
            "fixed",
            "--candidate",
            "candidate",
            "--queries",
            "queries",
            "--selection",
            "selection",
            "--output",
            "output",
            "--command-commit",
            "0123456789012345678901234567890123456789",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        let parsed = parse(arguments).expect("closed arguments");
        assert_eq!(parsed.output, PathBuf::from("output"));
        assert!(parse(Vec::new()).is_err());
    }
}
