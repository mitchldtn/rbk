use std::path::PathBuf;

pub fn base_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".ntx")
}

pub fn projects_dir() -> PathBuf {
    base_dir().join("projects")
}

pub fn project_dir(project: &str) -> PathBuf {
    projects_dir().join(project)
}

pub fn conf_dir(project: &str) -> PathBuf {
    project_dir(project).join("config")
}

pub fn notes_dir(project: &str) -> PathBuf {
    project_dir(project).join("notes")
}

pub fn list_projects() -> Vec<String> {
    let dir = projects_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut projects: Vec<String> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    projects.sort();
    projects
}

pub fn create_project(name: &str) {
    let dir = project_dir(name);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let _ = std::fs::create_dir_all(dir.join("notes"));
}
