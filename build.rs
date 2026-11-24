fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let ico_path = manifest_dir.join("assets").join("soundcloud.ico");
        let rc_path = out_dir.join("soundcloud.rc");
        let res_path = out_dir.join("soundcloud.res");
        let obj_path = out_dir.join("soundcloud_res.o");

        let rc_content = format!("1 ICON \"{}\"\n", ico_path.to_string_lossy().replace('\\', "/"));
        let _ = std::fs::write(&rc_path, rc_content);

        // Try windres first
        let windres_status = std::process::Command::new("x86_64-w64-mingw32-windres")
            .arg("-i")
            .arg(&rc_path)
            .arg("-O")
            .arg("coff")
            .arg("-o")
            .arg(&obj_path)
            .status();

        let mut compiled = false;
        if let Ok(s) = windres_status {
            if s.success() {
                compiled = true;
            }
        }

        if !compiled {
            // Fallback to llvm-rc and llvm-cvtres
            let rc_status = std::process::Command::new("llvm-rc")
                .arg("/fo")
                .arg(&res_path)
                .arg(&rc_path)
                .status();

            if let Ok(s) = rc_status {
                if s.success() {
                    let cvt_status = std::process::Command::new("llvm-cvtres")
                        .arg(format!("/out:{}", obj_path.to_string_lossy()))
                        .arg("/machine:X64")
                        .arg(&res_path)
                        .status();
                    if let Ok(s2) = cvt_status {
                        if s2.success() {
                            compiled = true;
                        }
                    }
                }
            }
        }

        if compiled {
            println!("cargo:rustc-link-arg={}", obj_path.to_string_lossy());
        }

        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rerun-if-changed=assets/soundcloud.ico");
    }
}
