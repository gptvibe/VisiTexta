use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProfile {
    Auto,
    #[default]
    CpuCompatible,
    AcceleratedIfAvailable,
}

impl RuntimeProfile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::CpuCompatible => "CPU compatible",
            Self::AcceleratedIfAvailable => "Accelerated if available",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    CpuCompatible,
    Cuda,
    Directml,
    Vulkan,
    Metal,
    GenericAccelerated,
}

impl RuntimeBackend {
    fn label(self) -> &'static str {
        match self {
            Self::CpuCompatible => "CPU compatible",
            Self::Cuda => "accelerated CUDA",
            Self::Directml => "accelerated DirectML",
            Self::Vulkan => "accelerated Vulkan",
            Self::Metal => "accelerated Metal",
            Self::GenericAccelerated => "accelerated runtime",
        }
    }

    fn is_accelerated(self) -> bool {
        !matches!(self, Self::CpuCompatible)
    }

    fn preference_order(self) -> u8 {
        match self {
            Self::CpuCompatible => 0,
            Self::Cuda => 1,
            Self::Directml => 2,
            Self::Vulkan => 3,
            Self::Metal => 4,
            Self::GenericAccelerated => 5,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct RuntimeStatus {
    pub selected_profile: RuntimeProfile,
    pub safe_default_profile: RuntimeProfile,
    pub usable_runtime: bool,
    pub cpu_runtime_available: bool,
    pub accelerated_runtime_available: bool,
    pub accelerated_runtime_compatible: bool,
    pub accelerated_runtime_label: Option<String>,
    pub effective_runtime_label: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeExecutable {
    pub path: PathBuf,
    pub backend: RuntimeBackend,
    pub label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeExecutionPlan {
    pub selected_profile: RuntimeProfile,
    pub server_runtimes: Vec<RuntimeExecutable>,
    pub cli_runtimes: Vec<RuntimeExecutable>,
    pub status: RuntimeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeBinaryKind {
    Server,
    Cli,
}

#[derive(Debug, Clone)]
struct DiscoveredRuntimeExecutable {
    path: PathBuf,
    backend: RuntimeBackend,
    kind: RuntimeBinaryKind,
    priority: u8,
}

#[derive(Debug, Default, Clone)]
struct RuntimeInventory {
    cpu_server_runtimes: Vec<DiscoveredRuntimeExecutable>,
    cpu_cli_runtimes: Vec<DiscoveredRuntimeExecutable>,
    accelerated_server_runtimes: Vec<DiscoveredRuntimeExecutable>,
    accelerated_cli_runtimes: Vec<DiscoveredRuntimeExecutable>,
}

#[derive(Debug, Clone)]
struct AccelerationGroup {
    backend: RuntimeBackend,
    compatible: bool,
    server_runtimes: Vec<RuntimeExecutable>,
    cli_runtimes: Vec<RuntimeExecutable>,
}

pub(crate) fn default_runtime_profile() -> RuntimeProfile {
    RuntimeProfile::CpuCompatible
}

pub(crate) fn resolve_execution_plan(profile: RuntimeProfile) -> RuntimeExecutionPlan {
    let inventory = scan_runtime_inventory(&runtime_root_candidates());
    build_execution_plan(profile, &inventory)
}

pub fn runtime_status(profile: RuntimeProfile) -> RuntimeStatus {
    resolve_execution_plan(profile).status
}

pub fn runtime_has_ocr_runner(profile: RuntimeProfile) -> bool {
    runtime_status(profile).usable_runtime
}

pub(crate) fn runtime_search_roots() -> Vec<PathBuf> {
    runtime_root_candidates()
}

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

fn build_execution_plan(
    profile: RuntimeProfile,
    inventory: &RuntimeInventory,
) -> RuntimeExecutionPlan {
    let cpu_server_runtimes = inventory
        .cpu_server_runtimes
        .iter()
        .cloned()
        .map(export_runtime_executable)
        .collect::<Vec<_>>();
    let cpu_cli_runtimes = inventory
        .cpu_cli_runtimes
        .iter()
        .cloned()
        .map(export_runtime_executable)
        .collect::<Vec<_>>();
    let acceleration_groups = collect_acceleration_groups(inventory);

    let cpu_runtime_available = !cpu_server_runtimes.is_empty() || !cpu_cli_runtimes.is_empty();
    let accelerated_runtime_available = !acceleration_groups.is_empty();
    let accelerated_runtime_compatible = acceleration_groups.iter().any(|group| group.compatible);
    let best_accelerated_group = acceleration_groups.first().cloned();
    let compatible_accelerated_group = acceleration_groups
        .iter()
        .find(|group| group.compatible)
        .cloned();

    let selected_accelerated_group = match profile {
        RuntimeProfile::CpuCompatible => None,
        RuntimeProfile::Auto => compatible_accelerated_group.clone(),
        RuntimeProfile::AcceleratedIfAvailable => compatible_accelerated_group
            .clone()
            .or(best_accelerated_group.clone()),
    };

    let mut server_runtimes = Vec::new();
    let mut cli_runtimes = Vec::new();

    if let Some(group) = selected_accelerated_group.as_ref() {
        server_runtimes.extend(group.server_runtimes.clone());
        cli_runtimes.extend(group.cli_runtimes.clone());
    }

    let include_cpu_fallback = match profile {
        RuntimeProfile::CpuCompatible => true,
        RuntimeProfile::Auto => cpu_runtime_available,
        RuntimeProfile::AcceleratedIfAvailable => cpu_runtime_available,
    };

    if include_cpu_fallback {
        server_runtimes.extend(cpu_server_runtimes.clone());
        cli_runtimes.extend(cpu_cli_runtimes.clone());
    }

    let usable_runtime = !server_runtimes.is_empty() || !cli_runtimes.is_empty();
    let accelerated_runtime_label = best_accelerated_group
        .as_ref()
        .map(|group| group.backend.label().to_string());

    let effective_runtime_label = if let Some(group) = selected_accelerated_group.as_ref() {
        group.backend.label().to_string()
    } else if cpu_runtime_available {
        RuntimeBackend::CpuCompatible.label().to_string()
    } else {
        "No usable runtime".to_string()
    };

    let summary = build_runtime_summary(
        profile,
        cpu_runtime_available,
        accelerated_runtime_available,
        accelerated_runtime_compatible,
        best_accelerated_group.as_ref(),
        selected_accelerated_group.as_ref(),
    );

    RuntimeExecutionPlan {
        selected_profile: profile,
        server_runtimes,
        cli_runtimes,
        status: RuntimeStatus {
            selected_profile: profile,
            safe_default_profile: default_runtime_profile(),
            usable_runtime,
            cpu_runtime_available,
            accelerated_runtime_available,
            accelerated_runtime_compatible,
            accelerated_runtime_label,
            effective_runtime_label,
            summary,
        },
    }
}

fn build_runtime_summary(
    profile: RuntimeProfile,
    cpu_runtime_available: bool,
    accelerated_runtime_available: bool,
    accelerated_runtime_compatible: bool,
    best_accelerated_group: Option<&AccelerationGroup>,
    selected_accelerated_group: Option<&AccelerationGroup>,
) -> String {
    let accelerated_label = best_accelerated_group
        .map(|group| group.backend.label())
        .unwrap_or("accelerated runtime");

    match profile {
        RuntimeProfile::CpuCompatible => {
            if cpu_runtime_available {
                "CPU-compatible runtime is selected. This is the widest-compatibility local OCR path.".into()
            } else if accelerated_runtime_available {
                format!(
                    "CPU-compatible runtime is selected, but no CPU bundle was found. Add the CPU-compatible runtime to keep the safe default. {accelerated_label} is optional."
                )
            } else {
                "No local OCR runtime was found. Add the bundled CPU-compatible runtime under bin/ or resources/bin.".into()
            }
        }
        RuntimeProfile::Auto => {
            if let Some(group) = selected_accelerated_group {
                if cpu_runtime_available {
                    format!(
                        "Auto can use {} on this PC and still keep the CPU-compatible fallback.",
                        group.backend.label()
                    )
                } else {
                    format!("Auto can use {} on this PC.", group.backend.label())
                }
            } else if accelerated_runtime_available && cpu_runtime_available {
                "Auto is staying on the CPU-compatible runtime because no bundled accelerated build looks compatible yet.".into()
            } else if cpu_runtime_available {
                "Auto is using the CPU-compatible runtime because no accelerated bundle was found."
                    .into()
            } else if accelerated_runtime_available && !accelerated_runtime_compatible {
                "Only accelerated runtimes were found, but none look compatible enough for Auto. Switch to Accelerated if available to try them manually.".into()
            } else {
                "No local OCR runtime was found. Add the bundled CPU-compatible runtime under bin/ or resources/bin.".into()
            }
        }
        RuntimeProfile::AcceleratedIfAvailable => {
            if let Some(group) = selected_accelerated_group {
                if group.compatible && cpu_runtime_available {
                    format!(
                        "{} will be tried first and VisiTexta will fall back to the CPU-compatible runtime if needed.",
                        group.backend.label()
                    )
                } else if group.compatible {
                    format!(
                        "{} will be used. No CPU-compatible fallback is bundled.",
                        group.backend.label()
                    )
                } else if cpu_runtime_available {
                    format!(
                        "{accelerated_label} is bundled, but this PC does not currently advertise that backend. VisiTexta will still try it first and fall back to CPU if startup fails."
                    )
                } else {
                    format!(
                        "{accelerated_label} is bundled, but this PC does not currently advertise that backend and no CPU-compatible fallback is available."
                    )
                }
            } else if cpu_runtime_available {
                "No accelerated bundle was found, so this profile will behave like the CPU-compatible path.".into()
            } else {
                "No local OCR runtime was found. Add the bundled CPU-compatible runtime under bin/ or resources/bin.".into()
            }
        }
    }
}

fn collect_acceleration_groups(inventory: &RuntimeInventory) -> Vec<AccelerationGroup> {
    let mut groups = Vec::new();
    let mut seen = HashSet::new();

    for backend in inventory
        .accelerated_server_runtimes
        .iter()
        .chain(inventory.accelerated_cli_runtimes.iter())
        .map(|runtime| runtime.backend)
        .collect::<Vec<_>>()
    {
        if !seen.insert(backend) {
            continue;
        }

        let server_runtimes = inventory
            .accelerated_server_runtimes
            .iter()
            .filter(|runtime| runtime.backend == backend)
            .cloned()
            .map(export_runtime_executable)
            .collect::<Vec<_>>();
        let cli_runtimes = inventory
            .accelerated_cli_runtimes
            .iter()
            .filter(|runtime| runtime.backend == backend)
            .cloned()
            .map(export_runtime_executable)
            .collect::<Vec<_>>();

        if server_runtimes.is_empty() && cli_runtimes.is_empty() {
            continue;
        }

        groups.push(AccelerationGroup {
            backend,
            compatible: backend_seems_compatible(
                backend,
                &server_runtimes
                    .iter()
                    .chain(cli_runtimes.iter())
                    .map(|runtime| runtime.path.clone())
                    .collect::<Vec<_>>(),
            ),
            server_runtimes,
            cli_runtimes,
        });
    }

    groups.sort_by_key(|group| group.backend.preference_order());
    groups
}

fn export_runtime_executable(runtime: DiscoveredRuntimeExecutable) -> RuntimeExecutable {
    RuntimeExecutable {
        path: runtime.path,
        backend: runtime.backend,
        label: runtime.backend.label().to_string(),
    }
}

fn scan_runtime_inventory(roots: &[PathBuf]) -> RuntimeInventory {
    let mut inventory = RuntimeInventory::default();
    let mut seen = HashSet::new();

    for root in roots {
        scan_runtime_root(root, 0, 3, &mut inventory, &mut seen);
    }

    inventory.sort();
    inventory
}

fn scan_runtime_root(
    root: &Path,
    depth: usize,
    max_depth: usize,
    inventory: &mut RuntimeInventory,
    seen: &mut HashSet<String>,
) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < max_depth {
                scan_runtime_root(&path, depth + 1, max_depth, inventory, seen);
            }
            continue;
        }

        let key = path.to_string_lossy().to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }

        if let Some(runtime) = classify_runtime_binary(&path) {
            inventory.push(runtime);
        }
    }
}

fn classify_runtime_binary(path: &Path) -> Option<DiscoveredRuntimeExecutable> {
    let file_name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    let stem = Path::new(&file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&file_name)
        .to_ascii_lowercase();

    let (kind, priority) = if stem.starts_with("llama-server") {
        (RuntimeBinaryKind::Server, 0)
    } else if stem.starts_with("llama-mtmd-cli") {
        (RuntimeBinaryKind::Cli, 0)
    } else if stem.starts_with("llama-cli") {
        (RuntimeBinaryKind::Cli, 1)
    } else {
        return None;
    };

    let backend = classify_runtime_backend(path, &stem);

    Some(DiscoveredRuntimeExecutable {
        path: path.to_path_buf(),
        backend,
        kind,
        priority,
    })
}

fn classify_runtime_backend(path: &Path, stem: &str) -> RuntimeBackend {
    if contains_runtime_marker(path, stem, &["cuda", "cublas"]) {
        RuntimeBackend::Cuda
    } else if contains_runtime_marker(path, stem, &["directml"]) || path_has_component(path, "dml")
    {
        RuntimeBackend::Directml
    } else if contains_runtime_marker(path, stem, &["vulkan"]) {
        RuntimeBackend::Vulkan
    } else if contains_runtime_marker(path, stem, &["metal"]) {
        RuntimeBackend::Metal
    } else if contains_runtime_marker(path, stem, &["accelerated", "accel"])
        || path_has_component(path, "gpu")
    {
        RuntimeBackend::GenericAccelerated
    } else {
        RuntimeBackend::CpuCompatible
    }
}

fn contains_runtime_marker(path: &Path, stem: &str, markers: &[&str]) -> bool {
    let lowered_path = path.to_string_lossy().to_ascii_lowercase();
    markers
        .iter()
        .any(|marker| stem.contains(marker) || lowered_path.contains(marker))
}

fn path_has_component(path: &Path, needle: &str) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(needle)
    })
}

