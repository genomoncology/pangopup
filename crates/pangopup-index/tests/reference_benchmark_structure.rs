const BENCHMARK: &str = include_str!("../benches/reference_formats.rs");

fn section<'a>(begin: &str, end: &str) -> &'a str {
    let (_, tail) = BENCHMARK
        .split_once(begin)
        .expect("benchmark section begin marker");
    let (body, _) = tail.split_once(end).expect("benchmark section end marker");
    body
}

#[test]
fn reference_candidate_benchmark_defers_report_work_until_all_timing_finishes() {
    let timing = section("// TIMING-ROUNDS-BEGIN", "// TIMING-ROUNDS-END");
    assert!(timing.contains("fn run_timing_rounds("));
    assert!(!timing.contains("zstd_size("));
    assert!(!timing.contains("trace_window("));
    assert!(!timing.contains("sort_unstable("));

    let report = section("// REPORT-WORK-BEGIN", "// REPORT-WORK-END");
    assert!(report.contains("fn finalize_candidate_reports("));
    assert!(report.contains("zstd_size("));
    assert!(report.contains("trace_window("));
    assert!(report.contains("sort_unstable("));

    let timing_call = BENCHMARK
        .find("let timings = run_timing_rounds(")
        .expect("timing call");
    let report_call = BENCHMARK
        .find("let reports = finalize_candidate_reports(")
        .expect("report call");
    assert!(timing_call < report_call);
}
