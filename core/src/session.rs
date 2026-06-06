use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub project: String,
    pub file_path: PathBuf,
    pub working_dir: Option<String>,
    pub shell: Option<String>,
    pub init_script: Option<String>,
    pub script_body: Option<String>,
    pub env_vars: Vec<(String, String)>,
}

impl Session {
    pub fn load(path: &Path, project: &str) -> Result<Self> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read {}", path.display()))?;
        let (props, env_vars, script_body) = parse_conf(&content);
        Ok(Self {
            name,
            project: project.to_string(),
            file_path: path.to_path_buf(),
            working_dir: props.get("dir").cloned(),
            shell: props.get("shell").cloned(),
            init_script: props.get("init_script").cloned(),
            script_body,
            env_vars,
        })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.file_path, self.to_conf_string())
            .with_context(|| format!("Cannot write {}", self.file_path.display()))
    }

    pub fn shell_bin(&self) -> String {
        self.shell
            .clone()
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/zsh".to_string())
    }

    pub fn working_dir_path(&self) -> PathBuf {
        self.working_dir
            .as_deref()
            .map(|s| PathBuf::from(expand_tilde(s)))
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"))
    }

    /// Writes init_script + inline [script] block to a temp file and returns its path.
    /// Returns None when there is nothing to initialise.
    pub fn build_init_file(&self) -> Option<PathBuf> {
        let mut lines = Vec::new();
        if let Some(ref path) = self.init_script {
            lines.push(format!("source {path}"));
        }
        if let Some(ref body) = self.script_body {
            lines.push(body.clone());
        }
        if lines.is_empty() {
            return None;
        }
        lines.push("clear".to_string());
        let tmp = std::env::temp_dir().join(format!("ntx_{}.sh", self.name));
        std::fs::write(&tmp, lines.join("\n")).ok()?;
        Some(tmp)
    }

    fn to_conf_string(&self) -> String {
        let header: String = self.working_dir.iter().map(|d| format!("dir = {d}\n"))
            .chain(self.shell.iter().map(|s| format!("shell = {s}\n")))
            .chain(self.init_script.iter().map(|s| format!("init_script = {s}\n")))
            .collect();
        let env_section: String = if self.env_vars.is_empty() {
            String::new()
        } else {
            std::iter::once("\n[env]\n".to_string())
                .chain(self.env_vars.iter().map(|(k, v)| format!("{k} = {v}\n")))
                .collect()
        };
        let script_section: String = self.script_body.as_ref()
            .map(|s| format!("\n[script]\n{s}\n"))
            .unwrap_or_default();
        header + &env_section + &script_section
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

fn parse_conf(content: &str) -> (HashMap<String, String>, Vec<(String, String)>, Option<String>) {
    #[derive(Clone, Copy, PartialEq)]
    enum Section { Top, Env, Script, Other }

    let (props, env_vars, script_lines, _) = content.lines().fold(
        (HashMap::new(), Vec::new(), Vec::<&str>::new(), Section::Top),
        |(mut props, mut env_vars, mut script_lines, section), line| {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("[env]") {
                return (props, env_vars, script_lines, Section::Env);
            }
            if trimmed.eq_ignore_ascii_case("[script]") {
                return (props, env_vars, script_lines, Section::Script);
            }
            if trimmed.starts_with('[') {
                return (props, env_vars, script_lines, Section::Other);
            }
            match section {
                Section::Script => script_lines.push(line),
                Section::Top | Section::Env => {
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        return (props, env_vars, script_lines, section);
                    }
                    if let Some(eq) = trimmed.find('=') {
                        let k = trimmed[..eq].trim().to_string();
                        let v = unquote(trimmed[eq + 1..].trim()).to_string();
                        if !k.is_empty() {
                            if section == Section::Env {
                                env_vars.push((k, v));
                            } else {
                                props.insert(k, v);
                            }
                        }
                    }
                }
                Section::Other => {}
            }
            (props, env_vars, script_lines, section)
        },
    );

    let script = if script_lines.is_empty() {
        None
    } else {
        let body = script_lines.join("\n").trim_end().to_string();
        if body.is_empty() { None } else { Some(body) }
    };
    (props, env_vars, script)
}

fn unquote(s: &str) -> &str {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\'')))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn expand_tilde(s: &str) -> String {
    if s == "~" {
        dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| s.to_string())
    } else if let Some(rest) = s.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(rest).to_string_lossy().to_string())
            .unwrap_or_else(|| s.to_string())
    } else {
        s.to_string()
    }
}