fn runtime_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("bin"));
            candidates.push(dir.join("resources").join("bin"));
            candidates.push(dir.to_path_buf());
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("bin"));
        candidates.push(cwd.join("resources").join("bin"));
        candidates.push(cwd.join("src-tauri").join("bin"));
    }

    candidates.push(PathBuf::from("bin"));
    candidates.push(PathBuf::from("src-tauri").join("bin"));

    let mut seen = HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));
    candidates
}

fn backend_seems_compatible(backend: RuntimeBackend, runtime_paths: &[PathBuf]) -> bool {
    match backend {
        RuntimeBackend::CpuCompatible => true,
        #[cfg(target_os = "windows")]
        RuntimeBackend::Cuda => win_try_load_dll("nvcuda.dll"),
        #[cfg(target_os = "windows")]
        RuntimeBackend::Vulkan => win_try_load_dll("vulkan-1.dll"),
        #[cfg(target_os = "windows")]
        RuntimeBackend::Directml => {
            let has_directml = win_try_load_dll("DirectML.dll")
                || runtime_paths.iter().any(|path| {
                    path.parent()
                        .map(|parent| parent.join("DirectML.dll").exists())
                        .unwrap_or(false)
                });
            let has_d3d12 = win_try_load_dll("d3d12.dll");
            let has_dxcore = win_try_load_dll("dxcore.dll") || win_try_load_dll("dxgi.dll");
            has_directml && has_d3d12 && has_dxcore
        }
        #[cfg(target_os = "windows")]
        RuntimeBackend::Metal => false,
        #[cfg(target_os = "windows")]
        RuntimeBackend::GenericAccelerated => false,
        #[cfg(target_os = "macos")]
        RuntimeBackend::Metal => true,
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        RuntimeBackend::Metal => false,
        #[cfg(not(target_os = "windows"))]
        RuntimeBackend::Cuda => false,
        #[cfg(not(target_os = "windows"))]
        RuntimeBackend::Vulkan => false,
        #[cfg(not(target_os = "windows"))]
        RuntimeBackend::Directml => false,
        #[cfg(not(target_os = "windows"))]
        RuntimeBackend::GenericAccelerated => false,
    }
}

