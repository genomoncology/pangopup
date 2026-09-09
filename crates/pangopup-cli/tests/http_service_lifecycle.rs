#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, process::Command};

#[cfg(feature = "service-test-fixtures")]
mod installed_success {
    use pangopup_assets::{
        MaskProfile, ModelProfile, ReferenceProfile, RuntimeProfile, ScoringProfile, SnvProfile,
        canonical_runtime_profile_bytes, inspect_snv_bundle, install_test_runtime_profile,
        install_transport,
    };
    use pangopup_index::reference_admission::inspect_reference_admission;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeSet,
        fs,
        io::{BufRead, BufReader, Read, Write},
        net::TcpStream,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        thread,
    };

    fn fixture(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures")
            .join(relative)
    }

    fn digest(path: &Path) -> String {
        format!("sha256:{:x}", Sha256::digest(fs::read(path).expect("read")))
    }

    fn install(root: &Path, scratch: &Path) -> (RuntimeProfile, PathBuf) {
        install_variant(root, scratch, &fixture("route-mask/domains.pgm"), 50)
    }

    /// Install the miniature runtime with one chosen mask asset and one chosen
    /// scoring distance. `install` supplies the defaults. A caller varies
    /// either input to change what the runtime profile identifies.
    fn install_variant(
        root: &Path,
        scratch: &Path,
        mask: &Path,
        distance: u64,
    ) -> (RuntimeProfile, PathBuf) {
        fs::set_permissions(scratch, fs::Permissions::from_mode(0o700)).expect("private");
        let snv_bundle = fixture("snv-regression/bundle");
        let transport = scratch.join("transport");
        pangopup_assets::pack_bundle(&snv_bundle, &transport).expect("pack SNV");
        install_transport(&transport, root).expect("install SNV");
        let snv = inspect_snv_bundle(&snv_bundle).expect("inspect SNV");
        let model = fixture("pangolin-model-kernel-mini/bundle");
        let model_manifest = fs::read(model.join("manifest.json")).expect("model manifest");
        let model_json: Value = serde_json::from_slice(&model_manifest).expect("model JSON");
        let model_member = model_json["members"]
            .as_array()
            .expect("members")
            .iter()
            .find(|member| member["filename"] == "model.onnx")
            .expect("model member");
        let reference = fixture("reference-route-test/bundle");
        let reference_facts = inspect_reference_admission(&reference).expect("reference");
        let reference_member = reference.join("reference.pgr");
        let profile = RuntimeProfile {
            schema: pangopup_assets::RUNTIME_PROFILE_SCHEMA.to_owned(),
            snv: SnvProfile {
                bundle_id: snv.bundle_id,
                format: snv.format,
                member_bytes: snv.member_bytes,
                member_sha256: snv.member_sha256,
            },
            model: ModelProfile {
                bundle_id: format!("sha256:{:x}", Sha256::digest(&model_manifest)),
                profile: model_json["profile"].as_str().expect("profile").to_owned(),
                representation: "singleton".to_owned(),
                member_bytes: model_member["bytes"].as_u64().expect("bytes"),
                member_sha256: model_member["sha256"].as_str().expect("sha").to_owned(),
            },
            reference: ReferenceProfile {
                bundle_id: reference_facts.bundle_id().to_owned(),
                profile: reference_facts.profile().to_owned(),
                format: reference_facts.format().to_owned(),
                assembly: reference_facts.assembly().to_owned(),
                assembly_accession: reference_facts.assembly_accession().to_owned(),
                sequence_set_sha256: reference_facts.sequence_set_sha256().to_owned(),
                member_bytes: fs::metadata(&reference_member).expect("reference").len(),
                member_sha256: digest(&reference_member),
            },
            mask: MaskProfile {
                format: "pangopup.gencode-v38-domains.v1".to_owned(),
                member_bytes: fs::metadata(mask).expect("mask").len(),
                member_sha256: digest(mask),
            },
            scoring: ScoringProfile {
                assembly: "GRCh38".to_owned(),
                semantics: "pangopup-variant-score-v1".to_owned(),
                distance,
                masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
                cpu_policy: "sequential:1/1".to_owned(),
            },
        };
        install_test_runtime_profile(&profile, &model, &reference, mask, root)
            .expect("install runtime");
        let profile_path = scratch.join("mini-profile.json");
        fs::write(
            &profile_path,
            canonical_runtime_profile_bytes(&profile).expect("profile bytes"),
        )
        .expect("profile file");
        (profile, profile_path)
    }

    fn start(data: &Path, profile: &Path) -> (Child, String) {
        let cache = profile
            .parent()
            .expect("profile parent")
            .join("service-cache.sqlite3");
        let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                data.to_str().expect("data"),
                "--model-cache",
                cache.to_str().expect("cache"),
            ])
            .env("PANGOPUP_SERVICE_TEST_PROFILE", profile)
            .env("PANGOPUP_SERVICE_TEST_JOB_DELAY_MS", "100")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().expect("stdout"))
            .read_line(&mut line)
            .expect("listening");
        if line.is_empty() {
            let status = child.wait().expect("status");
            let mut stderr = String::new();
            child
                .stderr
                .as_mut()
                .expect("stderr")
                .read_to_string(&mut stderr)
                .expect("stderr");
            panic!("startup failed: {status}: {stderr}");
        }
        let event: Value = serde_json::from_str(&line).expect("event");
        assert_eq!(event["event"], "listening");
        (
            child,
            event["address"].as_str().expect("address").to_owned(),
        )
    }

    fn start_with_policy(
        data: &Path,
        profile: &Path,
        cache: &Path,
        workers: &str,
        threads: &str,
    ) -> (Child, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                data.to_str().expect("data"),
                "--model-cache",
                cache.to_str().expect("cache"),
                "--model-workers",
                workers,
                "--model-threads",
                threads,
            ])
            .env("PANGOPUP_SERVICE_TEST_PROFILE", profile)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().expect("stdout"))
            .read_line(&mut line)
            .expect("listening");
        if line.is_empty() {
            let status = child.wait().expect("status");
            let mut stderr = String::new();
            child
                .stderr
                .as_mut()
                .expect("stderr")
                .read_to_string(&mut stderr)
                .expect("stderr");
            panic!("startup failed: {status}: {stderr}");
        }
        let event: Value = serde_json::from_str(&line).expect("event");
        assert_eq!(event["event"], "listening");
        (
            child,
            event["address"].as_str().expect("address").to_owned(),
        )
    }

    fn request(address: &str, method: &str, path: &str, body: &str) -> Vec<u8> {
        let content_types = if path == "/v1/score" {
            &["application/json"][..]
        } else {
            &[][..]
        };
        request_with_content_types(address, method, path, body, content_types)
    }

    fn request_with_content_types(
        address: &str,
        method: &str,
        path: &str,
        body: &str,
        content_types: &[&str],
    ) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).expect("connect");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n",
            body.len()
        )
        .expect("write");
        for content_type in content_types {
            write!(stream, "Content-Type: {content_type}\r\n").expect("write content type");
        }
        write!(stream, "Connection: close\r\n\r\n{body}").expect("write body");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read");
        response
    }

    fn response_body(response: &[u8]) -> &[u8] {
        let split = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP separator");
        &response[split + 4..]
    }

    #[test]
    fn real_executable_serves_all_routes_and_drains_accepted_model_work_on_sigterm() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        for path in ["/livez", "/readyz", "/v1/status"] {
            assert!(request(&address, "GET", path, "").starts_with(b"HTTP/1.1 200 OK\r\n"));
        }
        let status = request(&address, "GET", "/v1/status", "");
        let status: Value = serde_json::from_slice(response_body(&status)).expect("status JSON");
        let scoring_identity = status["scoring_identity"]
            .as_str()
            .expect("status scoring identity")
            .to_owned();
        let score_body = r#"{"variants":["GRCh38:chr12:6801301:G:A"]}"#;
        for content_types in [
            &[][..],
            &["text/plain"][..],
            &["application/json", "application/json"][..],
        ] {
            let response = request_with_content_types(
                &address,
                "POST",
                "/v1/score",
                score_body,
                content_types,
            );
            assert!(
                response.starts_with(b"HTTP/1.1 415 Unsupported Media Type\r\n"),
                "{}",
                String::from_utf8_lossy(&response)
            );
            assert_eq!(
                response_body(&response),
                b"{\"error\":{\"code\":\"UNSUPPORTED_MEDIA_TYPE\",\"message\":\"content-type must be application/json\"}}\n"
            );
        }
        let parameterized = request_with_content_types(
            &address,
            "POST",
            "/v1/score",
            score_body,
            &["Application/JSON; charset=utf-8"],
        );
        assert!(parameterized.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let lookup = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A"]}"#,
        );
        assert!(lookup.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let lookup: Value =
            serde_json::from_slice(response_body(&lookup)).expect("lookup score JSON");
        assert_eq!(lookup["results"][0]["scoring_identity"], scoring_identity);

        let variants = (0..10)
            .map(|_| "\"GRCh38:chr1:5051:A:AC\"".to_owned())
            .collect::<Vec<_>>()
            .join(",");
        let body = format!("{{\"variants\":[{variants}]}}");
        let scoring_address = address.clone();
        let scoring = thread::spawn(move || request(&scoring_address, "POST", "/v1/score", &body));
        let mut observed_running = false;
        for _ in 0..1_000 {
            let status = request(&address, "GET", "/v1/status", "");
            if status
                .windows(b"\"running\":1".len())
                .any(|w| w == b"\"running\":1")
            {
                observed_running = true;
                break;
            }
            thread::yield_now();
        }
        assert!(observed_running, "accepted model work must be observable");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        let scored = scoring.join().expect("scoring join");
        assert!(
            scored.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&scored)
        );
        let scored: Value =
            serde_json::from_slice(response_body(&scored)).expect("modeled score JSON");
        assert!(
            scored["results"]
                .as_array()
                .expect("results")
                .iter()
                .all(|result| result["scoring_identity"] == scoring_identity)
        );
        assert!(child.wait().expect("service exit").success());

        let (mut restarted, restarted_address) = start(&data, &profile_path);
        let restarted_status = request(&restarted_address, "GET", "/v1/status", "");
        let restarted_status: Value = serde_json::from_slice(response_body(&restarted_status))
            .expect("restarted status JSON");
        assert_eq!(restarted_status["scoring_identity"], scoring_identity);
        assert_eq!(
            unsafe { libc::kill(restarted.id() as i32, libc::SIGTERM) },
            0
        );
        assert!(restarted.wait().expect("restarted service exit").success());
    }

    // Everything a caller reads except the two fields a CPU policy is allowed
    // to move. A score, a position, a status and a rejection reason all stay.
    fn comparable_items(response: &[u8]) -> Vec<Value> {
        let value: Value = serde_json::from_slice(response_body(response)).expect("score JSON");
        value["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|item| {
                let mut item = item.clone();
                item.as_object_mut()
                    .expect("item")
                    .remove("scoring_identity");
                if let Some(provenance) = item["provenance"].as_object_mut() {
                    provenance.remove("effective_cpu_policy");
                }
                item
            })
            .collect()
    }

    #[test]
    fn a_deployment_worker_and_thread_setting_changes_no_modeled_score() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let body = r#"{"variants":["GRCh38:chr1:5051:A:C","GRCh38:chr1:5051:A:T","GRCh38:chr1:5051:A:G","GRCh38:chr1:5051:A:AC","GRCh38:chr1:INS:5051:5052:C","GRCh38:chr1:5051:A:TC"]}"#;
        let mut policies = Vec::new();
        let mut identities = Vec::new();
        let mut scored = Vec::new();
        for (index, (workers, threads)) in [("1", "1"), ("2", "4")].into_iter().enumerate() {
            // A separate cache per run, so the second run recomputes every
            // variant instead of reading the first run's rows back.
            let cache = temp.path().join(format!("policy-cache-{index}.sqlite3"));
            let (mut child, address) =
                start_with_policy(&data, &profile_path, &cache, workers, threads);
            let response = request(&address, "POST", "/v1/score", body);
            assert!(
                response.starts_with(b"HTTP/1.1 200 OK\r\n"),
                "{}",
                String::from_utf8_lossy(&response)
            );
            let value: Value =
                serde_json::from_slice(response_body(&response)).expect("score JSON");
            let results = value["results"].as_array().expect("results").clone();
            assert_eq!(results.len(), 6);
            assert!(
                results
                    .iter()
                    .filter(|item| item["provenance"]["kind"] == "model")
                    .count()
                    >= 5,
                "the compared batch must reach the model: {value}"
            );
            policies.push(
                results[0]["provenance"]["effective_cpu_policy"]
                    .as_str()
                    .expect("effective CPU policy")
                    .to_owned(),
            );
            identities.push(
                results[0]["scoring_identity"]
                    .as_str()
                    .expect("scoring identity")
                    .to_owned(),
            );
            scored.push(comparable_items(&response));
            assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
            assert!(child.wait().expect("service exit").success());
        }
        assert_ne!(
            policies[0], policies[1],
            "the two runs must report different effective CPU policies"
        );
        assert_ne!(
            identities[0], identities[1],
            "the scoring identity moves with the effective CPU policy today"
        );
        assert_eq!(
            scored[0], scored[1],
            "a deployment worker or thread setting must move no score, position, status or reason"
        );
    }

    /// One published status field, or a clear failure naming the field the
    /// service did not publish.
    fn published(status: &Value, field: &str) -> String {
        status[field]
            .as_str()
            .unwrap_or_else(|| panic!("status must publish {field}: {status}"))
            .to_owned()
    }

    /// Start one service under a chosen thread count, read `/v1/status`, and
    /// stop it again.
    fn status_under_threads(data: &Path, profile: &Path, cache: &Path, threads: &str) -> Value {
        let (mut child, address) = start_with_policy(data, profile, cache, "1", threads);
        let response = request(&address, "GET", "/v1/status", "");
        assert!(
            response.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&response)
        );
        let status: Value = serde_json::from_slice(response_body(&response)).expect("status JSON");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        status
    }

    /// Install one miniature runtime under its own data root and scratch
    /// directory, so several installations can stand side by side.
    fn install_under(
        temp: &Path,
        name: &str,
        mask: &Path,
        distance: u64,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let scratch = temp.join(format!("{name}-scratch"));
        fs::create_dir(&scratch).expect("scratch");
        let data = temp.join(format!("{name}-data"));
        let (_profile, profile_path) = install_variant(&data, &scratch, mask, distance);
        let cache = scratch.join("service-cache.sqlite3");
        (data, profile_path, cache)
    }

    // A consumer stores one value as its data-set version. That value must
    // track the inputs a score depends on and nothing else. A thread count
    // reaches the model runtime and reaches no answer. It must therefore leave
    // the stored value alone. A changed mask asset and a changed scoring input
    // must both move it.
    #[test]
    fn pinned_values_hold_across_cpu_policies_and_move_with_the_scored_inputs() {
        let temp = tempfile::tempdir().expect("temp");
        let (data, profile, cache) = install_under(
            temp.path(),
            "baseline",
            &fixture("route-mask/domains.pgm"),
            50,
        );
        let one = status_under_threads(&data, &profile, &cache, "1");
        let four = status_under_threads(
            &data,
            &profile,
            &cache.with_file_name("service-cache-four.sqlite3"),
            "4",
        );
        assert_eq!(
            one["model"]["effective_cpu_policy"], "sequential:1/1",
            "one thread renders the effective policy the compatibility contract quotes"
        );
        assert_eq!(
            four["model"]["effective_cpu_policy"], "sequential:4/1",
            "four threads render the effective policy the compatibility contract quotes"
        );
        assert_ne!(
            one["scoring_identity"], four["scoring_identity"],
            "the active scoring identity still covers the effective CPU policy"
        );
        assert_eq!(
            published(&one, "data_set_version"),
            published(&four, "data_set_version"),
            "a thread setting must not move the value a consumer stores as its data-set version"
        );
        assert_eq!(
            published(&one, "runtime_profile_id"),
            published(&four, "runtime_profile_id"),
            "a thread setting must not move the runtime profile identity"
        );
        assert_eq!(
            one["assets"], four["assets"],
            "the two thread settings must hold every asset digest fixed"
        );

        let (other_mask_data, other_mask_profile, other_mask_cache) = install_under(
            temp.path(),
            "other-mask",
            &fixture("gencode-mask-mini/domains.pgm"),
            50,
        );
        let other_mask = status_under_threads(
            &other_mask_data,
            &other_mask_profile,
            &other_mask_cache,
            "1",
        );
        assert_ne!(
            one["assets"]["mask_sha256"], other_mask["assets"]["mask_sha256"],
            "this run must install a different mask asset"
        );
        assert_ne!(
            published(&other_mask, "runtime_profile_id"),
            published(&one, "runtime_profile_id"),
            "a changed mask asset must move the runtime profile identity"
        );
        assert_ne!(
            published(&other_mask, "data_set_version"),
            published(&one, "data_set_version"),
            "a changed mask asset must move the data-set version"
        );

        let (other_distance_data, other_distance_profile, other_distance_cache) = install_under(
            temp.path(),
            "other-distance",
            &fixture("route-mask/domains.pgm"),
            51,
        );
        let other_distance = status_under_threads(
            &other_distance_data,
            &other_distance_profile,
            &other_distance_cache,
            "1",
        );
        assert_eq!(
            one["assets"], other_distance["assets"],
            "this run must change a scoring input with every asset digest fixed"
        );
        assert_ne!(
            published(&other_distance, "runtime_profile_id"),
            published(&one, "runtime_profile_id"),
            "a changed scoring distance must move the runtime profile identity"
        );
        assert_ne!(
            published(&other_distance, "data_set_version"),
            published(&one, "data_set_version"),
            "a changed scoring distance must move the data-set version"
        );
    }

    // A clinical consumer stores a hash it cannot check. Publishing both
    // preimage inputs beside it makes the stored value verifiable. This
    // assertion fails whenever a third input enters the preimage.
    #[test]
    fn status_publishes_a_recomputable_data_set_version() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let response = request(&address, "GET", "/v1/status", "");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        let status: Value = serde_json::from_slice(response_body(&response)).expect("status JSON");
        let software_version = status["version"].as_str().expect("status version");
        let runtime_profile_id = published(&status, "runtime_profile_id");
        let preimage = format!(
            "{{\"runtime_profile_id\":\"{runtime_profile_id}\",\
             \"schema\":\"pangopup.scoring-data-set-version.v1\",\
             \"software_version\":\"{software_version}\"}}"
        );
        assert_eq!(
            published(&status, "data_set_version"),
            format!("sha256:{:x}", Sha256::digest(preimage.as_bytes())),
            "a consumer must be able to recompute the data-set version from published values"
        );
    }

    fn provenance_fields(item: &Value) -> BTreeSet<String> {
        item["provenance"]
            .as_object()
            .unwrap_or_else(|| panic!("item must carry provenance: {item}"))
            .keys()
            .cloned()
            .collect()
    }

    fn field_set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    // The two routes pin to different depths in one response. A precomputed
    // item names the published dataset it came from. A modeled item names the
    // model run that produced it. One scoring semantics covers both routes. A
    // precomputed item carries no field for it. The status response therefore
    // reports it once for the whole service.
    #[test]
    fn each_route_reports_the_provenance_its_answer_used() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let status = request(&address, "GET", "/v1/status", "");
        let scored = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr1:5051:A:C"]}"#,
        );
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        assert!(
            scored.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&scored)
        );
        let status: Value = serde_json::from_slice(response_body(&status)).expect("status JSON");
        let value: Value = serde_json::from_slice(response_body(&scored)).expect("score JSON");
        let precomputed = &value["results"][0];
        let modeled = &value["results"][1];
        assert_eq!(precomputed["provenance"]["kind"], "precomputed");
        assert_eq!(modeled["provenance"]["kind"], "model");
        assert_eq!(
            provenance_fields(precomputed),
            field_set(&[
                "kind",
                "bundle_id",
                "source_doi",
                "source_archive_md5",
                "masked",
                "window",
            ]),
            "the precomputed provenance field set is a published contract"
        );
        assert_eq!(
            provenance_fields(modeled),
            field_set(&[
                "kind",
                "scoring_semantics",
                "model_bundle_id",
                "model_profile",
                "effective_cpu_policy",
                "reference_bundle_id",
                "reference_profile",
                "reference_sequence_set_sha256",
                "mask_bytes",
                "mask_sha256",
                "masked",
                "window",
            ]),
            "the modeled provenance field set is a published contract"
        );
        assert_eq!(
            published(&status, "scoring_semantics"),
            modeled["provenance"]["scoring_semantics"]
                .as_str()
                .expect("modeled scoring semantics"),
            "status must report the semantics a precomputed item does not carry"
        );
    }

    #[test]
    fn real_executable_reports_a_single_model_rejection_as_an_item() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let rejected = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr1:5051:A:TC"]}"#,
        );
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        assert!(
            rejected.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&rejected)
        );
        let value: Value =
            serde_json::from_slice(response_body(&rejected)).expect("rejection JSON");
        assert_eq!(value["results"][0]["input"], "GRCh38:chr1:5051:A:TC");
        assert_eq!(value["results"][0]["status"], "rejected");
        assert_eq!(value["results"][0]["error"]["code"], "MODEL_REJECTED");
        assert_eq!(value["results"][0]["reason"], "unsupported_variant_shape");
    }

    #[test]
    fn real_executable_preserves_valid_result_beside_model_rejection() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let scored = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr1:5051:A:TC"]}"#,
        );
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        assert!(
            scored.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&scored)
        );
        let value: Value = serde_json::from_slice(response_body(&scored)).expect("score JSON");
        assert_eq!(value["results"][0]["input"], "GRCh38:chr12:6801301:G:A");
        assert_eq!(value["results"][0]["status"], "found");
        assert_eq!(value["results"][0]["position"], 6_801_301);
        let scoring_identity = value["results"][0]["scoring_identity"].clone();
        assert_eq!(
            value["results"][1],
            serde_json::json!({
                "input": "GRCh38:chr1:5051:A:TC",
                "assembly": "GRCh38",
                "contig": "chr1",
                "position": 5051,
                "ref": "A",
                "alt": "TC",
                "status": "rejected",
                "records": [],
                "source_reference_ambiguities": [],
                "error": {"code": "MODEL_REJECTED", "message": "scoring failed"},
                "reason": "unsupported_variant_shape",
                "scoring_identity": scoring_identity
            })
        );
        assert!(value["results"][1].get("provenance").is_none());
    }

    #[test]
    fn real_executable_converts_exact_indels_before_routing() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let exact = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr1:INS:5051:5052:C"]}"#,
        );
        let literal = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr1:5051:A:AC"]}"#,
        );
        assert!(exact.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let mut exact: Value = serde_json::from_slice(response_body(&exact)).expect("exact JSON");
        let mut literal: Value =
            serde_json::from_slice(response_body(&literal)).expect("literal JSON");
        assert_eq!(
            exact["results"][0]
                .as_object_mut()
                .expect("exact item")
                .remove("input"),
            Some(serde_json::json!("GRCh38:chr1:INS:5051:5052:C"))
        );
        assert_eq!(
            literal["results"][0]
                .as_object_mut()
                .expect("literal item")
                .remove("input"),
            Some(serde_json::json!("GRCh38:chr1:5051:A:AC"))
        );
        assert_eq!(exact, literal);

        let mixed = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr1:DEL:5052:5052:C"]}"#,
        );
        assert!(mixed.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let value: Value = serde_json::from_slice(response_body(&mixed)).expect("mixed JSON");
        assert_eq!(value["results"][0]["input"], "GRCh38:chr12:6801301:G:A");
        assert_eq!(value["results"][1]["input"], "GRCh38:chr1:DEL:5052:5052:C");
        assert_eq!(value["results"][0]["status"], "found");
        assert_eq!(value["results"][1]["status"], "rejected");
        assert_eq!(value["results"][1]["position"], 5051);
        assert_eq!(value["results"][1]["ref"], "AC");
        assert_eq!(value["results"][1]["alt"], "A");

        let invalid = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr1:DEL:1:1:A"]}"#,
        );
        assert!(invalid.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let value: Value = serde_json::from_slice(response_body(&invalid)).expect("invalid JSON");
        assert_eq!(value["results"][0]["input"], "GRCh38:chr1:DEL:1:1:A");
        assert_eq!(value["results"][0]["status"], "rejected");
        assert_eq!(value["results"][0]["error"]["code"], "INVALID_VARIANT");
        assert_eq!(value["results"][0]["reason"], "invalid_exact_edit_geometry");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
    }

    #[test]
    fn real_executable_second_signal_forces_a_running_model_job() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());
        let (mut child, address) = start(&data, &profile_path);
        let body = format!(
            "{{\"variants\":[{}]}}",
            (0..10)
                .map(|_| "\"GRCh38:chr1:5051:A:AC\"")
                .collect::<Vec<_>>()
                .join(",")
        );
        let scoring_address = address.clone();
        let scoring = thread::spawn(move || request(&scoring_address, "POST", "/v1/score", &body));
        for _ in 0..1_000 {
            let status = request(&address, "GET", "/v1/status", "");
            if status
                .windows(b"\"running\":1".len())
                .any(|w| w == b"\"running\":1")
            {
                break;
            }
            thread::yield_now();
        }
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
        let status = child.wait().expect("forced exit");
        assert_eq!(status.code(), Some(130));
        let _ = scoring.join();
    }

    #[test]
    fn the_command_line_tool_and_the_service_report_the_same_gene_names() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());

        let (mut child, address) = start(&data, &profile_path);
        let response = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A"]}"#,
        );
        assert!(
            response.starts_with(b"HTTP/1.1 200 OK\r\n"),
            "{}",
            String::from_utf8_lossy(&response)
        );
        let scored: Value = serde_json::from_slice(response_body(&response)).expect("score JSON");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("service exit").success());
        let served = &scored["results"][0]["records"][0];

        let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "lookup",
                "--data-dir",
                data.to_str().expect("data"),
                "--variant",
                "GRCh38:chr12:6801301:G:A",
            ])
            .output()
            .expect("run lookup");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let line = String::from_utf8(output.stdout).expect("lookup is UTF-8");
        let line: Value =
            serde_json::from_str(line.lines().next().expect("one result")).expect("lookup JSON");
        let looked_up = &line["records"][0];

        assert_eq!(served["stable_gene"], "ENSG00000010610");
        assert_eq!(looked_up["stable_gene"], "ENSG00000010610");
        assert_eq!(
            served["gene_names"]["symbol"], "CD4",
            "the service names the gene from the index that ships with the build: {scored}"
        );
        assert_eq!(
            looked_up["gene_names"], served["gene_names"],
            "both surfaces read one index and report one name for one accession"
        );
        assert_eq!(looked_up["gene_names"]["source"], "hgnc");
        assert_eq!(looked_up["gene_names"]["hgnc_id"], "HGNC:1678");
    }

    #[test]
    fn a_gene_name_never_needs_an_install_and_never_moves_a_published_identity() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (_profile, profile_path) = install(&data, temp.path());

        // No naming asset is installed under this data root. Names come from
        // the build, so a freshly installed runtime already reports them.
        let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "lookup",
                "--data-dir",
                data.to_str().expect("data"),
                "--variant",
                "GRCh38:chr17:7687427:A:T",
            ])
            .output()
            .expect("run lookup");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let line = String::from_utf8(output.stdout).expect("lookup is UTF-8");
        let result: Value =
            serde_json::from_str(line.lines().next().expect("one result")).expect("lookup JSON");
        assert_eq!(result["records"][0]["gene_names"]["symbol"], "TP53");

        // Installing a naming source is not a step a deployment can take, so
        // the verb that took one is gone.
        let refused = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args(["assets", "naming", "install", "--source", "/dev/null"])
            .output()
            .expect("run assets naming install");
        assert!(
            !refused.status.success(),
            "installing a naming source must no longer be an accepted command"
        );

        // One build carries one naming vintage, so the status response has no
        // installed vintage to report. `status_publishes_a_recomputable_data_set_version`
        // above proves the gene-name index enters no published identity: it
        // recomputes `data_set_version` from the two published inputs and
        // fails whenever a third input joins the preimage.
        let status = status_under_threads(
            &data,
            &profile_path,
            &temp.path().join("identity-cache.sqlite3"),
            "1",
        );
        assert!(
            status.get("naming").is_none(),
            "the status response reports no separately installed naming vintage: {status}"
        );
    }

    #[test]
    fn incompatible_installed_profile_fails_before_listening() {
        let temp = tempfile::tempdir().expect("temp");
        let data = temp.path().join("data");
        let (mut profile, _profile_path) = install(&data, temp.path());
        profile.scoring.cpu_policy = "sequential:2/1".to_owned();
        let incompatible = temp.path().join("incompatible.json");
        fs::write(
            &incompatible,
            canonical_runtime_profile_bytes(&profile).expect("canonical"),
        )
        .expect("write");
        let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                data.to_str().expect("data"),
            ])
            .env("PANGOPUP_SERVICE_TEST_PROFILE", incompatible)
            .output()
            .expect("run");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("PROFILE_INCOMPATIBLE"));
    }
}

