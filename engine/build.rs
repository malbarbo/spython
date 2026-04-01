use std::process::Command;
use std::time::SystemTime;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_secs();
        let (y, m, d) = civil_from_days((secs / 86400) as i64);
        format!("{y:04}-{m:02}-{d:02}")
    };

    let libs_version = "rustpython 0.5.0, ty/ruff 0.15.6";
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let long_version = format!("{pkg_version} ({libs_version}, {build_date}, {git_hash})");

    println!("cargo::rustc-env=BUILD_DATE={build_date}");
    println!("cargo::rustc-env=GIT_HASH={git_hash}");
    println!("cargo::rustc-env=LIBS_VERSION={libs_version}");
    println!("cargo::rustc-env=LONG_VERSION={long_version}");

    println!("cargo::rerun-if-changed=../.git/HEAD");
    println!("cargo::rerun-if-changed=../.git/refs/");
    println!("cargo::rerun-if-changed=../.git/packed-refs");
}

/// Convert days since 1970-01-01 to (year, month, day).
/// Algorithm from Howard Hinnant.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
