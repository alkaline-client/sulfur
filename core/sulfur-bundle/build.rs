//! Gives the Windows executable Sulfur's icon and version information.

fn main() {
    println!("cargo:rerun-if-changed=../../app/icons/sulfur.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../app/icons/sulfur.ico")
        .set("ProductName", "Sulfur")
        .set("FileDescription", "Sulfur")
        .set("InternalName", "Sulfur");
    if let Err(error) = resource.compile() {
        // A missing resource compiler only costs the icon.
        println!("cargo:warning=no icon for the executable: {error}");
    }
}
