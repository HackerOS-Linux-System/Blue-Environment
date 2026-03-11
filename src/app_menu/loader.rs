use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct App {
    pub name:       String,
    pub exec:       String,
    pub icon:       Option<String>,
    pub comment:    Option<String>,
    pub categories: Vec<String>,
}

pub fn load_all() -> Vec<App> {
    let mut dirs = vec![
        std::path::PathBuf::from("/usr/share/applications/"),
        std::path::PathBuf::from("/usr/local/share/applications/"),
    ];
    if let Ok(xdg) = xdg::BaseDirectories::new() {
        dirs.push(xdg.get_data_home().join("applications"));
    }

    let mut apps: Vec<App> = dirs
    .iter()
    .flat_map(|dir| {
        WalkDir::new(dir)
        .max_depth(2)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
            && e.path().extension().map_or(false, |x| x == "desktop")
        })
        .filter_map(|e| parse(e.path()))
        .filter(|a| !a.exec.is_empty())
    })
    .collect();

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn parse(path: &std::path::Path) -> Option<App> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut kind:       Option<String> = None;
    let mut name:       Option<String> = None;
    let mut exec:       Option<String> = None;
    let mut icon:       Option<String> = None;
    let mut comment:    Option<String> = None;
    let mut categories: Vec<String>    = Vec::new();
    let mut no_display = false;
    let mut hidden     = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[Desktop Entry]" { in_entry = true;  continue; }
        if line.starts_with('[')     { in_entry = false; continue; }
        if !in_entry { continue; }

        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Type"       => kind       = Some(v.trim().to_string()),
                "Name"       => { if name.is_none()    { name    = Some(v.trim().to_string()); } }
                "Exec"       => exec       = Some(v.trim().to_string()),
                "Icon"       => icon       = Some(v.trim().to_string()),
                "Comment"    => { if comment.is_none() { comment = Some(v.trim().to_string()); } }
                "Categories" => categories = v.split(';').filter(|s| !s.is_empty()).map(String::from).collect(),
                "NoDisplay"  => no_display = v.trim() == "true",
                "Hidden"     => hidden     = v.trim() == "true",
                _ => {}
            }
        }
    }

    if hidden || no_display { return None; }
    if kind.as_deref() != Some("Application") { return None; }

    Some(App { name: name?, exec: exec?, icon, comment, categories })
}
