fn main() {
    let rp235xa = cfg!(feature = "rp235xa");
    let rp235xb = cfg!(feature = "rp235xb");

    let count = [rp235xa, rp235xb]
        .iter()
        .filter(|&&x| x)
        .count();

    if count == 0 {
        panic!(
            "\n[hazard3-hall-analog] No target selected.\n\
             Enable exactly one feature: rp235xa or rp235xb.\n"
        );
    }

    if count > 1 {
        panic!(
            "\n[hazard3-hall-analog] Multiple targets selected simultaneously.\n\
             These features are mutually exclusive: rp235xa, rp235xb.\n\
             Disable default features and enable only one.\n\
             Example: embassy-hall-analog = {{ version = \"0.4.0\", \
             default-features = false, features = [\"rp235xb\"] }}\n"
        );
    }

    if rp235xa { println!("cargo:rustc-cfg=hall_target=\"rp235xa\""); }
    if rp235xb { println!("cargo:rustc-cfg=hall_target=\"rp235xb\""); }
}