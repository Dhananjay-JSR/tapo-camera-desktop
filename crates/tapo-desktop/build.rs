fn main() {
    // Embed the app icon into the Windows .exe. No-op on other platforms.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/icon.ico"
        ));
        let _ = res.compile();
    }
}
