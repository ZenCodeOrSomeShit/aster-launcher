mod auth;
mod browser_oauth;
pub mod install;
pub mod storage;

use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
}
impl Default for Loader {
    fn default() -> Self {
        Self::Vanilla
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudWidget {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crosshair {
    pub enabled: bool,
    pub size: f32,
    pub gap: f32,
    pub thickness: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HudConfig {
    pub enabled: bool,
    pub scale: f32,
    pub accent: String,
    pub opacity: f32,
    pub fps: HudWidget,
    pub coordinates: HudWidget,
    pub keystrokes: HudWidget,
    pub crosshair: Crosshair,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyBody {
    pub name: String,
    pub color: String,
    pub azimuth: f32,
    pub elevation: f32,
    pub size: f32,
    pub enabled: bool,
}
impl Default for SkyBody {
    fn default() -> Self {
        Self {
            name: "Moon".into(),
            color: "#FFF1CF".into(),
            azimuth: 30.,
            elevation: 45.,
            size: 8.,
            enabled: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkyConfig {
    #[serde(default = "one")]
    pub meteor_size: f32,
    #[serde(default = "stars_color")]
    pub star_color: String,
    #[serde(default = "meteors_color")]
    pub meteor_color: String,
    #[serde(default = "bodies_default")]
    pub bodies: Vec<SkyBody>,
    pub preset: String,
    pub color: String,
    pub shooting_stars: bool,
    pub star_density: f32,
    pub meteor_frequency: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    #[serde(default = "background_default")]
    pub background: String,
    #[serde(default = "text_default")]
    pub text: String,
    #[serde(default = "button_default")]
    pub button: String,
    pub custom_menu: bool,
    pub reduced_motion: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsConfig {
    pub schema_version: u32,
    pub hud: HudConfig,
    pub sky: SkyConfig,
    pub ui: UiConfig,
}
impl Default for CosmeticsConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            hud: HudConfig {
                enabled: true,
                scale: 1.0,
                accent: "#B8C9FF".into(),
                opacity: 0.72,
                fps: HudWidget {
                    enabled: true,
                    x: 0.025,
                    y: 0.04,
                },
                coordinates: HudWidget {
                    enabled: true,
                    x: 0.025,
                    y: 0.16,
                },
                keystrokes: HudWidget {
                    enabled: true,
                    x: 0.025,
                    y: 0.72,
                },
                crosshair: Crosshair {
                    enabled: true,
                    size: 5.,
                    gap: 3.,
                    thickness: 1.,
                },
            },
            sky: SkyConfig {
                meteor_size: 1.,
                star_color: stars_color(),
                meteor_color: meteors_color(),
                bodies: bodies_default(),
                preset: "midnight".into(),
                color: "#192545".into(),
                shooting_stars: true,
                star_density: 0.7,
                meteor_frequency: 0.6,
            },
            ui: UiConfig {
                background: background_default(),
                text: text_default(),
                button: button_default(),
                custom_menu: true,
                reduced_motion: false,
            },
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader: Loader,
    #[serde(default)]
    pub loader_version: Option<String>,
    pub icon: String,
    pub color: String,
    pub memory_mb: u32,
    #[serde(default)]
    pub java_path: String,
    #[serde(default)]
    pub last_played: Option<String>,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
    #[serde(default)]
    pub cosmetics: CosmeticsConfig,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    pub java_path: String,
    pub microsoft_client_id: String,
    pub theme: String,
    pub reduced_motion: bool,
    pub close_on_launch: bool,
}
impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            java_path: String::new(),
            microsoft_client_id: option_env!("ASTER_MICROSOFT_CLIENT_ID")
                .unwrap_or("")
                .into(),
            theme: "light".into(),
            reduced_motion: false,
            close_on_launch: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub uuid: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRuntime {
    pub path: String,
    pub version: String,
    pub major: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherState {
    pub profiles: Vec<Profile>,
    pub settings: LauncherSettings,
    pub account: Option<Account>,
    pub java: Vec<JavaRuntime>,
    pub data_directory: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    pub id: String,
    pub release_time: String,
}
#[derive(Clone, Debug)]
pub struct ProgressEvent {
    pub profile_id: Option<String>,
    pub stage: String,
    pub message: String,
    pub percent: Option<f32>,
    pub code: Option<String>,
}
#[derive(Clone, Debug)]
pub enum BackendCommand {
    Refresh,
    SaveProfile(Profile),
    DeleteProfile(String),
    SaveSettings(LauncherSettings),
    Launch(String),
    ImportMods {
        profile_id: String,
        paths: Vec<PathBuf>,
    },
    ToggleMod {
        profile_id: String,
        file: String,
        enabled: bool,
    },
    InstallCosmetics(String),
    SaveCosmetics {
        profile_id: String,
        config: CosmeticsConfig,
    },
    OpenFolder(String),
    Login,
    Logout,
}
#[derive(Clone, Debug)]
pub enum BackendEvent {
    State(LauncherState),
    Versions(Vec<GameVersion>),
    Progress(ProgressEvent),
    Error(String),
    GameExited {
        profile_id: String,
        code: Option<i32>,
    },
}

pub struct Backend;
impl Backend {
    pub fn spawn(data_dir: PathBuf) -> (Sender<BackendCommand>, Receiver<BackendEvent>) {
        storage::spawn_worker(data_dir)
    }
}

fn one() -> f32 {
    1.
}
fn stars_color() -> String {
    "#C1D3FF".into()
}
fn meteors_color() -> String {
    "#D7E6FF".into()
}
fn bodies_default() -> Vec<SkyBody> {
    vec![SkyBody::default()]
}
fn background_default() -> String {
    "#111521".into()
}
fn text_default() -> String {
    "#E4E8F2".into()
}
fn button_default() -> String {
    "#2B374E".into()
}
