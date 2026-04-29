use std::path::{Path, PathBuf};
use std::process::Command;

use git2::{DiffOptions, Repository, Status, StatusOptions, StatusShow};

use super::types::{WorktreeDiffFile, WorktreeDiffStats, WorktreeStatus};

pub struct WorktreeCreateResult {
    pub branch_was_created: bool,
    pub repo_root: PathBuf,
    pub base_branch: Option<String>,
    pub base_commit: String,
    pub dirty_source: bool,
}

pub struct WorktreeDiffParts {
    pub files: Vec<WorktreeDiffFile>,
    pub stats: WorktreeDiffStats,
    pub patch: String,
    pub patch_truncated: bool,
}

fn status_options(show: StatusShow) -> StatusOptions {
    let mut options = StatusOptions::new();
    options
        .disable_pathspec_match(true)
        .include_ignored(false)
        .include_unmodified(false)
        .include_unreadable(false)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show(show);
    options
}

fn is_index_changed(status: Status) -> bool {
    status.intersects(
        Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE,
    )
}

fn is_workdir_changed(status: Status) -> bool {
    status.intersects(
        Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_RENAMED
            | Status::WT_TYPECHANGE,
    )
}

pub fn discover_repo(path: &Path) -> Result<Repository, String> {
    Repository::discover(path).map_err(|e| {
        format!(
            "Workspace is not a git repository '{}': {}",
            path.display(),
            e
        )
    })
}

pub fn repo_root(repo: &Repository) -> Result<PathBuf, String> {
    repo.workdir()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Repository has no working tree".to_string())
}

pub fn current_branch(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(|s| s.to_string()))
}

pub fn head_commit(repo: &Repository) -> Result<String, String> {
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to get HEAD commit: {}", e))?;
    Ok(commit.id().to_string())
}

pub fn commit_for_ref(repo: &Repository, refish: &str) -> Result<String, String> {
    let object = repo
        .revparse_single(refish)
        .map_err(|e| format!("Failed to resolve base '{}': {}", refish, e))?;
    let commit = object
        .peel_to_commit()
        .map_err(|e| format!("Base '{}' is not a commit: {}", refish, e))?;
    Ok(commit.id().to_string())
}

pub fn has_uncommitted_changes(repo: &Repository) -> Result<bool, String> {
    let statuses = repo
        .statuses(Some(&mut status_options(StatusShow::IndexAndWorkdir)))
        .map_err(|e| format!("Failed to get git status: {}", e))?;
    Ok(statuses.iter().any(|entry| {
        let status = entry.status();
        is_index_changed(status) || is_workdir_changed(status)
    }))
}

pub fn create_worktree(
    source_root: &Path,
    worktree_path: &Path,
    worktree_name: &str,
    branch_name: &str,
    base_ref: Option<&str>,
) -> Result<WorktreeCreateResult, String> {
    let repo = discover_repo(source_root)?;
    let repo_root = repo_root(&repo)?;
    let base_branch = base_ref
        .map(|s| s.to_string())
        .or_else(|| current_branch(&repo));
    let base_commit = match base_ref {
        Some(base) => commit_for_ref(&repo, base)?,
        None => head_commit(&repo)?,
    };
    let dirty_source = has_uncommitted_changes(&repo).unwrap_or(false);

    let commit_oid =
        git2::Oid::from_str(&base_commit).map_err(|e| format!("Invalid commit OID: {}", e))?;
    let commit = repo
        .find_commit(commit_oid)
        .map_err(|e| format!("Failed to find base commit: {}", e))?;

    let ref_name = format!("refs/heads/{}", branch_name);
    let (branch_ref, branch_was_created) = match repo.find_reference(&ref_name) {
        Ok(reference) => (reference, false),
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            let reference = repo
                .branch(branch_name, &commit, false)
                .map_err(|e| format!("Failed to create branch '{}': {}", branch_name, e))?
                .into_reference();
            (reference, true)
        }
        Err(e) => return Err(format!("Failed to look up branch '{}': {}", branch_name, e)),
    };

    let mut options = git2::WorktreeAddOptions::new();
    options.reference(Some(&branch_ref));
    repo.worktree(worktree_name, worktree_path, Some(&mut options))
        .map_err(|e| format!("Failed to create worktree '{}': {}", worktree_name, e))?;

    Ok(WorktreeCreateResult {
        branch_was_created,
        repo_root,
        base_branch,
        base_commit,
        dirty_source,
    })
}

