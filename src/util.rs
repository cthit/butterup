use std::path::Path;
use std::time::Duration;

pub fn format_duration(d: Duration) -> String {
    let seconds = d.as_secs_f32() % 60.0;
    let minutes = d.as_secs() / 60 % 60;
    let hours = d.as_secs() / 60 / 60;

    match (hours, minutes) {
        (0, 0) => format!("{:.2}s", seconds),
        (0, _) => format!("{}m {:.2}s", minutes, seconds),
        (_, 0) => format!("{}h {:.2}s", hours, seconds),
        (_, _) => format!("{}h {}m {:.2}s", hours, minutes, seconds),
    }
}

pub fn path_as_utf8(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::format_err!("path not utf-8"))
}
