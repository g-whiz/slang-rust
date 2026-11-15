use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

const SLANG_REPOSITORY: &str = "https://github.com/shader-slang/slang.git";
const CARGO_NET_OFFLINE: &str = "CARGO_NET_OFFLINE";
const OUT_DIR: &str = "OUT_DIR";
const TARGET: &str = "TARGET";
const CACHEDIR_TAG: &str = "CACHEDIR.TAG";
const SHARED_FOLDER: &str = "slang-src";

const REQUIRED_SUBMODULES: [&str; 10] = [
    "external/cmark",
    "external/fast_float",
    "external/lua",
    "external/lz4",
    "external/miniz",
    "external/spirv-headers",
    "external/unordered_dense",
    "external/vulkan",
    "external/glslang",
    "external/spirv-tools",
];

const NETWORK_SETTINGS: [&str; 8] = [
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.https.allow=always",
    "-c",
    "http.lowSpeedLimit=1000",
    "-c",
    "http.lowSpeedTime=60",
];

const GIT_REPOSITORY_VARIABLES: [&str; 15] = [
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

const OFFLINE_ADVICE: &str = "A vendored build needs network access for its first build in a target directory, and a build without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK and needs no network access.";

const BUILD_TAG_FORM: &str = "<major>.<minor>, optionally followed by .<patch>, optionally followed by -<count>-g<hash>, in which each number is 0 or a decimal number without a leading zero and <hash> is at least nine lowercase hexadecimal digits";

trait Host {
    fn var(&self, name: &str) -> Option<OsString>;
    fn exists(&self, path: &Path) -> io::Result<bool>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn lock(&self, path: &Path) -> io::Result<()>;
    fn list(&self, dir: &Path) -> io::Result<Vec<OsString>>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn write(&self, path: &Path, text: &str) -> io::Result<()>;
    fn write_stamp(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn process_id(&self) -> u32;
    fn run(&self, invocation: &Invocation) -> io::Result<Finished>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    args: Vec<String>,
    dir: PathBuf,
    env: Vec<(&'static str, &'static str)>,
    env_remove: Vec<&'static str>,
    network: bool,
}

impl fmt::Display for Invocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("git")?;
        for arg in &self.args {
            write!(formatter, " {arg}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finished {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Finished {
    fn status(&self) -> String {
        self.code.map_or_else(
            || "no exit status, because a signal ended it".to_owned(),
            |code| format!("exit status {code}"),
        )
    }
}

struct BuildError(String);

impl fmt::Debug for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn configure(host: &impl Host, directives: &mut Vec<String>) -> Result<(), BuildError> {
    check_recorded(RECORDED_COMMIT, RECORDED_BUILD_TAG)?;
    let out = out_dir(host.var(OUT_DIR).as_deref())?;
    let target = host.var(TARGET);
    let shared = base_dir(host, &out, target.as_deref()).join(SHARED_FOLDER);
    let sources = shared.join("src");
    create_dir(host, &sources)?;
    let lock = sources.join(format!("{RECORDED_COMMIT}.lock"));
    host.lock(&lock)
        .map_err(|error| file_error("lock", &lock, &error))?;
    let entry = sources.join(RECORDED_COMMIT);
    let present = host
        .exists(&entry)
        .map_err(|error| file_error("inspect", &entry, &error))?;
    if present {
        verify_entry(host, &entry)?;
    } else if host.var(CARGO_NET_OFFLINE).as_deref() == Some(OsStr::new("true")) {
        return Err(BuildError(format!(
            "the vendored build needs to clone Slang at the commit {RECORDED_COMMIT}, which no earlier build in this target directory cloned, but CARGO_NET_OFFLINE is true, so the build cannot reach the network. {OFFLINE_ADVICE}"
        )));
    } else {
        clone_entry(host, &sources, &entry)?;
    }
    directives.push(format!(
        "cargo::rerun-if-changed={}",
        entry.join("stamp").display()
    ));
    directives.push(format!(
        "cargo::metadata=root={}",
        entry.join("tree").display()
    ));
    directives.push(format!("cargo::metadata=commit={RECORDED_COMMIT}"));
    directives.push(format!("cargo::metadata=build_tag={RECORDED_BUILD_TAG}"));
    directives.push(format!("cargo::metadata=shared_dir={}", shared.display()));
    directives.push(format!(
        "cargo::rustc-env=SLANG_SRC_SHARED_DIR={}",
        shared.display()
    ));
    Ok(())
}

fn check_recorded(commit: &str, build_tag: &str) -> Result<(), BuildError> {
    if commit.len() != 40 || !is_lowercase_hex(commit) {
        return Err(BuildError(format!(
            "the recorded commit is {}, which is not a full commit hash of 40 lowercase hexadecimal digits",
            quoted(OsStr::new(commit))
        )));
    }
    if slang_version(build_tag, commit).is_none() {
        return Err(BuildError(format!(
            "the recorded build tag is {}, which does not have the form of a build tag: {BUILD_TAG_FORM}",
            quoted(OsStr::new(build_tag))
        )));
    }
    Ok(())
}

fn slang_version(build_tag: &str, commit: &str) -> Option<String> {
    let (release, described) = match build_tag.split_once('-') {
        Some((release, rest)) => {
            let (count, hash) = rest.split_once("-g")?;
            if !is_number(count) || hash.len() < 9 || !is_lowercase_hex(hash) {
                return None;
            }
            (release, Some((count, hash)))
        }
        None => (build_tag, None),
    };
    let numbers: Vec<&str> = release.split('.').collect();
    let (major, minor, patch) = match numbers.as_slice() {
        [major, minor] => (*major, *minor, "0"),
        [major, minor, patch] => (*major, *minor, *patch),
        _ => return None,
    };
    if ![major, minor, patch].into_iter().all(is_number) {
        return None;
    }
    let (count, hash) = described.unwrap_or(("0", commit));
    let abbreviated = hash.get(..9)?;
    Some(format!("{major}.{minor}.{patch}.{count}.g{abbreviated}"))
}

fn is_number(text: &str) -> bool {
    text == "0"
        || (!text.is_empty()
            && !text.starts_with('0')
            && text.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_lowercase_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn quoted(value: &OsStr) -> String {
    format!("\"{}\"", value.to_string_lossy().escape_debug())
}

fn out_dir(value: Option<&OsStr>) -> Result<PathBuf, BuildError> {
    value
        .and_then(OsStr::to_str)
        .filter(|text| !text.contains(['\n', '\r']) && Path::new(text).is_absolute())
        .map(PathBuf::from)
        .ok_or_else(|| {
            let shown = value.map_or_else(|| "not set".to_owned(), quoted);
            BuildError(format!(
                "OUT_DIR is {shown}, which is not an absolute path in UTF-8 without line breaks. Cargo sets OUT_DIR when it runs a build script, so run the build through Cargo"
            ))
        })
}

fn base_dir(host: &impl Host, out: &Path, target: Option<&OsStr>) -> PathBuf {
    let tagged = |dir: &Path| host.exists(&dir.join(CACHEDIR_TAG)).unwrap_or(false);
    let Some(nearest) = out.ancestors().find(|dir| tagged(dir)) else {
        return out.to_path_buf();
    };
    match (target, nearest.file_name(), nearest.parent()) {
        (Some(target), Some(name), Some(parent)) if name == target && tagged(parent) => {
            parent.to_path_buf()
        }
        _ => nearest.to_path_buf(),
    }
}

fn verify_entry(host: &impl Host, entry: &Path) -> Result<(), BuildError> {
    let advice = "Another build may be reading the entry, so the build leaves it in place. Remove the entry, or run cargo clean, when no build runs";
    let tree = entry.join("tree");
    let present = host
        .exists(&tree)
        .map_err(|error| file_error("inspect", &tree, &error))?;
    if !present {
        return Err(BuildError(format!(
            "git cannot read the clone entry {} of Slang, because its work tree {} does not exist. {advice}",
            entry.display(),
            tree.display()
        )));
    }
    let invocation = git(tree, &["rev-parse", "--verify", "HEAD^{commit}"]);
    let finished = start(host, &invocation)?;
    if finished.code != Some(0) {
        return Err(BuildError(format!(
            "git cannot read the clone entry {} of Slang: `{invocation}` ended with {}: {}. {advice}",
            entry.display(),
            finished.status(),
            finished.stderr.trim()
        )));
    }
    let commit = finished.stdout.trim();
    if commit != RECORDED_COMMIT {
        return Err(BuildError(format!(
            "the clone entry {} of Slang holds the commit {commit}, not the recorded commit {RECORDED_COMMIT}. {advice}",
            entry.display()
        )));
    }
    Ok(())
}

fn clone_entry(host: &impl Host, sources: &Path, entry: &Path) -> Result<(), BuildError> {
    let prefix = format!("{RECORDED_COMMIT}.tmp-");
    let names = host
        .list(sources)
        .map_err(|error| file_error("list", sources, &error))?;
    for name in names {
        let stale = name
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .is_some_and(|number| {
                !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
            });
        if stale {
            let path = sources.join(name);
            host.remove_dir_all(&path)
                .map_err(|error| file_error("remove", &path, &error))?;
        }
    }
    let temporary = sources.join(format!("{prefix}{}", host.process_id()));
    let tree = temporary.join("tree");
    create_dir(host, &tree)?;
    succeed(host, &git(tree.clone(), &["init"]))?;
    succeed(
        host,
        &git(tree.clone(), &["remote", "add", "origin", SLANG_REPOSITORY]),
    )?;
    succeed(
        host,
        &network_git(
            tree.clone(),
            &["fetch", "--depth", "1", "origin", RECORDED_COMMIT],
        ),
    )?;
    succeed(
        host,
        &git(tree.clone(), &["checkout", "--detach", "FETCH_HEAD"]),
    )?;
    check_gitlinks(host, &tree)?;
    let mut update = vec!["submodule", "update", "--init", "--depth", "1", "--"];
    update.extend(REQUIRED_SUBMODULES);
    succeed(host, &network_git(tree.clone(), &update))?;
    for submodule in REQUIRED_SUBMODULES {
        let dot_git = tree.join(submodule).join(".git");
        let present = host
            .exists(&dot_git)
            .map_err(|error| file_error("inspect", &dot_git, &error))?;
        if !present {
            return Err(missing_submodule(submodule));
        }
    }
    let build_tag = temporary.join("build-tag");
    host.write(&build_tag, &format!("{RECORDED_BUILD_TAG}\n"))
        .map_err(|error| file_error("write", &build_tag, &error))?;
    let stamp = temporary.join("stamp");
    host.write_stamp(&stamp)
        .map_err(|error| file_error("write", &stamp, &error))?;
    host.rename(&temporary, entry).map_err(|error| {
        BuildError(format!(
            "cannot rename {} to {}: {error}",
            temporary.display(),
            entry.display()
        ))
    })
}

fn check_gitlinks(host: &impl Host, tree: &Path) -> Result<(), BuildError> {
    let mut args = vec!["ls-tree", "-z", "HEAD", "--"];
    args.extend(REQUIRED_SUBMODULES);
    let listing = succeed(host, &git(tree.to_path_buf(), &args))?;
    let gitlinks: Vec<&str> = listing
        .stdout
        .split('\0')
        .filter_map(|record| {
            let (meta, path) = record.split_once('\t')?;
            meta.starts_with("160000 commit ").then_some(path)
        })
        .collect();
    match REQUIRED_SUBMODULES
        .into_iter()
        .find(|submodule| !gitlinks.contains(submodule))
    {
        Some(submodule) => Err(missing_submodule(submodule)),
        None => Ok(()),
    }
}

fn missing_submodule(submodule: &str) -> BuildError {
    BuildError(format!(
        "the Slang commit {RECORDED_COMMIT} lacks the submodule {submodule}, which the native builds of Slang need. A commit patched in through the [patch] table of Cargo may lack it, so use the recorded commit of slang-src or a commit that has every submodule that it requires"
    ))
}

fn git(dir: PathBuf, args: &[&str]) -> Invocation {
    Invocation {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        dir,
        env: vec![("GIT_OPTIONAL_LOCKS", "0")],
        env_remove: GIT_REPOSITORY_VARIABLES.to_vec(),
        network: false,
    }
}

fn network_git(dir: PathBuf, args: &[&str]) -> Invocation {
    let mut invocation = git(dir, &[NETWORK_SETTINGS.as_slice(), args].concat());
    invocation.env.push(("GIT_TERMINAL_PROMPT", "0"));
    invocation.env_remove.push("GIT_ALLOW_PROTOCOL");
    invocation.network = true;
    invocation
}

fn start(host: &impl Host, invocation: &Invocation) -> Result<Finished, BuildError> {
    host.run(invocation).map_err(|error| {
        let advice = if error.kind() == io::ErrorKind::NotFound {
            ". A vendored build needs git on PATH"
        } else {
            ""
        };
        BuildError(format!("cannot start `{invocation}`: {error}{advice}"))
    })
}

fn succeed(host: &impl Host, invocation: &Invocation) -> Result<Finished, BuildError> {
    let finished = start(host, invocation)?;
    if finished.code == Some(0) {
        return Ok(finished);
    }
    let stderr = finished.stderr.trim();
    Err(BuildError(if invocation.network {
        format!(
            "`{invocation}` ended with {} while cloning Slang at the commit {RECORDED_COMMIT}: {stderr}. {OFFLINE_ADVICE}",
            finished.status()
        )
    } else {
        format!(
            "`{invocation}` in {} ended with {}: {stderr}",
            invocation.dir.display(),
            finished.status()
        )
    }))
}

fn create_dir(host: &impl Host, path: &Path) -> Result<(), BuildError> {
    host.create_dir_all(path)
        .map_err(|error| file_error("create", path, &error))
}

fn file_error(action: &str, path: &Path, error: &io::Error) -> BuildError {
    BuildError(format!("cannot {action} {}: {error}", path.display()))
}
