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
    NoLicense,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub protection: ProtectionStatus,
    pub version: String,
    pub raw: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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

/// Result of running a CLI command, including exit code and both streams.
struct CmdResult {
    exit_success: bool,
    stdout: String,
    stderr: String,
}

impl CmdResult {
    /// Combined output: stdout + stderr (prefers stdout if non-empty).
    fn combined(&self) -> String {
        let mut s = self.stdout.clone();
        if !self.stderr.is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(&self.stderr);
        }
        s
    }

    /// First non-empty line of combined output (for notifications).
    fn first_line(&self) -> String {
        let c = self.combined();
        strip_ansi(c.lines().next().unwrap_or("").trim())
    }
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
    let r = run_cmd_full(&["adguard-cli", "config", "show"]);
    let raw = strip_ansi(&r.combined());
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

    let r = run_cmd_full(&["adguard-cli", "status"]);
    let combined = strip_ansi(&r.combined());
    let lower = combined.to_lowercase();

    // If the CLI returned a non-zero exit code, protection is NOT running.
    // Common case: "You need to activate an AdGuard license to use this command"
    if !r.exit_success {
        let protection = if lower.contains("you need to activate") || lower.contains("license") {
            ProtectionStatus::NoLicense
        } else {
            ProtectionStatus::Stopped
        };
        return Status {
            protection,
            version: get_version(),
            raw: combined,
        };
    }

    // Exit code 0 — parse the actual status output with precise phrases
    let protection = if lower.contains("proxy is running")
        || lower.contains("protection is running")
        || lower.contains("is running")
        || lower.contains("protection is enabled")
    {
        ProtectionStatus::Running
    } else if lower.contains("stopped")
        || lower.contains("not running")
        || lower.contains("disabled")
        || lower.contains("is not running")
    {
        ProtectionStatus::Stopped
    } else {
        ProtectionStatus::Unknown
    };

    let version = get_version();
    Status { protection, version, raw: combined }
}

pub fn get_version() -> String {
    let r = run_cmd_full(&["adguard-cli", "--version"]);
    strip_ansi(&r.stdout).trim().to_string()
}

