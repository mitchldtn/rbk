use ntx_core::session::Session;

use crate::paths;

/// Load project config from ~/.ntx/projects/<project>/config/.
/// Uses the first .conf found, or writes a default if none exists.
pub fn load(project: &str) -> Session {
    let dir = paths::conf_dir(project);
    let _ = std::fs::create_dir_all(&dir);

    let conf_path = std::fs::read_dir(&dir)
        .ok()
        .and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.extension().and_then(|e| e.to_str()) == Some("conf"))
        });

    match conf_path {
        Some(path) => Session::load(&path, project).unwrap_or_else(|_| blank(project)),
        None => {
            let sess = blank(project);
            let _ = sess.save();
            sess
        }
    }
}

fn blank(project: &str) -> Session {
    Session {
        name: "default".to_string(),
        project: project.to_string(),
        file_path: paths::conf_dir(project).join("default.conf"),
        working_dir: None,
        shell: None,
        init_script: None,
        script_body: None,
        env_vars: Vec::new(),
    }
}
