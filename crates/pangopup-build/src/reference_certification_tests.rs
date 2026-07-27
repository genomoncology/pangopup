use super::*;

fn passing_qualification<'a>(pages: &'a [u64]) -> ReferenceQualificationMeasurements<'a> {
    ReferenceQualificationMeasurements {
        total_bases: 3_088_286_401,
        sequence_set_sha256: "sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4",
        extra_record_count: 680,
        extra_accessions_sha256: "sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
        contexts_verified: 14,
        headline_p50_ns: QUALIFICATION_MAX_P50_NS,
        headline_p95_ns: QUALIFICATION_MAX_P95_NS,
        allocation_calls: 0,
        allocation_bytes: 0,
        open_peak_heap_bytes: QUALIFICATION_MAX_OPEN_HEAP_BYTES,
        builder_peak_heap_bytes: QUALIFICATION_MAX_BUILDER_HEAP_BYTES,
        dense_bytes_read_during_open: 0,
        member_bytes: QUALIFICATION_MAX_MEMBER_BYTES,
        unique_dense_pages: pages,
        per_case_page_count_sum: 20,
    }
}

#[test]
fn qualification_thresholds_keep_boundaries_and_report_every_observed_limit() {
    let pages = [
        31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
    ];
    evaluate_reference_qualification(&passing_qualification(&pages)).expect("boundaries pass");

    let assert_rejection =
        |observed: &ReferenceQualificationMeasurements<'_>, class, message: &str| {
            let rejection = evaluate_reference_qualification(observed).expect_err("must reject");
            assert_eq!(rejection.class, class);
            assert_eq!(rejection.message, message);
            assert!(rejection.message.is_ascii());
            assert!(rejection.message.len() <= 256);
        };

    let mut observed = passing_qualification(&pages);
    observed.total_bases -= 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::LogicalSequence,
        "logical-sequence b=3088286400/3088286401 s=sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
    );
    let mut observed = passing_qualification(&pages);
    observed.sequence_set_sha256 =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::LogicalSequence,
        "logical-sequence b=3088286401/3088286401 s=sha256:0000000000000000000000000000000000000000000000000000000000000000/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
    );
    let mut observed = passing_qualification(&pages);
    observed.contexts_verified += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::LogicalSequence,
        "logical-sequence b=3088286401/3088286401 s=sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=15/14",
    );
    let mut observed = passing_qualification(&pages);
    observed.extra_record_count += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::LogicalExtras,
        "logical-extras n=681/680 s=sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
    );
    let mut observed = passing_qualification(&pages);
    observed.extra_accessions_sha256 =
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::LogicalExtras,
        "logical-extras n=680/680 s=sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
    );
    let mut observed = passing_qualification(&pages);
    observed.headline_p50_ns += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Latency,
        "latency p50_ns=5587/5586 p95_ns=6100/6100",
    );
    let mut observed = passing_qualification(&pages);
    observed.headline_p95_ns += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Latency,
        "latency p50_ns=5586/5586 p95_ns=6101/6100",
    );
    let mut observed = passing_qualification(&pages);
    observed.allocation_calls = 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Allocations,
        "allocations calls=1/0 bytes=0/0",
    );
    let mut observed = passing_qualification(&pages);
    observed.allocation_bytes = 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Allocations,
        "allocations calls=0/0 bytes=1/0",
    );
    let mut observed = passing_qualification(&pages);
    observed.open_peak_heap_bytes += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Heap,
        "heap open_bytes=2097153/2097152 builder_bytes=16777216/16777216",
    );
    let mut observed = passing_qualification(&pages);
    observed.builder_peak_heap_bytes += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Heap,
        "heap open_bytes=2097152/2097152 builder_bytes=16777217/16777216",
    );
    let mut observed = passing_qualification(&pages);
    observed.dense_bytes_read_during_open = 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Storage,
        "storage dense_open_bytes=1/0 member_bytes=773124288/773124288",
    );
    let mut observed = passing_qualification(&pages);
    observed.member_bytes += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Storage,
        "storage dense_open_bytes=0/0 member_bytes=773124289/773124288",
    );
    let wrong_pages = [31_748];
    let observed = passing_qualification(&wrong_pages);
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Pages,
        "pages unique=[31748]/[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495] per_case_sum=20/20",
    );
    let mut observed = passing_qualification(&pages);
    observed.per_case_page_count_sum += 1;
    assert_rejection(
        &observed,
        ReferenceQualificationFailureClass::Pages,
        "pages unique=[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495]/[31748, 109204, 119053, 119054, 133714, 133715, 152494, 152495] per_case_sum=21/20",
    );
}

