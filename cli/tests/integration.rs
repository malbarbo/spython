//! What the binary prints for every file under `tests/inputs` and
//! `tests/images`, one test per file. The list is written by `build.rs`, which
//! reads the directories at build time: a single test walking them would run
//! them in series, and the files are what there is most of.

use assert_cmd::cargo;
use insta::{Settings, assert_snapshot};
use std::process::Command;

/// The binary runs from the directory of the file it is given, and takes only
/// its name, so a path in a diagnostic is the one a user of that file sees and
/// says nothing of where these tests keep it.
fn output_of(level: u8, file: &str) -> (String, String) {
    let (dir, name) = file.rsplit_once('/').expect("a file in a directory");
    let out = Command::new(cargo::cargo_bin!("spython"))
        // A FORCE_COLOR the machine set for something else would put escape
        // codes in every snapshot here.
        .env_remove("FORCE_COLOR")
        .current_dir(format!("{}/{dir}", env!("CARGO_MANIFEST_DIR")))
        .args(["run", "--level", &level.to_string(), name])
        .output()
        .expect("run spython");
    let text = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .replace('\r', "")
            .replace('\\', "/")
    };
    (text(&out.stdout), text(&out.stderr))
}

/// The snapshot is named after what the file is run for and the file it was run
/// on, the way `glob!` named it: `run_file@ok_simple.py`.
fn bind(file: &str) -> impl Drop {
    let mut settings = Settings::clone_current();
    settings.set_snapshot_suffix(file.rsplit('/').next().expect("a file name"));
    settings.set_omit_expression(true);
    settings.bind_to_scope()
}

fn check(snapshot: &str, level: u8, file: &str) {
    let _guard = bind(file);
    let (out, err) = output_of(level, file);
    assert_snapshot!(snapshot, format!("STDOUT\n{out}STDERR\n{err}"));
}

/// An image is the whole of what its file prints, and nothing around it.
fn check_image(snapshot: &str, level: u8, file: &str) {
    let _guard = bind(file);
    let (out, _) = output_of(level, file);
    assert_snapshot!(snapshot, out);
}

include!(concat!(env!("OUT_DIR"), "/file_tests.rs"));
