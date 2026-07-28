use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu")
    {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let object = out.join("host_name_shim.o");
    let compiler = env::var("CC_i686_pc_windows_gnu")
        .or_else(|_| env::var("CC_i686_PC_WINDOWS_GNU"))
        .unwrap_or_else(|_| "i686-w64-mingw32-gcc".to_owned());
    let status = Command::new(&compiler)
        .args(["-c", "host_name_shim.c", "-o"])
        .arg(&object)
        .status()
        .expect("invoke MinGW C compiler");
    assert!(
        status.success(),
        "{compiler} failed compiling host_name_shim.c"
    );
    println!("cargo:rustc-link-arg={}", object.display());
    println!("cargo:rerun-if-changed=host_name_shim.c");
}
