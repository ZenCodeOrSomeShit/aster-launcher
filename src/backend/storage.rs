use super::auth::Auth;
use super::install::{discover_java, local_java, Installer};
use super::*;
use anyhow::{ensure, Context, Result};
use reqwest::blocking::Client;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn validate_id(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 80
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "Invalid profile identifier"
    );
    Ok(())
}
pub fn valid_mod_file(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 220
            && !value.starts_with('.')
            && !value
                .chars()
                .any(|c| c.is_control() || "\\/:<>\"|?*".contains(c))
            && (value.to_lowercase().ends_with(".jar")
                || value.to_lowercase().ends_with(".jar.disabled")),
        "Invalid mod filename"
    );
    Ok(())
}
pub fn supported_version(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    if !(2..=3).contains(&parts.len())
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    let numbers: Option<Vec<u32>> = parts.iter().map(|p| p.parse().ok()).collect();
    let Some(parts) = numbers else {
        return false;
    };
    (parts[0] == 1 && parts[1] >= 7)
        || (parts[0] > 1 && (parts[0] < 26 || parts[0] == 26 && parts[1] <= 3))
}
fn number(n: f32, min: f32, max: f32) -> Result<()> {
    ensure!(
        n.is_finite() && n >= min && n <= max,
        "Setting must be between {min} and {max}"
    );
    Ok(())
}
fn color(value: &str) -> Result<()> {
    ensure!(
        value.len() == 7
            && value.starts_with('#')
            && value[1..].chars().all(|c| c.is_ascii_hexdigit()),
        "Expected a six-digit color"
    );
    Ok(())
}
pub fn validate_cosmetics(c: &CosmeticsConfig) -> Result<()> {
    ensure!(c.schema_version == 1, "Unknown cosmetics settings version");
    number(c.hud.scale, 0.5, 2.)?;
    number(c.hud.opacity, 0.1, 1.)?;
    color(&c.hud.accent)?;
    for widget in [&c.hud.fps, &c.hud.coordinates, &c.hud.keystrokes] {
        number(widget.x, 0., 1.)?;
        number(widget.y, 0., 1.)?;
    }
    number(c.hud.crosshair.size, 1., 20.)?;
    number(c.hud.crosshair.gap, 0., 15.)?;
    number(c.hud.crosshair.thickness, 1., 4.)?;
    ensure!(
        ["vanilla", "midnight", "aurora"].contains(&c.sky.preset.as_str()),
        "Unknown sky preset"
    );
    color(&c.sky.color)?;
    color(&c.sky.star_color)?;
    color(&c.sky.meteor_color)?;
    color(&c.ui.background)?;
    color(&c.ui.text)?;
    color(&c.ui.button)?;
    number(c.sky.meteor_size, 0.2, 5.)?;
    ensure!(c.sky.bodies.len() <= 64, "At most 64 sky objects");
    for b in &c.sky.bodies {
        color(&b.color)?;
        number(b.azimuth, -180., 180.)?;
        number(b.elevation, 0., 90.)?;
        number(b.size, 0.1, 30.)?;
    }
    number(c.sky.star_density, 0., 1.)?;
    number(c.sky.meteor_frequency, 0., 1.)?;
    Ok(())
}
pub fn validate_profile(p: &Profile) -> Result<()> {
    validate_id(&p.id)?;
    ensure!(
        !p.name.trim().is_empty() && p.name.len() <= 64,
        "Give your profile a name (up to 64 characters)"
    );
    ensure!(
        supported_version(&p.version),
        "Choose a release between Minecraft 1.7 and 26.3"
    );
    ensure!(
        (512..=32768).contains(&p.memory_mb),
        "Memory must be between 512 and 32768 MB"
    );
    color(&p.color)?;
    ensure!(p.java_path.len() <= 1024, "Java path too long");
    if let Some(loader) = &p.loader_version {
        ensure!(
            loader.len() <= 80
                && !loader.is_empty()
                && loader
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".+_-".contains(c)),
            "Invalid loader version"
        );
    }
    validate_cosmetics(&p.cosmetics)
}
fn validate_settings(s: &LauncherSettings) -> Result<()> {
    ensure!(s.java_path.len() <= 1024, "Java path too long");
    ensure!(
        s.microsoft_client_id.is_empty() || uuid::Uuid::parse_str(&s.microsoft_client_id).is_ok(),
        "Enter a registered Microsoft application client ID"
    );
    ensure!(
        ["dark", "light", "system", "custom"].contains(&s.theme.as_str()),
        "Unknown appearance"
    );
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Saved {
    profiles: Vec<Profile>,
    settings: LauncherSettings,
}
pub struct Store {
    pub root: PathBuf,
    saved: Saved,
}
impl Store {
    pub fn load(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let file = root.join("state.json");
        let saved = if file.exists() {
            let saved: Saved = serde_json::from_slice(&fs::read(file)?).context(
                "Launcher settings could not be read. Your game instances are unchanged.",
            )?;
            validate_settings(&saved.settings)?;
            for p in &saved.profiles {
                validate_profile(p)?;
            }
            saved
        } else {
            Saved {
                settings: LauncherSettings::default(),
                profiles: vec![
                    Profile {
                        id: "aster-default".into(),
                        name: "Aster essentials".into(),
                        version: "1.21.1".into(),
                        loader: Loader::Fabric,
                        loader_version: None,
                        icon: "sparkles".into(),
                        color: "#B8C9FF".into(),
                        memory_mb: 4096,
                        java_path: String::new(),
                        last_played: None,
                        mods: vec![],
                        cosmetics: CosmeticsConfig::default(),
                    },
                    Profile {
                        id: "vanilla-latest".into(),
                        name: "Vanilla".into(),
                        version: "1.21.1".into(),
                        loader: Loader::Vanilla,
                        loader_version: None,
                        icon: "mountain".into(),
                        color: "#A8D9B8".into(),
                        memory_mb: 4096,
                        java_path: String::new(),
                        last_played: None,
                        mods: vec![],
                        cosmetics: CosmeticsConfig::default(),
                    },
                    Profile {
                        id: "legacy-189".into(),
                        name: "Legacy 1.8.9".into(),
                        version: "1.8.9".into(),
                        loader: Loader::Vanilla,
                        loader_version: None,
                        icon: "box".into(),
                        color: "#DEBE97".into(),
                        memory_mb: 2048,
                        java_path: String::new(),
                        last_played: None,
                        mods: vec![],
                        cosmetics: CosmeticsConfig::default(),
                    },
                ],
            }
        };
        let store = Self { root, saved };
        store.persist()?;
        Ok(store)
    }
    fn persist(&self) -> Result<()> {
        let temp = self.root.join("state.json.tmp");
        fs::write(&temp, serde_json::to_vec_pretty(&self.saved)?)?;
        fs::rename(temp, self.root.join("state.json"))?;
        Ok(())
    }
    pub fn profile(&self, id: &str) -> Result<&Profile> {
        validate_id(id)?;
        self.saved
            .profiles
            .iter()
            .find(|p| p.id == id)
            .context("Profile not found")
    }
    pub fn instance(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self.root.join("instances").join(id))
    }
    pub fn save_profile(&mut self, mut profile: Profile) -> Result<()> {
        validate_profile(&profile)?;
        profile.name = profile.name.trim().into();
        profile.mods.clear();
        if let Some(index) = self.saved.profiles.iter().position(|p| p.id == profile.id) {
            profile.last_played = self.saved.profiles[index].last_played.clone();
            self.saved.profiles[index] = profile;
        } else {
            ensure!(
                self.saved.profiles.len() < 100,
                "Maximum of 100 profiles reached"
            );
            self.saved.profiles.push(profile);
        }
        self.persist()
    }
    fn mods(&self, id: &str) -> Result<Vec<ModEntry>> {
        let dir = self.instance(id)?.join("mods");
        fs::create_dir_all(&dir)?;
        let mut mods = vec![];
        for entry in fs::read_dir(dir)?.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if valid_mod_file(&file).is_err() {
                continue;
            }
            let ty = entry.file_type()?;
            if !ty.is_file() || ty.is_symlink() {
                continue;
            }
            mods.push(ModEntry {
                name: file
                    .trim_end_matches(".disabled")
                    .trim_end_matches(".jar")
                    .into(),
                enabled: !file.ends_with(".disabled"),
                size: entry.metadata()?.len(),
                file,
            });
        }
        mods.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(mods)
    }
    fn snapshot(&self, account: Option<Account>, java: &[JavaRuntime]) -> Result<LauncherState> {
        let mut profiles = self.saved.profiles.clone();
        for p in &mut profiles {
            if let Ok(bytes) = fs::read(self.instance(&p.id)?.join("config/aster-cosmetics.json")) {
                if let Ok(c) = serde_json::from_slice::<CosmeticsConfig>(&bytes) {
                    if validate_cosmetics(&c).is_ok() {
                        p.cosmetics = c;
                    }
                }
            }
        }
        for p in &mut profiles {
            p.mods = self.mods(&p.id)?;
        }
        Ok(LauncherState {
            profiles,
            settings: self.saved.settings.clone(),
            account,
            java: java.to_vec(),
            data_directory: self.root.to_string_lossy().into_owned(),
        })
    }
    fn write_cosmetics(&self, id: &str) -> Result<()> {
        let p = self.profile(id)?;
        validate_cosmetics(&p.cosmetics)?;
        let dir = self.instance(id)?.join("config");
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("aster-cosmetics.json"),
            serde_json::to_vec_pretty(&p.cosmetics)?,
        )?;
        Ok(())
    }
}

