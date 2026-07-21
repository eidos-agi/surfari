use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use crate::color;

struct SkillInfo {
    name: String,
    description: String,
    dir: PathBuf,
    /// When true, the skill is omitted from `skills list` and `skills get --all`
    /// but can still be fetched by name via `skills get <name>`. Used for
    /// bootstrap stubs that exist for external tooling (e.g. `npx skills add`)
    /// but aren't the intended entry point for agents already inside the CLI.
    hidden: bool,
}

/// Skill content is split across two directories:
///
/// - `skills/` — discovery stubs (picked up by `npx skills add`). Carry
///   `hidden: true` so they don't show up in `skills list` or `skills get
///   --all` inside the CLI, since they exist only to redirect external
///   agents to `skills get core`.
/// - `skill-data/` — runtime skill content served by the CLI (`core`,
///   `electron`, `slack`, `dogfood`, etc.).
///
/// Both are shipped in the npm package and searched by `discover_skills`.
const SKILL_DIRS: &[&str] = &["skills", "skill-data"];

/// Locate the packaged skill root relative to a given executable path.
///
/// This is the "packaged fallback" tier: the skill content that shipped inside
/// the binary's install tree. It is pure over `exe` so it can be exercised by
/// the standalone-install-layout test without depending on the real
/// `current_exe()`.
///
/// Resolution order (relative to `exe`):
/// 1. `../` relative to the executable (npm/standalone installs: binary is in
///    `bin/`, skills sit next to it under the package root).
/// 2. Walk up from the executable to find a project root with `skills/`
///    (dev builds where the binary is in `target/debug/` or `target/release/`).
fn package_root_from_exe(exe: &Path) -> Option<PathBuf> {
    let parent = exe.parent()?;

    // npm/standalone install layout: bin/<binary> -> ../
    let candidate = parent.join("..");
    if candidate.join("skills").is_dir() {
        return Some(candidate.canonicalize().unwrap_or(candidate));
    }

    // dev build layout: walk up from target/debug/ or target/release/
    let mut dir = parent;
    loop {
        if dir.join("skills").is_dir() {
            return Some(dir.to_path_buf());
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => return None,
        }
    }
}

/// Inputs to skill-directory resolution, injected so precedence can be tested
/// without touching process-global env vars or the real executable path.
struct SkillsResolveConfig {
    /// `AGENT_BROWSER_SKILLS_DIR` — explicit override, used exclusively.
    override_dir: Option<PathBuf>,
    /// Root of the external versioned cache (`<root>/active` holds live content).
    cache_root: Option<PathBuf>,
    /// Canonicalized executable path, used to find the packaged fallback.
    exe: Option<PathBuf>,
}

/// Resolve the skill directories to search, honoring the cache precedence:
///
/// 1. **Explicit override** (`AGENT_BROWSER_SKILLS_DIR`) — used as-is, alone.
/// 2. **Verified Surfari cache** — `<cache_root>/active`, only when it carries
///    valid provenance and at least one discoverable skill.
/// 3. **Packaged fallback** — the `skills/` + `skill-data/` shipped in the
///    install tree.
///
/// This is rebuild-free: a successful `skills update` swaps the cache's `active`
/// dir, and the very next `list`/`get`/`path` sees it — no recompile, no reinstall.
/// It never fetches; resolution is pure filesystem inspection.
fn resolve_skills_dirs(cfg: &SkillsResolveConfig) -> Vec<PathBuf> {
    // 1. Explicit override wins outright.
    if let Some(dir) = &cfg.override_dir {
        if dir.is_dir() {
            return vec![dir.clone()];
        }
    }

    // 2. Verified external cache, if current.
    if let Some(root) = &cfg.cache_root {
        let active = active_dir(root);
        if is_cache_current(&active) {
            return vec![active];
        }
    }

    // 3. Packaged fallback.
    if let Some(root) = cfg.exe.as_deref().and_then(package_root_from_exe) {
        return SKILL_DIRS
            .iter()
            .map(|d| root.join(d))
            .filter(|p| p.is_dir())
            .collect();
    }

    vec![]
}

/// Collect all skill directories to search, from the live environment.
fn find_skills_dirs() -> Vec<PathBuf> {
    let cfg = SkillsResolveConfig {
        override_dir: env::var("AGENT_BROWSER_SKILLS_DIR").ok().map(PathBuf::from),
        cache_root: skills_cache_root(),
        exe: env::current_exe()
            .ok()
            .map(|e| e.canonicalize().unwrap_or(e)),
    };
    resolve_skills_dirs(&cfg)
}

// ---------------------------------------------------------------------------
// External versioned skill cache + updater
// ---------------------------------------------------------------------------
//
// The cache is a small, self-contained tree that lets `skills update` refresh
// skill content from the public `eidos-agi/surfari` repo without rebuilding or
// reinstalling the binary:
//
//   <cache_root>/
//     active/    <- live content (skill dirs) + PROVENANCE.json  (tier 2 above)
//     backup/    <- last-known-good; retained for rollback
//     staging/   <- transient; content is fully written & validated here first
//
// Activation is a directory rotate: `active` -> `backup`, then the freshly
// validated `staging` -> `active` via `fs::rename` (atomic on the final step).
// A failed or offline update never touches `active`, so the CLI keeps serving
// whatever was last good — it fails closed.

/// Provenance recorded alongside activated cache content. `commit` is the exact
/// resolved commit the content was fetched at.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Provenance {
    /// e.g. "github:eidos-agi/surfari" or "local".
    source: String,
    /// The requested ref (branch/tag/sha) before resolution.
    reference: String,
    /// The exact resolved commit the content came from.
    commit: String,
    /// RFC3339 timestamp of when the content was activated.
    fetched_at: String,
    /// Number of discoverable skills in the activated content.
    skill_count: usize,
    /// Always true once written — a marker that validation ran and passed.
    validated: bool,
}

/// A single file to be written into the cache, path relative to the skill root
/// (e.g. `core/SKILL.md`).
#[derive(Debug, Clone)]
struct SkillFile {
    path: String,
    contents: Vec<u8>,
}

/// The result of fetching skill content from a source, before validation.
#[derive(Debug, Clone)]
struct FetchedSkillData {
    source: String,
    reference: String,
    commit: String,
    files: Vec<SkillFile>,
}

