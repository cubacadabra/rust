use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/logs/HEAD");

    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR");
    let sha = manifest_dir
        .and_then(|directory| {
            Command::new("git")
                .args(["rev-parse", "--short=8", "HEAD"])
                .current_dir(directory)
                .output()
                .ok()
        })
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_owned())
        .filter(|sha| sha.len() == 8 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .unwrap_or_else(|| "UNKNOWN".to_owned());

    println!("cargo:rustc-env=CUBACADABRA_GIT_SHA={sha}");
}
