use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icons/yazi-gui.rc");
    println!("cargo:rerun-if-changed=assets/icons/yazi-gui.ico");
    println!("cargo:rerun-if-changed=assets/icons/yazi-gui.svg");
    println!("cargo:rerun-if-changed=assets/icons/ui");
    embed_resource::compile("assets/icons/yazi-gui.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();

    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let out_dir =
            PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo did not provide OUT_DIR"));
        let target_profile = out_dir
            .parent()
            .and_then(|path| path.parent())
            .and_then(|path| path.parent())
            .expect("Cargo OUT_DIR is not inside a profile directory");
        let destination = target_profile.join("assets/icons/yazi-gui.svg");
        std::fs::create_dir_all(
            destination
                .parent()
                .expect("icon destination has no parent directory"),
        )
        .expect("failed to create the release icon directory");
        std::fs::copy("assets/icons/yazi-gui.svg", &destination)
            .expect("failed to stage the release SVG icon");
    }
}