pub fn remove_worktree(
    source_root: &Path,
    worktree_name: &str,
    worktree_path: &Path,
) -> Vec<String> {
    let mut warnings = Vec::new();
    match discover_repo(source_root) {
        Ok(repo) => match repo.find_worktree(worktree_name) {
            Ok(worktree) => {
                let mut options = git2::WorktreePruneOptions::new();
                options.valid(true);
                options.locked(true);
                options.working_tree(true);
                if let Err(e) = worktree.prune(Some(&mut options)) {
                    warnings.push(format!(
                        "Failed to prune worktree '{}': {}",
                        worktree_name, e
                    ));
                }
            }
            Err(e) => warnings.push(format!(
                "Could not find worktree '{}' in git metadata: {}",
                worktree_name, e
            )),
        },
        Err(e) => warnings.push(e),
    }

    if worktree_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(worktree_path) {
            warnings.push(format!(
                "Failed to remove worktree directory '{}': {}",
                worktree_path.display(),
                e
            ));
        }
    }
    warnings
}

pub fn delete_branch(source_root: &Path, branch_name: &str) -> Result<bool, String> {
    let repo = discover_repo(source_root)?;
    let result = match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(mut branch) => {
            branch
                .delete()
                .map_err(|e| format!("Failed to delete branch '{}': {}", branch_name, e))?;
            Ok(true)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
        Err(e) => Err(format!("Failed to find branch '{}': {}", branch_name, e)),
    };
    result
}

pub fn status_for_path(path: &Path) -> WorktreeStatus {
    if !path.exists() {
        return WorktreeStatus {
            path_exists: false,
            is_git_worktree: false,
            dirty: false,
            staged_count: 0,
            unstaged_count: 0,
            untracked_count: 0,
            branch: None,
            head_commit: None,
            error: None,
        };
    }

    match discover_repo(path) {
        Ok(repo) => {
            let mut status = WorktreeStatus {
                path_exists: true,
                is_git_worktree: true,
                dirty: false,
                staged_count: 0,
                unstaged_count: 0,
                untracked_count: 0,
                branch: current_branch(&repo),
                head_commit: head_commit(&repo).ok(),
                error: None,
            };
            match repo.statuses(Some(&mut status_options(StatusShow::IndexAndWorkdir))) {
                Ok(statuses) => {
                    for entry in statuses.iter() {
                        let entry_status = entry.status();
                        if is_index_changed(entry_status) {
                            status.staged_count += 1;
                        }
                        if entry_status.is_wt_new() && !is_index_changed(entry_status) {
                            status.untracked_count += 1;
                        } else if is_workdir_changed(entry_status) {
                            status.unstaged_count += 1;
                        }
                    }
                    status.dirty = status.staged_count > 0
                        || status.unstaged_count > 0
                        || status.untracked_count > 0;
                }
                Err(e) => {
                    status.error = Some(format!("Failed to read git status: {}", e));
                }
            }
            status
        }
        Err(e) => WorktreeStatus {
            path_exists: true,
            is_git_worktree: false,
            dirty: false,
            staged_count: 0,
            unstaged_count: 0,
            untracked_count: 0,
            branch: None,
            head_commit: None,
            error: Some(e),
        },
    }
}

fn run_git(path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git {:?}: {}", args, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn run_git_lossy(path: &Path, args: &[&str]) -> String {
    run_git(path, args).unwrap_or_default()
}

fn parse_name_status(output: &str, source: &str) -> Vec<WorktreeDiffFile> {
    output
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                return None;
            }
            let status = parts[0].chars().next().unwrap_or('M').to_string();
            let path = parts.last().unwrap_or(&parts[1]).to_string();
            Some(WorktreeDiffFile {
                path,
                status,
                source: source.to_string(),
            })
        })
        .collect()
}

fn list_untracked(path: &Path) -> Vec<WorktreeDiffFile> {
    run_git_lossy(path, &["ls-files", "--others", "--exclude-standard"])
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| WorktreeDiffFile {
            path: line.to_string(),
            status: "A".to_string(),
            source: "untracked".to_string(),
        })
        .collect()
}

fn push_bounded(target: &mut String, text: &str, max_bytes: usize, truncated: &mut bool) {
    if *truncated || target.len() >= max_bytes {
        *truncated = true;
        return;
    }
    let remaining = max_bytes - target.len();
    if text.len() <= remaining {
        target.push_str(text);
        return;
    }
    let mut end = remaining;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    target.push_str(&text[..end]);
    target.push_str("\n");
    *truncated = true;
}

