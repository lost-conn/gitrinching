use git2::{Repository, Sort};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::state::CommitNode;

/// Scan a directory for git repositories.
/// Checks the root itself, then walks up to 2 levels of subdirectories.
/// Skips hidden directories (starting with '.').
pub fn scan_for_repos(root: &Path) -> Vec<PathBuf> {
    let mut repos = Vec::new();

    // Check root itself
    if Repository::open(root).is_ok() {
        repos.push(root.to_path_buf());
    }

    // Walk 2 levels deep
    for depth in 0..2 {
        let dirs_to_scan: Vec<PathBuf> = if depth == 0 {
            match std::fs::read_dir(root) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .map(|e| e.path())
                    .collect(),
                Err(_) => Vec::new(),
            }
        } else {
            // Collect level-1 dirs, then scan their children
            let mut level2 = Vec::new();
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    if entry.file_name().to_string_lossy().starts_with('.') {
                        continue;
                    }
                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                        for sub in sub_entries.filter_map(|e| e.ok()) {
                            if sub.file_type().map(|t| t.is_dir()).unwrap_or(false)
                                && !sub.file_name().to_string_lossy().starts_with('.')
                            {
                                level2.push(sub.path());
                            }
                        }
                    }
                }
            }
            level2
        };

        for dir in dirs_to_scan {
            if !repos.contains(&dir) && Repository::open(&dir).is_ok() {
                repos.push(dir);
            }
        }
    }

    repos.sort();
    repos.dedup();
    repos
}

/// Load commits from a git repository at the given path.
/// Returns commits in topological + time order (newest first), max 500.
pub fn load_repo(path: &str) -> Result<Vec<CommitNode>, String> {
    let repo = Repository::open(path).map_err(|e| format!("Failed to open repo: {e}"))?;

    // Collect branch labels: oid -> list of branch names
    let mut branch_map: HashMap<String, Vec<String>> = HashMap::new();
    if let Ok(branches) = repo.branches(None) {
        for branch_result in branches {
            if let Ok((branch, _btype)) = branch_result {
                if let (Some(name), Ok(reference)) = (
                    branch.name().ok().flatten().map(|s| s.to_string()),
                    branch.into_reference().peel_to_commit(),
                ) {
                    branch_map
                        .entry(reference.id().to_string())
                        .or_default()
                        .push(name);
                }
            }
        }
    }

    // Find HEAD oid
    let head_oid = repo
        .head()
        .ok()
        .and_then(|r| r.peel_to_commit().ok())
        .map(|c| c.id().to_string());

    // Revwalk from all branch tips
    let mut revwalk = repo.revwalk().map_err(|e| format!("Revwalk error: {e}"))?;
    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME).ok();

    // Push all references
    if revwalk.push_glob("refs/*").is_err() {
        // Fallback: push HEAD
        revwalk.push_head().map_err(|e| format!("Push HEAD error: {e}"))?;
    }

    let mut commits = Vec::new();
    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let oid_str = oid.to_string();
        let short_oid = oid_str[..7.min(oid_str.len())].to_string();
        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let author = commit
            .author()
            .name()
            .unwrap_or("Unknown")
            .to_string();
        let timestamp = commit.time().seconds();
        let parent_oids: Vec<String> = commit.parent_ids().map(|p| p.to_string()).collect();
        let branch_labels = branch_map.remove(&oid_str).unwrap_or_default();
        let is_head = head_oid.as_deref() == Some(&oid_str);

        commits.push(CommitNode {
            oid: oid_str,
            short_oid,
            message,
            author,
            timestamp,
            parent_oids,
            branch_labels,
            is_head,
            lane: 0,
            row: 0,
        });

        if commits.len() >= 500 {
            break;
        }
    }

    Ok(commits)
}
