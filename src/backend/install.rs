use super::storage::{supported_version, validate_id};
use super::{Account, BackendEvent, GameVersion, JavaRuntime, Loader, Profile, ProgressEvent};
use anyhow::{bail, ensure, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::Sender,
        Mutex,
    },
    time::Duration,
};

pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        !relative.contains('\\') && !relative.contains(':') && !relative.is_empty(),
        "Invalid relative file path"
    );
    let path = Path::new(relative);
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "Path escapes the game directory"
    );
    Ok(root.join(path))
}
pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}
fn regex_matches(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern)
        .map(|r| r.is_match(text))
        .unwrap_or(false)
}
/// Evaluate the rules container used by both library and argument metadata.
pub fn rules_allow(container: &Value) -> bool {
    let Some(rules) = container.get("rules").and_then(Value::as_array) else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let os = &rule["os"];
        if let Some(name) = os["name"].as_str() {
            if name != os_name() {
                continue;
            }
        }
        if let Some(arch) = os["arch"].as_str() {
            if !regex_matches(arch, std::env::consts::ARCH) {
                continue;
            }
        }
        if let Some(version) = os["version"].as_str() {
            if !regex_matches(version, if cfg!(windows) { "10.0" } else { "" }) {
                continue;
            }
        }
        let mut features_match = true;
        if let Some(features) = rule["features"].as_object() {
            for (key, expected) in features {
                let value = key == "has_custom_resolution";
                if expected.as_bool() != Some(value) {
                    features_match = false;
                    break;
                }
            }
        }
        if features_match {
            allowed = rule["action"] == "allow";
        }
    }
    allowed
}
pub fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_owned();
    for (key, value) in vars {
        result = result.replace(&format!("${{{key}}}"), value);
    }
    result
}
pub fn split_legacy_arguments(input: &str) -> Result<Vec<String>> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in input.chars() {
        if c == '"' || c == '\'' {
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(c);
            } else {
                current.push(c);
            }
            started = true;
        } else if c.is_whitespace() && quote.is_none() {
            if started {
                result.push(std::mem::take(&mut current));
                started = false;
            }
        } else {
            current.push(c);
            started = true;
        }
    }
    ensure!(quote.is_none(), "Unclosed quote in legacy game arguments");
    if started {
        result.push(current);
    }
    Ok(result)
}
pub fn merge_profile(base: &Value, layer: &Value) -> Value {
    let mut merged = base.clone();
    if let Some(object) = layer.as_object() {
        for (key, value) in object {
            if key == "libraries" {
                let mut libs = merged[key].as_array().cloned().unwrap_or_default();
                libs.extend(value.as_array().cloned().unwrap_or_default());
                merged[key] = Value::Array(libs);
            } else if key == "arguments" {
                for group in ["game", "jvm"] {
                    let mut args = merged["arguments"][group]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    args.extend(value[group].as_array().cloned().unwrap_or_default());
                    if !merged["arguments"].is_object() {
                        merged["arguments"] = json!({});
                    }
                    merged["arguments"][group] = Value::Array(args);
                }
            } else {
                merged[key] = value.clone();
            }
        }
    }
    merged
}
pub fn inspect_java(path: &str) -> Option<JavaRuntime> {
    let name = Path::new(path).file_name()?.to_str()?.to_ascii_lowercase();
    if !["java", "java.exe", "javaw.exe"].contains(&name.as_str()) {
        return None;
    }
    let mut command = Command::new(path);
    command.arg("-version");
    hide_console(&mut command);
    let output = command.output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let version = regex::Regex::new(r#"(?:openjdk|java) version "([^"]+)""#)
        .ok()?
        .captures(&text)?
        .get(1)?
        .as_str()
        .to_owned();
    let major = if version.starts_with("1.") {
        version.split('.').nth(1)?.parse().ok()?
    } else {
        version.split(['.', '+', '_', '-']).next()?.parse().ok()?
    };
    Some(JavaRuntime {
        path: path.into(),
        version,
        major,
    })
}
pub fn discover_java(extra: &[String]) -> Vec<JavaRuntime> {
    let mut candidates: HashSet<PathBuf> = extra
        .iter()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.insert(PathBuf::from(home).join("bin").join(executable));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let file = dir.join(executable);
            if file.is_file() {
                candidates.insert(file);
            }
        }
    }
    fn walk(root: &Path, depth: usize, candidates: &mut HashSet<PathBuf>, executable: &str) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(ty) = entry.file_type() else {
                continue;
            };
            if ty.is_symlink() {
                continue;
            }
            if ty.is_dir() {
                walk(&entry.path(), depth - 1, candidates, executable);
            } else if entry.file_name() == executable {
                candidates.insert(entry.path());
            }
        }
    }
    if cfg!(windows) {
        let programs = PathBuf::from(
            std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into()),
        );
        for vendor in ["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "BellSoft"] {
            walk(&programs.join(vendor), 5, &mut candidates, executable);
        }
        if let Some(roaming) = dirs::data_dir() {
            walk(
                &roaming.join(".minecraft/runtime"),
                6,
                &mut candidates,
                executable,
            );
        }
    }
    let mut java: Vec<_> = candidates
        .into_iter()
        .filter_map(|p| inspect_java(&p.to_string_lossy()))
        .collect();
    java.sort_by(|a, b| b.major.cmp(&a.major).then(a.path.cmp(&b.path)));
    java
}
pub fn local_java(root: &Path) -> Vec<JavaRuntime> {
    fn walk(dir: &Path, depth: usize, found: &mut Vec<JavaRuntime>) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                walk(&entry.path(), depth - 1, found);
            } else if entry.file_name() == "java.exe" {
                if let Some(runtime) = inspect_java(&entry.path().to_string_lossy()) {
                    found.push(runtime);
                }
            }
        }
    }
    let mut found = vec![];
    walk(&root.join("runtimes"), 5, &mut found);
    found
}
/// Java requirement is declared by Mojang; fallbacks cover older metadata only.
pub fn required_java(version: &str, metadata: &Value) -> Result<u32> {
    if let Some(major) = metadata["javaVersion"]["majorVersion"].as_u64() {
        ensure!(
            (8..=99).contains(&major),
            "Invalid Java runtime requirement"
        );
        return Ok(major as u32);
    }
    let p: Vec<u32> = version.split('.').filter_map(|s| s.parse().ok()).collect();
    ensure!(
        p.first() == Some(&1),
        "This release did not declare a Java requirement"
    );
    let minor = *p.get(1).context("Invalid Minecraft version")?;
    let patch = *p.get(2).unwrap_or(&0);
    Ok(if minor <= 16 {
        8
    } else if minor == 17 {
        16
    } else if minor < 20 || minor == 20 && patch < 5 {
        17
    } else {
        21
    })
}
pub fn extract_runtime_zip(archive: &Path, destination: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    let mut expanded = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry
            .enclosed_name()
            .context("Runtime archive contains an unsafe path")?
            .to_owned();
        ensure!(
            !entry.name().contains('\\') && !entry.name().contains(':'),
            "Runtime archive contains an unsafe path"
        );
        ensure!(
            entry.unix_mode().map_or(true, |m| m & 0o170000 != 0o120000),
            "Runtime archive contains a symbolic link"
        );
        expanded = expanded.saturating_add(entry.size());
        ensure!(
            expanded < 2_000_000_000,
            "Runtime archive exceeds size limit"
        );
        let target = destination.join(name);
        if entry.is_dir() {
            fs::create_dir_all(target)?;
        } else {
            fs::create_dir_all(target.parent().context("Runtime file has no parent")?)?;
            let mut file = File::create(target)?;
            std::io::copy(&mut entry, &mut file)?;
        }
    }
    Ok(())
}
pub fn provision_java(
    root: &Path,
    client: &Client,
    major: u32,
    events: &Sender<BackendEvent>,
    id: &str,
) -> Result<JavaRuntime> {
    ensure!(cfg!(windows) && cfg!(target_arch = "x86_64"), "Automatic Java installation currently supports Windows x64. Choose Java manually in Settings.");
    if let Some(runtime) = local_java(root).into_iter().find(|j| j.major == major) {
        return Ok(runtime);
    }
    emit(
        events,
        id,
        "install",
        format!("Finding Eclipse Temurin Java {major}…"),
        None,
    );
    let mut asset = None;
    for image in ["jre", "jdk"] {
        let url = format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture=x64&heap_size=normal&image_type={image}&jvm_impl=hotspot&os=windows&vendor=eclipse");
        let response = client.get(url).send()?;
        if response.status().as_u16() == 404 {
            continue;
        }
        let data: Value = response.error_for_status()?.json()?;
        if let Some(first) = data.as_array().and_then(|a| a.first()) {
            asset = Some(first.clone());
            break;
        }
    }
    let asset = asset.with_context(|| {
        format!(
            "Eclipse Temurin Java {major} is unavailable. Choose a matching java.exe in Settings."
        )
    })?;
    let package = &asset["binary"]["package"];
    let url = reqwest::Url::parse(
        package["link"]
            .as_str()
            .context("Java download URL missing")?,
    )?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url.path().starts_with("/adoptium/"),
        "Untrusted Java runtime download URL"
    );
    let checksum = package["checksum"]
        .as_str()
        .context("Java SHA-256 checksum missing")?;
    ensure!(
        checksum.len() == 64 && checksum.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid Java checksum"
    );
    let runtimes = root.join("runtimes");
    fs::create_dir_all(&runtimes)?;
    let archive = runtimes.join(format!("temurin-{major}.zip.part"));
    let staging = runtimes.join(format!("java-{major}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&staging)?;
    let result = (|| -> Result<JavaRuntime> {
        emit(
            events,
            id,
            "install",
            format!("Downloading Java {major} for this launcher…"),
            None,
        );
        let mut response = client
            .get(url)
            .timeout(Duration::from_secs(600))
            .send()?
            .error_for_status()?;
        let mut file = File::create(&archive)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut done = 0u64;
        let total = package["size"].as_u64().unwrap_or(0);
        let mut last = 0u64;
        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            done += count as u64;
            ensure!(done < 800_000_000, "Java download exceeds size limit");
            if done.saturating_sub(last) > 2_000_000 {
                last = done;
                emit(
                    events,
                    id,
                    "install",
                    format!("Downloading Java {major} · {} MB", done / 1048576),
                    if total > 0 {
                        Some(done as f32 / total as f32 * 100.)
                    } else {
                        None
                    },
                );
            }
        }
        file.flush()?;
        drop(file);
        ensure!(
            total == 0 || done == total,
            "Java archive download was incomplete"
        );
        ensure!(
            hex::encode(hash.finalize()).eq_ignore_ascii_case(checksum),
            "Java archive checksum did not match"
        );
        emit(
            events,
            id,
            "install",
            format!("Installing Java {major} locally…"),
            None,
        );
        extract_runtime_zip(&archive, &staging)?;
        let found = {
            fn find(dir: &Path, depth: u32) -> Option<PathBuf> {
                if depth == 0 {
                    return None;
                }
                for entry in fs::read_dir(dir).ok()?.flatten() {
                    if entry.file_name() == "java.exe" && entry.path().is_file() {
                        return Some(entry.path());
                    }
                    if entry.file_type().ok()?.is_dir() {
                        if let Some(path) = find(&entry.path(), depth - 1) {
                            return Some(path);
                        }
                    }
                }
                None
            }
            find(&staging, 5).context("The Java archive did not contain java.exe")?
        };
        let runtime = inspect_java(&found.to_string_lossy())
            .context("The downloaded Java runtime did not start")?;
        ensure!(
            runtime.major == major,
            "Downloaded Java has the wrong version"
        );
        // Keep the verified unique directory; no existing installations are moved or removed.
        Ok(runtime)
    })();
    let _ = fs::remove_file(archive);
    // Failed staging folders are left for inspection; never remove a computed tree.
    result
}
fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}
fn json_get(client: &Client, url: &str) -> Result<Value> {
    Ok(client.get(url).send()?.error_for_status()?.json()?)
}
fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        let len = file.read(&mut buf)?;
        if len == 0 {
            break;
        }
        hash.update(&buf[..len]);
    }
    Ok(hex::encode(hash.finalize()))
}
#[derive(Clone)]
struct Download {
    path: PathBuf,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}
