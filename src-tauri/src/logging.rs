//! Local file logging. Logs never contain conversation content by default:
//! call sites log events, error details and timings only.

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init(log_dir: &Path) -> Option<WorkerGuard> {
    if std::fs::create_dir_all(log_dir).is_err() {
        return None;
    }
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("local-assistant")
        .filename_suffix("log")
        .max_log_files(7)
        .build(log_dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_new(
        std::env::var("LOCAL_ASSISTANT_LOG").unwrap_or_else(|_| "info".into()),
    )
    .unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = fmt::layer().with_writer(writer).with_ansi(false).with_target(true);
    let registry = tracing_subscriber::registry().with(filter).with(file_layer);
    #[cfg(debug_assertions)]
    let registry = registry.with(fmt::layer().with_target(true));
    let _ = registry.try_init();
    Some(guard)
}
