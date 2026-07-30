use std::path::Path;

fn main() {
    // `bundle.externalBin` makes the CLI a sidecar so that `Tome.app` ships the
    // app and `tome` from one build (S4-9). Tauri resolves it as a resource
    // path and fails with `resource path ... doesn't exist`, which says nothing
    // about how to fix it. Fail earlier, and name the command.
    let triple = std::env::var("TARGET").unwrap_or_default();
    let sidecar = format!("binaries/tome-{triple}");
    if !Path::new(&sidecar).exists() {
        panic!(
            "the `tome` CLI sidecar is not staged: {sidecar} is missing.\n\
             \n\
             The app bundle ships the CLI at Contents/MacOS/tome, so the binary has\n\
             to exist before the app is compiled. Build it:\n\
             \n\
             \x20   TOME_CLI_PROFILE=debug ./scripts/build-cli-sidecar.sh\n\
             \n\
             `./scripts/check.sh` and `beforeBuildCommand` both do this for you;\n\
             a bare `cargo build -p tome-app` in a fresh clone does not."
        );
    }
    // A sidecar staged once and never restaged is the failure this whole
    // arrangement exists to prevent: the bundle would look correct and ship a
    // CLI from some earlier tree. Cargo cannot see into the copy, so tell it.
    println!("cargo:rerun-if-changed={sidecar}");

    tauri_build::build()
}
