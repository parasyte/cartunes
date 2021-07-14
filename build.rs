fn main() {
    #[cfg(windows)]
    embed_resource::compile("assets/cartunes.rc");
}
