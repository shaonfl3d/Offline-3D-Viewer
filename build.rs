fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "3D Viewer");
        res.set("CompanyName", "Azizul Haque Shaon");
        res.compile().unwrap();
    }
}