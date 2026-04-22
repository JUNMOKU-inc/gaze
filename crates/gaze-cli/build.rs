fn main() {
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg-bin=gaze=-Wl,-rpath,/usr/lib/swift");
}