#[test]
fn qualification_diagnostics_are_bounded_ascii_and_redact_invalid_identity_text() {
    let pages = [
        31_748, 109_204, 119_053, 119_054, 133_714, 133_715, 152_494, 152_495,
    ];
    let secret = "not-a-sha\nsecret-token-\u{2603}";
    let mut observed = passing_qualification(&pages);
    observed.sequence_set_sha256 = secret;
    let rejection = evaluate_reference_qualification(&observed).expect_err("invalid identity");
    assert_eq!(
        rejection.class,
        ReferenceQualificationFailureClass::LogicalSequence
    );
    assert_eq!(
        rejection.message,
        format!(
            "logical-sequence b=3088286401/3088286401 s=invalid(len=26,sha256:{:x})/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=14/14",
            Sha256::digest(secret.as_bytes())
        )
    );
    assert!(rejection.message.is_ascii());
    assert!(rejection.message.len() <= 256);
    assert!(!rejection.message.contains("secret-token"));

    let huge_pages = [u64::MAX; 64];
    let observed = passing_qualification(&huge_pages);
    let rejection = evaluate_reference_qualification(&observed).expect_err("huge page vector");
    assert_eq!(rejection.class, ReferenceQualificationFailureClass::Pages);
    assert!(
        rejection
            .message
            .starts_with("pages unique_len=64 unique_u64be_sha256=sha256:")
    );
    assert!(rejection.message.is_ascii());
    assert!(rejection.message.len() <= 256);
}

#[test]
fn qualification_diagnostics_are_constructively_bounded_over_maximal_public_values() {
    let pages = QUALIFICATION_UNIQUE_DENSE_PAGES;
    let assert_exact =
        |observed: &ReferenceQualificationMeasurements<'_>, class, expected: String| {
            let rejection = evaluate_reference_qualification(observed).expect_err("must reject");
            assert_eq!(rejection.class, class);
            assert_eq!(rejection.message, expected);
            assert!(rejection.message.is_ascii());
            assert!(rejection.message.len() <= 256);
        };
    let invalid = "x\nsecret-\u{2603}".repeat(65_536);

    let mut observed = passing_qualification(&pages);
    observed.total_bases = u64::MAX;
    observed.sequence_set_sha256 = &invalid;
    observed.contexts_verified = u64::MAX;
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::LogicalSequence,
        format!(
            "logical-sequence b=18446744073709551615/3088286401 s=invalid(len={},sha256:{:x})/sha256:2a970f2c70fcb5ff4baa179a8d801f8cf7509ca32b86dac789344e9d49927fa4 c=18446744073709551615/14",
            invalid.len(),
            Sha256::digest(invalid.as_bytes())
        ),
    );

    let mut observed = passing_qualification(&pages);
    observed.extra_record_count = u64::MAX;
    observed.extra_accessions_sha256 = &invalid;
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::LogicalExtras,
        format!(
            "logical-extras n=18446744073709551615/680 s=invalid(len={},sha256:{:x})/sha256:0ed644cffeca1da89dfb9cbe6156aedc2e66a0df59ea5be027d15074343ec0fb",
            invalid.len(),
            Sha256::digest(invalid.as_bytes())
        ),
    );

    let mut observed = passing_qualification(&pages);
    observed.headline_p50_ns = u64::MAX;
    observed.headline_p95_ns = u64::MAX;
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::Latency,
        "latency p50_ns=18446744073709551615/5586 p95_ns=18446744073709551615/6100".to_owned(),
    );

    let mut observed = passing_qualification(&pages);
    observed.allocation_calls = u64::MAX;
    observed.allocation_bytes = u64::MAX;
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::Allocations,
        "allocations calls=18446744073709551615/0 bytes=18446744073709551615/0".to_owned(),
    );

    let mut observed = passing_qualification(&pages);
    observed.open_peak_heap_bytes = u64::MAX;
    observed.builder_peak_heap_bytes = u64::MAX;
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::Heap,
        "heap open_bytes=18446744073709551615/2097152 builder_bytes=18446744073709551615/16777216"
            .to_owned(),
    );

    let mut observed = passing_qualification(&pages);
    observed.dense_bytes_read_during_open = u64::MAX;
    observed.member_bytes = u64::MAX;
    assert_exact(
            &observed,
            ReferenceQualificationFailureClass::Storage,
            "storage dense_open_bytes=18446744073709551615/0 member_bytes=18446744073709551615/773124288".to_owned(),
        );

    let huge_pages = [u64::MAX; 4_096];
    let observed = passing_qualification(&huge_pages);
    let mut hash = Sha256::new();
    for page in &huge_pages {
        hash.update(page.to_be_bytes());
    }
    assert_exact(
        &observed,
        ReferenceQualificationFailureClass::Pages,
        format!(
            "pages unique_len=4096 unique_u64be_sha256=sha256:{:x} limit={:?} per_case_sum=20/20",
            hash.finalize(),
            QUALIFICATION_UNIQUE_DENSE_PAGES
        ),
    );

    let unbounded = "\u{2603}".repeat(300);
    let rejection = qualification_rejection(
        ReferenceQualificationFailureClass::Latency,
        unbounded.clone(),
    );
    assert_eq!(
        rejection.message,
        format!(
            "latency diagnostic_len={} diagnostic_sha256=sha256:{:x}",
            unbounded.len(),
            Sha256::digest(unbounded.as_bytes())
        )
    );
    assert!(rejection.message.is_ascii());
    assert!(rejection.message.len() <= 256);
}
