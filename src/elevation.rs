//! Polkit/pkexec elevation for system-scope operations.

use std::path::Path;
use std::process;

/// Returns true if the current process is running as root (e.g. via pkexec).
pub fn is_elevated() -> bool {
    nix::unistd::Uid::current().is_root()
}

/// Returns true if the path is under the system install directory.
pub fn is_system_scope(path: &Path, system_install_dir: &Path) -> bool {
    path.starts_with(system_install_dir)
}

/// Re-execute the current binary with pkexec for elevation.
/// Passes through all current args. Exits with the child's exit code.
/// Preserves HOME so the elevated process can read the invoking user's config (sources.list).
pub fn re_exec_with_pkexec() -> ! {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: cannot get executable path: {}", e);
            process::exit(1);
        }
    };

    let args: Vec<String> = std::env::args().skip(1).collect();

    // Pass HOME so elevated process reads user's ~/.config/mcp/sources.list, not /root/.config
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

    let status = process::Command::new("pkexec")
        .arg("env")
        .arg(format!("HOME={}", home))
        .arg(&exe)
        .args(&args)
        .status();

    match status {
        Ok(s) => process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!("Error: pkexec not found. Install polkit for system-scope operations:");
                eprintln!("  pacman -S polkit   (Arch Linux)");
                eprintln!("  apt install policykit-1  (Debian/Ubuntu)");
                eprintln!("Alternatively, run with sudo: sudo dmcp ...");
            } else {
                eprintln!("Error: pkexec failed: {}", e);
                eprintln!("Make sure polkit is installed. You can also try: sudo dmcp ...");
            }
            process::exit(1);
        }
    }
}
