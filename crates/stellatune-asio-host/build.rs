fn main() {
    // `asio-sys` (pulled by `cpal/asio`) uses Win32 Registry APIs to enumerate drivers.
    // On MSVC targets those symbols live in `advapi32.lib`, but it may not be linked
    // automatically depending on the build graph.
    #[cfg(all(windows, feature = "asio"))]
    println!("cargo:rustc-link-lib=advapi32");
}