/// A source of skill content for `skills update`. Implementations must not
/// execute any fetched content — they only return bytes to be stored.
trait SkillUpdateSource {
    fn fetch(&self) -> Result<FetchedSkillData, UpdateError>;
}

#[derive(Debug)]
enum UpdateError {
    /// Fetch failed — offline, DNS, HTTP error, etc. Fails closed.
    Network(String),
    /// Content failed validation (malformed frontmatter, path traversal, limits).
    Validation(String),
    /// Local filesystem error while staging or activating.
    Io(String),
}

impl UpdateError {
    fn message(&self) -> &str {
        match self {
            UpdateError::Network(m) | UpdateError::Validation(m) | UpdateError::Io(m) => m,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            UpdateError::Network(_) => "network",
            UpdateError::Validation(_) => "validation",
            UpdateError::Io(_) => "io",
        }
    }
}

// Content limits. Skill content is small markdown; these caps bound a malicious
// or runaway source without rejecting any legitimate skill tree.
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_FILE_COUNT: usize = 4000;

/// The upstream skill content source.
const SURFARI_REPO: &str = "eidos-agi/surfari";
const SURFARI_SKILL_SUBDIR: &str = "skill-data";

fn active_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("active")
}

fn backup_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("backup")
}

fn staging_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("staging")
}

/// Root of the external cache. `AGENT_BROWSER_SKILLS_CACHE_DIR` overrides the
/// default per-user data location (and is what tests point at a tempdir).
fn skills_cache_root() -> Option<PathBuf> {
    if let Ok(dir) = env::var("AGENT_BROWSER_SKILLS_CACHE_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("agent-browser").join("skills-cache"))
}

/// Read and parse the provenance file from a cache content directory.
fn read_provenance(dir: &Path) -> Option<Provenance> {
    let raw = fs::read_to_string(dir.join("PROVENANCE.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

/// A cache content dir is "current" when it carries valid provenance and holds
/// at least one discoverable skill. Anything short of that falls through to the
/// packaged fallback rather than serving a half-written or empty cache.
fn is_cache_current(active: &Path) -> bool {
    if !active.is_dir() {
        return false;
    }
    let Some(prov) = read_provenance(active) else {
        return false;
    };
    if !prov.validated || prov.commit.trim().is_empty() {
        return false;
    }
    !discover_skills(&[active.to_path_buf()]).is_empty()
}

/// Validate that a path from a source is a safe relative path — no traversal,
/// no absolute paths, no Windows drive/UNC or NUL tricks.
fn validate_relative_path(path: &str) -> Result<(), UpdateError> {
    if path.is_empty() {
        return Err(UpdateError::Validation("empty file path".to_string()));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(UpdateError::Validation(format!(
            "absolute file path rejected: {}",
            path
        )));
    }
    if path.contains('\\') || path.contains('\0') || path.contains(':') {
        return Err(UpdateError::Validation(format!(
            "unsafe characters in file path: {}",
            path
        )));
    }
    for comp in path.split('/') {
        if comp.is_empty() || comp == "." || comp == ".." {
            return Err(UpdateError::Validation(format!(
                "path traversal rejected: {}",
                path
            )));
        }
    }
    Ok(())
}

/// A slug is safe when it is a single, non-empty path component of the limited
/// charset skill/dir names use. Rejects separators and traversal.
fn is_safe_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug != "."
        && slug != ".."
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !slug.contains("..")
}

/// Validate fetched content before it is written anywhere. Enforces path
/// safety, content limits, and SKILL.md frontmatter correctness. Returns the
/// number of skills found so provenance can record it.
fn validate_fetched(data: &FetchedSkillData) -> Result<usize, UpdateError> {
    if data.commit.trim().is_empty() {
        return Err(UpdateError::Validation(
            "source did not resolve a commit".to_string(),
        ));
    }
    if data.files.is_empty() {
        return Err(UpdateError::Validation(
            "source returned no files".to_string(),
        ));
    }
    if data.files.len() > MAX_FILE_COUNT {
        return Err(UpdateError::Validation(format!(
            "too many files: {} (max {})",
            data.files.len(),
            MAX_FILE_COUNT
        )));
    }

    let mut total = 0usize;
    let mut skill_count = 0usize;
    for file in &data.files {
        validate_relative_path(&file.path)?;

        if file.contents.len() > MAX_FILE_BYTES {
            return Err(UpdateError::Validation(format!(
                "file exceeds size limit ({} bytes): {}",
                MAX_FILE_BYTES, file.path
            )));
        }
        total = total.saturating_add(file.contents.len());
        if total > MAX_TOTAL_BYTES {
            return Err(UpdateError::Validation(format!(
                "content exceeds total size limit ({} bytes)",
                MAX_TOTAL_BYTES
            )));
        }

        // A SKILL.md that lives directly under a skill directory must carry
        // valid frontmatter, and that directory's name must be a safe slug.
        let parts: Vec<&str> = file.path.split('/').collect();
        if parts.len() == 2 && parts[1] == "SKILL.md" {
            let dir_name = parts[0];
            if !is_safe_slug(dir_name) {
                return Err(UpdateError::Validation(format!(
                    "unsafe skill directory name: {}",
                    dir_name
                )));
            }
            let text = std::str::from_utf8(&file.contents).map_err(|_| {
                UpdateError::Validation(format!("SKILL.md is not valid UTF-8: {}", file.path))
            })?;
            let Some((name, _desc, _hidden)) = parse_frontmatter(text) else {
                return Err(UpdateError::Validation(format!(
                    "SKILL.md is missing valid frontmatter: {}",
                    file.path
                )));
            };
            if !is_safe_slug(&name) {
                return Err(UpdateError::Validation(format!(
                    "SKILL.md frontmatter name is unsafe: {}",
                    name
                )));
            }
            skill_count += 1;
        }
    }

    if skill_count == 0 {
        return Err(UpdateError::Validation(
            "no valid SKILL.md found in fetched content".to_string(),
        ));
    }

    Ok(skill_count)
}

/// Write validated files into a clean staging directory, then write provenance.
fn stage_content(
    cache_root: &Path,
    data: &FetchedSkillData,
    skill_count: usize,
) -> Result<Provenance, UpdateError> {
    let staging = staging_dir(cache_root);
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|e| UpdateError::Io(format!("failed to clear staging dir: {}", e)))?;
    }
    fs::create_dir_all(&staging)
        .map_err(|e| UpdateError::Io(format!("failed to create staging dir: {}", e)))?;

    for file in &data.files {
        // Path already validated; join and confirm it stays within staging.
        let dest = staging.join(&file.path);
        if !dest.starts_with(&staging) {
            return Err(UpdateError::Validation(format!(
                "path escaped staging dir: {}",
                file.path
            )));
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| UpdateError::Io(format!("failed to create dir: {}", e)))?;
        }
        fs::write(&dest, &file.contents)
            .map_err(|e| UpdateError::Io(format!("failed to write {}: {}", file.path, e)))?;
    }

    let provenance = Provenance {
        source: data.source.clone(),
        reference: data.reference.clone(),
        commit: data.commit.clone(),
        fetched_at: now_rfc3339(),
        skill_count,
        validated: true,
    };
    let prov_json = serde_json::to_string_pretty(&provenance)
        .map_err(|e| UpdateError::Io(format!("failed to serialize provenance: {}", e)))?;
    fs::write(staging.join("PROVENANCE.json"), prov_json)
        .map_err(|e| UpdateError::Io(format!("failed to write provenance: {}", e)))?;

    Ok(provenance)
}