#[cfg(unix)]
mod retained_production {
    use serde_json::Value;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpStream,
        path::Path,
        process::{Child, Command, Stdio},
    };

    fn request(address: &str, method: &str, path: &str, body: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).expect("connect");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\n",
            body.len()
        )
        .expect("write");
        // The service rejects a scoring request that declares no media type.
        if path == "/v1/score" {
            write!(stream, "Content-Type: application/json\r\n").expect("write content type");
        }
        write!(stream, "Connection: close\r\n\r\n{body}").expect("write body");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read");
        response
    }

    fn response_body(response: &[u8]) -> &[u8] {
        let split = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP separator");
        &response[split + 4..]
    }

    fn start_retained(data: &Path, cache: &Path, workers: &str, threads: &str) -> (Child, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                data.to_str().expect("data"),
                "--model-cache",
                cache.to_str().expect("cache"),
                "--model-workers",
                workers,
                "--model-threads",
                threads,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().expect("stdout"))
            .read_line(&mut line)
            .expect("listening");
        let event: Value = serde_json::from_str(&line).expect("event");
        (
            child,
            event["address"].as_str().expect("address").to_owned(),
        )
    }

    fn comparable_items(response: &[u8]) -> Vec<Value> {
        let value: Value = serde_json::from_slice(response_body(response)).expect("score JSON");
        value["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|item| {
                let mut item = item.clone();
                item.as_object_mut()
                    .expect("item")
                    .remove("scoring_identity");
                if let Some(provenance) = item["provenance"].as_object_mut() {
                    provenance.remove("effective_cpu_policy");
                }
                item
            })
            .collect()
    }

    // The published contract says a deployment's worker and thread settings
    // move no score. The gate runs that claim against the miniature graph. The
    // miniature graph's arithmetic cannot reorder. Only the production model can
    // reorder a float sum. The production proof therefore lives here and a
    // maintainer runs it against the retained assets.
    #[test]
    #[ignore = "requires retained qualified production assets"]
    fn retained_assets_score_identically_under_two_cpu_policies() {
        let data =
            std::env::var_os("PANGOPUP_RETAINED_DATA_DIR").expect("set PANGOPUP_RETAINED_DATA_DIR");
        let data = Path::new(&data);
        let temp = tempfile::tempdir().expect("temp");
        std::fs::set_permissions(
            temp.path(),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .expect("private cache");
        // Variants that reach the model and carry a non-zero gain or loss, so
        // the comparison has values in it rather than a column of 0.00.
        let body = r#"{"variants":["GRCh38:chr12:6801303:G:GA","GRCh38:chr17:7687421:G:GACG","GRCh38:chr12:6801303:GG:AC","GRCh38:chr17:7687421:GCCC:ATTA","GRCh38:chr12:6801303:GG:G","GRCh38:chr17:7687421:GCCC:G","GRCh38:chr17:INS:7669673:7669674:A","GRCh38:chr17:INS:7676038:7676039:A"]}"#;
        let mut policies = Vec::new();
        let mut scored = Vec::new();
        for (index, (workers, threads)) in [("1", "1"), ("2", "4")].into_iter().enumerate() {
            let cache = temp.path().join(format!("policy-cache-{index}.sqlite3"));
            let (mut child, address) = start_retained(data, &cache, workers, threads);
            let response = request(&address, "POST", "/v1/score", body);
            assert!(
                response.starts_with(b"HTTP/1.1 200 OK\r\n"),
                "{}",
                String::from_utf8_lossy(&response)
            );
            let value: Value =
                serde_json::from_slice(response_body(&response)).expect("score JSON");
            let results = value["results"].as_array().expect("results");
            assert_eq!(results.len(), 8);
            assert!(
                results
                    .iter()
                    .all(|item| item["provenance"]["kind"] == "model"),
                "every compared item must come from the model: {value}"
            );
            assert!(
                results.iter().any(|item| {
                    item["records"].as_array().is_some_and(|records| {
                        records.iter().any(|record| {
                            record["gain_score"] != "0.00" || record["loss_score"] != "0.00"
                        })
                    })
                }),
                "the compared batch must carry a non-zero score: {value}"
            );
            policies.push(
                results[0]["provenance"]["effective_cpu_policy"]
                    .as_str()
                    .expect("effective CPU policy")
                    .to_owned(),
            );
            scored.push(comparable_items(&response));
            assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
            assert!(child.wait().expect("service exit").success());
        }
        assert_ne!(
            policies[0], policies[1],
            "the two runs must report different effective CPU policies"
        );
        assert_eq!(
            scored[0], scored[1],
            "the production model must return the same score under either CPU policy"
        );
    }

    #[test]
    #[ignore = "requires retained qualified production assets"]
    fn retained_assets_serve_all_routes_and_order_lookup_then_m09_model() {
        let data =
            std::env::var_os("PANGOPUP_RETAINED_DATA_DIR").expect("set PANGOPUP_RETAINED_DATA_DIR");
        let cache = tempfile::tempdir().expect("cache");
        std::fs::set_permissions(
            cache.path(),
            <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
        )
        .expect("private cache");
        let mut child = Command::new(env!("CARGO_BIN_EXE_pangopup"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--data-dir",
                Path::new(&data).to_str().expect("data"),
                "--model-cache",
                cache.path().join("model.sqlite3").to_str().expect("cache"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn");
        let mut line = String::new();
        BufReader::new(child.stdout.as_mut().expect("stdout"))
            .read_line(&mut line)
            .expect("listening");
        let event: Value = serde_json::from_str(&line).expect("event");
        let address = event["address"].as_str().expect("address");
        for path in ["/livez", "/readyz", "/v1/status"] {
            assert!(request(address, "GET", path, "").starts_with(b"HTTP/1.1 200 OK\r\n"));
        }
        let scored = request(
            address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A","GRCh38:chr12:6801303:G:GA"]}"#,
        );
        assert!(scored.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let value: Value = serde_json::from_slice(response_body(&scored)).expect("score JSON");
        assert_eq!(value["results"][0]["position"], 6_801_301);
        assert_eq!(value["results"][0]["provenance"]["kind"], "precomputed");
        assert_eq!(value["results"][1]["position"], 6_801_303);
        assert_eq!(value["results"][1]["provenance"]["kind"], "model");
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        assert!(child.wait().expect("exit").success());
    }
}

#[test]
fn missing_assets_fail_before_listener_and_direct_user_to_sync() {
    let temp = tempfile::tempdir().expect("temp");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("private temp");
    let output = Command::new(env!("CARGO_BIN_EXE_pangopup"))
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--data-dir",
            temp.path().to_str().expect("UTF-8"),
        ])
        .output()
        .expect("run service");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "must fail before listening event");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(stderr.contains("ASSETS_MISSING"));
    assert!(stderr.contains("run pangopup sync"));
}
