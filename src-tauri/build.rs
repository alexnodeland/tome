// Build script for Tauri
// This runs before the main compilation and can be used for:
// - Generating code
// - Compiling native dependencies
// - Setting up environment variables

fn main() {
    // Tauri build step - required for Tauri to work correctly
    tauri_build::build();

    // Re-run this script if these files change
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=Cargo.toml");

    // You can add additional build-time checks here
    // For example, verifying that required environment variables are set:
    //
    // if std::env::var("SOME_REQUIRED_VAR").is_err() {
    //     panic!("SOME_REQUIRED_VAR environment variable must be set");
    // }
}
