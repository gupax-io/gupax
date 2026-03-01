// This [build.rs] is for setting Windows icons.
// The icon in [File Explorer] gets set here.
// The icon in the taskbar and top of the App window gets
// set in [src/main.rs, src/constants.rs] at runtime with
// pre-compiled bytes using [include_bytes!()] on the images in [images/].
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    set_commit_env();

    static_vcruntime::metabuild();
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/images/icons/icon.ico");
    res.compile()
}

#[cfg(unix)]
fn main() {
    set_commit_env();
}

// Set the current git commit to the env var [COMMIT].
fn set_commit_env() {
    println!("cargo:rerun-if-changed=.git/refs/heads/");

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();

    let commit = String::from_utf8(output.stdout).unwrap();

    assert!(commit.len() >= 40);

    println!("cargo:rustc-env=COMMIT={commit}");
}
