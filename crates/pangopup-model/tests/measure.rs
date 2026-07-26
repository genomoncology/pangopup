use pangopup_model::{ModelContext, ModelKernel, Strand};
use std::{
    env, fs,
    hint::black_box,
    path::Path,
    time::{Duration, Instant},
};

const WARMUPS: usize = 3;
const SAMPLES: usize = 20;

fn patterned_context(length: usize) -> ModelContext {
    const BASES: &[u8] = b"ACGTN";
    let sequence = (0..length)
        .map(|index| BASES[index % BASES.len()])
        .collect::<Vec<_>>();
    ModelContext::new(sequence).expect("valid deterministic context")
}

fn percentile(samples: &mut [Duration], numerator: usize, denominator: usize) -> Duration {
    samples.sort_unstable();
    let rank = samples
        .len()
        .saturating_mul(numerator)
        .div_ceil(denominator)
        .saturating_sub(1);
    samples[rank.min(samples.len() - 1)]
}

fn vm_hwm_kib() -> u64 {
    fs::read_to_string("/proc/self/status")
        .expect("read Linux process status")
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")
                .and_then(|value| value.split_ascii_whitespace().next())
                .and_then(|value| value.parse().ok())
        })
        .expect("VmHWM in Linux process status")
}

fn cpu_allow_list() -> String {
    fs::read_to_string("/proc/self/status")
        .expect("read Linux process status")
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .map(str::trim)
        .map(str::to_owned)
        .expect("Cpus_allowed_list in Linux process status")
}

fn measure(kernel: &mut ModelKernel, context: &ModelContext, strand: Strand) -> (u128, u128) {
    for _ in 0..WARMUPS {
        black_box(kernel.infer(context, strand).expect("warmup inference"));
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        black_box(kernel.infer(context, strand).expect("timed inference"));
        samples.push(started.elapsed());
    }
    let p50 = percentile(&mut samples, 50, 100);
    let p95 = percentile(&mut samples, 95, 100);
    (p50.as_nanos(), p95.as_nanos())
}

#[test]
#[ignore = "maintainer-only production-bundle measurement"]
fn cpu_kernel_release_measurement() {
    let bundle = env::var_os("PANGOPUP_MODEL_BUNDLE")
        .expect("set PANGOPUP_MODEL_BUNDLE to an authenticated production bundle");
    let opened = Instant::now();
    let mut kernel = ModelKernel::open(Path::new(&bundle)).expect("open production model bundle");
    let open_ns = opened.elapsed().as_nanos();

    let minimum = patterned_context(10_101);
    let maximum = patterned_context(10_200);
    let (minimum_p50_ns, minimum_p95_ns) = measure(&mut kernel, &minimum, Strand::Plus);
    let (maximum_p50_ns, maximum_p95_ns) = measure(&mut kernel, &maximum, Strand::Plus);

    println!(
        "{{\"affinity\":\"{}\",\"bundle_id\":\"{}\",\"context\":\"repeating-ACGTN-plus\",\
         \"maximum_rss_kib\":{},\"open_ns\":{},\"samples\":{},\"warmups\":{},\
         \"n10101_p50_ns\":{},\"n10101_p95_ns\":{},\"n10200_p50_ns\":{},\
         \"n10200_p95_ns\":{}}}",
        cpu_allow_list(),
        kernel.bundle_identity(),
        vm_hwm_kib(),
        open_ns,
        SAMPLES,
        WARMUPS,
        minimum_p50_ns,
        minimum_p95_ns,
        maximum_p50_ns,
        maximum_p95_ns,
    );
}
