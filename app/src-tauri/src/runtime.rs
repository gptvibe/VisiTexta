pub(crate) fn hydrate_path_for_binaries() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let mut bin_candidates = vec![dir.join("bin"), dir.join("resources").join("bin")];
            if let Some(parent) = dir.parent() {
                bin_candidates.push(parent.join("bin"));
            }

            let mut first_bin: Option<std::path::PathBuf> = None;

            for cand in &bin_candidates {
                if cand.exists() {
                    if first_bin.is_none() {
                        first_bin = Some(cand.clone());
                    }
                    if let Some(cand_str) = cand.to_str() {
                        if let Some(path) = std::env::var_os("PATH") {
                            let mut new_path = std::ffi::OsString::from(cand_str);
                            new_path.push(";");
                            new_path.push(path);
                            std::env::set_var("PATH", new_path);
                        } else {
                            std::env::set_var("PATH", cand_str);
                        }
                    }
                }
            }

            #[cfg(target_os = "windows")]
            if let Some(ref bin_dir) = first_bin {
                win_set_dll_directory(bin_dir);
                win_preload_dll(bin_dir, "pdfium.dll");
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn win_set_dll_directory(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe { SetDllDirectoryW(wide.as_ptr()) };
}

#[cfg(target_os = "windows")]
fn win_preload_dll(bin_dir: &std::path::Path, dll_name: &str) {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
    }

    let dll_path = bin_dir.join(dll_name);
    if !dll_path.exists() {
        return;
    }

    let wide: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe { LoadLibraryW(wide.as_ptr()) };
}
