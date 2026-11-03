//! Configuration file loading and parsing.
//!
//! Bureau looks for configuration at `.bureau/config` or `.bureau.toml` in the
//! repository root. If neither file exists, default configuration is used.
//!
//! # Format
//!
//! A simple TOML-compatible key-value configuration file with sections:
//!
//! ```toml
//! [report]
//! type = "worklog"
//! no_diff = true
//!
//! [tasks]
//! all = true
//! staleness_days = 21
//!
//! [templates]
//! dossier = "templates/custom-dossier.md"
//! entry = "templates/custom-entry.md"
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::report::ReportType;

/// Configuration file locations relative to repository root.
pub const CONFIG_DIR_FILE: &str = ".bureau/config";
pub const CONFIG_FILE: &str = ".bureau.toml";

/// Bureau configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Report command configuration.
    pub report: ReportConfig,
    /// Tasks command configuration.
    pub tasks: TasksConfig,
    /// Templates configuration.
    pub templates: TemplatesConfig,
}

/// Settings for `bureau report`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportConfig {
    /// Default report type.
    pub r#type: Option<ReportType>,
    /// Default value for `--no-diff`.
    pub no_diff: bool,
}

/// Settings for `bureau tasks`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TasksConfig {
    /// Default value for `--all`.
    pub all: bool,
    /// Staleness threshold in days (defaults to 14 if unspecified).
    pub staleness_days: Option<u32>,
}

/// Custom template paths relative to repository root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplatesConfig {
    /// Path to a custom dossier template.
    pub dossier: Option<PathBuf>,
    /// Path to a custom daily entry template.
    pub entry: Option<PathBuf>,
}

/// Locate the configuration file in the repository root, if one exists.
#[must_use]
pub fn locate(root: &Path) -> Option<PathBuf> {
    let dir_config = root.join(CONFIG_DIR_FILE);
    if dir_config.is_file() {
        return Some(dir_config);
    }
    let file_config = root.join(CONFIG_FILE);
    if file_config.is_file() {
        return Some(file_config);
    }
    None
}

/// Load configuration from the repository, returning default config if none is present.
///
/// # Errors
///
/// Fails when the configuration file exists but cannot be read or contains invalid syntax.
pub fn load_from(root: &Path) -> Result<Config> {
    let Some(path) = locate(root) else {
        return Ok(Config::default());
    };
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read configuration file at {}", path.display()))?;
    parse(&content).with_context(|| format!("error in configuration file {}", path.display()))
}

/// Parse configuration from a string.
///
/// # Errors
///
/// Fails with a line number and description on syntax or semantic error.
pub fn parse(content: &str) -> Result<Config> {
    let mut config = Config::default();
    let mut current_section: Option<String> = None;

    for (line_index, raw_line) in content.lines().enumerate() {
        let line_number = line_index.saturating_add(1);
        let trimmed = raw_line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') {
            if !trimmed.ends_with(']') {
                bail!("line {line_number}: unclosed section header '{trimmed}'");
            }
            let section_name = trimmed
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .map(str::trim)
                .unwrap_or_default();

            match section_name {
                "report" | "tasks" | "templates" => {
                    current_section = Some(section_name.to_owned());
                }
                _ => {
                    bail!("line {line_number}: unknown section '[{section_name}]'");
                }
            }
            continue;
        }

        let Some((key_part, val_part)) = trimmed.split_once('=') else {
            bail!("line {line_number}: expected 'key = value', found '{trimmed}'");
        };

        let key = key_part.trim();
        let raw_val = val_part.trim();

        if key.is_empty() {
            bail!("line {line_number}: missing key before '='");
        }
        if raw_val.is_empty() {
            bail!("line {line_number}: missing value for key '{key}'");
        }

        let Some(section) = current_section.as_deref() else {
            bail!("line {line_number}: key '{key}' found outside of any [section]");
        };

        apply_key_value(&mut config, section, key, raw_val, line_number)?;
    }

    Ok(config)
}

/// Strip surrounding matching quotes from a string value.
fn unquote(val: &str, line_number: usize) -> Result<String> {
    if (val.starts_with('"') && !val.ends_with('"'))
        || (val.starts_with('\'') && !val.ends_with('\''))
        || (!val.starts_with('"') && val.ends_with('"'))
        || (!val.starts_with('\'') && val.ends_with('\''))
    {
        bail!("line {line_number}: mismatched or unclosed quotes in value: {val}");
    }

    if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
        || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
    {
        let without_prefix = val.chars().skip(1);
        let mut chars: Vec<char> = without_prefix.collect();
        chars.pop();
        let unquoted: String = chars.into_iter().collect();
        return Ok(unquoted);
    }

    Ok(val.to_owned())
}

/// Parse a boolean value.
fn parse_bool(val: &str, key: &str, line_number: usize) -> Result<bool> {
    let unquoted = unquote(val, line_number)?;
    match unquoted.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => bail!(
            "line {line_number}: invalid boolean value '{val}' for key '{key}' (expected: true, false)"
        ),
    }
}

/// Parse an unsigned integer value.
fn parse_u32(val: &str, key: &str, line_number: usize) -> Result<u32> {
    let unquoted = unquote(val, line_number)?;
    unquoted
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("line {line_number}: invalid number '{val}' for key '{key}'"))
}

/// Parse a report type value.
fn parse_report_type(val: &str, line_number: usize) -> Result<ReportType> {
    let unquoted = unquote(val, line_number)?;
    match unquoted.to_ascii_lowercase().as_str() {
        "minimal" => Ok(ReportType::Minimal),
        "worklog" => Ok(ReportType::Worklog),
        "full" => Ok(ReportType::Full),
        _ => bail!(
            "line {line_number}: invalid report type '{val}' (expected: minimal, worklog, full)"
        ),
    }
}

