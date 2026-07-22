fn main() {
    println!("cargo:rerun-if-env-changed=ZSTD_SYS_USE_PKG_CONFIG");
    if std::env::var_os("ZSTD_SYS_USE_PKG_CONFIG").is_some() {
        panic!("Pangopup requires bundled libzstd; unset ZSTD_SYS_USE_PKG_CONFIG");
    }
}