/// Atomically activate staged content: rotate the current `active` to `backup`
/// (retained for rollback), then rename `staging` into place as the new
/// `active`. The final rename is the atomic activation point.
fn activate_staging(cache_root: &Path) -> Result<(), UpdateError> {
    let active = active_dir(cache_root);
    let backup = backup_dir(cache_root);
    let staging = staging_dir(cache_root);

    if active.exists() {
        if backup.exists() {
            fs::remove_dir_all(&backup)
                .map_err(|e| UpdateError::Io(format!("failed to clear backup dir: {}", e)))?;
        }
        fs::rename(&active, &backup)
            .map_err(|e| UpdateError::Io(format!("failed to rotate active to backup: {}", e)))?;
    }

    fs::rename(&staging, &active)
        .map_err(|e| UpdateError::Io(format!("failed to activate staged content: {}", e)))?;

    Ok(())
}

/// Fetch, validate, stage, and atomically activate new skill content. On any
/// error the current `active` content is left untouched (fail closed).
fn perform_update(
    source: &dyn SkillUpdateSource,
    cache_root: &Path,
) -> Result<Provenance, UpdateError> {
    fs::create_dir_all(cache_root)
        .map_err(|e| UpdateError::Io(format!("failed to create cache dir: {}", e)))?;

    // Fetch first. If this fails (offline), we return before touching `active`.
    let data = source.fetch()?;

    // Validate fully before writing a single byte into the cache.
    let skill_count = validate_fetched(&data)?;

    // Stage into a scratch dir, then swap it in atomically.
    let provenance = stage_content(cache_root, &data, skill_count)?;
    activate_staging(cache_root)?;

    Ok(provenance)
}

/// Roll back to the last-known-good content by swapping `active` and `backup`.
/// The swap is reversible (a second rollback redoes it).
fn rollback(cache_root: &Path) -> Result<Provenance, UpdateError> {
    let active = active_dir(cache_root);
    let backup = backup_dir(cache_root);

    if !backup.exists() {
        return Err(UpdateError::Validation(
            "no last-known-good content to roll back to".to_string(),
        ));
    }

    let swap = cache_root.join(".rollback-swap");
    if swap.exists() {
        fs::remove_dir_all(&swap)
            .map_err(|e| UpdateError::Io(format!("failed to clear swap dir: {}", e)))?;
    }

    if active.exists() {
        fs::rename(&active, &swap)
            .map_err(|e| UpdateError::Io(format!("failed to move active aside: {}", e)))?;
    }
    fs::rename(&backup, &active)
        .map_err(|e| UpdateError::Io(format!("failed to promote backup: {}", e)))?;
    if swap.exists() {
        fs::rename(&swap, &backup)
            .map_err(|e| UpdateError::Io(format!("failed to demote previous active: {}", e)))?;
    }

    read_provenance(&active)
        .ok_or_else(|| UpdateError::Validation("rolled-back content has no provenance".to_string()))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// A source that copies skill content from a local directory tree. Used when
/// `AGENT_BROWSER_SKILLS_UPDATE_SOURCE` points at a checkout, and by tests to
/// inject a fixture. Symlinks are not followed.
struct LocalDirSource {
    root: PathBuf,
    reference: String,
    commit: String,
}

impl LocalDirSource {
    fn new(root: PathBuf, reference: String, commit: String) -> Self {
        LocalDirSource {
            root,
            reference,
            commit,
        }
    }

    fn collect(dir: &Path, prefix: &str, out: &mut Vec<SkillFile>) -> Result<(), UpdateError> {
        let entries = fs::read_dir(dir)
            .map_err(|e| UpdateError::Network(format!("failed to read source dir: {}", e)))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };
            // Do not follow symlinks — they could point outside the source tree.
            let meta = fs::symlink_metadata(&path)
                .map_err(|e| UpdateError::Network(format!("failed to stat {}: {}", rel, e)))?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                Self::collect(&path, &rel, out)?;
            } else if meta.is_file() {
                let contents = fs::read(&path)
                    .map_err(|e| UpdateError::Network(format!("failed to read {}: {}", rel, e)))?;
                out.push(SkillFile {
                    path: rel,
                    contents,
                });
            }
        }
        Ok(())
    }
}

impl SkillUpdateSource for LocalDirSource {
    fn fetch(&self) -> Result<FetchedSkillData, UpdateError> {
        if !self.root.is_dir() {
            return Err(UpdateError::Network(format!(
                "update source is not a directory: {}",
                self.root.display()
            )));
        }
        let mut files = Vec::new();
        Self::collect(&self.root, "", &mut files)?;
        Ok(FetchedSkillData {
            source: "local".to_string(),
            reference: self.reference.clone(),
            commit: self.commit.clone(),
            files,
        })
    }
}

/// The production source: the public `eidos-agi/surfari` repo. Resolves the
/// requested ref to an exact commit, then downloads each blob under
/// `skill-data/` at that commit. Only public, unauthenticated GitHub endpoints
/// are used; no credentials, and fetched bytes are stored, never executed.
struct GitHubSource {
    repo: String,
    reference: String,
    subdir: String,
}

