use serde::Serialize;
use std::path::Path;
use std::process::Command;

fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

fn parse_status_code(code: char) -> Option<GitFileState> {
    match code {
        'M' => Some(GitFileState::Modified),
        'A' => Some(GitFileState::Added),
        'D' => Some(GitFileState::Deleted),
        'R' => Some(GitFileState::Renamed),
        '?' => Some(GitFileState::Untracked),
        'U' => Some(GitFileState::Conflicted),
        _ => None,
    }
}

/// Parses `git status --porcelain=v1` lines: each is two status
/// characters (index/staged, then worktree/unstaged) followed by a
/// space and the path. See `git status --help`'s "Short Format" for the
/// exact column meanings this mirrors.
fn parse_porcelain(raw: &str) -> Vec<GitFileEntry> {
    let mut entries = Vec::new();
    for line in raw.lines() {
        if line.len() < 4 { continue; }
        let mut chars = line.chars();
        let index_char = chars.next().unwrap_or(' ');
        let worktree_char = chars.next().unwrap_or(' ');
        let path = line[3..].to_string();

        if index_char == '?' && worktree_char == '?' {
            entries.push(GitFileEntry { path, staged: false, state: GitFileState::Untracked });
            continue;
        }
        if index_char == 'U' || worktree_char == 'U' {
            entries.push(GitFileEntry { path, staged: false, state: GitFileState::Conflicted });
            continue;
        }
        if let Some(state) = parse_status_code(index_char) {
            entries.push(GitFileEntry { path: path.clone(), staged: true, state });
        }
        if let Some(state) = parse_status_code(worktree_char) {
            entries.push(GitFileEntry { path, staged: false, state });
        }
    }
    entries
}

#[derive(Debug, Clone, Serialize)]
pub struct GitRepoStatus {
    pub is_repo: bool,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub files: Vec<GitFileEntry>,
}

#[tauri::command(async)]
pub fn git_repo_status(path: String) -> GitRepoStatus {
    if !Path::new(&path).join(".git").exists() && run_git(&path, &["rev-parse", "--is-inside-work-tree"]).is_err() {
        return GitRepoStatus { is_repo: false, branch: String::new(), ahead: 0, behind: 0, files: vec![] };
    }
    let branch = run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default().trim().to_string();

    // `git status -sb` (short + branch header) gives the ahead/behind
    // counts in one call rather than a separate `rev-list --count`
    // round trip for each.
    let full = run_git(&path, &["status", "--porcelain=v1", "-b"]).unwrap_or_default();
    let mut ahead = 0u32;
    let mut behind = 0u32;
    let mut body_start = 0;
    if let Some(first_line) = full.lines().next() {
        if first_line.starts_with("##") {
            if let Some(a) = first_line.split("ahead ").nth(1) {
                ahead = a.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("0").parse().unwrap_or(0);
            }
            if let Some(b) = first_line.split("behind ").nth(1) {
                behind = b.split(|c: char| !c.is_ascii_digit()).next().unwrap_or("0").parse().unwrap_or(0);
            }
            body_start = first_line.len() + 1;
        }
    }
    let files = parse_porcelain(full.get(body_start..).unwrap_or(""));

    GitRepoStatus { is_repo: true, branch, ahead, behind, files }
}

/// Unified diff for one file. `staged` selects `git diff --cached`
/// (index vs HEAD) vs plain `git diff` (worktree vs index) — matches
/// which column in `GitFileEntry` the frontend clicked. An untracked
/// file has no diff against anything git knows about; the frontend
/// shows its raw content instead in that case (this returns an error,
/// which the Sidebar's diff viewer treats as "nothing to diff, show
/// current content").
#[tauri::command(async)]
pub fn git_diff(path: String, file: String, staged: bool) -> Result<String, String> {
    let mut args = vec!["diff", "--no-color"];
    if staged { args.push("--cached"); }
    args.push("--");
    args.push(&file);
    run_git(&path, &args)
}

#[tauri::command(async)]
pub fn git_stage(path: String, files: Vec<String>) -> Result<(), String> {
    let mut args = vec!["add", "--"];
    let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(refs);
    run_git(&path, &args).map(|_| ())
}

