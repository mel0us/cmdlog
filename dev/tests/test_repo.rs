use std::fs;
use std::path::PathBuf;

use cmdlog::repo::{extract_short_name, RepoCache, RepoResolver};

fn tmp_dir(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cmdlog_test_repo_{}_{}", std::process::id(), suffix
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// extract_short_name
// ---------------------------------------------------------------------------

#[test]
fn extract_https_url() {
    assert_eq!(
        extract_short_name("https://github.com/owner/repo.git"),
        "owner/repo"
    );
}

#[test]
fn extract_https_no_git_suffix() {
    assert_eq!(
        extract_short_name("https://github.com/owner/repo"),
        "owner/repo"
    );
}

#[test]
fn extract_ssh_url() {
    assert_eq!(
        extract_short_name("git@github.com:owner/repo.git"),
        "owner/repo"
    );
}

#[test]
fn extract_ssh_no_git_suffix() {
    assert_eq!(
        extract_short_name("git@github.com:owner/repo"),
        "owner/repo"
    );
}

#[test]
fn extract_git_protocol() {
    assert_eq!(
        extract_short_name("git://github.com/owner/repo.git"),
        "owner/repo"
    );
}

#[test]
fn extract_ssh_scheme() {
    assert_eq!(
        extract_short_name("ssh://git@github.com/owner/repo.git"),
        "owner/repo"
    );
}

#[test]
fn extract_http_url() {
    assert_eq!(
        extract_short_name("http://github.com/owner/repo.git"),
        "owner/repo"
    );
}

#[test]
fn extract_deep_path() {
    // Only last two segments kept
    assert_eq!(
        extract_short_name("https://gitlab.com/group/subgroup/repo.git"),
        "subgroup/repo"
    );
}

#[test]
fn extract_two_segment_path() {
    // After stripping scheme, "host/repo" has 2 segments → "host/repo"
    assert_eq!(extract_short_name("https://host/repo.git"), "host/repo");
}

#[test]
fn extract_bare_single_segment() {
    // A bare path with only one segment
    assert_eq!(extract_short_name("repo"), "repo");
}

#[test]
fn extract_empty_url() {
    assert_eq!(extract_short_name(""), "");
}

#[test]
fn extract_ssh_with_port() {
    // git@host:owner/repo is SSH shorthand (colon before path)
    assert_eq!(
        extract_short_name("git@gitlab.nvidia.com:owner/repo.git"),
        "owner/repo"
    );
}

// ---------------------------------------------------------------------------
// Repo resolution via RepoCache (covers URL parsing + fallback)
// ---------------------------------------------------------------------------

#[test]
fn resolve_https_origin() {
    let dir = tmp_dir("resolve_https");
    let repo = dir.join("project");
    let git = repo.join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(
        git.join("config"),
        "[core]\n\
         \trepositoryformatversion = 0\n\
         [remote \"origin\"]\n\
         \turl = https://github.com/user/project.git\n\
         \tfetch = +refs/heads/*:refs/remotes/origin/*\n\
         [branch \"main\"]\n\
         \tremote = origin\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let info = cache.resolve(repo.to_str().unwrap()).unwrap();
    assert_eq!(info.name, "user/project");

    cleanup(&dir);
}

#[test]
fn resolve_ssh_origin() {
    let dir = tmp_dir("resolve_ssh");
    let repo = dir.join("repo");
    let git = repo.join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(
        git.join("config"),
        "[remote \"origin\"]\n\
         \turl = git@github.com:owner/repo.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let info = cache.resolve(repo.to_str().unwrap()).unwrap();
    assert_eq!(info.name, "owner/repo");

    cleanup(&dir);
}

#[test]
fn resolve_no_origin_falls_back_to_other_remote() {
    let dir = tmp_dir("resolve_no_origin");
    let repo = dir.join("proj");
    let git = repo.join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(
        git.join("config"),
        "[core]\n\
         \trepositoryformatversion = 0\n\
         [remote \"upstream\"]\n\
         \turl = https://github.com/other/repo.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let info = cache.resolve(repo.to_str().unwrap()).unwrap();
    assert_eq!(info.name, "other/repo");

    cleanup(&dir);
}

#[test]
fn resolve_origin_preferred_over_other_remotes() {
    let dir = tmp_dir("resolve_prefer_origin");
    let repo = dir.join("proj");
    let git = repo.join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(
        git.join("config"),
        "[remote \"upstream\"]\n\
         \turl = https://github.com/upstream/proj.git\n\
         [remote \"origin\"]\n\
         \turl = https://github.com/origin/proj.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let info = cache.resolve(repo.to_str().unwrap()).unwrap();
    assert_eq!(info.name, "origin/proj");

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// resolve_repo_name — fallback to user/dirname
// ---------------------------------------------------------------------------

#[test]
fn repo_cache_resolves_local_repo_without_remote() {
    let dir = tmp_dir("no_remote");
    let repo_dir = dir.join("my-project");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        "[core]\n\trepositoryformatversion = 0\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let result = cache.resolve(repo_dir.to_str().unwrap());
    assert!(result.is_some());
    let info = result.unwrap();
    // Should be $USER/my-project (or local/my-project if USER unset)
    assert!(info.name.ends_with("/my-project"), "got: {}", info.name);
    assert_eq!(info.root, repo_dir.to_str().unwrap());

    cleanup(&dir);
}

#[test]
fn repo_cache_falls_back_to_non_origin_remote() {
    let dir = tmp_dir("non_origin");
    let repo_dir = dir.join("proj");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        "[remote \"upstream\"]\n\
         \turl = https://github.com/team/proj.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let info = cache.resolve(repo_dir.to_str().unwrap()).unwrap();
    assert_eq!(info.name, "team/proj");

    cleanup(&dir);
}

// ---------------------------------------------------------------------------
// RepoCache
// ---------------------------------------------------------------------------

#[test]
fn repo_cache_load_empty() {
    let dir = tmp_dir("cache_empty");
    let cache = RepoCache::load(&dir);
    // No crash, empty cache
    drop(cache);
    cleanup(&dir);
}

#[test]
fn repo_cache_save_load_roundtrip() {
    let dir = tmp_dir("cache_roundtrip");

    // Create a fake git repo so resolve() works
    let repo_dir = dir.join("myrepo");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        "[remote \"origin\"]\n\
         \turl = https://github.com/test/myrepo.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let result = cache.resolve(repo_dir.to_str().unwrap());
    assert!(result.is_some());
    let info = result.unwrap();
    assert_eq!(info.name, "test/myrepo");
    assert_eq!(info.root, repo_dir.to_str().unwrap());

    cache.save();

    // Reload and verify cached
    let mut cache2 = RepoCache::load(&dir);
    let result2 = cache2.resolve(repo_dir.to_str().unwrap());
    assert!(result2.is_some());
    let info2 = result2.unwrap();
    assert_eq!(info2.name, "test/myrepo");
    assert_eq!(info2.root, repo_dir.to_str().unwrap());

    cleanup(&dir);
}

#[test]
fn repo_cache_repo_name() {
    let dir = tmp_dir("cache_name");

    let repo_dir = dir.join("project");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        "[remote \"origin\"]\n\
         \turl = git@github.com:org/project.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    assert_eq!(cache.repo_name(repo_dir.to_str().unwrap()), "org/project");

    cleanup(&dir);
}

#[test]
fn repo_cache_relpath() {
    let dir = tmp_dir("cache_relpath");

    let repo_dir = dir.join("project");
    let sub_dir = repo_dir.join("src").join("main");
    let git_dir = repo_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(
        git_dir.join("config"),
        "[remote \"origin\"]\n\
         \turl = https://github.com/org/project.git\n",
    )
    .unwrap();

    let mut cache = RepoCache::load(&dir);
    let relpath = cache.resolve_name_and_relpath(sub_dir.to_str().unwrap()).1;
    assert_eq!(relpath, "src/main");

    cleanup(&dir);
}

#[test]
fn repo_cache_not_in_repo() {
    let dir = tmp_dir("cache_no_repo");
    let outside = dir.join("not_a_repo");
    fs::create_dir_all(&outside).unwrap();

    let mut cache = RepoCache::load(&dir);
    assert_eq!(cache.repo_name(outside.to_str().unwrap()), "");
    assert_eq!(cache.resolve_name_and_relpath(outside.to_str().unwrap()).1, "");

    cleanup(&dir);
}
