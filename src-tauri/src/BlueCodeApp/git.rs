use git2::{
    build::CheckoutBuilder, BranchType, DiffFormat, DiffOptions, ErrorCode, Repository, Signature, Status,
    StatusOptions,
};
use serde::Serialize;
use std::path::Path;

fn open(path: &str) -> Result<Repository, String> {
    Repository::discover(path).map_err(|e| e.message().to_string())
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum GitFileState {
    Modified, Added, Deleted, Renamed, Untracked, Conflicted,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitFileEntry {
    pub path: String,
    pub staged: bool,
    pub state: GitFileState,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitRepoStatus {
    pub is_repo: bool,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFileEntry>,
}

/// Zamienia bity statusu libgit2 na wpisy (plik może mieć zmianę w indeksie
/// ORAZ w katalogu roboczym — wtedy dwa wpisy, jak dawniej przy "MM").
fn entries_for(path: &str, st: Status) -> Vec<GitFileEntry> {
    let mut v = Vec::new();
    let mut push = |staged: bool, state: GitFileState| v.push(GitFileEntry { path: path.to_string(), staged, state });
    if st.contains(Status::CONFLICTED) {
        push(false, GitFileState::Conflicted);
        return v;
    }
    if st.contains(Status::WT_NEW) && !st.intersects(Status::INDEX_NEW) {
        push(false, GitFileState::Untracked);
    }
    if st.contains(Status::INDEX_NEW) { push(true, GitFileState::Added); }
    if st.contains(Status::INDEX_MODIFIED) || st.contains(Status::INDEX_TYPECHANGE) { push(true, GitFileState::Modified); }
    if st.contains(Status::INDEX_DELETED) { push(true, GitFileState::Deleted); }
    if st.contains(Status::INDEX_RENAMED) { push(true, GitFileState::Renamed); }
    if st.contains(Status::WT_MODIFIED) || st.contains(Status::WT_TYPECHANGE) { push(false, GitFileState::Modified); }
    if st.contains(Status::WT_DELETED) { push(false, GitFileState::Deleted); }
    if st.contains(Status::WT_RENAMED) { push(false, GitFileState::Renamed); }
    v
}

fn branch_name(repo: &Repository) -> String {
    match repo.head() {
        Ok(h) if h.is_branch() => h.shorthand().unwrap_or("HEAD").to_string(),
        Ok(_) => "HEAD".to_string(), // detached
        // Świeże repo bez commitów: HEAD wskazuje na jeszcze nieistniejącą gałąź.
        Err(e) if e.code() == ErrorCode::UnbornBranch => repo
            .find_reference("HEAD")
            .ok()
            .and_then(|r| r.symbolic_target().map(|s| s.trim_start_matches("refs/heads/").to_string()))
            .unwrap_or_else(|| "main".to_string()),
        Err(_) => String::new(),
    }
}

fn ahead_behind(repo: &Repository) -> (u32, u32) {
    let Ok(head) = repo.head() else { return (0, 0) };
    let Some(local) = head.target() else { return (0, 0) };
    let Some(name) = head.shorthand() else { return (0, 0) };
    let Ok(branch) = repo.find_branch(name, BranchType::Local) else { return (0, 0) };
    let Ok(up) = branch.upstream() else { return (0, 0) };
    let Some(remote) = up.get().target() else { return (0, 0) };
    repo.graph_ahead_behind(local, remote).map(|(a, b)| (a as u32, b as u32)).unwrap_or((0, 0))
}

#[tauri::command(async)]
pub fn git_repo_status(path: String) -> GitRepoStatus {
    let Ok(repo) = open(&path) else {
        return GitRepoStatus { is_repo: false, branch: String::new(), ahead: 0, behind: 0, files: vec![] };
    };
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true).renames_head_to_index(true);
    let mut files = Vec::new();
    if let Ok(statuses) = repo.statuses(Some(&mut opts)) {
        for s in statuses.iter() {
            if let Some(p) = s.path() {
                files.extend(entries_for(p, s.status()));
            }
        }
    }
    let (ahead, behind) = ahead_behind(&repo);
    GitRepoStatus { is_repo: true, branch: branch_name(&repo), ahead, behind, files }
}

fn print_diff(diff: &git2::Diff) -> Result<String, String> {
    let mut out = String::new();
    diff.print(DiffFormat::Patch, |_, _, line| {
        match line.origin() {
            '+' | '-' | ' ' => out.push(line.origin()),
            _ => {}
        }
        out.push_str(&String::from_utf8_lossy(line.content()));
        true
    })
    .map_err(err)?;
    Ok(out)
}

/// Diff jednego pliku: `staged` = indeks vs HEAD, inaczej katalog roboczy vs
/// indeks. Plik nieśledzony nie ma diffu — zwracamy pusty tekst (frontend
/// pokazuje wtedy surową zawartość).
#[tauri::command(async)]
pub fn git_diff(path: String, file: String, staged: bool) -> Result<String, String> {
    let repo = open(&path)?;
    let mut opts = DiffOptions::new();
    opts.pathspec(&file);
    let diff = if staged {
        let tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(tree.as_ref(), None, Some(&mut opts)).map_err(err)?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut opts)).map_err(err)?
    };
    print_diff(&diff)
}