fn download(client: &Client, file: &Download) -> Result<()> {
    let url = reqwest::Url::parse(&file.url)?;
    ensure!(url.scheme() == "https", "Downloads must use HTTPS");
    if file.path.is_file() {
        let size_ok = file.size.map_or(true, |s| {
            fs::metadata(&file.path).map_or(false, |m| m.len() == s)
        });
        let hash_ok = file.sha1.as_ref().map_or(false, |h| {
            sha1_file(&file.path).map_or(false, |found| &found == h)
        });
        if size_ok && hash_ok {
            return Ok(());
        }
    }
    fs::create_dir_all(
        file.path
            .parent()
            .context("Download has no parent directory")?,
    )?;
    let temp = file.path.with_file_name(format!(
        "{}.part",
        file.path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let result = (|| -> Result<()> {
        let mut response = client
            .get(url)
            .timeout(Duration::from_secs(300))
            .send()?
            .error_for_status()?;
        let mut output = File::create(&temp)?;
        std::io::copy(&mut response, &mut output)?;
        output.flush()?;
        if let Some(size) = file.size {
            ensure!(
                fs::metadata(&temp)?.len() == size,
                "Download size did not match"
            );
        }
        if let Some(hash) = &file.sha1 {
            ensure!(
                &sha1_file(&temp)? == hash,
                "Download checksum did not match"
            );
        }
        fs::rename(&temp, &file.path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.with_context(|| {
        format!(
            "Could not download {}",
            file.path.file_name().unwrap_or_default().to_string_lossy()
        )
    })
}
fn emit(
    events: &Sender<BackendEvent>,
    id: &str,
    stage: &str,
    message: impl Into<String>,
    percent: Option<f32>,
) {
    let _ = events.send(BackendEvent::Progress(ProgressEvent {
        profile_id: Some(id.into()),
        stage: stage.into(),
        message: message.into(),
        percent,
        code: None,
    }));
}
fn download_all(
    client: &Client,
    files: Vec<Download>,
    events: &Sender<BackendEvent>,
    id: &str,
    label: &str,
) -> Result<()> {
    let total = files.len();
    if total == 0 {
        return Ok(());
    }
    emit(events, id, "install", label, Some(0.));
    let index = AtomicUsize::new(0);
    let complete = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..8.min(total) {
            scope.spawn(|| loop {
                if !errors.lock().unwrap().is_empty() {
                    break;
                }
                let i = index.fetch_add(1, Ordering::Relaxed);
                if i >= total {
                    break;
                }
                let mut result = download(client, &files[i]);
                if result.is_err() {
                    result = download(client, &files[i]);
                }
                if let Err(error) = result {
                    errors.lock().unwrap().push(error);
                    break;
                }
                let done = complete.fetch_add(1, Ordering::Relaxed) + 1;
                if done % 20 == 0 || done == total {
                    emit(
                        events,
                        id,
                        "install",
                        format!("{label} · {done}/{total}"),
                        Some(done as f32 / total as f32 * 100.),
                    );
                }
            });
        }
    });
    if let Some(error) = errors.into_inner().unwrap().into_iter().next() {
        return Err(error);
    }
    Ok(())
}
fn download_info(path: PathBuf, data: &Value) -> Result<Download> {
    Ok(Download {
        path,
        url: data["url"].as_str().context("Download URL missing")?.into(),
        sha1: data["sha1"].as_str().map(str::to_owned),
        size: data["size"].as_u64(),
    })
}

pub struct Installer {
    pub root: PathBuf,
    pub client: Client,
    manifest: Vec<Value>,
}
impl Installer {
    pub fn new(root: PathBuf, client: Client) -> Self {
        Self {
            root,
            client,
            manifest: Vec::new(),
        }
    }
    pub fn versions(&mut self) -> Result<Vec<GameVersion>> {
        let cache = self.root.join("versions-cache.json");
        match json_get(
            &self.client,
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        ) {
            Ok(data) => {
                self.manifest = data["versions"]
                    .as_array()
                    .context("Invalid Mojang version catalogue")?
                    .iter()
                    .filter(|v| {
                        v["type"] == "release" && v["id"].as_str().map_or(false, supported_version)
                    })
                    .cloned()
                    .collect();
                fs::write(cache, serde_json::to_vec(&self.manifest)?)?;
            }
            Err(error) => {
                if self.manifest.is_empty() {
                    self.manifest = serde_json::from_slice(&fs::read(cache).context(
                        "Could not reach Mojang’s version catalogue. Check your connection.",
                    )?)
                    .context("Invalid cached version catalogue")?;
                }
                if self.manifest.is_empty() {
                    return Err(error);
                }
            }
        }
        Ok(self
            .manifest
            .iter()
            .filter_map(|v| {
                Some(GameVersion {
                    id: v["id"].as_str()?.into(),
                    release_time: v["releaseTime"].as_str()?.into(),
                })
            })
            .collect())
    }
    pub fn install_and_launch(
        &mut self,
        profile: &Profile,
        account: &Account,
        token: &str,
        xuid: &str,
        java_path: &str,
        client_id: &str,
        events: &Sender<BackendEvent>,
    ) -> Result<Child> {
        validate_id(&profile.id)?;
        if profile.loader == Loader::Forge {
            bail!("Forge installation is not available in this build. Use Fabric for compatible releases or a vanilla profile.");
        }
        if self.manifest.is_empty() {
            self.versions()?;
        }
        let meta = self
            .manifest
            .iter()
            .find(|v| v["id"] == profile.version)
            .context("This version is not a published Minecraft release in the supported range.")?;
        let url = meta["url"]
            .as_str()
            .context("Version metadata URL missing")?;
        let parsed_url = reqwest::Url::parse(url)?;
        ensure!(
            parsed_url.scheme() == "https"
                && [
                    "piston-meta.mojang.com",
                    "launchermeta.mojang.com",
                    "launcher.mojang.com"
                ]
                .contains(&parsed_url.host_str().unwrap_or("")),
            "Untrusted version metadata URL"
        );
        emit(
            events,
            &profile.id,
            "install",
            format!("Preparing Minecraft {}…", profile.version),
            None,
        );
        let resources = self.root.join("minecraft");
        let version_dir = safe_join(&resources.join("versions"), &profile.version)?;
        let json_path = version_dir.join(format!("{}.json", profile.version));
        download(
            &self.client,
            &Download {
                path: json_path.clone(),
                url: url.into(),
                sha1: meta["sha1"].as_str().map(str::to_owned),
                size: None,
            },
        )?;
        let base: Value = serde_json::from_slice(&fs::read(&json_path)?)?;
        let java_major = required_java(&profile.version, &base)?;
        let custom = if profile.java_path.is_empty() {
            java_path
        } else {
            &profile.java_path
        };
        let java = if custom.is_empty() {
            discover_java(&[])
                .into_iter()
                .chain(local_java(&self.root))
                .find(|j| j.major == java_major)
                .map(Ok)
                .unwrap_or_else(|| {
                    provision_java(&self.root, &self.client, java_major, events, &profile.id)
                })?
        } else {
            inspect_java(custom).filter(|j| j.major == java_major).with_context(|| format!("Minecraft {} needs Java {}. The selected executable did not match; choose that runtime or clear the override for automatic installation.", profile.version, java_major))?
        };
        let mut version = base.clone();
        let game_path = self.root.join("instances").join(&profile.id);
        fs::create_dir_all(&game_path)?;
        if profile.loader == Loader::Fabric {
            emit(
                events,
                &profile.id,
                "install",
                "Finding a compatible Fabric loader…",
                None,
            );
            let loaders = json_get(
                &self.client,
                &format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}",
                    profile.version
                ),
            )?;
            let list = loaders
                .as_array()
                .context("Fabric returned invalid loader metadata")?;
            let chosen = if let Some(wanted) = &profile.loader_version {
                list.iter()
                    .find(|l| l["loader"]["version"] == wanted.as_str())
            } else {
                list.iter()
                    .find(|l| l["loader"]["stable"] == true)
                    .or_else(|| list.first())
            }
            .context("Fabric is unavailable for this Minecraft version or loader selection.")?;
            let loader = chosen["loader"]["version"]
                .as_str()
                .context("Fabric loader version missing")?;
            let layer = json_get(
                &self.client,
                &format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
                    profile.version, loader
                ),
            )?;
            version = merge_profile(&base, &layer);
            let layer_id = layer["id"]
                .as_str()
                .context("Fabric profile identifier missing")?;
            let layer_dir = safe_join(&resources.join("versions"), layer_id)?;
            fs::create_dir_all(&layer_dir)?;
            fs::write(
                layer_dir.join(format!("{layer_id}.json")),
                serde_json::to_vec_pretty(&layer)?,
            )?;
        }
        let natives = game_path.join("natives");
        fs::create_dir_all(&natives)?;
        let mut downloads = vec![download_info(
            version_dir.join(format!("{}.jar", profile.version)),
            &base["downloads"]["client"],
        )?];
        let mut classpath = Vec::new();
        let mut native_archives: Vec<(PathBuf, Vec<String>)> = Vec::new();
        let libraries = version["libraries"]
            .as_array()
            .context("Game libraries missing")?;
        let mut library_seen = HashSet::new();
        // Child profiles can replace a dependency by group+artifact; select the last declaration.
        for library in libraries.iter().rev() {
            if !rules_allow(library) {
                continue;
            }
            let name = library["name"].as_str().context("Library name missing")?;
            let parts: Vec<&str> = name.split(':').collect();
            ensure!(parts.len() >= 3, "Invalid Maven library name");
            let key = format!("{}:{}:{}", parts[0], parts[1], parts.get(3).unwrap_or(&""));
            if !library_seen.insert(key) {
                continue;
            }
            let artifact = &library["downloads"]["artifact"];
            if artifact.is_object() {
                let path = safe_join(
                    &resources.join("libraries"),
                    artifact["path"].as_str().context("Library path missing")?,
                )?;
                classpath.push(path.clone());
                downloads.push(download_info(path, artifact)?);
            } else if !library["downloads"].is_object() {
                let path_part = format!(
                    "{}/{}/{}/{}-{}{}.jar",
                    parts[0].replace('.', "/"),
                    parts[1],
                    parts[2],
                    parts[1],
                    parts[2],
                    parts.get(3).map(|s| format!("-{s}")).unwrap_or_default()
                );
                let path = safe_join(&resources.join("libraries"), &path_part)?;
                let host = library["url"]
                    .as_str()
                    .unwrap_or("https://libraries.minecraft.net/");
                classpath.push(path.clone());
                downloads.push(Download {
                    path,
                    url: format!("{}/{path_part}", host.trim_end_matches('/')),
                    sha1: None,
                    size: None,
                });
            }
            if let Some(native) = library["natives"][os_name()].as_str() {
                let classifier = native.replace(
                    "${arch}",
                    if cfg!(target_pointer_width = "64") {
                        "64"
                    } else {
                        "32"
                    },
                );
                let data = &library["downloads"]["classifiers"][&classifier];
                let path = safe_join(
                    &resources.join("libraries"),
                    data["path"]
                        .as_str()
                        .context("Native library path missing")?,
                )?;
                let excludes = library["extract"]["exclude"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_else(|| vec!["META-INF/".into()]);
                native_archives.push((path.clone(), excludes));
                downloads.push(download_info(path, data)?);
            }
        }
        classpath.reverse();
        classpath.push(version_dir.join(format!("{}.jar", profile.version)));
        download_all(
            &self.client,
            downloads,
            events,
            &profile.id,
            "Downloading game files and libraries",
        )?;
        for (archive, excludes) in native_archives {
            extract_natives(&archive, &natives, &excludes)?;
        }
        self.assets(&base, &resources, events, &profile.id)?;
        let logging = self.logging(&base, &resources, &profile.version)?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        let vars: HashMap<String, String> = HashMap::from([
            ("auth_player_name".into(), account.name.clone()),
            ("auth_uuid".into(), account.uuid.clone()),
            ("auth_access_token".into(), token.into()),
            (
                "auth_session".into(),
                format!("token:{token}:{}", account.uuid),
            ),
            ("user_type".into(), "mojang".into()),
            ("user_properties".into(), "{}".into()),
            (
                "version_name".into(),
                version["id"].as_str().unwrap_or(&profile.version).into(),
            ),
            ("version_type".into(), "release".into()),
            (
                "game_directory".into(),
                game_path.to_string_lossy().into_owned(),
            ),
            (
                "assets_root".into(),
                resources.join("assets").to_string_lossy().into_owned(),
            ),
            (
                "assets_index_name".into(),
                base["assetIndex"]["id"]
                    .as_str()
                    .context("Asset index ID missing")?
                    .into(),
            ),
            (
                "game_assets".into(),
                resources
                    .join("assets/virtual")
                    .join(base["assetIndex"]["id"].as_str().unwrap_or("legacy"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "natives_directory".into(),
                natives.to_string_lossy().into_owned(),
            ),
            ("launcher_name".into(), "Aster".into()),
            ("launcher_version".into(), "0.1.0".into()),
            (
                "classpath".into(),
                classpath
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(separator),
            ),
            ("classpath_separator".into(), separator.into()),
            (
                "library_directory".into(),
                resources.join("libraries").to_string_lossy().into_owned(),
            ),
            ("resolution_width".into(), "1280".into()),
            ("resolution_height".into(), "800".into()),
            ("clientid".into(), client_id.into()),
            ("auth_xuid".into(), xuid.into()),
        ]);
        let mut args = vec![
            "-Xms512M".into(),
            format!("-Xmx{}M", profile.memory_mb),
            "-Dlog4j2.formatMsgNoLookups=true".into(),
        ];
        if let Some(jvm) = version["arguments"]["jvm"].as_array() {
            args.extend(resolve_arguments(jvm, &vars));
        } else {
            args.extend([
                format!("-Djava.library.path={}", natives.display()),
                "-cp".into(),
                vars["classpath"].clone(),
            ]);
        }
        if let Some(log_argument) = logging {
            args.push(log_argument);
        }
        args.push(
            version["mainClass"]
                .as_str()
                .context("Minecraft main class missing")?
                .into(),
        );
        if let Some(game) = version["arguments"]["game"].as_array() {
            args.extend(resolve_arguments(game, &vars));
        } else {
            args.extend(
                split_legacy_arguments(
                    base["minecraftArguments"]
                        .as_str()
                        .context("Minecraft launch arguments missing")?,
                )?
                .iter()
                .map(|a| substitute(a, &vars)),
            );
        }
        ensure!(
            !args.iter().any(|a| a.contains("${")),
            "This release uses unsupported launch placeholders"
        );
        emit(
            events,
            &profile.id,
            "launch",
            "Starting Minecraft…",
            Some(100.),
        );
        let mut command = Command::new(&java.path);
        command
            .args(args)
            .current_dir(&game_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console(&mut command);
        command
            .spawn()
            .context("Minecraft could not start. Check your Java installation.")
    }
    fn assets(
        &self,
        version: &Value,
        resources: &Path,
        events: &Sender<BackendEvent>,
        id: &str,
    ) -> Result<()> {
        let index = &version["assetIndex"];
        let index_id = index["id"].as_str().context("Asset index ID missing")?;
        let index_path = safe_join(
            &resources.join("assets/indexes"),
            &format!("{index_id}.json"),
        )?;
        download(&self.client, &download_info(index_path.clone(), index)?)?;
        let data: Value = serde_json::from_slice(&fs::read(index_path)?)?;
        let objects = data["objects"]
            .as_object()
            .context("Asset object list missing")?;
        let mut files = Vec::new();
        let mut copies = Vec::new();
        let mut seen = HashSet::new();
        for (name, object) in objects {
            let hash = object["hash"].as_str().context("Asset checksum missing")?;
            ensure!(
                hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()),
                "Invalid asset hash"
            );
            let relative = format!("{}/{}", &hash[..2], hash);
            let path = safe_join(&resources.join("assets/objects"), &relative)?;
            if seen.insert(hash.to_owned()) {
                files.push(Download {
                    path: path.clone(),
                    url: format!("https://resources.download.minecraft.net/{relative}"),
                    sha1: Some(hash.into()),
                    size: object["size"].as_u64(),
                });
            }
            if data["virtual"] == true {
                copies.push((
                    path.clone(),
                    safe_join(&resources.join("assets/virtual").join(index_id), name)?,
                ));
            }
            if data["map_to_resources"] == true {
                copies.push((
                    path.clone(),
                    safe_join(
                        &self.root.join("instances").join(id).join("resources"),
                        name,
                    )?,
                ));
            }
        }
        download_all(
            &self.client,
            files,
            events,
            id,
            "Downloading Minecraft assets",
        )?;
        for (source, target) in copies {
            fs::create_dir_all(target.parent().context("Asset target has no parent")?)?;
            if !target.exists() {
                if fs::hard_link(&source, &target).is_err() {
                    fs::copy(&source, &target)?;
                }
            }
        }
        Ok(())
    }
    fn logging(
        &self,
        version: &Value,
        resources: &Path,
        game_version: &str,
    ) -> Result<Option<String>> {
        let parts: Vec<u32> = game_version
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect();
        if parts.first() == Some(&1) && parts.get(1).copied().unwrap_or(99) <= 16 {
            let (hash, name) = if parts[1] < 12 {
                (
                    "dd2b723346a8dcd48e7f4d245f6bf09e98db9696",
                    "log4j2_17-111.xml",
                )
            } else {
                (
                    "02937d122c86ce73319ef9975b58896fc1b491d1",
                    "log4j2_112-116.xml",
                )
            };
            let path = resources.join("assets/log_configs").join(name);
            download(
                &self.client,
                &Download {
                    path: path.clone(),
                    url: format!("https://launcher.mojang.com/v1/objects/{hash}/{name}"),
                    sha1: Some(hash.into()),
                    size: None,
                },
            )?;
            return Ok(Some(format!(
                "-Dlog4j.configurationFile={}",
                path.display()
            )));
        }
        let logging = &version["logging"]["client"];
        if logging.is_object() {
            let file = &logging["file"];
            let path = safe_join(
                &resources.join("assets/log_configs"),
                file["id"].as_str().context("Logging filename missing")?,
            )?;
            download(&self.client, &download_info(path.clone(), file)?)?;
            return Ok(Some(
                logging["argument"]
                    .as_str()
                    .context("Logging argument missing")?
                    .replace("${path}", &path.to_string_lossy()),
            ));
        }
        Ok(None)
    }
}
fn resolve_arguments(arguments: &[Value], vars: &HashMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for argument in arguments {
        if let Some(text) = argument.as_str() {
            out.push(substitute(text, vars));
        } else if rules_allow(argument) {
            match &argument["value"] {
                Value::String(text) => out.push(substitute(text, vars)),
                Value::Array(values) => out.extend(
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|s| substitute(s, vars)),
                ),
                _ => {}
            }
        }
    }
    out
}
fn extract_natives(archive: &Path, destination: &Path, excludes: &[String]) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir()
            || entry.name().starts_with("META-INF/")
            || excludes.iter().any(|p| entry.name().starts_with(p))
        {
            continue;
        }
        let enclosed = entry
            .enclosed_name()
            .context("Native archive contains an unsafe path")?
            .to_owned();
        // Only native binaries are needed. Flattening mirrors the game's native search path.
        let name = enclosed
            .file_name()
            .context("Native entry filename missing")?;
        let target = destination.join(name);
        let mut file = File::create(target)?;
        std::io::copy(&mut entry, &mut file)?;
    }
    Ok(())
}
