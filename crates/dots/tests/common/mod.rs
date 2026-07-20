use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

#[allow(dead_code)]
pub fn write_tmp(content: &str) -> PathBuf {
    let tmp = tempdir().unwrap().keep();
    let p = tmp.join("file.txt");
    fs::write(&p, content).unwrap();
    p
}

#[allow(dead_code)]
pub fn fake_dots_dir() -> TempDir {
    let tmp = tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("src/zsh/zsh")).unwrap();
    fs::write(tmp.path().join("src/zsh/zsh/.aliases"), "").unwrap();
    fs::create_dir_all(tmp.path().join("bin")).unwrap();
    tmp
}

/// Returns (root_TempDir, bare_path, local_path).
/// Root must be kept alive for paths to remain valid.
#[allow(dead_code)]
pub fn fake_git_repo_behind(n: u32) -> (TempDir, PathBuf, PathBuf) {
    let root = tempdir().unwrap();
    let bare = root.path().join("origin");
    let local = root.path().join("local");
    fs::create_dir_all(&bare).unwrap();

    fn git(dir: &Path, args: &[&str]) {
        std::process::Command::new("git")
            .arg("-C").arg(dir)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .ok();
    }

    git(&bare, &["init"]);
    git(&bare, &["config", "user.email", "test@example.com"]);
    git(&bare, &["config", "user.name", "Test"]);
    fs::write(bare.join("a.txt"), b"init").unwrap();
    git(&bare, &["add", "."]);
    git(&bare, &["-c", "commit.gpgsign=false", "commit", "-m", "init"]);

    git(root.path(), &["clone", bare.to_str().unwrap(), local.to_str().unwrap()]);
    git(&local, &["config", "user.email", "test@example.com"]);
    git(&local, &["config", "user.name", "Test"]);

    for i in 0..n {
        fs::write(bare.join(format!("file{i}.txt")), b"content").unwrap();
        git(&bare, &["add", "."]);
        git(&bare, &["-c", "commit.gpgsign=false", "commit", "-m", &format!("commit {i}")]);
    }

    (root, bare, local)
}
