fn main() {
    println!("cargo:rerun-if-changed=assets/icons/yazi-gui.rc");
    println!("cargo:rerun-if-changed=assets/icons/yazi-gui.ico");
    embed_resource::compile("assets/icons/yazi-gui.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
