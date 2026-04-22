use std::process::Command;

fn main() {
    // Add rpath for Swift runtime libraries so that libswift_Concurrency.dylib
    // (and other Swift runtime dylibs) can be found at runtime.
    //
    // The `screencapturekit` crate's build.rs emits these same flags, but
    // `cargo:rustc-link-arg` does NOT propagate from dependency crates to the
    // final binary (see https://github.com/rust-lang/cargo/issues/9554).
    // We must emit them here, in the *binary* crate's build script.
    #[cfg(target_os = "macos")]
    {
        // System Swift runtime (macOS 12.0+)
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        // Xcode toolchain Swift runtime (needed for Swift Concurrency on older macOS
        // or when the system copy is not present)
        if let Ok(output) = Command::new("xcode-select").arg("-p").output() {
            if output.status.success() {
                let xcode_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

                // swift-5.5 path (where libswift_Concurrency.dylib typically lives)
                println!(
                    "cargo:rustc-link-arg=-Wl,-rpath,{}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx",
                    xcode_path
                );
                // Standard swift/macosx path
                println!(
                    "cargo:rustc-link-arg=-Wl,-rpath,{}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx",
                    xcode_path
                );
            }
        }
    }

    tauri_build::build()
}
