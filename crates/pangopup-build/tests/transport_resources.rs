use flate2::{Compression, write::GzEncoder};
use pangopup_assets::{pack_bundle, unpack_transport, verify_transport};
use pangopup_build::build_bundle;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

struct TrackingAllocator;
static CURRENT: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);

fn add(bytes: usize) {
    let current = CURRENT.fetch_add(bytes as u64, Ordering::SeqCst) + bytes as u64;
    PEAK.fetch_max(current, Ordering::SeqCst);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The unchanged allocation request is delegated to System.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            add(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        CURRENT.fetch_sub(layout.size() as u64, Ordering::SeqCst);
        // SAFETY: The pair was supplied by the caller for this allocation.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The unchanged pair and requested new size are delegated.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            if new_size >= layout.size() {
                add(new_size - layout.size());
            } else {
                CURRENT.fetch_sub((layout.size() - new_size) as u64, Ordering::SeqCst);
            }
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("pangopup-resources-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("temporary directory");
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove temporary directory");
    }
}

fn large_bundle(root: &Path) -> PathBuf {
    const LOCI: u32 = 100_000;
    let source = root.join("source");
    fs::create_dir(&source).expect("source directory");
    let file = File::create(source.join("ENSG00000000001.tsv.gz")).expect("source member");
    let mut gzip = GzEncoder::new(file, Compression::best());
    writeln!(
        gzip,
        "chrom\tpos\tref\talt\tgain_score\tgain_pos\tloss_score\tloss_pos"
    )
    .expect("header");
    for position in 1..=LOCI {
        for alternate in ['C', 'G', 'T'] {
            writeln!(
                gzip,
                "chr1\t{position}\tA\t{alternate}\t0.0\t-50\t-0.0\t-50"
            )
            .expect("source row");
        }
    }
    gzip.finish().expect("finish source");
    let reference = root.join("reference.fa");
    let mut fasta = File::create(&reference).expect("reference");
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
    for accession in accessions {
        writeln!(fasta, ">{accession}").expect("FASTA header");
        if accession == "NC_000001.11" {
            for _ in 0..LOCI {
                fasta.write_all(b"A").expect("sequence");
            }
            writeln!(fasta).expect("sequence line");
        } else {
            writeln!(fasta, "A").expect("short sequence");
        }
    }
    let bundle = root.join("bundle");
    build_bundle(&source, &reference, &bundle).expect("build large fixture");
    bundle
}

#[cfg(target_os = "linux")]
fn fd_count() -> u64 {
    fs::read_dir("/proc/self/fd")
        .expect("file descriptors")
        .count() as u64
}
#[cfg(not(target_os = "linux"))]
fn fd_count() -> u64 {
    0
}

#[cfg(target_os = "linux")]
fn rss_bytes() -> u64 {
    let text = fs::read_to_string("/proc/self/statm").expect("statm");
    let pages: u64 = text
        .split_ascii_whitespace()
        .nth(1)
        .expect("RSS")
        .parse()
        .expect("numeric RSS");
    // SAFETY: This is a side-effect-free process configuration query.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    assert!(size > 0);
    pages * size as u64
}
#[cfg(not(target_os = "linux"))]
fn rss_bytes() -> u64 {
    0
}

fn measure(operation: impl FnOnce()) -> (u64, u64, u64) {
    let stop = Arc::new(AtomicBool::new(false));
    let peak_fd = Arc::new(AtomicU64::new(fd_count()));
    let peak_rss = Arc::new(AtomicU64::new(rss_bytes()));
    let thread_stop = stop.clone();
    let thread_fd = peak_fd.clone();
    let thread_rss = peak_rss.clone();
    let monitor = thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            thread_fd.fetch_max(fd_count(), Ordering::Relaxed);
            thread_rss.fetch_max(rss_bytes(), Ordering::Relaxed);
            thread::sleep(Duration::from_millis(1));
        }
    });
    let baseline_heap = CURRENT.load(Ordering::SeqCst);
    let baseline_fd = fd_count();
    let baseline_rss = rss_bytes();
    PEAK.store(baseline_heap, Ordering::SeqCst);
    operation();
    stop.store(true, Ordering::Relaxed);
    monitor.join().expect("monitor");
    (
        PEAK.load(Ordering::SeqCst).saturating_sub(baseline_heap),
        peak_fd.load(Ordering::Relaxed).saturating_sub(baseline_fd),
        peak_rss
            .load(Ordering::Relaxed)
            .saturating_sub(baseline_rss),
    )
}

#[test]
fn transport_streaming_resource_subprocess() {
    let Some(mode) = std::env::var_os("PANGOPUP_RESOURCE_MODE") else {
        for mode in ["pack", "verify", "unpack"] {
            let output = Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--exact",
                    "transport_streaming_resource_subprocess",
                    "--nocapture",
                ])
                .env("PANGOPUP_RESOURCE_MODE", mode)
                .output()
                .expect("resource subprocess");
            assert!(
                output.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
        return;
    };
    let mode = mode.to_string_lossy();
    let temp = Temp::new();
    let bundle = large_bundle(&temp.0);
    let transport = temp.0.join("transport");
    if mode != "pack" {
        pack_bundle(&bundle, &transport).expect("prepare transport");
    }
    let output = temp.0.join("unpacked");
    let (heap, fds, rss) = measure(|| match mode.as_ref() {
        "pack" => {
            pack_bundle(&bundle, &transport).expect("measured pack");
        }
        "verify" => {
            verify_transport(&transport).expect("measured verify");
        }
        "unpack" => {
            unpack_transport(&transport, &output).expect("measured unpack");
        }
        _ => panic!("unknown resource mode"),
    });
    assert!(heap <= 16 * 1024 * 1024, "Rust allocator grew by {heap}");
    assert!(fds <= 8, "additional file descriptors {fds}");
    eprintln!(
        "transport-resources mode={mode} rust_allocator_peak_delta={heap} fd_peak_delta={fds} native_inclusive_rss_delta={rss} payload_bytes={}",
        fs::metadata(bundle.join("scores.pgi"))
            .expect("scores metadata")
            .len()
    );
}
