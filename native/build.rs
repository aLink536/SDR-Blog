fn main() {
    println!("cargo:rerun-if-changed=assets/radio-log.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winres::WindowsResource::new()
            .set_icon("assets/radio-log.ico")
            .set("ProductName", "Radio Log")
            .set(
                "FileDescription",
                "Radio Log - SDR and amateur radio logbook",
            )
            .compile()
            .expect("Could not embed the Windows app icon");
    }
}