impl GitHubSource {
    fn new(repo: &str, reference: &str, subdir: &str) -> Self {
        GitHubSource {
            repo: repo.to_string(),
            reference: reference.to_string(),
            subdir: subdir.to_string(),
        }
    }

    async fn fetch_async(&self) -> Result<FetchedSkillData, UpdateError> {
        let client = reqwest::Client::builder()
            .user_agent("surfari-cli")
            .build()
            .map_err(|e| UpdateError::Network(format!("failed to build HTTP client: {}", e)))?;

        // 1. Resolve the ref to an exact commit SHA.
        let commit_url = format!(
            "https://api.github.com/repos/{}/commits/{}",
            self.repo, self.reference
        );
        let commit = client
            .get(&commit_url)
            .header("Accept", "application/vnd.github.sha")
            .send()
            .await
            .map_err(|e| UpdateError::Network(format!("failed to resolve commit: {}", e)))?
            .error_for_status()
            .map_err(|e| UpdateError::Network(format!("failed to resolve commit: {}", e)))?
            .text()
            .await
            .map_err(|e| UpdateError::Network(format!("failed to read commit: {}", e)))?
            .trim()
            .to_string();
        if commit.len() < 7 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(UpdateError::Network(format!(
                "unexpected commit response: {}",
                commit
            )));
        }

        // 2. List the tree at that commit and pick out the skill subdir.
        let tree_url = format!(
            "https://api.github.com/repos/{}/git/trees/{}?recursive=1",
            self.repo, commit
        );
        let tree: serde_json::Value = client
            .get(&tree_url)
            .send()
            .await
            .map_err(|e| UpdateError::Network(format!("failed to list tree: {}", e)))?
            .error_for_status()
            .map_err(|e| UpdateError::Network(format!("failed to list tree: {}", e)))?
            .json()
            .await
            .map_err(|e| UpdateError::Network(format!("failed to parse tree: {}", e)))?;

        let prefix = format!("{}/", self.subdir);
        let mut targets: Vec<String> = Vec::new();
        if let Some(items) = tree.get("tree").and_then(|t| t.as_array()) {
            for item in items {
                if item.get("type").and_then(|t| t.as_str()) != Some("blob") {
                    continue;
                }
                if let Some(path) = item.get("path").and_then(|p| p.as_str()) {
                    if path.starts_with(&prefix) {
                        targets.push(path.to_string());
                    }
                }
            }
        }
        if targets.is_empty() {
            return Err(UpdateError::Network(format!(
                "no {} content found at {}",
                self.subdir, commit
            )));
        }

        // 3. Download each blob's raw bytes at the resolved commit.
        let mut files = Vec::new();
        for full_path in targets {
            let raw_url = format!(
                "https://raw.githubusercontent.com/{}/{}/{}",
                self.repo, commit, full_path
            );
            let contents = client
                .get(&raw_url)
                .send()
                .await
                .map_err(|e| UpdateError::Network(format!("failed to fetch {}: {}", full_path, e)))?
                .error_for_status()
                .map_err(|e| UpdateError::Network(format!("failed to fetch {}: {}", full_path, e)))?
                .bytes()
                .await
                .map_err(|e| UpdateError::Network(format!("failed to read {}: {}", full_path, e)))?
                .to_vec();
            let rel = full_path
                .strip_prefix(&prefix)
                .unwrap_or(&full_path)
                .to_string();
            files.push(SkillFile {
                path: rel,
                contents,
            });
        }

        Ok(FetchedSkillData {
            source: format!("github:{}", self.repo),
            reference: self.reference.clone(),
            commit,
            files,
        })
    }
}

impl SkillUpdateSource for GitHubSource {
    fn fetch(&self) -> Result<FetchedSkillData, UpdateError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| UpdateError::Network(format!("failed to start async runtime: {}", e)))?;
        rt.block_on(self.fetch_async())
    }
}

/// Parse YAML frontmatter from a SKILL.md file. Returns (name, description, hidden).
fn parse_frontmatter(content: &str) -> Option<(String, String, bool)> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let after_opening = &content[3..];
    let end = after_opening.find("\n---")?;
    let frontmatter = &after_opening[..end];

    let mut name = None;
    let mut description = None;
    let mut hidden = false;

    let lines: Vec<&str> = frontmatter.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            let mut desc = val.trim().to_string();
            // Consume YAML continuation lines (indented with spaces or tab)
            while i + 1 < lines.len()
                && (lines[i + 1].starts_with("  ") || lines[i + 1].starts_with('\t'))
            {
                i += 1;
                desc.push(' ');
                desc.push_str(lines[i].trim());
            }
            description = Some(desc);
        } else if let Some(val) = line.strip_prefix("hidden:") {
            hidden = matches!(val.trim(), "true" | "yes");
        }
        i += 1;
    }

    Some((name?, description.unwrap_or_default(), hidden))
}

/// Discover all skills across the given directories.
fn discover_skills(dirs: &[PathBuf]) -> Vec<SkillInfo> {
    let mut skills = Vec::new();

    for skills_dir in dirs {
        let entries = match fs::read_dir(skills_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }
            let content = match fs::read_to_string(&skill_md) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some((name, description, hidden)) = parse_frontmatter(&content) {
                skills.push(SkillInfo {
                    name,
                    description,
                    dir: path,
                    hidden,
                });
            }
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn truncate_description(desc: &str, max_len: usize) -> String {
    if desc.len() <= max_len {
        return desc.to_string();
    }
    let boundary = desc
        .char_indices()
        .take_while(|(i, _)| *i <= max_len)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(max_len);
    let end = desc[..boundary].rfind(' ').unwrap_or(boundary);
    format!("{}...", &desc[..end])
}

/// Read the full SKILL.md content (including frontmatter).
fn read_skill_full(skill_md: &Path) -> Option<String> {
    fs::read_to_string(skill_md).ok()
}

/// Collect all supplementary files (references/, templates/) for a skill.
fn collect_supplementary_files(skill_dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for subdir_name in &["references", "templates"] {
        let subdir = skill_dir.join(subdir_name);
        if !subdir.is_dir() {
            continue;
        }
        let mut entries: Vec<_> = match fs::read_dir(&subdir) {
            Ok(e) => e.flatten().collect(),
            Err(_) => continue,
        };
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    let rel = format!(
                        "{}/{}",
                        subdir_name,
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    files.push((rel, content));
                }
            }
        }
    }
    files
}

