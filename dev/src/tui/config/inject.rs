//! Inject-specific config functions: init, load, save, defaults, validation.

use std::fs;
use std::path::Path;

use super::schema::ConfigFile;

/// Copy default.conf to config_path if it doesn't exist yet.
pub fn init_config(config_path: &Path, default_path: &Path) {
    if config_path.exists() {
        return;
    }
    if default_path.exists() {
        let _ = fs::copy(default_path, config_path);
    }
}

/// Check if TIOCSTI ioctl is available on the current kernel.
/// On Linux < 6.2, TIOCSTI is always available.
/// On Linux >= 6.2, requires dev.tty.legacy_tiocsti=1.
pub(super) fn is_tiocsti_available() -> bool {
    match std::fs::read_to_string("/proc/sys/dev/tty/legacy_tiocsti") {
        Err(_) => true,  // file absent -> old kernel, TIOCSTI available
        Ok(s) => s.trim() == "1",
    }
}

/// Default inject method for a shell.
pub fn default_inject_method(shell: &str) -> &'static str {
    match shell {
        "bash" => "readline",
        "zsh" => "print-z",
        "tcsh" if is_tiocsti_available() => "tiocsti",
        "tcsh" => "history",
        _ => "history",
    }
}

/// Valid inject methods for a shell (accounts for TIOCSTI availability).
pub fn valid_inject_methods(shell: &str) -> Vec<&'static str> {
    let tiocsti = is_tiocsti_available();
    match shell {
        "bash" => {
            let mut v = vec!["readline"];
            if tiocsti { v.push("tiocsti"); }
            v.push("history");
            v
        }
        "zsh" => {
            let mut v = vec!["print-z"];
            if tiocsti { v.push("tiocsti"); }
            v.push("history");
            v
        }
        "tcsh" => {
            let mut v = Vec::new();
            if tiocsti { v.push("tiocsti"); }
            v.push("history");
            v
        }
        _ => vec!["history"],
    }
}

/// Save an inject method for a shell to the config file.
pub fn save_inject_method(config_path: &Path, shell: &str, method: &str) {
    let content = fs::read_to_string(config_path).unwrap_or_default();
    if let Ok(mut doc) = content.parse::<toml_edit::DocumentMut>() {
        if doc.get("inject").is_none() {
            doc["inject"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        doc["inject"][shell] = toml_edit::value(method);
        let _ = fs::write(config_path, doc.to_string());
    }
}

/// Read the inject method for a given shell from the config file.
/// Returns the method string, defaulting per shell.
/// If the config file exists but the shell key is missing, writes the
/// default value back so the user can discover and edit it.
pub fn load_inject_method(config_path: &Path, shell: &str) -> String {
    let default = default_inject_method(shell);
    let content = match fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return default.to_string(),
    };
    let cfg: ConfigFile = match toml::from_str(&content) {
        Ok(c) => c,
        Err(_) => return default.to_string(),
    };

    // Check if the value is already set
    let existing = cfg.inject.as_ref().and_then(|i| match shell {
        "bash" => i.bash.as_deref(),
        "zsh" => i.zsh.as_deref(),
        "tcsh" => i.tcsh.as_deref(),
        _ => None,
    });

    if let Some(val) = existing {
        return val.to_string();
    }

    // Value missing -- write the default back to config
    save_inject_method(config_path, shell, default);

    default.to_string()
}
