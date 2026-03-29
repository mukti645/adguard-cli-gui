use std::process::Command;
use regex::Regex;

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
    // adguard-cli start exits on its own (~1.7s) after spawning the daemon.
    // Running it normally (blocking) is the correct approach.
    let out = run_cmd(&["adguard-cli", "start"]);
    let clean = strip_ansi(&out);
    let lower = clean.to_lowercase();
    if lower.contains("successfully") || lower.contains("is running") || lower.contains("listening") {
        Ok("Protection started".to_string())
    } else if lower.contains("error") || lower.contains("failed") || lower.contains("can't") || lower.contains("cannot") {
        Err(clean)
    } else {
        // Ambiguous output — verify via status
        let status_out = run_cmd(&["adguard-cli", "status"]);
        let status = strip_ansi(&status_out);
        if status.to_lowercase().contains("is running") {
            Ok("Protection started".to_string())
        } else {
            Err(status)
        }
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

    // adguard-cli outputs: "License status: APP_ACTIVE" or "TRIAL" etc.
    let status = if lower.contains("app_active") || lower.contains("activated") || lower.contains("active") {
        "Active ✓"
    } else if lower.contains("trial") {
        "Trial"
    } else if lower.contains("free") {
        "Free"
    } else if lower.contains("expired") {
        "Expired"
    } else {
        "Unknown"
    };

    // "License key: XXXX" → extract value after last ':'
    let key = raw.lines()
        .find(|l| l.to_lowercase().contains("license key"))
        .and_then(|l| l.splitn(2, ':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // "License owner: email" → show as expires/owner field
    let expires = raw.lines()
        .find(|l| l.to_lowercase().contains("owner") || l.to_lowercase().contains("expir"))
        .and_then(|l| l.splitn(2, ':').nth(1))
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
    // Format: "[x] |   ID | Filter Name    date"  (enabled)
    //         "    |   ID | Filter Name    Filter is not added"  (disabled)
    //         "CategoryName"  (section header — no '|')
    let mut filters = Vec::new();

    for line in raw.lines() {
        // Must contain '|' to be a filter row (not a header)
        if !line.contains('|') {
            continue;
        }

        let enabled = line.trim_start().starts_with("[x]");
        // "not added" means filter exists but isn't installed
        let not_added = line.contains("not added") || line.contains("is not added");

        // Extract name: third column after splitting by '|'
        // "[x] |   2 | AdGuard Base filter    2026-..."
        //  col0   col1   col2
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        // col2 = "AdGuard Base filter    2026-03-29 21:15:48"
        // trim the date/status suffix — take up to two consecutive spaces
        let name_raw = parts[2].trim();
        let name = if let Some(idx) = name_raw.find("  ") {
            name_raw[..idx].trim().to_string()
        } else {
            name_raw.to_string()
        };

        if name.len() < 3 || name.starts_with("ID") || name.starts_with("Title") {
            continue; // skip header row
        }

        filters.push(Filter {
            id: filters.len().to_string(),
            name,
            enabled: enabled && !not_added,
            url: String::new(),
        });
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