impl RuntimeInventory {
    fn push(&mut self, runtime: DiscoveredRuntimeExecutable) {
        match (runtime.backend.is_accelerated(), runtime.kind) {
            (false, RuntimeBinaryKind::Server) => self.cpu_server_runtimes.push(runtime),
            (false, RuntimeBinaryKind::Cli) => self.cpu_cli_runtimes.push(runtime),
            (true, RuntimeBinaryKind::Server) => self.accelerated_server_runtimes.push(runtime),
            (true, RuntimeBinaryKind::Cli) => self.accelerated_cli_runtimes.push(runtime),
        }
    }

    fn sort(&mut self) {
        let sorter = |left: &DiscoveredRuntimeExecutable, right: &DiscoveredRuntimeExecutable| {
            left.sort_key().cmp(&right.sort_key())
        };

        self.cpu_server_runtimes.sort_by(sorter);
        self.cpu_cli_runtimes.sort_by(sorter);
        self.accelerated_server_runtimes.sort_by(sorter);
        self.accelerated_cli_runtimes.sort_by(sorter);
    }
}

impl DiscoveredRuntimeExecutable {
    fn sort_key(&self) -> (u8, u8, usize, String) {
        (
            self.backend.preference_order(),
            self.priority,
            self.path.components().count(),
            self.path.to_string_lossy().to_ascii_lowercase(),
        )
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

#[cfg(target_os = "windows")]
fn win_try_load_dll(dll_name: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;

    extern "system" {
        fn LoadLibraryW(lpLibFileName: *const u16) -> *mut std::ffi::c_void;
        fn FreeLibrary(hLibModule: *mut std::ffi::c_void) -> i32;
    }

    let wide: Vec<u16> = std::ffi::OsStr::new(dll_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        return false;
    }
    unsafe {
        FreeLibrary(handle);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cpu_profile_stays_cpu_default() {
        let root = tempdir().unwrap();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("llama-mtmd-cli.exe"), b"stub").unwrap();
        fs::write(bin.join("llama-server.exe"), b"stub").unwrap();
        fs::create_dir_all(bin.join("accelerated").join("vulkan")).unwrap();
        fs::write(
            bin.join("accelerated")
                .join("vulkan")
                .join("llama-mtmd-cli.exe"),
            b"stub",
        )
        .unwrap();

        let inventory = scan_runtime_inventory(&[bin]);
        let plan = build_execution_plan(RuntimeProfile::CpuCompatible, &inventory);

        assert!(plan.status.cpu_runtime_available);
        assert_eq!(plan.status.selected_profile, RuntimeProfile::CpuCompatible);
        assert_eq!(plan.status.effective_runtime_label, "CPU compatible");
        assert!(plan
            .cli_runtimes
            .iter()
            .all(|runtime| runtime.backend == RuntimeBackend::CpuCompatible));
    }

    #[test]
    fn accelerated_profile_falls_back_to_cpu_when_only_cpu_exists() {
        let root = tempdir().unwrap();
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("llama-mtmd-cli.exe"), b"stub").unwrap();

        let inventory = scan_runtime_inventory(&[bin]);
        let plan = build_execution_plan(RuntimeProfile::AcceleratedIfAvailable, &inventory);

        assert!(plan.status.usable_runtime);
        assert_eq!(plan.status.effective_runtime_label, "CPU compatible");
        assert_eq!(plan.cli_runtimes.len(), 1);
        assert_eq!(plan.cli_runtimes[0].backend, RuntimeBackend::CpuCompatible);
    }

    #[test]
    fn classifies_nested_accelerated_runtime_dirs() {
        let root = tempdir().unwrap();
        let runtime_dir = root.path().join("bin").join("accelerated").join("vulkan");
        fs::create_dir_all(&runtime_dir).unwrap();
        fs::write(runtime_dir.join("llama-server.exe"), b"stub").unwrap();

        let inventory = scan_runtime_inventory(&[root.path().join("bin")]);

        assert_eq!(inventory.accelerated_server_runtimes.len(), 1);
        assert_eq!(
            inventory.accelerated_server_runtimes[0].backend,
            RuntimeBackend::Vulkan
        );
    }
}