/// Apply a key-value pair to the appropriate section in `Config`.
fn apply_key_value(
    config: &mut Config,
    section: &str,
    key: &str,
    val: &str,
    line_number: usize,
) -> Result<()> {
    match section {
        "report" => match key {
            "type" => {
                config.report.r#type = Some(parse_report_type(val, line_number)?);
            }
            "no_diff" => {
                config.report.no_diff = parse_bool(val, key, line_number)?;
            }
            _ => bail!("line {line_number}: unknown key '{key}' in [report] section"),
        },
        "tasks" => match key {
            "all" => {
                config.tasks.all = parse_bool(val, key, line_number)?;
            }
            "staleness_days" => {
                config.tasks.staleness_days = Some(parse_u32(val, key, line_number)?);
            }
            _ => bail!("line {line_number}: unknown key '{key}' in [tasks] section"),
        },
        "templates" => match key {
            "dossier" => {
                let path = unquote(val, line_number)?;
                config.templates.dossier = Some(PathBuf::from(path));
            }
            "entry" => {
                let path = unquote(val, line_number)?;
                config.templates.entry = Some(PathBuf::from(path));
            }
            _ => bail!("line {line_number}: unknown key '{key}' in [templates] section"),
        },
        _ => bail!("line {line_number}: unknown section '[{section}]'"),
    }
    Ok(())
}

/// Load the dossier template, checking custom config path first and falling back to embedded.
///
/// # Errors
///
/// Fails if a custom dossier template is configured but cannot be read.
pub fn load_dossier_template(root: &Path, config: &Config) -> Result<String> {
    if let Some(rel_path) = &config.templates.dossier {
        let full_path = root.join(rel_path);
        return fs::read_to_string(&full_path).with_context(|| {
            format!(
                "failed to read custom dossier template at {}",
                full_path.display()
            )
        });
    }
    Ok(crate::template::DOSSIER.to_owned())
}

/// Load the daily entry template, checking custom config path first and falling back to embedded.
///
/// # Errors
///
/// Fails if a custom daily entry template is configured but cannot be read.
pub fn load_entry_template(root: &Path, config: &Config) -> Result<String> {
    if let Some(rel_path) = &config.templates.entry {
        let full_path = root.join(rel_path);
        return fs::read_to_string(&full_path).with_context(|| {
            format!(
                "failed to read custom daily entry template at {}",
                full_path.display()
            )
        });
    }
    Ok(crate::template::DAILY_ENTRY.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_and_comments() {
        let config = parse("").unwrap();
        assert_eq!(config, Config::default());

        let config = parse("# A comment line\n\n   # Another comment\n").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn parse_valid_full_config() {
        let content = r#"
# Global bureau config
[report]
type = "worklog"
no_diff = true

[tasks]
all = true
staleness_days = 21

[templates]
dossier = "templates/custom-dossier.md"
entry = 'templates/custom-entry.md'
"#;
        let config = parse(content).unwrap();
        assert_eq!(config.report.r#type, Some(ReportType::Worklog));
        assert!(config.report.no_diff);
        assert!(config.tasks.all);
        assert_eq!(config.tasks.staleness_days, Some(21));
        assert_eq!(
            config.templates.dossier,
            Some(PathBuf::from("templates/custom-dossier.md"))
        );
        assert_eq!(
            config.templates.entry,
            Some(PathBuf::from("templates/custom-entry.md"))
        );
    }

    #[test]
    fn parse_unquoted_values() {
        let content = r"
[report]
type = minimal
no_diff = false

[tasks]
all = false
staleness_days = 10
";
        let config = parse(content).unwrap();
        assert_eq!(config.report.r#type, Some(ReportType::Minimal));
        assert!(!config.report.no_diff);
        assert!(!config.tasks.all);
        assert_eq!(config.tasks.staleness_days, Some(10));
    }

    #[test]
    fn parse_unknown_section_fails() {
        let content = "[unknown]\nfoo = bar\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(err.contains("line 1: unknown section '[unknown]'"), "{err}");
    }

    #[test]
    fn parse_unknown_key_fails() {
        let content = "[report]\nunknown_key = 123\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 2: unknown key 'unknown_key' in [report] section"),
            "{err}"
        );
    }

    #[test]
    fn parse_key_outside_section_fails() {
        let content = "type = worklog\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 1: key 'type' found outside of any [section]"),
            "{err}"
        );
    }

    #[test]
    fn parse_invalid_boolean_fails() {
        let content = "[report]\nno_diff = maybe\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 2: invalid boolean value 'maybe'"),
            "{err}"
        );
    }

    #[test]
    fn parse_invalid_number_fails() {
        let content = "[tasks]\nstaleness_days = invalid\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(err.contains("line 2: invalid number 'invalid'"), "{err}");
    }

    #[test]
    fn parse_invalid_report_type_fails() {
        let content = "[report]\ntype = unsupported\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 2: invalid report type 'unsupported'"),
            "{err}"
        );
    }

    #[test]
    fn parse_unclosed_quotes_fails() {
        let content = "[templates]\ndossier = \"unclosed\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 2: mismatched or unclosed quotes"),
            "{err}"
        );
    }

    #[test]
    fn parse_syntax_error_fails() {
        let content = "[report]\njust_a_word\n";
        let err = parse(content).unwrap_err().to_string();
        assert!(
            err.contains("line 2: expected 'key = value', found 'just_a_word'"),
            "{err}"
        );
    }
}