pub fn spawn_worker(root: PathBuf) -> (Sender<BackendCommand>, Receiver<BackendEvent>) {
    let (commands, receiver) = mpsc::channel();
    let (events, results) = mpsc::channel();
    std::thread::Builder::new()
        .name("aster-backend".into())
        .spawn(move || {
            if let Err(error) = worker(root, receiver, &events) {
                let _ = events.send(BackendEvent::Error(format!("{error:#}")));
            }
        })
        .expect("Could not create launcher worker");
    (commands, results)
}
fn send_state(store: &Store, auth: &Auth, java: &[JavaRuntime], events: &Sender<BackendEvent>) {
    match store.snapshot(auth.account(), java) {
        Ok(state) => {
            let _ = events.send(BackendEvent::State(state));
        }
        Err(error) => {
            let _ = events.send(BackendEvent::Error(format!("{error:#}")));
        }
    }
}
fn worker(
    root: PathBuf,
    receiver: Receiver<BackendCommand>,
    events: &Sender<BackendEvent>,
) -> Result<()> {
    let mut store = Store::load(root.clone())?;
    let client = Client::builder()
        .user_agent("AsterLauncher/0.1.0")
        .timeout(Duration::from_secs(30))
        .build()?;
    let mut auth = Auth::new(&root, client.clone());
    let mut installer = Installer::new(root, client);
    let running: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let mut java = vec![];
    send_state(&store, &auth, &java, events);
    java = discover_java(&[store.saved.settings.java_path.clone()]);
    java.extend(local_java(&store.root));
    send_state(&store, &auth, &java, events);
    match installer.versions() {
        Ok(versions) => {
            let _ = events.send(BackendEvent::Versions(versions));
        }
        Err(error) => {
            let _ = events.send(BackendEvent::Error(format!("{error:#}")));
        }
    }
    for command in receiver {
        let idle = |id: &str| -> Result<()> {
            validate_id(id)?;
            ensure!(
                !running.lock().unwrap().contains(id),
                "Close this Minecraft instance before changing its files"
            );
            Ok(())
        };
        let result = (|| -> Result<()> {
            match command {
                BackendCommand::Refresh => {
                    java = discover_java(&[store.saved.settings.java_path.clone()]);
                    java.extend(local_java(&store.root));
                    let _ = events.send(BackendEvent::Versions(installer.versions()?));
                }
                BackendCommand::SaveProfile(p) => {
                    idle(&p.id)?;
                    let id = p.id.clone();
                    store.save_profile(p)?;
                    store.write_cosmetics(&id)?;
                }
                BackendCommand::DeleteProfile(id) => {
                    idle(&id)?;
                    store.profile(&id)?;
                    store.saved.profiles.retain(|p| p.id != id);
                    store.persist()?;
                }
                BackendCommand::SaveSettings(settings) => {
                    validate_settings(&settings)?;
                    if settings.microsoft_client_id != store.saved.settings.microsoft_client_id {
                        auth.logout()?;
                    }
                    store.saved.settings = settings;
                    store.persist()?;
                    java = discover_java(&[store.saved.settings.java_path.clone()]);
                }
                BackendCommand::SaveCosmetics { profile_id, config } => {
                    idle(&profile_id)?;
                    validate_cosmetics(&config)?;
                    let mut p = store.profile(&profile_id)?.clone();
                    p.cosmetics = config;
                    store.save_profile(p)?;
                    store.write_cosmetics(&profile_id)?;
                }
                BackendCommand::ImportMods { profile_id, paths } => {
                    idle(&profile_id)?;
                    ensure!(
                        store.profile(&profile_id)?.loader != Loader::Vanilla,
                        "Choose Fabric for this profile before adding mods"
                    );
                    let dir = store.instance(&profile_id)?.join("mods");
                    fs::create_dir_all(&dir)?;
                    for source in paths {
                        let name = source
                            .file_name()
                            .context("Mod filename missing")?
                            .to_string_lossy()
                            .into_owned();
                        valid_mod_file(&name)?;
                        ensure!(
                            name.to_lowercase().ends_with(".jar") && source.is_file(),
                            "Select a .jar mod file"
                        );
                        let mut magic = [0u8; 4];
                        File::open(&source)?.read_exact(&mut magic)?;
                        ensure!(magic[..2] == [0x50, 0x4b], "{name} is not a Java archive");
                        let target = dir.join(name);
                        ensure!(!target.exists(), "A mod with that filename already exists");
                        fs::copy(source, target)?;
                    }
                }
                BackendCommand::ToggleMod {
                    profile_id,
                    file,
                    enabled,
                } => {
                    idle(&profile_id)?;
                    store.profile(&profile_id)?;
                    valid_mod_file(&file)?;
                    let dir = store.instance(&profile_id)?.join("mods");
                    let source = dir.join(&file);
                    let info = fs::symlink_metadata(&source)?;
                    ensure!(
                        info.is_file() && !info.is_symlink(),
                        "Only regular mod archives can be toggled"
                    );
                    let name = if enabled {
                        file.trim_end_matches(".disabled").to_owned()
                    } else if file.ends_with(".disabled") {
                        file.clone()
                    } else {
                        format!("{file}.disabled")
                    };
                    if name != file {
                        let target = dir.join(name);
                        ensure!(!target.exists(), "A mod with that filename already exists");
                        fs::rename(source, target)?;
                    }
                }
                BackendCommand::InstallCosmetics(id) => {
                    idle(&id)?;
                    install_cosmetics(&store, &installer.client, &id)?;
                    store.write_cosmetics(&id)?;
                    let _ = events.send(BackendEvent::Progress(ProgressEvent {
                        profile_id: Some(id),
                        stage: "idle".into(),
                        message: "Aster Cosmetics installed. Your settings are ready for launch."
                            .into(),
                        percent: Some(100.),
                        code: None,
                    }));
                }
                BackendCommand::OpenFolder(id) => {
                    store.profile(&id)?;
                    let path = store.instance(&id)?;
                    fs::create_dir_all(&path)?;
                    open::that(path)?;
                }
                BackendCommand::Login => {
                    auth.login(&store.saved.settings.microsoft_client_id, events)?;
                    let _ = events.send(BackendEvent::Progress(ProgressEvent {
                        profile_id: None,
                        stage: "idle".into(),
                        message: format!(
                            "Signed in as {}.",
                            auth.account().map(|a| a.name).unwrap_or_default()
                        ),
                        percent: None,
                        code: None,
                    }));
                }
                BackendCommand::Logout => {
                    auth.logout()?;
                    let _ = events.send(BackendEvent::Progress(ProgressEvent {
                        profile_id: None,
                        stage: "idle".into(),
                        message: "Signed out. Ready when you are.".into(),
                        percent: None,
                        code: None,
                    }));
                }
                BackendCommand::Launch(id) => {
                    idle(&id)?;
                    let _ = events.send(BackendEvent::Progress(ProgressEvent {
                        profile_id: Some(id.clone()),
                        stage: "auth".into(),
                        message: "Checking Microsoft sign-in…".into(),
                        percent: None,
                        code: None,
                    }));
                    let (account, token, xuid) =
                        auth.token(&store.saved.settings.microsoft_client_id)?;
                    let config_path = store.instance(&id)?.join("config/aster-cosmetics.json");
                    if let Ok(bytes) = fs::read(config_path) {
                        if let Ok(c) = serde_json::from_slice::<CosmeticsConfig>(&bytes) {
                            if validate_cosmetics(&c).is_ok() {
                                if let Some(p) =
                                    store.saved.profiles.iter_mut().find(|p| p.id == id)
                                {
                                    p.cosmetics = c;
                                }
                            }
                        }
                    }
                    let profile = store.profile(&id)?.clone();
                    store.write_cosmetics(&id)?;
                    let mut child = installer.install_and_launch(
                        &profile,
                        &account,
                        &token,
                        &xuid,
                        &store.saved.settings.java_path,
                        &store.saved.settings.microsoft_client_id,
                        events,
                    )?;
                    running.lock().unwrap().insert(id.clone());
                    java.extend(local_java(&store.root));
                    if let Some(p) = store.saved.profiles.iter_mut().find(|p| p.id == id) {
                        p.last_played = Some(
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                                .to_string(),
                        );
                    }
                    store.persist()?;
                    let _ = events.send(BackendEvent::Progress(ProgressEvent {
                        profile_id: Some(id.clone()),
                        stage: "running".into(),
                        message: format!("Minecraft {} is running.", profile.version),
                        percent: Some(100.),
                        code: None,
                    }));
                    let running = running.clone();
                    let events = events.clone();
                    let dir = store.instance(&id)?;
                    let log =
                        Arc::new(Mutex::new(File::create(dir.join("aster-last-launch.log"))?));
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();
                    if let Some(pipe) = stdout {
                        capture_log(pipe, log.clone(), token.clone());
                    }
                    if let Some(pipe) = stderr {
                        capture_log(pipe, log, token);
                    }
                    std::thread::spawn(move || {
                        let code = child.wait().ok().and_then(|s| s.code());
                        running.lock().unwrap().remove(&id);
                        let _ = events.send(BackendEvent::GameExited {
                            profile_id: id,
                            code,
                        });
                    });
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = events.send(BackendEvent::Error(format!("{error:#}")));
        }
        send_state(&store, &auth, &java, events);
    }
    Ok(())
}
use std::collections::HashSet;
fn capture_log<R: Read + Send + 'static>(pipe: R, file: Arc<Mutex<File>>, token: String) {
    std::thread::spawn(move || {
        for line in BufReader::new(pipe)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let clean = line.replace(&token, "[redacted]");
            if let Ok(mut file) = file.lock() {
                let _ = writeln!(file, "{clean}");
            }
        }
    });
}
fn install_cosmetics(store: &Store, client: &Client, id: &str) -> Result<()> {
    let p = store.profile(id)?;
    ensure!(p.version == "1.21.1" && p.loader == Loader::Fabric, "Aster Cosmetics currently supports Minecraft 1.21.1 with Fabric. Select or create a compatible profile.");
    let jar_name = "aster-cosmetics-0.1.0.jar";
    // The complete mod (including its Rust native library) travels inside the launcher executable.
    const COSMETICS_JAR: &[u8] =
        include_bytes!("../../cosmetics-mod/build/libs/aster-cosmetics-0.1.0.jar");
    let versions: Value = client.get("https://api.modrinth.com/v2/project/P7dR8mSH/version?loaders=%5B%22fabric%22%5D&game_versions=%5B%221.21.1%22%5D").send()?.error_for_status()?.json()?;
    let files = versions[0]["files"]
        .as_array()
        .context("No compatible Fabric API release found")?;
    let file = files
        .iter()
        .find(|f| f["primary"] == true)
        .or_else(|| files.first())
        .context("Fabric API download missing")?;
    let filename = file["filename"]
        .as_str()
        .context("Fabric API filename missing")?;
    valid_mod_file(filename)?;
    let url = file["url"].as_str().context("Fabric API URL missing")?;
    let url = reqwest::Url::parse(url)?;
    ensure!(
        url.scheme() == "https" && url.host_str() == Some("cdn.modrinth.com"),
        "Untrusted Fabric API download URL"
    );
    let dir = store.instance(id)?.join("mods");
    fs::create_dir_all(&dir)?;
    let bytes = client
        .get(url)
        .timeout(Duration::from_secs(120))
        .send()?
        .error_for_status()?
        .bytes()?;
    let hash = file["hashes"]["sha1"]
        .as_str()
        .context("Fabric API checksum missing")?;
    use sha1::{Digest, Sha1};
    ensure!(
        hex::encode(Sha1::digest(&bytes)) == hash,
        "Fabric API checksum did not match"
    );
    fs::write(dir.join(filename), bytes)?;
    fs::write(dir.join(jar_name), COSMETICS_JAR)?;
    Ok(())
}
