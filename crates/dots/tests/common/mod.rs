use std::fs;
use std::path::PathBuf;
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