fn run_list(skills_dirs: &[PathBuf], json_mode: bool) {
    let skills: Vec<SkillInfo> = discover_skills(skills_dirs)
        .into_iter()
        .filter(|s| !s.hidden)
        .collect();
    if skills.is_empty() {
        if json_mode {
            println!(
                "{}",
                serde_json::to_string(&json!({ "success": true, "data": [] })).unwrap_or_default()
            );
        } else {
            println!("No skills found");
        }
        return;
    }

    if json_mode {
        let items: Vec<serde_json::Value> = skills
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "description": s.description,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&json!({ "success": true, "data": items })).unwrap_or_default()
        );
    } else {
        let max_name = skills.iter().map(|s| s.name.len()).max().unwrap_or(0);
        for s in &skills {
            println!(
                "  {:<width$}  {}",
                s.name,
                truncate_description(&s.description, 70),
                width = max_name
            );
        }
    }
}

fn run_get(skills_dirs: &[PathBuf], names: &[String], get_all: bool, full: bool, json_mode: bool) {
    let all_skills = discover_skills(skills_dirs);

    let targets: Vec<&SkillInfo> = if get_all {
        all_skills.iter().filter(|s| !s.hidden).collect()
    } else {
        let mut targets = Vec::new();
        for name in names {
            if name.starts_with('-') {
                eprintln!(
                    "{} Unknown flag ignored: {}",
                    color::warning_indicator(),
                    name
                );
                continue;
            }
            match all_skills.iter().find(|s| s.name == *name) {
                Some(s) => targets.push(s),
                None => {
                    if json_mode {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "success": false,
                                "error": format!("Skill not found: {}", name),
                            }))
                            .unwrap_or_default()
                        );
                    } else {
                        eprintln!("{} Skill not found: {}", color::error_indicator(), name);
                    }
                    exit(1);
                }
            }
        }
        targets
    };

    if targets.is_empty() {
        if json_mode {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "No skill name provided. Usage: agent-browser skills get <name>",
                }))
                .unwrap_or_default()
            );
        } else {
            eprintln!(
                "{} No skill name provided. Usage: agent-browser skills get <name>",
                color::error_indicator()
            );
        }
        exit(1);
    }

    if json_mode {
        let items: Vec<serde_json::Value> = targets
            .iter()
            .map(|s| {
                let skill_md = s.dir.join("SKILL.md");
                let content = read_skill_full(&skill_md).unwrap_or_default();
                let mut obj = json!({
                    "name": s.name,
                    "content": content,
                });
                if full {
                    let supplementary = collect_supplementary_files(&s.dir);
                    if !supplementary.is_empty() {
                        let files: Vec<serde_json::Value> = supplementary
                            .iter()
                            .map(|(path, content)| json!({ "path": path, "content": content }))
                            .collect();
                        obj["files"] = json!(files);
                    }
                }
                obj
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&json!({ "success": true, "data": items })).unwrap_or_default()
        );
    } else {
        for (i, s) in targets.iter().enumerate() {
            if i > 0 {
                println!("\n---\n");
            }
            let skill_md = s.dir.join("SKILL.md");
            if let Some(content) = read_skill_full(&skill_md) {
                print!("{}", content);
                if !content.ends_with('\n') {
                    println!();
                }
            }
            if full {
                let supplementary = collect_supplementary_files(&s.dir);
                for (path, content) in &supplementary {
                    println!("\n--- {} ---\n", path);
                    print!("{}", content);
                    if !content.ends_with('\n') {
                        println!();
                    }
                }
            }
        }
    }
}

fn run_path(skills_dirs: &[PathBuf], name: Option<&str>, json_mode: bool) {
    match name {
        Some(name) => {
            let all_skills = discover_skills(skills_dirs);
            match all_skills.iter().find(|s| s.name == name) {
                Some(s) => {
                    let path = s.dir.to_string_lossy().to_string();
                    if json_mode {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "success": true,
                                "data": { "name": s.name, "path": path },
                            }))
                            .unwrap_or_default()
                        );
                    } else {
                        println!("{}", path);
                    }
                }
                None => {
                    if json_mode {
                        println!(
                            "{}",
                            serde_json::to_string(&json!({
                                "success": false,
                                "error": format!("Skill not found: {}", name),
                            }))
                            .unwrap_or_default()
                        );
                    } else {
                        eprintln!("{} Skill not found: {}", color::error_indicator(), name);
                    }
                    exit(1);
                }
            }
        }
        None => {
            let paths: Vec<String> = skills_dirs
                .iter()
                .map(|d| d.to_string_lossy().to_string())
                .collect();
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "success": true,
                        "data": { "paths": paths },
                    }))
                    .unwrap_or_default()
                );
            } else {
                for p in &paths {
                    println!("{}", p);
                }
            }
        }
    }
}

/// Handle `skills update` and `skills update --rollback`. This is the only path
/// that reaches the network; `list`/`get`/`path` never fetch.
fn run_update(args: &[String], json_mode: bool) {
    let cache_root = match skills_cache_root() {
        Some(root) => root,
        None => {
            let msg =
                "Could not determine a skills cache directory. Set AGENT_BROWSER_SKILLS_CACHE_DIR.";
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string(&json!({ "success": false, "error": msg }))
                        .unwrap_or_default()
                );
            } else {
                eprintln!("{} {}", color::error_indicator(), msg);
            }
            exit(1);
        }
    };

    let opts = &args[2..];

    if opts.iter().any(|a| a == "--rollback") {
        match rollback(&cache_root) {
            Ok(prov) => report_update_success(&cache_root, &prov, true, json_mode),
            Err(e) => report_update_error(&e, json_mode),
        }
        return;
    }

    // `--ref <ref>` selects the branch/tag/commit to update to (default `main`).
    let mut reference = "main".to_string();
    let mut i = 0;
    while i < opts.len() {
        if opts[i] == "--ref" {
            if let Some(val) = opts.get(i + 1) {
                reference = val.clone();
                i += 1;
            }
        }
        i += 1;
    }

    // An injected local checkout (used for offline updates and tests) takes
    // precedence over the network source. It never fetches.
    let source: Box<dyn SkillUpdateSource> = match env::var("AGENT_BROWSER_SKILLS_UPDATE_SOURCE") {
        Ok(dir) if !dir.is_empty() => Box::new(LocalDirSource::new(
            PathBuf::from(dir),
            reference.clone(),
            "local".to_string(),
        )),
        _ => Box::new(GitHubSource::new(
            SURFARI_REPO,
            &reference,
            SURFARI_SKILL_SUBDIR,
        )),
    };

    match perform_update(source.as_ref(), &cache_root) {
        Ok(prov) => report_update_success(&cache_root, &prov, false, json_mode),
        Err(e) => report_update_error(&e, json_mode),
    }
}