fn append_patch_section(
    patch: &mut String,
    title: &str,
    body: &str,
    max_bytes: usize,
    truncated: &mut bool,
) {
    if body.trim().is_empty() {
        return;
    }
    push_bounded(patch, &format!("\n## {}\n", title), max_bytes, truncated);
    push_bounded(patch, body, max_bytes, truncated);
    if !body.ends_with('\n') {
        push_bounded(patch, "\n", max_bytes, truncated);
    }
}

fn append_untracked_patch(
    root: &Path,
    files: &[WorktreeDiffFile],
    patch: &mut String,
    max_bytes: usize,
    truncated: &mut bool,
) {
    for file in files {
        if *truncated {
            return;
        }
        let file_path = root.join(&file.path);
        let Ok(content) = std::fs::read_to_string(&file_path) else {
            continue;
        };
        let mut body = format!(
            "diff --git a/{} b/{}\nnew file mode 100644\n--- /dev/null\n+++ b/{}\n@@\n",
            file.path, file.path, file.path
        );
        for line in content.lines() {
            body.push('+');
            body.push_str(line);
            body.push('\n');
        }
        append_patch_section(
            patch,
            &format!("untracked {}", file.path),
            &body,
            max_bytes,
            truncated,
        );
    }
}

pub fn diff_for_path(
    root: &Path,
    base_commit: Option<&str>,
    max_patch_bytes: usize,
) -> Result<WorktreeDiffParts, String> {
    discover_repo(root)?;
    let mut files = Vec::new();
    let mut stats = WorktreeDiffStats::default();
    let mut patch = String::new();
    let mut patch_truncated = false;

    if let Some(base) = base_commit {
        let committed = parse_name_status(
            &run_git_lossy(root, &["diff", "--name-status", &format!("{}..HEAD", base)]),
            "committed",
        );
        stats.committed_files = committed.len();
        files.extend(committed);
        let committed_patch =
            run_git_lossy(root, &["diff", "--no-ext-diff", &format!("{}..HEAD", base)]);
        append_patch_section(
            &mut patch,
            "committed",
            &committed_patch,
            max_patch_bytes,
            &mut patch_truncated,
        );
    }

    let staged = parse_name_status(
        &run_git_lossy(root, &["diff", "--cached", "--name-status"]),
        "staged",
    );
    stats.staged_files = staged.len();
    files.extend(staged);
    let staged_patch = run_git_lossy(root, &["diff", "--no-ext-diff", "--cached"]);
    append_patch_section(
        &mut patch,
        "staged",
        &staged_patch,
        max_patch_bytes,
        &mut patch_truncated,
    );

    let unstaged = parse_name_status(&run_git_lossy(root, &["diff", "--name-status"]), "unstaged");
    stats.unstaged_files = unstaged.len();
    files.extend(unstaged);
    let unstaged_patch = run_git_lossy(root, &["diff", "--no-ext-diff"]);
    append_patch_section(
        &mut patch,
        "unstaged",
        &unstaged_patch,
        max_patch_bytes,
        &mut patch_truncated,
    );

    let untracked = list_untracked(root);
    stats.untracked_files = untracked.len();
    append_untracked_patch(
        root,
        &untracked,
        &mut patch,
        max_patch_bytes,
        &mut patch_truncated,
    );
    files.extend(untracked);
    stats.files_changed = files.len();

    if patch_truncated {
        push_bounded(
            &mut patch,
            "\n[patch truncated]\n",
            max_patch_bytes.saturating_add(128),
            &mut false,
        );
    }

    Ok(WorktreeDiffParts {
        files,
        stats,
        patch,
        patch_truncated,
    })
}

#[allow(dead_code)]
pub fn diff_head_to_workdir_as_string(
    repository: &Repository,
    max_size: usize,
) -> Result<String, String> {
    let mut diff_options = DiffOptions::new();
    diff_options.include_untracked(true);
    diff_options.recurse_untracked_dirs(true);
    let head = repository
        .head()
        .and_then(|head_ref| head_ref.peel_to_tree())
        .map_err(|e| format!("Failed to get HEAD tree: {}", e))?;
    let diff = repository
        .diff_tree_to_workdir(Some(&head), Some(&mut diff_options))
        .map_err(|e| format!("Failed to generate diff: {}", e))?;
    let mut diff_str = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        let line_content = std::str::from_utf8(line.content()).unwrap_or("");
        if diff_str.len() + line_content.len() < max_size {
            diff_str.push(line.origin());
            diff_str.push_str(line_content);
        }
        true
    })
    .map_err(|e| format!("Failed to print diff: {}", e))?;
    Ok(diff_str)
}
