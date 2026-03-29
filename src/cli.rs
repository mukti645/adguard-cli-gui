use std::process::Command;
use regex::Regex;
extern crate libc;

/// Strip ANSI escape codes from string
pub fn strip_ansi(s: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*[mGKHF]").unwrap();
    re.replace_all(s, "").to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProtectionStatus {
    Running,
    Stopped,
    Unknown,
    NotInstalled,
    NotConfigured,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub protection: ProtectionStatus,
    pub version: String,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub status: String,
    pub key: String,
    pub expires: String,
    pub raw: String,
}

pub fn is_installed() -> bool {
    which("adguard-cli")
}

pub fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_configured() -> bool {
    let out = run_cmd(&["adguard-cli", "config", "show"]);
    let raw = strip_ansi(&out);
    !raw.to_lowercase().contains("no configuration yaml")
        && !raw.to_lowercase().contains("run `adguard-cli configure`")
}

pub fn get_status() -> Status {
    if !is_installed() {
        return Status {
            protection: ProtectionStatus::NotInstalled,
            version: String::new(),
            raw: String::new(),
        };
    }

    if !is_configured() {
        return Status {
            protection: ProtectionStatus::NotConfigured,
            version: get_version(),
            raw: String::new(),
        };
    }

    let out = run_cmd(&["adguard-cli", "status"]);
    let raw = strip_ansi(&out);
    let lower = raw.to_lowercase();

    let protection = if lower.contains("running") || lower.contains("started") || lower.contains("enabled") {
        ProtectionStatus::Running
    } else if lower.contains("stopped") || lower.contains("not running") || lower.contains("disabled") {
        ProtectionStatus::Stopped
    } else {
        ProtectionStatus::Unknown
    };

    let version = get_version();

    Status { protection, version, raw }
}

pub fn get_version() -> String {
    let out = run_cmd(&["adguard-cli", "--version"]);
    strip_ansi(&out).trim().to_string()
}

pub fn start() -> Result<String, String> {
    // adguard-cli start runs in foreground — spawn detached so GUI doesn't hang
    use std::os::unix::process::CommandExt;
    let result = unsafe {
        Command::new("adguard-cli")
            .arg("start")
            .pre_exec(|| {
                // Detach from process group so it survives GUI close
                libc::setsid();
                Ok(())
            })
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    match result {
        Ok(_) => {
            // Give it a moment to start, then check status
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let status_out = run_cmd(&["adguard-cli", "status"]);
            let clean = strip_ansi(&status_out);
            let lower = clean.to_lowercase();
            if lower.contains("not running") || lower.contains("failed") {
                Err(clean)
            } else {
                Ok("Protection started".to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn open_configure_terminal() {
    let terminals = ["konsole", "kitty", "alacritty", "gnome-terminal", "xfce4-terminal", "xterm"];
    for term in &terminals {
        let mut cmd = Command::new(term);
        if *term == "gnome-terminal" {
            cmd.arg("--").arg("adguard-cli").arg("configure");
        } else {
            cmd.arg("-e").arg("adguard-cli configure");
        }
        if cmd.spawn().is_ok() {
            return;
        }
    }
}

pub fn stop() -> Result<String, String> {
    let out = run_cmd_sudo(&["adguard-cli", "stop"]);
    let clean = strip_ansi(&out);
    let lower = clean.to_lowercase();
    // "not running" is not really an error
    if (lower.contains("error") || lower.contains("failed")) && !lower.contains("not running") {
        Err(clean)
    } else {
        Ok("Protection stopped".to_string())
    }
}

pub fn get_license() -> LicenseInfo {
    if !is_installed() {
        return LicenseInfo {
            status: "Not installed".to_string(),
            key: String::new(),
            expires: String::new(),
            raw: String::new(),
        };
    }

    let out = run_cmd(&["adguard-cli", "license"]);
    let raw = strip_ansi(&out);
    let lower = raw.to_lowercase();

    let status = if lower.contains("activated") || lower.contains("valid") || lower.contains("premium") {
        "Activated"
    } else if lower.contains("trial") {
        "Trial"
    } else if lower.contains("free") {
        "Free"
    } else {
        "Unknown"
    };

    // Extract license key if present
    let key = raw.lines()
        .find(|l| l.to_lowercase().contains("key") || l.to_lowercase().contains("license"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let expires = raw.lines()
        .find(|l| l.to_lowercase().contains("expir") || l.to_lowercase().contains("until") || l.to_lowercase().contains("valid"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    LicenseInfo {
        status: status.to_string(),
        key,
        expires,
        raw,
    }
}

pub fn activate_license(key: &str) -> Result<String, String> {
    let out = run_cmd_sudo(&["adguard-cli", "activate", key]);
    let clean = strip_ansi(&out);
    let lower = clean.to_lowercase();
    if lower.contains("error") || lower.contains("failed") || lower.contains("invalid") {
        Err(clean)
    } else {
        Ok(clean)
    }
}

pub fn reset_license() -> Result<String, String> {
    let out = run_cmd_sudo(&["adguard-cli", "reset-license"]);
    let clean = strip_ansi(&out);
    if clean.to_lowercase().contains("error") {
        Err(clean)
    } else {
        Ok(clean)
    }
}

pub fn list_filters() -> Vec<Filter> {
    if !is_installed() {
        return vec![];
    }

    let out = run_cmd(&["adguard-cli", "filters", "list", "--all"]);
    let raw = strip_ansi(&out);
    parse_filters(&raw)
}

fn parse_filters(raw: &str) -> Vec<Filter> {
    let mut filters = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Try to detect enabled/disabled markers
        // Common formats: "[✓] Filter Name", "[x] Filter Name", "1. [enabled] Name", etc.
        let enabled = line.contains("[+]") || line.contains("✓") || line.contains("enabled") || line.contains("[on]");
        let disabled = line.contains("[-]") || line.contains("✗") || line.contains("disabled") || line.contains("[off]");

        if !enabled && !disabled {
            continue;
        }

        // Extract filter name - take the part after the status marker
        let name = line
            .trim_start_matches(|c: char| !c.is_alphabetic())
            .to_string();

        if name.len() > 3 {
            filters.push(Filter {
                id: filters.len().to_string(),
                name,
                enabled,
                url: String::new(),
            });
        }
    }

    // If parsing failed, try simpler approach - just list lines
    if filters.is_empty() {
        for (i, line) in raw.lines().enumerate() {
            let line = line.trim();
            if line.len() > 5 && !line.starts_with("Filter") && !line.starts_with("---") {
                filters.push(Filter {
                    id: i.to_string(),
                    name: line.to_string(),
                    enabled: true,
                    url: String::new(),
                });
            }
        }
    }

    filters
}

pub fn export_logs(path: &str) -> Result<String, String> {
    let out = run_cmd_sudo(&["adguard-cli", "export-logs", "-o", path]);
    let clean = strip_ansi(&out);
    if clean.to_lowercase().contains("error") {
        Err(clean)
    } else {
        Ok(clean)
    }
}

pub fn check_update() -> String {
    let out = run_cmd(&["adguard-cli", "check-update"]);
    strip_ansi(&out)
}

pub fn update() -> Result<String, String> {
    let out = run_cmd_sudo(&["adguard-cli", "update"]);
    let clean = strip_ansi(&out);
    if clean.to_lowercase().contains("error") {
        Err(clean)
    } else {
        Ok(clean)
    }
}

pub fn install_via_aur() -> Result<String, String> {
    // Try paru first, then yay
    let helper = if which("paru") { "paru" } else if which("yay") { "yay" } else {
        return Err("No AUR helper found (paru or yay required)".to_string());
    };

    let out = run_raw(&[helper, "-S", "--noconfirm", "--needed", "adguard-cli-bin"]);
    let clean = strip_ansi(&out);
    if clean.to_lowercase().contains("error") {
        Err(clean)
    } else {
        Ok(clean)
    }
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn run_cmd(args: &[&str]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let out = Command::new(args[0])
        .args(&args[1..])
        .output();

    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            if stdout.is_empty() { stderr } else { stdout }
        }
        Err(e) => e.to_string(),
    }
}

fn run_cmd_sudo(args: &[&str]) -> String {
    if args.is_empty() {
        return String::new();
    }
    // Try running directly first (sudoers NOPASSWD may allow it)
    let out = Command::new(args[0])
        .args(&args[1..])
        .output();

    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            if stdout.is_empty() { stderr } else { stdout }
        }
        Err(e) => e.to_string(),
    }
}

fn run_raw(args: &[&str]) -> String {
    if args.is_empty() {
        return String::new();
    }
    let out = Command::new(args[0])
        .args(&args[1..])
        .output();

    match out {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            combined
        }
        Err(e) => e.to_string(),
    }
}
