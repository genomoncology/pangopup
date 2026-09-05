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
        let mask = fixture("route-mask/domains.pgm");
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
                member_bytes: fs::metadata(&mask).expect("mask").len(),
                member_sha256: digest(&mask),
            },
            scoring: ScoringProfile {
                assembly: "GRCh38".to_owned(),
                semantics: "pangopup-variant-score-v1".to_owned(),
                distance: 50,
                masking_policy: "pangolin-gencode-v38-order-sensitive-v1".to_owned(),
                cpu_policy: "sequential:1/1".to_owned(),
            },
        };
        install_test_runtime_profile(&profile, &model, &reference, &mask, root)
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

    fn request(address: &str, method: &str, path: &str, body: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).expect("connect");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write");
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
        let lookup = request(
            &address,
            "POST",
            "/v1/score",
            r#"{"variants":["GRCh38:chr12:6801301:G:A"]}"#,
        );
        assert!(lookup.starts_with(b"HTTP/1.1 200 OK\r\n"));

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
        assert!(child.wait().expect("service exit").success());
    }

    #[test]
    fn real_executable_answers_model_rejection_with_422() {
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
            rejected.starts_with(b"HTTP/1.1 422 Unprocessable Entity\r\n"),
            "{}",
            String::from_utf8_lossy(&rejected)
        );
        assert_eq!(
            response_body(&rejected),
            b"{\"error\":{\"code\":\"MODEL_REJECTED\",\"message\":\"scoring failed\"}}\n"
        );
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
        process::{Command, Stdio},
    };

    fn request(address: &str, method: &str, path: &str, body: &str) -> Vec<u8> {
        let mut stream = TcpStream::connect(address).expect("connect");
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write");
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
