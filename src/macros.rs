macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION");
    };
}