fn report_update_success(cache_root: &Path, prov: &Provenance, rolled_back: bool, json_mode: bool) {
    let active = active_dir(cache_root).to_string_lossy().to_string();
    if json_mode {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "success": true,
                "data": {
                    "rolledBack": rolled_back,
                    "source": prov.source,
                    "reference": prov.reference,
                    "commit": prov.commit,
                    "skillCount": prov.skill_count,
                    "activePath": active,
                },
            }))
            .unwrap_or_default()
        );
    } else if rolled_back {
        println!(
            "{} Rolled back to {} skill(s) at commit {}",
            color::green("✓"),
            prov.skill_count,
            &prov.commit
        );
    } else {
        println!(
            "{} Updated to {} skill(s) from {} at commit {}",
            color::green("✓"),
            prov.skill_count,
            prov.source,
            &prov.commit
        );
    }
}

fn report_update_error(e: &UpdateError, json_mode: bool) {
    if json_mode {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "success": false,
                "error": e.message(),
                "type": e.kind(),
            }))
            .unwrap_or_default()
        );
    } else {
        eprintln!("{} {}", color::error_indicator(), e.message());
    }
    exit(1);
}

pub fn run_skills(args: &[String], json_mode: bool) {
    // `update` manages the external cache; it must run even when no skills dir
    // is resolvable yet, so handle it before the discovery check below.
    if args.get(1).map(|s| s.as_str()) == Some("update") {
        run_update(args, json_mode);
        return;
    }

    let skills_dirs = find_skills_dirs();
    if skills_dirs.is_empty() {
        if json_mode {
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "success": false,
                    "error": "Skills directory not found. Set AGENT_BROWSER_SKILLS_DIR or reinstall via npm.",
                }))
                .unwrap_or_default()
            );
        } else {
            eprintln!(
                "{} Skills directory not found. Set AGENT_BROWSER_SKILLS_DIR or reinstall via npm.",
                color::error_indicator()
            );
        }
        exit(1);
    }

    let subcommand = args.get(1).map(|s| s.as_str());

    match subcommand {
        None | Some("list") => run_list(&skills_dirs, json_mode),
        Some("get") => {
            let names: Vec<String> = args[2..]
                .iter()
                .filter(|a| *a != "--full" && *a != "--all")
                .cloned()
                .collect();
            let full = args[2..].iter().any(|a| a == "--full");
            let get_all = args[2..].iter().any(|a| a == "--all");
            run_get(&skills_dirs, &names, get_all, full, json_mode);
        }
        Some("path") => {
            let name = args.get(2).map(|s| s.as_str());
            run_path(&skills_dirs, name, json_mode);
        }
        Some(unknown) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "success": false,
                        "error": format!("Unknown skills subcommand: {}", unknown),
                    }))
                    .unwrap_or_default()
                );
            } else {
                eprintln!(
                    "{} Unknown skills subcommand: {}",
                    color::error_indicator(),
                    unknown
                );
            }
            exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_skill(dir: &Path, name: &str, description: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: {}\ndescription: {}\n---\n\n# {}\n\nContent here.\n",
                name, description, name
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_parse_frontmatter_basic() {
        let content = "---\nname: test-skill\ndescription: A test skill.\n---\n\n# Test\n";
        let (name, desc, hidden) = parse_frontmatter(content).unwrap();
        assert_eq!(name, "test-skill");
        assert_eq!(desc, "A test skill.");
        assert!(!hidden);
    }

    #[test]
    fn test_parse_frontmatter_multiline_description() {
        let content =
            "---\nname: test\ndescription: First line\n  continued here\n  and here\n---\n";
        let (name, desc, hidden) = parse_frontmatter(content).unwrap();
        assert_eq!(name, "test");
        assert_eq!(desc, "First line continued here and here");
        assert!(!hidden);
    }

    #[test]
    fn test_parse_frontmatter_hidden_true() {
        let content = "---\nname: stub\ndescription: A bootstrap stub.\nhidden: true\n---\n";
        let (name, desc, hidden) = parse_frontmatter(content).unwrap();
        assert_eq!(name, "stub");
        assert_eq!(desc, "A bootstrap stub.");
        assert!(hidden);
    }

    #[test]
    fn test_parse_frontmatter_hidden_false() {
        let content = "---\nname: visible\ndescription: Visible.\nhidden: false\n---\n";
        let (_, _, hidden) = parse_frontmatter(content).unwrap();
        assert!(!hidden);
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "# Just a heading\n\nNo frontmatter here.\n";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_frontmatter_missing_name() {
        let content = "---\ndescription: No name field\n---\n";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn test_discover_skills_single_dir() {
        let tmp = tempfile::tempdir().unwrap();
        create_test_skill(tmp.path(), "alpha", "Alpha skill");
        create_test_skill(tmp.path(), "beta", "Beta skill");

        // Non-skill directory (no SKILL.md)
        fs::create_dir_all(tmp.path().join("not-a-skill")).unwrap();
        fs::write(tmp.path().join("not-a-skill").join("README.md"), "hi").unwrap();

        let dirs = vec![tmp.path().to_path_buf()];
        let skills = discover_skills(&dirs);
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "alpha");
        assert_eq!(skills[1].name, "beta");
    }

    #[test]
    fn test_discover_skills_multiple_dirs() {
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();
        create_test_skill(tmp1.path(), "alpha", "Alpha skill");
        create_test_skill(tmp2.path(), "beta", "Beta skill");
        create_test_skill(tmp2.path(), "gamma", "Gamma skill");

        let dirs = vec![tmp1.path().to_path_buf(), tmp2.path().to_path_buf()];
        let skills = discover_skills(&dirs);
        assert_eq!(skills.len(), 3);
        assert_eq!(skills[0].name, "alpha");
        assert_eq!(skills[1].name, "beta");
        assert_eq!(skills[2].name, "gamma");
    }

    #[test]
    fn test_truncate_description() {
        assert_eq!(truncate_description("short", 10), "short");
        assert_eq!(
            truncate_description("this is a longer description that should be truncated", 20),
            "this is a longer..."
        );
    }

    #[test]
    fn test_truncate_description_multibyte() {
        let desc = "Browse \u{00e9}l\u{00e9}ments and \u{65e5}\u{672c}\u{8a9e} pages quickly";
        let result = truncate_description(desc, 20);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 30);
    }

    #[test]
    fn test_collect_supplementary_files() {
        let tmp = tempfile::tempdir().unwrap();
        let refs_dir = tmp.path().join("references");
        fs::create_dir_all(&refs_dir).unwrap();
        fs::write(refs_dir.join("auth.md"), "# Auth\n").unwrap();
        fs::write(refs_dir.join("commands.md"), "# Commands\n").unwrap();

        let templates_dir = tmp.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();
        fs::write(templates_dir.join("example.sh"), "#!/bin/bash\n").unwrap();

        let files = collect_supplementary_files(tmp.path());
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].0, "references/auth.md");
        assert_eq!(files[1].0, "references/commands.md");
        assert_eq!(files[2].0, "templates/example.sh");
    }

    // ---- External cache precedence + updater tests ----

    /// A source that returns pre-built data, or a Network error when empty
    /// (simulating an offline fetch).
    struct FixtureSource {
        data: Option<FetchedSkillData>,
    }

    impl SkillUpdateSource for FixtureSource {
        fn fetch(&self) -> Result<FetchedSkillData, UpdateError> {
            self.data
                .clone()
                .ok_or_else(|| UpdateError::Network("offline".to_string()))
        }
    }

    fn skill_md_bytes(name: &str, body: &str) -> Vec<u8> {
        format!(
            "---\nname: {}\ndescription: {} skill.\n---\n\n# {}\n\n{}\n",
            name, name, name, body
        )
        .into_bytes()
    }

    fn fetched(commit: &str, body: &str) -> FetchedSkillData {
        FetchedSkillData {
            source: "local".to_string(),
            reference: "main".to_string(),
            commit: commit.to_string(),
            files: vec![SkillFile {
                path: "core/SKILL.md".to_string(),
                contents: skill_md_bytes("core", body),
            }],
        }
    }

    #[test]
    fn test_resolve_precedence_override_wins() {
        // Explicit override beats a current cache and the packaged fallback.
        let override_tmp = tempfile::tempdir().unwrap();
        create_test_skill(override_tmp.path(), "override-skill", "From override");

        let cache_tmp = tempfile::tempdir().unwrap();
        let active = active_dir(cache_tmp.path());
        fs::create_dir_all(&active).unwrap();
        create_test_skill(&active, "cache-skill", "From cache");
        write_provenance(&active, "cafe1234");

        let cfg = SkillsResolveConfig {
            override_dir: Some(override_tmp.path().to_path_buf()),
            cache_root: Some(cache_tmp.path().to_path_buf()),
            exe: None,
        };
        let dirs = resolve_skills_dirs(&cfg);
        assert_eq!(dirs, vec![override_tmp.path().to_path_buf()]);
    }

    #[test]
    fn test_resolve_precedence_cache_when_current() {
        // With no override, a verified-current cache is used.
        let cache_tmp = tempfile::tempdir().unwrap();
        let active = active_dir(cache_tmp.path());
        fs::create_dir_all(&active).unwrap();
        create_test_skill(&active, "cache-skill", "From cache");
        write_provenance(&active, "cafe1234");

        let cfg = SkillsResolveConfig {
            override_dir: None,
            cache_root: Some(cache_tmp.path().to_path_buf()),
            exe: None,
        };
        let dirs = resolve_skills_dirs(&cfg);
        assert_eq!(dirs, vec![active]);
    }

    #[test]
    fn test_resolve_precedence_fallback_when_cache_not_current() {
        // A cache without provenance is ignored; resolution falls through to
        // the packaged fallback layout discovered from the exe path.
        let cache_tmp = tempfile::tempdir().unwrap();
        let active = active_dir(cache_tmp.path());
        fs::create_dir_all(&active).unwrap();
        create_test_skill(&active, "cache-skill", "No provenance"); // no PROVENANCE.json

        let pkg = tempfile::tempdir().unwrap();
        let bin_dir = pkg.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let skill_data = pkg.path().join("skill-data");
        fs::create_dir_all(pkg.path().join("skills")).unwrap();
        create_test_skill(&skill_data, "packaged", "From package");
        let exe = bin_dir.join("agent-browser");
        fs::write(&exe, b"binary").unwrap();

        let cfg = SkillsResolveConfig {
            override_dir: None,
            cache_root: Some(cache_tmp.path().to_path_buf()),
            exe: Some(exe),
        };
        let dirs = resolve_skills_dirs(&cfg);
        // Packaged skill-data is present and searched; the stale cache is not.
        assert!(dirs.iter().any(|d| d.ends_with("skill-data")));
        assert!(!dirs.iter().any(|d| d.starts_with(cache_tmp.path())));
        let skills = discover_skills(&dirs);
        assert!(skills.iter().any(|s| s.name == "packaged"));
    }

    #[test]
    fn test_standalone_install_layout() {
        // bin/<binary> next to a package root holding skills/ + skill-data/.
        let pkg = tempfile::tempdir().unwrap();
        let root = pkg.path().canonicalize().unwrap();
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(root.join("skills")).unwrap();
        create_test_skill(&root.join("skill-data"), "core", "Core skill");
        let exe = bin_dir.join("agent-browser");
        fs::write(&exe, b"binary").unwrap();

        let resolved = package_root_from_exe(&exe).unwrap();
        assert_eq!(resolved, root);

        let cfg = SkillsResolveConfig {
            override_dir: None,
            cache_root: None,
            exe: Some(exe),
        };
        let dirs = resolve_skills_dirs(&cfg);
        let skills = discover_skills(&dirs);
        assert!(skills.iter().any(|s| s.name == "core"));
    }

    #[test]
    fn test_update_valid_via_local_fixture() {
        // A local checkout is copied in, activated, and immediately resolvable.
        let fixture = tempfile::tempdir().unwrap();
        create_test_skill(fixture.path(), "core", "Core skill");
        create_test_skill(fixture.path(), "electron", "Electron skill");

        let cache = tempfile::tempdir().unwrap();
        let source = LocalDirSource::new(
            fixture.path().to_path_buf(),
            "main".to_string(),
            "abc1234".to_string(),
        );
        let prov = perform_update(&source, cache.path()).unwrap();

        assert_eq!(prov.commit, "abc1234");
        assert_eq!(prov.skill_count, 2);

        let active = active_dir(cache.path());
        assert!(active.join("core/SKILL.md").exists());
        assert!(is_cache_current(&active));

        // The freshly activated cache resolves without a rebuild.
        let cfg = SkillsResolveConfig {
            override_dir: None,
            cache_root: Some(cache.path().to_path_buf()),
            exe: None,
        };
        let dirs = resolve_skills_dirs(&cfg);
        assert_eq!(dirs, vec![active]);
        let skills = discover_skills(&dirs);
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn test_update_records_provenance() {
        let fixture = tempfile::tempdir().unwrap();
        create_test_skill(fixture.path(), "core", "Core skill");

        let cache = tempfile::tempdir().unwrap();
        let source = LocalDirSource::new(
            fixture.path().to_path_buf(),
            "release".to_string(),
            "deadbeef42".to_string(),
        );
        perform_update(&source, cache.path()).unwrap();

        let prov = read_provenance(&active_dir(cache.path())).unwrap();
        assert_eq!(prov.commit, "deadbeef42");
        assert_eq!(prov.reference, "release");
        assert_eq!(prov.source, "local");
        assert!(prov.validated);
        assert_eq!(prov.skill_count, 1);
    }

    #[test]
    fn test_validate_rejects_missing_frontmatter() {
        let data = FetchedSkillData {
            source: "local".to_string(),
            reference: "main".to_string(),
            commit: "abc1234".to_string(),
            files: vec![SkillFile {
                path: "core/SKILL.md".to_string(),
                contents: b"# Core\n\nNo frontmatter here.\n".to_vec(),
            }],
        };
        let err = validate_fetched(&data).unwrap_err();
        assert!(matches!(err, UpdateError::Validation(_)));
    }

    #[test]
    fn test_validate_rejects_path_traversal() {
        let data = FetchedSkillData {
            source: "local".to_string(),
            reference: "main".to_string(),
            commit: "abc1234".to_string(),
            files: vec![
                SkillFile {
                    path: "core/SKILL.md".to_string(),
                    contents: skill_md_bytes("core", "ok"),
                },
                SkillFile {
                    path: "../../etc/evil.md".to_string(),
                    contents: b"pwned".to_vec(),
                },
            ],
        };
        let err = validate_fetched(&data).unwrap_err();
        match err {
            UpdateError::Validation(msg) => assert!(msg.contains("traversal")),
            other => panic!("expected traversal rejection, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_rejects_absolute_path() {
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("core/../../x").is_err());
        assert!(validate_relative_path("core\\SKILL.md").is_err());
        assert!(validate_relative_path("core/SKILL.md").is_ok());
    }

    #[test]
    fn test_validate_rejects_oversized_file() {
        let data = FetchedSkillData {
            source: "local".to_string(),
            reference: "main".to_string(),
            commit: "abc1234".to_string(),
            files: vec![
                SkillFile {
                    path: "core/SKILL.md".to_string(),
                    contents: skill_md_bytes("core", "ok"),
                },
                SkillFile {
                    path: "core/big.md".to_string(),
                    contents: vec![b'x'; MAX_FILE_BYTES + 1],
                },
            ],
        };
        let err = validate_fetched(&data).unwrap_err();
        assert!(matches!(err, UpdateError::Validation(_)));
    }

    #[test]
    fn test_update_atomic_rollback() {
        let cache = tempfile::tempdir().unwrap();

        // v1 activates as the only content.
        perform_update(
            &FixtureSource {
                data: Some(fetched("1111aaa", "version one")),
            },
            cache.path(),
        )
        .unwrap();

        // v2 rotates v1 into backup and activates v2.
        perform_update(
            &FixtureSource {
                data: Some(fetched("2222bbb", "version two")),
            },
            cache.path(),
        )
        .unwrap();

        let active = active_dir(cache.path());
        assert_eq!(read_provenance(&active).unwrap().commit, "2222bbb");
        assert_eq!(
            read_provenance(&backup_dir(cache.path())).unwrap().commit,
            "1111aaa"
        );

        // Rollback restores the last-known-good (v1) atomically.
        let prov = rollback(cache.path()).unwrap();
        assert_eq!(prov.commit, "1111aaa");
        assert_eq!(read_provenance(&active).unwrap().commit, "1111aaa");
        let content = fs::read_to_string(active.join("core/SKILL.md")).unwrap();
        assert!(content.contains("version one"));
        // The rolled-over content is retained as the new backup for redo.
        assert_eq!(
            read_provenance(&backup_dir(cache.path())).unwrap().commit,
            "2222bbb"
        );
    }

    #[test]
    fn test_update_offline_fails_closed() {
        let cache = tempfile::tempdir().unwrap();

        // Establish a good current state.
        perform_update(
            &FixtureSource {
                data: Some(fetched("1111aaa", "version one")),
            },
            cache.path(),
        )
        .unwrap();

        // An offline update must error and leave `active` untouched.
        let err = perform_update(&FixtureSource { data: None }, cache.path()).unwrap_err();
        assert!(matches!(err, UpdateError::Network(_)));

        let active = active_dir(cache.path());
        assert!(is_cache_current(&active));
        assert_eq!(read_provenance(&active).unwrap().commit, "1111aaa");
    }

    #[test]
    fn test_update_offline_on_empty_cache_leaves_nothing() {
        // No prior content: an offline update fails without creating an active dir.
        let cache = tempfile::tempdir().unwrap();
        let err = perform_update(&FixtureSource { data: None }, cache.path()).unwrap_err();
        assert!(matches!(err, UpdateError::Network(_)));
        assert!(!active_dir(cache.path()).exists());
    }

    fn write_provenance(dir: &Path, commit: &str) {
        let prov = Provenance {
            source: "local".to_string(),
            reference: "main".to_string(),
            commit: commit.to_string(),
            fetched_at: "1970-01-01T00:00:00Z".to_string(),
            skill_count: 1,
            validated: true,
        };
        fs::write(
            dir.join("PROVENANCE.json"),
            serde_json::to_string_pretty(&prov).unwrap(),
        )
        .unwrap();
    }
}