#[tauri::command(async)]
pub fn git_unstage(path: String, files: Vec<String>) -> Result<(), String> {
    let mut args = vec!["restore", "--staged", "--"];
    let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
    args.extend(refs);
    run_git(&path, &args).map(|_| ())
}

/// Discards *unstaged* changes to a file (`git restore <file>` — never
/// touches the index) or, for an untracked file, deletes it outright.
/// Destructive and irreversible, same as the equivalent action in any
/// git GUI — the frontend must confirm with the person before calling
/// this (see Sidebar.svelte's Git panel).
#[tauri::command(async)]
pub fn git_discard(path: String, file: String, is_untracked: bool) -> Result<(), String> {
    if is_untracked {
        let full = Path::new(&path).join(&file);
        return std::fs::remove_file(&full).map_err(|e| e.to_string());
    }
    run_git(&path, &["restore", "--", &file]).map(|_| ())
}

#[tauri::command(async)]
pub fn git_commit(path: String, message: String, files: Vec<String>) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Commit message cannot be empty.".to_string());
    }
    if !files.is_empty() {
        git_stage(path.clone(), files)?;
    }
    run_git(&path, &["commit", "-m", &message])
}

#[derive(Debug, Clone, Serialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[tauri::command(async)]
pub fn git_log(path: String, limit: u32) -> Vec<GitLogEntry> {
    // Custom `%x1f`(unit separator)/`%x1e`(record separator)-delimited
    // format — safer to split on than any character that might appear
    // in a real commit message (a plain "|" or "," could easily show up
    // in someone's commit message; these two control characters
    // essentially never will).
    let format = "%H%x1f%h%x1f%an%x1f%ad%x1f%s%x1e";
    let limit_str = limit.to_string();
    let raw = run_git(&path, &["log", &format!("--pretty=format:{format}"), "--date=relative", "-n", &limit_str]).unwrap_or_default();
    raw.split('\u{1e}')
        .filter(|r| !r.trim().is_empty())
        .filter_map(|record| {
            let parts: Vec<&str> = record.split('\u{1f}').collect();
            if parts.len() < 5 { return None; }
            Some(GitLogEntry {
                hash: parts[0].to_string(), short_hash: parts[1].to_string(),
                author: parts[2].to_string(), date: parts[3].to_string(),
                message: parts[4].trim().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modified_and_untracked_entries() {
        let raw = " M src/main.rs\n?? new_file.rs\nA  staged_new.rs\n";
        let entries = parse_porcelain(raw);
        assert!(entries.iter().any(|e| e.path == "src/main.rs" && !e.staged && e.state == GitFileState::Modified));
        assert!(entries.iter().any(|e| e.path == "new_file.rs" && e.state == GitFileState::Untracked));
        assert!(entries.iter().any(|e| e.path == "staged_new.rs" && e.staged && e.state == GitFileState::Added));
    }

    #[test]
    fn parses_both_staged_and_unstaged_change_to_same_file() {
        // "MM" = staged modification AND further unstaged modification
        // on top of that — must produce two entries, not one.
        let raw = "MM both_changed.rs\n";
        let entries = parse_porcelain(raw);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.staged));
        assert!(entries.iter().any(|e| !e.staged));
    }

    #[test]
    fn parses_conflicted_entry() {
        let raw = "UU conflicted.rs\n";
        let entries = parse_porcelain(raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].state, GitFileState::Conflicted);
    }

    #[test]
    fn ignores_short_or_blank_lines() {
        assert_eq!(parse_porcelain("\n\nab\n").len(), 0);
    }

    #[test]
    fn non_repo_path_reports_is_repo_false() {
        let dir = std::env::temp_dir().join(format!("blue-code-git-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let status = git_repo_status(dir.to_string_lossy().to_string());
        assert!(!status.is_repo);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commit_rejects_empty_message() {
        let result = git_commit("/tmp".to_string(), "   ".to_string(), vec![]);
        assert!(result.is_err());
    }
}
