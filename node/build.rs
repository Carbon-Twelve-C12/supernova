use vergen::EmitBuilder;

fn main() {
    // Generate build information
    EmitBuilder::builder()
        .build_timestamp()
        .git_sha(true)
        .rustc_semver()
        .emit()
        .expect("Failed to generate build information");
} 