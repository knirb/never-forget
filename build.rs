fn main() {
    // Link required macOS frameworks
    println!("cargo:rustc-link-lib=framework=EventKit");
    println!("cargo:rustc-link-lib=framework=AppKit");
}