#[tauri::command(async)]
pub fn git_stage(path: String, files: Vec<String>) -> Result<(), String> {
    let repo = open(&path)?;
    let workdir = repo.workdir().ok_or("repozytorium bez katalogu roboczego")?.to_path_buf();
    let mut index = repo.index().map_err(err)?;
    for f in &files {
        let rel = Path::new(f);
        if workdir.join(rel).exists() {
            index.add_path(rel).map_err(err)?;
        } else {
            index.remove_path(rel).map_err(err)?; // plik usunięty z dysku
        }
    }
    index.write().map_err(err)
}

#[tauri::command(async)]
pub fn git_unstage(path: String, files: Vec<String>) -> Result<(), String> {
    let repo = open(&path)?;
    let head_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let result = match &head_commit {
        Some(commit) => repo
            .reset_default(Some(commit.as_object()), files.iter().map(|s| s.as_str()))
            .map_err(err),
        None => {
            // Brak HEAD (pierwszy commit): "odstage'owanie" = usunięcie z indeksu.
            let mut index = repo.index().map_err(err)?;
            for f in &files {
                index.remove_path(Path::new(f)).map_err(err)?;
            }
            index.write().map_err(err)
        }
    };
    drop(head_commit);
    result
}

/// Odrzuca zmiany *niezatwierdzone w indeksie* (jak `git restore <plik>`) albo,
/// dla pliku nieśledzonego, kasuje go. Nieodwracalne — frontend pyta o
/// potwierdzenie przed wywołaniem (Sidebar.svelte, panel Git).
#[tauri::command(async)]
pub fn git_discard(path: String, file: String, is_untracked: bool) -> Result<(), String> {
    let repo = open(&path)?;
    if is_untracked {
        let workdir = repo.workdir().ok_or("repozytorium bez katalogu roboczego")?;
        return std::fs::remove_file(workdir.join(&file)).map_err(|e| e.to_string());
    }
    let mut cb = CheckoutBuilder::new();
    cb.path(&file).force();
    repo.checkout_index(None, Some(&mut cb)).map_err(err)
}

fn signature(repo: &Repository) -> Result<Signature<'static>, String> {
    repo.signature()
        .or_else(|_| Signature::now("Blue Code", "blue-code@localhost"))
        .map_err(err)
}

#[tauri::command(async)]
pub fn git_commit(path: String, message: String, files: Vec<String>) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty.".to_string());
    }
    if !files.is_empty() {
        git_stage(path.clone(), files)?;
    }
    let repo = open(&path)?;
    let mut index = repo.index().map_err(err)?;
    let tree = repo.find_tree(index.write_tree().map_err(err)?).map_err(err)?;
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    if let Some(p) = &parent {
        if p.tree_id() == tree.id() {
            return Err("Nothing to commit (brak zmian w indeksie).".to_string());
        }
    } else if tree.is_empty() {
        return Err("Nothing to commit (brak zmian w indeksie).".to_string());
    }
    let sig = signature(&repo)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message.trim_end(), &tree, &parents).map_err(err)?;
    let short = oid.to_string()[..7].to_string();
    Ok(format!("[{} {}] {}", branch_name(&repo), short, message.lines().next().unwrap_or("")))
}

#[derive(Debug, Clone, Serialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