pub fn start() -> Result<String, String> {
    let r = run_cmd_full(&["adguard-cli", "start"]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();

    if !r.exit_success {
        return Err(truncate_msg(&clean));
    }

    if lower.contains("successfully") || lower.contains("is running") || lower.contains("listening") {
        Ok("Protection started".to_string())
    } else if lower.contains("error") || lower.contains("failed") || lower.contains("can't") || lower.contains("cannot") {
        Err(truncate_msg(&clean))
    } else {
        // Ambiguous — verify via status
        let sr = run_cmd_full(&["adguard-cli", "status"]);
        if sr.exit_success && strip_ansi(&sr.combined()).to_lowercase().contains("is running") {
            Ok("Protection started".to_string())
        } else {
            Err(truncate_msg(&strip_ansi(&sr.combined())))
        }
    }
}

pub fn open_configure_terminal() {
    let terminals = ["konsole", "kitty", "alacritty", "gnome-terminal", "xfce4-terminal", "xterm"];
    for term in &terminals {
        let mut cmd = Command::new(term);
        if *term == "gnome-terminal" {
            cmd.arg("--").arg("adguard-cli").arg("configure");
        } else if *term == "xterm" {
            // xterm -e needs separate args
            cmd.arg("-e").arg("adguard-cli").arg("configure");
        } else {
            cmd.arg("-e").arg("adguard-cli configure");
        }
        if cmd.spawn().is_ok() {
            return;
        }
    }
}

pub fn stop() -> Result<String, String> {
    let r = run_cmd_full(&["adguard-cli", "stop"]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();

    // Non-zero exit code is an error (but "not running" is acceptable)
    if !r.exit_success && !lower.contains("not running") {
        return Err(truncate_msg(&clean));
    }

    if (lower.contains("error") || lower.contains("failed")) && !lower.contains("not running") {
        Err(truncate_msg(&clean))
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

    let r = run_cmd_full(&["adguard-cli", "license"]);
    let raw = strip_ansi(&r.combined());
    let lower = raw.to_lowercase();

    // If exit code is non-zero or output contains license-needed message, report it
    if !r.exit_success || lower.contains("you need to activate") {
        return LicenseInfo {
            status: "No license".to_string(),
            key: String::new(),
            expires: String::new(),
            raw: r.first_line(),
        };
    }

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

    let key = raw.lines()
        .find(|l| l.to_lowercase().contains("license key"))
        .and_then(|l| l.splitn(2, ':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

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
    let r = run_cmd_full(&["adguard-cli", "activate", key]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();

    // Check exit code first
    if !r.exit_success {
        return Err(truncate_msg(&clean));
    }

    // Even with exit code 0, check for known failure phrases
    if lower.contains("does not exist")
        || lower.contains("not found")
        || lower.contains("not valid")
        || lower.contains("invalid")
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("expired")
        || lower.contains("you need to activate")
    {
        Err(truncate_msg(&clean))
    } else if lower.contains("activated") || lower.contains("success") {
        Ok("License activated successfully".to_string())
    } else {
        // Unknown output — show it but treat as error to be safe
        Err(truncate_msg(&clean))
    }
}

pub fn reset_license() -> Result<String, String> {
    let r = run_cmd_full(&["adguard-cli", "reset-license"]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();

    if !r.exit_success
        || lower.contains("you need to activate")
        || lower.contains("error")
        || lower.contains("failed")
    {
        Err(truncate_msg(&clean))
    } else {
        Ok(truncate_msg(&clean))
    }
}

pub fn list_filters() -> Vec<Filter> {
    if !is_installed() {
        return vec![];
    }

    let r = run_cmd_full(&["adguard-cli", "filters", "list", "--all"]);

    // If the command failed (e.g. license error), return empty
    if !r.exit_success {
        return vec![];
    }

    let raw = strip_ansi(&r.stdout);
    parse_filters(&raw)
}

fn parse_filters(raw: &str) -> Vec<Filter> {
    let mut filters = Vec::new();

    for line in raw.lines() {
        if !line.contains('|') {
            continue;
        }

        let enabled = line.trim_start().starts_with("[x]");
        let not_added = line.contains("not added") || line.contains("is not added");

        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let name_raw = parts[2].trim();
        let name = if let Some(idx) = name_raw.find("  ") {
            name_raw[..idx].trim().to_string()
        } else {
            name_raw.to_string()
        };

        if name.len() < 3 || name.starts_with("ID") || name.starts_with("Title") {
            continue;
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
    let r = run_cmd_full(&["adguard-cli", "export-logs", "-o", path]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();
    if !r.exit_success
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fail")
    {
        Err(truncate_msg(&clean))
    } else {
        Ok(truncate_msg(&clean))
    }
}

pub fn check_update() -> String {
    let r = run_cmd_full(&["adguard-cli", "check-update"]);
    strip_ansi(&r.combined())
}

pub fn update() -> Result<String, String> {
    let r = run_cmd_full(&["adguard-cli", "update"]);
    let clean = strip_ansi(&r.combined());
    let lower = clean.to_lowercase();
    if !r.exit_success
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fail")
    {
        Err(truncate_msg(&clean))
    } else {
        Ok(truncate_msg(&clean))
    }
}

/// Open the AdGuard CLI GitHub releases page in the default browser.
pub fn open_download_page() -> Result<String, String> {
    let url = "https://github.com/AdguardTeam/AdGuardCLI/releases";
    let openers = ["xdg-open", "open", "sensible-browser"];
    for opener in &openers {
        if Command::new(opener).arg(url).spawn().is_ok() {
            return Ok(format!("Opened {url} in browser"));
        }
    }
    Err(format!("Could not open browser. Visit: {url}"))
}

// ─── Internal helpers ───────────────────────────────────────────────────────

fn run_cmd_full(args: &[&str]) -> CmdResult {
    if args.is_empty() {
        return CmdResult { exit_success: false, stdout: String::new(), stderr: String::new() };
    }
    match Command::new(args[0]).args(&args[1..]).output() {
        Ok(o) => CmdResult {
            exit_success: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).to_string(),
        },
        Err(e) => CmdResult {
            exit_success: false,
            stdout: String::new(),
            stderr: e.to_string(),
        },
    }
}

/// Truncate a message to at most ~200 chars / first meaningful line for notifications.
fn truncate_msg(s: &str) -> String {
    // Take the first non-empty line
    let first = s.lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .unwrap_or(s);
    if first.len() > 200 {
        format!("{}…", &first[..197])
    } else {
        first.to_string()
    }
}