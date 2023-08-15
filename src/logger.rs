pub use log::{debug, error, info, warn};

const ENVKEY_RUST_LOG: &str = "RUST_LOG";

pub fn init_logger() {
    if std::env::var(ENVKEY_RUST_LOG).is_err() {
        #[cfg(debug_assertions)]
        std::env::set_var(ENVKEY_RUST_LOG, "debug");
        #[cfg(not(debug_assertions))]
        std::env::set_var(ENVKEY_RUST_LOG, "info");
    }

    env_logger::builder()
        .default_format()
        .format_timestamp_millis()
        .format_indent(None)
        .init();
}