/// "3 hours ago" w stylu `git log --date=relative`.
pub fn relative_time(now: i64, then: i64) -> String {
    let d = (now - then).max(0);
    let (n, unit) = match d {
        0..=59 => (d, "second"),
        60..=3599 => (d / 60, "minute"),
        3600..=86399 => (d / 3600, "hour"),
        86400..=1_209_599 => (d / 86400, "day"),
        1_209_600..=5_183_999 => (d / 604_800, "week"),
        5_184_000..=62_999_999 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[tauri::command(async)]
pub fn git_log(path: String, limit: u32) -> Vec<GitLogEntry> {
    let Ok(repo) = open(&path) else { return vec![] };
    let Ok(mut walk) = repo.revwalk() else { return vec![] };
    if walk.push_head().is_err() {
        return vec![]; // brak commitów
    }
    let _ = walk.set_sorting(git2::Sort::TIME);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    walk.filter_map(|id| id.ok())
        .filter_map(|id| repo.find_commit(id).ok())
        .take(limit as usize)
        .map(|c| {
            let hash = c.id().to_string();
            GitLogEntry {
                short_hash: hash[..7].to_string(),
                hash,
                author: c.author().name().unwrap_or("").to_string(),
                date: relative_time(now, c.time().seconds()),
                message: c.summary().unwrap_or("").trim().to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Tymczasowe repo z ustawioną tożsamością (CI/sandbox nie ma ~/.gitconfig).
    fn temp_repo(tag: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("blue-code-git2-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let repo = Repository::init(&dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Tester").unwrap();
        cfg.set_str("user.email", "t@example.com").unwrap();
        let p = dir.to_string_lossy().to_string();
        (dir, p)
    }

    #[test]
    fn non_repo_path_reports_is_repo_false() {
        let dir = std::env::temp_dir().join(format!("blue-code-git-none-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        assert!(!git_repo_status(dir.to_string_lossy().to_string()).is_repo);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn full_flow_stage_commit_diff_log_discard() {
        let (dir, p) = temp_repo("flow");
        fs::write(dir.join("a.txt"), "one\n").unwrap();

        let st = git_repo_status(p.clone());
        assert!(st.is_repo);
        assert!(st.files.iter().any(|f| f.path == "a.txt" && f.state == GitFileState::Untracked && !f.staged));

        git_stage(p.clone(), vec!["a.txt".into()]).unwrap();
        let st = git_repo_status(p.clone());
        assert!(st.files.iter().any(|f| f.path == "a.txt" && f.state == GitFileState::Added && f.staged));
        assert!(git_diff(p.clone(), "a.txt".into(), true).unwrap().contains("+one"));

        let msg = git_commit(p.clone(), "first".into(), vec![]).unwrap();
        assert!(msg.contains("first"));
        assert!(git_repo_status(p.clone()).files.is_empty());
        assert!(git_commit(p.clone(), "again".into(), vec![]).is_err()); // nic do zacommitowania

        // zmiana w katalogu roboczym → diff niezestage'owany, potem discard
        fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        let st = git_repo_status(p.clone());
        assert!(st.files.iter().any(|f| f.path == "a.txt" && f.state == GitFileState::Modified && !f.staged));
        assert!(git_diff(p.clone(), "a.txt".into(), false).unwrap().contains("+two"));
        git_discard(p.clone(), "a.txt".into(), false).unwrap();
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "one\n");

        let log = git_log(p.clone(), 10);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].message, "first");
        assert_eq!(log[0].author, "Tester");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn staged_and_unstaged_change_gives_two_entries_and_unstage_works() {
        let (dir, p) = temp_repo("mm");
        fs::write(dir.join("b.txt"), "1\n").unwrap();
        git_commit(p.clone(), "init".into(), vec!["b.txt".into()]).unwrap();
        fs::write(dir.join("b.txt"), "2\n").unwrap();
        git_stage(p.clone(), vec!["b.txt".into()]).unwrap();
        fs::write(dir.join("b.txt"), "3\n").unwrap();
        let st = git_repo_status(p.clone());
        assert_eq!(st.files.iter().filter(|f| f.path == "b.txt").count(), 2);
        git_unstage(p.clone(), vec!["b.txt".into()]).unwrap();
        assert!(git_repo_status(p.clone()).files.iter().all(|f| !f.staged));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn deleted_file_and_empty_message() {
        let (dir, p) = temp_repo("del");
        fs::write(dir.join("c.txt"), "x\n").unwrap();
        git_commit(p.clone(), "init".into(), vec!["c.txt".into()]).unwrap();
        fs::remove_file(dir.join("c.txt")).unwrap();
        git_stage(p.clone(), vec!["c.txt".into()]).unwrap();
        assert!(git_repo_status(p.clone()).files.iter().any(|f| f.state == GitFileState::Deleted && f.staged));
        assert!(git_commit(p.clone(), "   ".into(), vec![]).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn relative_time_format() {
        assert_eq!(relative_time(100, 100), "0 seconds ago");
        assert_eq!(relative_time(3600, 0), "1 hour ago");
        assert_eq!(relative_time(2 * 86400, 0), "2 days ago");
        assert_eq!(relative_time(70 * 86400, 0), "2 months ago");
    }
}
