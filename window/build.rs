//! Compiles the window's .slint files into Rust, with the digit font inside.

fn main() {
    let config = slint_build::CompilerConfiguration::new().embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    if let Err(error) = slint_build::compile_with_config("ui/app.slint", config) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
