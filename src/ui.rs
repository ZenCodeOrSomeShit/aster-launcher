use crate::{backend::*, scenery};
use eframe::egui::{
    self, vec2, Align, Align2, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Vec2,
};
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

const INK: Color32 = Color32::from_rgb(35, 39, 44);
const MUTED: Color32 = Color32::from_rgb(122, 126, 131);
const ACCENT: Color32 = Color32::from_rgb(188, 204, 246);
const SIDEBAR: Color32 = Color32::from_rgb(28, 33, 37);
const PAPER: Color32 = Color32::from_rgb(245, 245, 242);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Play,
    Profiles,
    Mods,
    Hud,
    Sky,
    Settings,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Play => "Minecraft",
            Self::Profiles => "Profiles",
            Self::Mods => "Make room for more.",
            Self::Hud => "HUD layout",
            Self::Sky => "Sky editor",
            Self::Settings => "Settings",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Profiles => "Profiles",
            Self::Mods => "Mods",
            Self::Hud => "HUD studio",
            Self::Sky => "Sky studio",
            Self::Settings => "Settings",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Play => "Select a profile to launch.",
            Self::Profiles => "Separate versions, mods, and a look of their own.",
            Self::Mods => "Your mods. Your instances. No crossed wires.",
            Self::Hud => "Drag the details. Keep what matters. Hide the rest.",
            Self::Sky => "An original night sky. Made from geometry, without shader packs.",
            Self::Settings => "A few thoughtful defaults. All yours to change.",
        }
    }
}

pub struct Aster {
    tx: Sender<BackendCommand>,
    rx: Receiver<BackendEvent>,
    state: LauncherState,
    versions: Vec<GameVersion>,
    page: Page,
    selected: String,
    cosmetics: CosmeticsConfig,
    dirty: bool,
    settings: LauncherSettings,
    progress: Option<ProgressEvent>,
    logs: Vec<String>,
    show_logs: bool,
    toast: Option<(String, Instant)>,
    editing: Option<Profile>,
    delete_id: Option<String>,
    started: Instant,
    loaded: bool,
}
impl Aster {
    pub fn new(cc: &eframe::CreationContext<'_>, dir: PathBuf) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(data) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
            fonts
                .font_data
                .insert("system".into(), egui::FontData::from_owned(data).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "system".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        let (tx, rx) = Backend::spawn(dir);
        Self {
            tx,
            rx,
            state: LauncherState {
                profiles: vec![],
                settings: LauncherSettings::default(),
                account: None,
                java: vec![],
                data_directory: String::new(),
            },
            versions: vec![],
            page: Page::Play,
            selected: String::new(),
            cosmetics: CosmeticsConfig::default(),
            dirty: false,
            settings: LauncherSettings::default(),
            progress: None,
            logs: vec![],
            show_logs: false,
            toast: None,
            editing: None,
            delete_id: None,
            started: Instant::now(),
            loaded: false,
        }
    }
    fn send(&self, c: BackendCommand) {
        let _ = self.tx.send(c);
    }
    fn notify(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }
    fn selected_profile(&self) -> Option<Profile> {
        self.state
            .profiles
            .iter()
            .find(|p| p.id == self.selected)
            .cloned()
    }
    fn select(&mut self, id: String) {
        if self.dirty {
            self.send(BackendCommand::SaveCosmetics {
                profile_id: self.selected.clone(),
                config: self.cosmetics.clone(),
            });
        }
        self.selected = id;
        self.dirty = false;
        if let Some(p) = self.selected_profile() {
            self.cosmetics = p.cosmetics;
        }
    }
    fn save_cosmetics(&mut self) {
        self.send(BackendCommand::SaveCosmetics {
            profile_id: self.selected.clone(),
            config: self.cosmetics.clone(),
        });
        self.dirty = false;
        self.notify("Saving this profile's look…");
    }
    fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                BackendEvent::State(s) => {
                    if !self.loaded {
                        self.settings = s.settings.clone();
                        self.loaded = true;
                    }
                    self.state = s;
                    if !self.state.profiles.iter().any(|p| p.id == self.selected) {
                        self.selected = self
                            .state
                            .profiles
                            .first()
                            .map(|p| p.id.clone())
                            .unwrap_or_default();
                        self.dirty = false;
                    }
                    if !self.dirty {
                        if let Some(p) = self.selected_profile() {
                            self.cosmetics = p.cosmetics;
                        }
                    }
                }
                BackendEvent::Versions(v) => self.versions = v,
                BackendEvent::Progress(p) => {
                    self.logs.push(match &p.profile_id {
                        Some(id) => format!("[{id}] {}", p.message),
                        None => p.message.clone(),
                    });
                    if self.logs.len() > 200 {
                        self.logs.remove(0);
                    }
                    self.progress = Some(p);
                }
                BackendEvent::Error(e) => {
                    self.logs.push(format!("Error: {e}"));
                    self.notify(e);
                    self.progress = None;
                }
                BackendEvent::GameExited { profile_id, code } => {
                    self.send(BackendCommand::Refresh);
                    self.notify(format!(
                        "Minecraft closed ({profile_id}, exit {}).",
                        code.map(|x| x.to_string())
                            .unwrap_or_else(|| "unknown".into())
                    ));
                    self.progress = None;
                }
            }
        }
    }
    fn theme(&self, ctx: &egui::Context) {
        let custom = self.state.settings.theme == "custom";
        let dark = self.state.settings.theme == "dark" || custom;
        let mut v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = if dark {
            Color32::from_rgb(35, 40, 45)
        } else {
            PAPER
        };
        v.window_fill = if dark {
            Color32::from_rgb(42, 47, 53)
        } else {
            Color32::WHITE
        };
        v.override_text_color = Some(if dark {
            Color32::from_rgb(237, 238, 235)
        } else {
            INK
        });
        v.selection.bg_fill = ACCENT;
        v.selection.stroke = Stroke::new(1.0_f32, INK);
        v.widgets.inactive.corner_radius = egui::CornerRadius::same(8);
        v.widgets.hovered.corner_radius = egui::CornerRadius::same(8);
        v.widgets.active.corner_radius = egui::CornerRadius::same(8);
        v.widgets.inactive.bg_fill = if dark {
            Color32::from_rgb(55, 62, 68)
        } else {
            Color32::from_rgb(234, 235, 232)
        };
        if custom {
            v.panel_fill = scenery::color(&self.cosmetics.ui.background);
            v.window_fill = v.panel_fill;
            v.override_text_color = Some(scenery::color(&self.cosmetics.ui.text));
            v.widgets.inactive.bg_fill = scenery::color(&self.cosmetics.ui.button);
            v.widgets.inactive.weak_bg_fill = v.widgets.inactive.bg_fill;
            v.selection.bg_fill = scenery::color(&self.cosmetics.hud.accent);
        }
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(10., 10.);
            s.spacing.button_padding = vec2(14., 9.);
            s.spacing.slider_width = 170.;
            s.text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(14.));
            s.text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(14.));
            s.text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(26.));
        });
    }
    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation")
            .exact_width(204.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(47, 55, 61);
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_rgb(47, 55, 61);
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(62, 72, 80);
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(62, 72, 80);
                ui.visuals_mut().widgets.active.bg_fill = Color32::from_rgb(73, 85, 96);
                ui.visuals_mut().widgets.active.weak_bg_fill = Color32::from_rgb(73, 85, 96);
                ui.visuals_mut().widgets.noninteractive.bg_stroke.color =
                    Color32::from_rgb(59, 70, 76);
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(218, 226, 231));
                let p = ui.painter();
                let top = ui.cursor().min;
                scenery::star(p, top + vec2(17., 27.), 17., ACCENT);
                p.text(
                    top + vec2(45., 8.),
                    Align2::LEFT_TOP,
                    "aster",
                    FontId::proportional(34.),
                    Color32::from_rgb(242, 242, 235),
                );
                ui.add_space(75.);
                ui.label(
                    RichText::new("YOUR MINECRAFT, RECONSIDERED")
                        .size(8.5)
                        .color(Color32::from_rgb(135, 147, 154)),
                );
                ui.add_space(27.);
                for page in [Page::Play, Page::Profiles, Page::Mods, Page::Hud, Page::Sky] {
                    let (r, res) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 43.), Sense::click());
                    let t = ctx.animate_bool_with_time(
                        egui::Id::new(("nav", page as u8)),
                        self.page == page,
                        if self.settings.reduced_motion {
                            0.
                        } else {
                            0.16
                        },
                    );
                    if t > 0. {
                        ui.painter().rect_filled(
                            r,
                            9.,
                            Color32::from_rgba_unmultiplied(196, 211, 246, (t * 28.) as u8),
                        );
                    }
                    if res.hovered() && t < 0.1 {
                        ui.painter()
                            .rect_filled(r, 9., Color32::from_white_alpha(9));
                    }
                    let color = if self.page == page {
                        ACCENT
                    } else {
                        Color32::from_rgb(172, 181, 186)
                    };
                    nav_icon(
                        ui.painter(),
                        r.left_center() + vec2(21., 0.),
                        page,
                        8.,
                        color,
                    );
                    ui.painter().text(
                        r.left_center() + vec2(43., 0.),
                        Align2::LEFT_CENTER,
                        page.label(),
                        FontId::proportional(14.),
                        color,
                    );
                    if res.clicked() {
                        self.page = page;
                    }
                }
                ui.add_space(24.);
                ui.separator();
                ui.add_space(15.);
                ui.label(
                    RichText::new("CURRENT PROFILE")
                        .size(10.)
                        .color(Color32::from_rgb(126, 139, 147)),
                );
                let mut chosen = self.selected.clone();
                egui::ComboBox::from_id_salt("sidebar-profile")
                    .width(153.)
                    .selected_text(
                        RichText::new(
                            self.selected_profile()
                                .map(|p| p.name)
                                .unwrap_or_else(|| "Loading…".into()),
                        )
                        .color(ACCENT),
                    )
                    .show_ui(ui, |ui| {
                        for p in &self.state.profiles {
                            ui.selectable_value(&mut chosen, p.id.clone(), &p.name);
                        }
                    });
                if chosen != self.selected {
                    self.select(chosen);
                }
                let remaining = ui.available_height();
                ui.add_space((remaining - 150.).max(12.));
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Settings").color(Color32::from_rgb(172, 181, 186)),
                        )
                        .fill(Color32::TRANSPARENT)
                        .frame(false),
                    )
                    .clicked()
                {
                    self.page = Page::Settings;
                }
                ui.add_space(14.);
                ui.separator();
                ui.add_space(12.);
                if let Some(a) = &self.state.account {
                    ui.label(RichText::new(&a.name).color(Color32::WHITE));
                    ui.label(RichText::new("Microsoft account").size(11.).color(MUTED));
                } else {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("+   Connect account").color(ACCENT))
                                .fill(Color32::TRANSPARENT)
                                .frame(false),
                        )
                        .clicked()
                    {
                        self.send(BackendCommand::Login);
                    }
                    ui.label(
                        RichText::new("A place for your next adventure.")
                            .size(10.)
                            .color(Color32::from_rgb(120, 134, 143)),
                    );
                }
                ui.add_space(8.);
                ui.label(
                    RichText::new("NATIVE RUST  /  0.1.0")
                        .size(8.5)
                        .color(Color32::from_rgb(110, 124, 132)),
                );
            });
    }
    fn heading(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "WORKSPACE  /  {}",
                    self.page.label().to_uppercase()
                ))
                .size(10.)
                .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Activity").clicked() {
                    self.show_logs = !self.show_logs;
                }
            });
        });
        ui.add_space(19.);
        ui.label(RichText::new(self.page.title()).size(32.).strong());
        ui.label(
            RichText::new(self.page.description())
                .size(13.)
                .color(MUTED),
        );
        ui.add_space(22.);
    }
    fn play(&mut self, ui: &mut egui::Ui) {
        let profile = self.selected_profile();
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 310.), Sense::hover());
        scenery::draw(
            ui.painter(),
            r,
            &self.cosmetics.sky,
            self.started.elapsed().as_secs_f64(),
            self.settings.reduced_motion,
        );
        let p = ui.painter().with_clip_rect(r);
        p.rect_filled(r, 0., Color32::from_black_alpha(18));
        p.text(
            r.min + vec2(30., 30.),
            Align2::LEFT_TOP,
            "ASTER / MINECRAFT JAVA",
            FontId::proportional(10.),
            Color32::from_rgb(213, 221, 237),
        );
        p.text(
            r.min + vec2(28., 78.),
            Align2::LEFT_TOP,
            "Minecraft\nwith Aster.",
            FontId::proportional(45.),
            Color32::from_rgb(245, 243, 233),
        );
        p.text(
            r.min + vec2(31., 207.),
            Align2::LEFT_TOP,
            "Your profile. Your cosmetics.",
            FontId::proportional(14.),
            Color32::from_rgb(213, 219, 226),
        );
        p.text(
            r.right_bottom() - vec2(22., 20.),
            Align2::RIGHT_BOTTOM,
            "THE ASTER COLLECTION   /   01",
            FontId::proportional(9.),
            Color32::from_rgb(200, 213, 219),
        );
        let launch = Rect::from_min_size(r.min + vec2(30., 250.), vec2(178., 40.));
        let res = ui.interact(launch, ui.id().with("hero-play"), Sense::click());
        p.rect_filled(
            launch,
            8.,
            if res.is_pointer_button_down_on() {
                Color32::from_rgb(159, 181, 231)
            } else {
                ACCENT
            },
        );
        let busy = self
            .progress
            .as_ref()
            .is_some_and(|p| matches!(p.stage.as_str(), "install" | "launch" | "auth"));
        p.text(
            launch.center(),
            Align2::CENTER_CENTER,
            if busy {
                "Preparing…"
            } else {
                "Launch Minecraft"
            },
            FontId::proportional(14.),
            INK,
        );
        if res.clicked() && !busy {
            if let Some(p) = profile.clone() {
                self.save_cosmetics();
                self.send(BackendCommand::Launch(p.id));
            }
        }
        ui.add_space(12.);
        ui.horizontal(|ui| {
            if let Some(p) = &profile {
                ui.label(RichText::new(&p.name).strong());
                pill(ui, &format!("{} · {}", p.version, loader_name(&p.loader)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("Configure profile").clicked() {
                        self.editing = Some(p.clone());
                    }
                });
            }
        });
        ui.add_space(24.);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Cosmetics").size(19.).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("CRAFTED IN-HOUSE").size(9.).color(MUTED));
            });
        });
        ui.add_space(10.);
        let gap = 14.;
        let card_w = (ui.available_width() - gap * 2.) / 3.;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            if feature(
                ui,
                card_w,
                Page::Hud,
                "HUD layout",
                "Move, scale, and color every\nHUD detail.",
                Color32::from_rgb(230, 236, 248),
            ) {
                self.page = Page::Hud;
            }
            if feature(
                ui,
                card_w,
                Page::Sky,
                "Sky editor",
                "A custom sky, quiet stars,\nand passing meteors.",
                Color32::from_rgb(230, 237, 231),
            ) {
                self.page = Page::Sky;
            }
            if feature(
                ui,
                card_w,
                Page::Mods,
                "Mods",
                "Keep each mod collection\nin its own profile.",
                Color32::from_rgb(242, 232, 218),
            ) {
                self.page = Page::Mods;
            }
        });
        ui.add_space(19.);
        ui.label(RichText::new("Launcher: published releases 1.7–26.3  ·  Aster cosmetics: Fabric 1.21.1  ·  No shader pack").size(10.).color(MUTED));
    }
    fn profiles(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if primary(ui, "+  New profile").clicked() {
                self.editing = Some(Profile {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: "My world".into(),
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
                });
            }
            if ui.button("Refresh versions").clicked() {
                self.send(BackendCommand::Refresh);
            }
        });
        ui.add_space(16.);
        for p in self.state.profiles.clone() {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(48., 48.), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 12., scenery::color(&p.color).gamma_multiply(0.28));
                    scenery::star(ui.painter(), r.center(), 12., scenery::color(&p.color));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&p.name).size(17.).strong());
                        ui.label(
                            RichText::new(format!(
                                "{}  ·  {}  ·  {} GB",
                                p.version,
                                loader_name(&p.loader),
                                p.memory_mb as f32 / 1024.
                            ))
                            .color(MUTED)
                            .size(12.),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Edit").clicked() {
                            self.editing = Some(p.clone());
                        }
                        if ui
                            .button(if self.selected == p.id {
                                "Selected"
                            } else {
                                "Select"
                            })
                            .clicked()
                        {
                            self.select(p.id.clone());
                        }
                        if ui.small_button("Folder").clicked() {
                            self.send(BackendCommand::OpenFolder(p.id.clone()));
                        }
                    });
                });
            });
            ui.add_space(10.);
        }
        ui.add_space(12.);
        ui.label(
            RichText::new("Each profile has isolated mods, game settings, worlds, and cosmetics.")
                .color(MUTED),
        );
        ui.label(RichText::new("Forge is not implemented yet. Use Fabric or vanilla; choose matching mod versions.").size(12.).color(MUTED));
    }
    fn profile_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("EDITING").size(10.).color(MUTED));
            ui.label(
                RichText::new(self.selected_profile().map(|p| p.name).unwrap_or_default()).strong(),
            );
            if let Some(p) = self.selected_profile() {
                pill(ui, &format!("{} · {}", p.version, loader_name(&p.loader)));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(
                    ui,
                    if self.dirty {
                        "Save changes"
                    } else {
                        "Save look"
                    },
                )
                .clicked()
                {
                    self.save_cosmetics();
                }
            });
        });
        ui.add_space(14.);
    }
    fn compatibility(&self, ui: &mut egui::Ui) {
        if let Some(p) = self.selected_profile() {
            if p.version != "1.21.1" || p.loader != Loader::Fabric {
                ui.label(RichText::new("Preview freely. In-game Aster cosmetics currently require a Fabric 1.21.1 profile.").size(12.).color(Color32::from_rgb(168,128,64)));
                ui.add_space(10.);
            }
        }
    }
    fn mods(&mut self, ui: &mut egui::Ui) {
        let Some(profile) = self.selected_profile() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&profile.name).size(19.).strong());
            pill(
                ui,
                &format!("{} · {}", profile.version, loader_name(&profile.loader)),
            );
        });
        ui.add_space(15.);
        card(ui, |ui| {
            ui.label(RichText::new("Aster cosmetics").size(21.).strong());
            ui.label("An original HUD, a rethought menu, and a sky with a little life.");
            ui.add_space(8.);
            ui.label(
                RichText::new("Rust procedural geometry · Fabric bridge · Windows x64")
                    .color(MUTED)
                    .size(12.),
            );
            ui.add_space(10.);
            let compatible = profile.version == "1.21.1" && profile.loader == Loader::Fabric;
            if ui
                .add_enabled(
                    compatible,
                    egui::Button::new("Install Aster cosmetics + Fabric API").fill(ACCENT),
                )
                .clicked()
            {
                self.send(BackendCommand::InstallCosmetics(profile.id.clone()));
            }
            ui.label(
                RichText::new(if compatible {
                    "Installs into this profile only. Restart Minecraft to apply changes."
                } else {
                    "Create or select a Fabric 1.21.1 profile to install."
                })
                .size(11.)
                .color(MUTED),
            );
        });
        ui.add_space(22.);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Your collection").size(20.).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("+  Add .jar files").clicked() {
                    if profile.loader == Loader::Vanilla {
                        self.notify("Choose a Fabric profile to use mods.");
                    } else if let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Minecraft mods", &["jar"])
                        .pick_files()
                    {
                        self.send(BackendCommand::ImportMods {
                            profile_id: profile.id.clone(),
                            paths,
                        });
                    }
                }
                if ui.small_button("Open folder").clicked() {
                    self.send(BackendCommand::OpenFolder(profile.id.clone()));
                }
            });
        });
        ui.add_space(12.);
        if profile.mods.is_empty() {
            card(ui, |ui| {
                ui.add_space(15.);
                ui.label(RichText::new("A clean slate.").size(20.));
                ui.label(
                    RichText::new(
                        "Add matching Fabric .jar files, or start with Aster cosmetics above.",
                    )
                    .color(MUTED),
                );
                ui.add_space(15.);
            });
        }
        for m in profile.mods {
            card(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut enabled = m.enabled;
                    if ui.checkbox(&mut enabled, "").changed() {
                        self.send(BackendCommand::ToggleMod {
                            profile_id: profile.id.clone(),
                            file: m.file.clone(),
                            enabled,
                        });
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&m.name).strong());
                        ui.label(
                            RichText::new(format!(
                                "{}  ·  {:0.1} MB",
                                m.file,
                                m.size as f64 / 1_048_576.
                            ))
                            .size(11.)
                            .color(MUTED),
                        );
                    });
                });
            });
            ui.add_space(8.);
        }
    }
    fn hud(&mut self, ui: &mut egui::Ui) {
        self.profile_bar(ui);
        ui.collapsing("Colors & sky objects", |ui| {
            ui.horizontal(|ui| {
                ui.label("Panel");
                color_control(ui, &mut self.cosmetics.ui.background);
                ui.label("Text");
                color_control(ui, &mut self.cosmetics.ui.text);
                ui.label("Buttons");
                color_control(ui, &mut self.cosmetics.ui.button);
            });
            ui.horizontal(|ui| {
                ui.label("Stars");
                color_control(ui, &mut self.cosmetics.sky.star_color);
                ui.label("Meteors");
                color_control(ui, &mut self.cosmetics.sky.meteor_color);
            });
            ui.add(
                egui::Slider::new(&mut self.cosmetics.sky.meteor_size, 0.2..=5.)
                    .text("Meteor size"),
            );
            if ui.button("Add sky object").clicked() {
                self.cosmetics.sky.bodies.push(SkyBody::default());
            }
            let mut remove = None;
            for (i, b) in self.cosmetics.sky.bodies.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut b.enabled, "");
                        ui.text_edit_singleline(&mut b.name);
                        color_control(ui, &mut b.color);
                        if ui.button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.add(egui::Slider::new(&mut b.azimuth, -180.0..=180.).text("Azimuth"));
                    ui.add(egui::Slider::new(&mut b.elevation, 0.0..=90.).text("Elevation"));
                    ui.add(egui::Slider::new(&mut b.size, 1.0..=30.).text("Size"));
                });
            }
            if let Some(i) = remove {
                self.cosmetics.sky.bodies.remove(i);
            }
            self.dirty = true;
        });
        self.compatibility(ui);
        let old = serde_json::to_string(&self.cosmetics).unwrap_or_default();
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 295.), Sense::hover());
        scenery::draw(
            ui.painter(),
            r,
            &self.cosmetics.sky,
            self.started.elapsed().as_secs_f64(),
            self.settings.reduced_motion,
        );
        let p = ui.painter().with_clip_rect(r);
        p.text(
            r.right_top() + vec2(-16., 14.),
            Align2::RIGHT_TOP,
            "HUD PREVIEW  ·  DRAG TO ARRANGE",
            FontId::proportional(9.),
            Color32::from_rgb(222, 230, 237),
        );
        if self.cosmetics.hud.enabled {
            let scale = self.cosmetics.hud.scale;
            let tint = scenery::color(&self.cosmetics.hud.accent);
            let opacity = self.cosmetics.hud.opacity;
            drag_widget(
                ui,
                r,
                "fps",
                &mut self.cosmetics.hud.fps,
                "144 FPS",
                vec2(78., 24.) * scale,
                tint,
                opacity,
            );
            drag_widget(
                ui,
                r,
                "coordinates",
                &mut self.cosmetics.hud.coordinates,
                "X 128   Y 64   Z −256",
                vec2(164., 24.) * scale,
                tint,
                opacity,
            );
            drag_widget(
                ui,
                r,
                "keystrokes",
                &mut self.cosmetics.hud.keystrokes,
                "       W\n  A   S   D\n LMB   RMB",
                vec2(84., 86.) * scale,
                tint,
                opacity,
            );
            let cross = &self.cosmetics.hud.crosshair;
            if cross.enabled {
                let c = r.center();
                let g = cross.gap;
                let s = cross.size;
                for (a, b) in [
                    (vec2(-g - s, 0.), vec2(-g, 0.)),
                    (vec2(g, 0.), vec2(g + s, 0.)),
                    (vec2(0., -g - s), vec2(0., -g)),
                    (vec2(0., g), vec2(0., g + s)),
                ] {
                    p.line_segment([c + a, c + b], Stroke::new(cross.thickness, tint));
                }
            }
        }
        ui.add_space(18.);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("The essentials").size(18.).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.checkbox(&mut self.cosmetics.hud.enabled, "Show custom HUD");
                });
            });
            ui.add_space(6.);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.cosmetics.hud.fps.enabled, "FPS counter");
                ui.checkbox(&mut self.cosmetics.hud.coordinates.enabled, "Coordinates");
                ui.checkbox(&mut self.cosmetics.hud.keystrokes.enabled, "Keystrokes");
                ui.checkbox(&mut self.cosmetics.hud.crosshair.enabled, "Crosshair");
            });
            ui.add_space(7.);
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut self.cosmetics.hud.scale, 0.65..=1.8).text("Scale"));
                ui.add_space(12.);
                ui.add(
                    egui::Slider::new(&mut self.cosmetics.hud.opacity, 0.1..=1.).text("Opacity"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Accent");
                color_control(ui, &mut self.cosmetics.hud.accent);
                ui.add_space(18.);
                ui.add(
                    egui::Slider::new(&mut self.cosmetics.hud.crosshair.size, 2.0..=16.)
                        .text("Crosshair size"),
                );
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut self.cosmetics.hud.crosshair.gap, 0.0..=12.).text("Gap"),
                );
                ui.add(
                    egui::Slider::new(&mut self.cosmetics.hud.crosshair.thickness, 1.0..=4.)
                        .text("Thickness"),
                );
            });
        });
        ui.add_space(12.);
        ui.horizontal(|ui| {
            if ui.button("Reset HUD layout").clicked() {
                self.cosmetics.hud = CosmeticsConfig::default().hud;
                self.dirty = true;
            }
            ui.label(
                RichText::new("In game, press Right Shift to edit.")
                    .size(12.)
                    .color(MUTED),
            );
        });
        if old != serde_json::to_string(&self.cosmetics).unwrap_or_default() {
            self.dirty = true;
        }
    }
    fn sky(&mut self, ui: &mut egui::Ui) {
        self.profile_bar(ui);
        ui.collapsing("Colors & sky objects", |ui| {
            ui.horizontal(|ui| {
                ui.label("Panel");
                color_control(ui, &mut self.cosmetics.ui.background);
                ui.label("Text");
                color_control(ui, &mut self.cosmetics.ui.text);
                ui.label("Buttons");
                color_control(ui, &mut self.cosmetics.ui.button);
            });
            ui.horizontal(|ui| {
                ui.label("Stars");
                color_control(ui, &mut self.cosmetics.sky.star_color);
                ui.label("Meteors");
                color_control(ui, &mut self.cosmetics.sky.meteor_color);
            });
            ui.add(
                egui::Slider::new(&mut self.cosmetics.sky.meteor_size, 0.2..=5.)
                    .text("Meteor size"),
            );
            if ui.button("Add sky object").clicked() {
                self.cosmetics.sky.bodies.push(SkyBody::default());
            }
            let mut remove = None;
            for (i, b) in self.cosmetics.sky.bodies.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut b.enabled, "");
                        ui.text_edit_singleline(&mut b.name);
                        color_control(ui, &mut b.color);
                        if ui.button("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.add(egui::Slider::new(&mut b.azimuth, -180.0..=180.).text("Azimuth"));
                    ui.add(egui::Slider::new(&mut b.elevation, 0.0..=90.).text("Elevation"));
                    ui.add(egui::Slider::new(&mut b.size, 1.0..=30.).text("Size"));
                });
            }
            if let Some(i) = remove {
                self.cosmetics.sky.bodies.remove(i);
            }
            self.dirty = true;
        });
        self.compatibility(ui);
        let old = serde_json::to_string(&self.cosmetics).unwrap_or_default();
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 300.), Sense::hover());
        scenery::draw(
            ui.painter(),
            r,
            &self.cosmetics.sky,
            self.started.elapsed().as_secs_f64(),
            self.settings.reduced_motion,
        );
        let p = ui.painter();
        p.text(
            r.left_bottom() + vec2(23., -22.),
            Align2::LEFT_BOTTOM,
            "Sky preview",
            FontId::proportional(22.),
            Color32::from_rgb(242, 241, 229),
        );
        ui.add_space(16.);
        ui.horizontal(|ui| {
            for (key, title, col) in [
                ("midnight", "Midnight", Color32::from_rgb(41, 51, 78)),
                ("aurora", "Aurora", Color32::from_rgb(82, 133, 132)),
                ("vanilla", "Original sky", Color32::from_rgb(83, 120, 156)),
            ] {
                let selected = self.cosmetics.sky.preset == key;
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!(
                                "{}  {}",
                                if selected { "●" } else { "○" },
                                title
                            ))
                            .color(if selected {
                                INK
                            } else {
                                ui.visuals().text_color()
                            }),
                        )
                        .fill(if selected {
                            ACCENT
                        } else {
                            ui.visuals().widgets.inactive.bg_fill
                        }),
                    )
                    .clicked()
                {
                    self.cosmetics.sky.preset = key.into();
                    if key == "midnight" {
                        self.cosmetics.sky.color = "#192545".into();
                    }
                    if key == "aurora" {
                        self.cosmetics.sky.color = "#173E48".into();
                    }
                }
                let _ = col;
            }
        });
        ui.add_space(16.);
        card(ui, |ui| {
            ui.label(RichText::new("Set the atmosphere").size(18.).strong());
            ui.add_space(7.);
            ui.horizontal(|ui| {
                ui.label("Sky tint");
                color_control(ui, &mut self.cosmetics.sky.color);
                ui.add_space(18.);
                ui.checkbox(&mut self.cosmetics.sky.shooting_stars, "Shooting stars");
            });
            ui.add(
                egui::Slider::new(&mut self.cosmetics.sky.star_density, 0.0..=1.)
                    .text("Star density"),
            );
            ui.add(
                egui::Slider::new(&mut self.cosmetics.sky.meteor_frequency, 0.0..=1.)
                    .text("Meteor frequency"),
            );
            ui.label(RichText::new("Custom presets replace the Overworld sky. Vanilla restores the original renderer.").size(11.).color(MUTED));
        });
        ui.add_space(15.);
        card(ui, |ui| {
            ui.label(RichText::new("Main menu").strong());
            ui.checkbox(
                &mut self.cosmetics.ui.custom_menu,
                "Use the custom Aster title screen",
            );
            ui.checkbox(
                &mut self.cosmetics.ui.reduced_motion,
                "Reduce cosmetic motion in game",
            );
        });
        if old != serde_json::to_string(&self.cosmetics).unwrap_or_default() {
            self.dirty = true;
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Appearance").size(20.).strong());
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.settings.theme, "light".into(), "Light");
                ui.selectable_value(&mut self.settings.theme, "dark".into(), "Dark");
                ui.selectable_value(
                    &mut self.settings.theme,
                    "custom".into(),
                    "Cosmetic palette",
                );
            });
            ui.checkbox(&mut self.settings.reduced_motion, "Reduce launcher motion");
            ui.label(
                RichText::new("Motion stays quiet. Controls respond immediately.")
                    .color(MUTED)
                    .size(12.),
            );
        });
        ui.add_space(15.);
        card(ui, |ui| {
            ui.label(RichText::new("Java runtime").size(20.).strong());
            ui.label(
                RichText::new("Aster checks each version's required Java before launch.")
                    .color(MUTED),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.java_path)
                        .hint_text("Auto-detect, or select javaw.exe")
                        .desired_width(ui.available_width() - 100.),
                );
                if ui.button("Browse…").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Java executable", &["exe"])
                        .pick_file()
                    {
                        self.settings.java_path = p.to_string_lossy().to_string();
                    }
                }
            });
            if self.state.java.is_empty() {
                ui.label(RichText::new("No Java runtime detected. Install the Java release required by your Minecraft version.").size(12.).color(Color32::from_rgb(167,127,65)));
            }
            for j in &self.state.java {
                ui.label(
                    RichText::new(format!("Java {}  ·  {}", j.major, j.path))
                        .size(11.)
                        .color(MUTED),
                );
            }
        });
        ui.add_space(15.);
        card(ui, |ui| {
            ui.label(RichText::new("Microsoft account").size(20.).strong());
            if let Some(a) = self.state.account.clone() {
                ui.label(format!("Connected as {}", a.name));
                if ui.button("Disconnect account").clicked() {
                    self.send(BackendCommand::Logout);
                }
            } else {
                ui.label("Connect an account that owns Minecraft Java Edition.");
                ui.collapsing("Developer OAuth configuration",|ui|{ui.label(
                    RichText::new("Microsoft public application ID")
                        .size(12.)
                        .strong(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.microsoft_client_id)
                        .hint_text("Your registered application's client ID")
                        .desired_width(ui.available_width()),
                );
                ui.label(RichText::new("Publisher configuration: personal Microsoft accounts, desktop http://localhost redirect, and Minecraft API access.").color(MUTED).size(11.));});
                if self.settings.microsoft_client_id.is_empty()
                    && option_env!("ASTER_MICROSOFT_CLIENT_ID")
                        .unwrap_or("")
                        .is_empty()
                {
                    ui.label(
                        "Sign-in is unavailable: this build has no Aster application registration.",
                    );
                }
                if ui.button("Sign in with Microsoft").clicked() {
                    self.send(BackendCommand::SaveSettings(self.settings.clone()));
                    self.send(BackendCommand::Login);
                }
            }
        });
        ui.add_space(15.);
        ui.horizontal(|ui| {
            if primary(ui, "Save settings").clicked() {
                self.send(BackendCommand::SaveSettings(self.settings.clone()));
                self.notify("Saving settings…");
            }
            if ui.button("Refresh Java & versions").clicked() {
                self.send(BackendCommand::Refresh);
            }
        });
        ui.add_space(12.);
        ui.label(
            RichText::new(format!("Stored locally: {}", self.state.data_directory))
                .size(11.)
                .color(MUTED),
        );
        ui.label(RichText::new("Aster 0.1.0 · Native Rust · Independent project, not affiliated with Mojang or Microsoft.").size(10.).color(MUTED));
    }
    fn modal(&mut self, ctx: &egui::Context) {
        if let Some(mut p) = self.editing.clone() {
            let mut open = true;
            let mut save = false;
            let mut delete = false;
            let mut cancel = false;
            egui::Window::new("Profile settings").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER,Vec2::ZERO).default_width(420.).open(&mut open).show(ctx,|ui|{
            ui.label(RichText::new("A space of your own.").size(24.).strong());ui.add_space(8.);ui.label("Name");ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(390.));ui.add_space(8.);
            ui.horizontal(|ui|{ui.label("Minecraft");egui::ComboBox::from_id_salt("version").selected_text(&p.version).width(230.).height(220.).show_ui(ui,|ui|{if self.versions.is_empty(){ui.label("Loading official version catalog…");}for v in &self.versions{ui.selectable_value(&mut p.version,v.id.clone(),&v.id);}});});
            ui.horizontal(|ui|{ui.label("Mod loader");egui::ComboBox::from_id_salt("loader").selected_text(loader_name(&p.loader)).show_ui(ui,|ui|{ui.selectable_value(&mut p.loader,Loader::Vanilla,"Vanilla");ui.selectable_value(&mut p.loader,Loader::Fabric,"Fabric");});});
            ui.label(RichText::new("Fabric availability depends on the selected game version.").size(11.).color(MUTED));
            ui.horizontal(|ui|{ui.label("Profile color");color_control(ui,&mut p.color);});
            ui.add(egui::Slider::new(&mut p.memory_mb,1024..=16384).step_by(512.).text("Memory (MB)"));
            ui.label("Java override (optional)");ui.add(egui::TextEdit::singleline(&mut p.java_path).hint_text("Use launcher setting").desired_width(390.));
            ui.label(RichText::new("Changing versions or loader keeps files. Use a new profile for different mod collections.").color(MUTED).size(11.));ui.add_space(14.);
            ui.horizontal(|ui|{if primary(ui,"Save profile").clicked(){save=true;}if ui.button("Cancel").clicked(){cancel=true;}if self.state.profiles.iter().any(|x|x.id==p.id)&&ui.small_button("Remove profile").clicked(){delete=true;}});
        });
            if save {
                if self.dirty && p.id == self.selected {
                    p.cosmetics = self.cosmetics.clone();
                }
                self.send(BackendCommand::SaveProfile(p.clone()));
                self.editing = None;
                self.notify("Saving profile…");
            } else if delete {
                self.delete_id = Some(p.id.clone());
                self.editing = None;
            } else {
                self.editing = if open && !cancel { Some(p) } else { None };
            }
        }
        if let Some(id) = self.delete_id.clone() {
            let mut cancel = false;
            let mut confirm = false;
            egui::Window::new("Remove this profile?")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("This removes the profile from Aster. Instance files are retained.");
                    ui.horizontal(|ui| {
                        if ui.button("Keep profile").clicked() {
                            cancel = true;
                        }
                        if ui.button("Remove profile").clicked() {
                            confirm = true;
                        }
                    });
                });
            if confirm {
                self.send(BackendCommand::DeleteProfile(id));
                self.delete_id = None;
            }
            if cancel {
                self.delete_id = None;
            }
        }
    }
}
impl eframe::App for Aster {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.theme(ctx);
        self.sidebar(ctx);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(if self.state.settings.theme == "custom" {
                        scenery::color(&self.cosmetics.ui.background)
                    } else if self.state.settings.theme == "dark" {
                        Color32::from_rgb(30, 35, 40)
                    } else {
                        Color32::from_rgb(236, 238, 234)
                    })
                    .inner_margin(egui::Margin::symmetric(22, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("●")
                            .color(Color32::from_rgb(119, 160, 133))
                            .size(9.),
                    );
                    let status = self
                        .progress
                        .as_ref()
                        .map(|p| p.message.clone())
                        .unwrap_or_else(|| {
                            if self.loaded {
                                "Ready when you are.".into()
                            } else {
                                "Opening your workspace…".into()
                            }
                        });
                    ui.add(
                        egui::Label::new(RichText::new(status).size(11.).color(MUTED)).truncate(),
                    );
                    if let Some(p) = &self.progress {
                        if let Some(n) = p.percent {
                            ui.add(egui::ProgressBar::new(n / 100.).desired_width(90.));
                        }
                    }
                });
            });
        if self.show_logs {
            egui::TopBottomPanel::bottom("activity")
                .resizable(true)
                .default_height(145.)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Activity");
                        if ui.small_button("Clear").clicked() {
                            self.logs.clear();
                        }
                        if ui.small_button("Close").clicked() {
                            self.show_logs = false;
                        }
                    });
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.logs {
                                ui.monospace(line);
                            }
                        });
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if self.state.settings.theme == "custom" {
                        scenery::color(&self.cosmetics.ui.background)
                    } else if self.state.settings.theme == "dark" {
                        Color32::from_rgb(35, 40, 45)
                    } else {
                        PAPER
                    })
                    .inner_margin(egui::Margin::symmetric(30, 24)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.heading(ui);
                        if let Some(p) = &self.progress {
                            if p.stage == "auth" {
                                if let Some(code) = &p.code {
                                    card(ui, |ui| {
                                        ui.label("Sign in in your browser with this code:");
                                        ui.label(RichText::new(code).size(30.).strong());
                                        if ui.button("Copy code").clicked() {
                                            ctx.copy_text(code.clone());
                                        }
                                        ui.label(&p.message);
                                    });
                                    ui.add_space(12.);
                                }
                            }
                        }
                        match self.page {
                            Page::Play => self.play(ui),
                            Page::Profiles => self.profiles(ui),
                            Page::Mods => self.mods(ui),
                            Page::Hud => self.hud(ui),
                            Page::Sky => self.sky(ui),
                            Page::Settings => self.settings(ui),
                        }
                    });
            });
        // Feedback floats above the workspace; it never moves the user's drag target.
        if let Some((message, since)) = self.toast.clone() {
            if since.elapsed() < Duration::from_secs(18) {
                egui::Area::new(egui::Id::new("feedback-toast"))
                    .order(egui::Order::Foreground)
                    .anchor(Align2::RIGHT_BOTTOM, vec2(-22., -50.))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(ui.visuals().window_fill)
                            .stroke(Stroke::new(1.0_f32, ACCENT))
                            .corner_radius(10)
                            .inner_margin(16)
                            .show(ui, |ui| {
                                ui.set_width(390.);
                                ui.horizontal_top(|ui| {
                                    ui.add(egui::Label::new(&message).wrap());
                                    if ui.small_button("x").clicked() {
                                        self.toast = None;
                                    }
                                });
                            });
                    });
            }
        }
        self.modal(ctx);
        ctx.request_repaint_after(Duration::from_millis(if self.settings.reduced_motion {
            100
        } else {
            33
        }));
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.dirty {
            self.send(BackendCommand::SaveCosmetics {
                profile_id: self.selected.clone(),
                config: self.cosmetics.clone(),
            });
        }
    }
}
fn loader_name(l: &Loader) -> &'static str {
    match l {
        Loader::Vanilla => "Vanilla",
        Loader::Fabric => "Fabric",
        Loader::Forge => "Forge (unavailable)",
    }
}
fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(INK))
            .fill(ACCENT)
            .corner_radius(8.),
    )
}
fn pill(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(135, 153, 161, 22))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(10.).color(MUTED));
        });
}
fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .corner_radius(12)
        .inner_margin(20)
        .stroke(Stroke::new(
            1.0_f32,
            Color32::from_rgba_unmultiplied(140, 150, 150, 25),
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui)
        })
        .inner
}
fn feature(ui: &mut egui::Ui, w: f32, icon: Page, title: &str, body: &str, color: Color32) -> bool {
    let (r, res) = ui.allocate_exact_size(vec2(w, 155.), Sense::click());
    let p = ui.painter();
    let dark = ui.visuals().dark_mode;
    p.rect_filled(
        r,
        11.,
        if dark {
            Color32::from_rgb(45, 52, 57)
        } else {
            Color32::WHITE
        },
    );
    let icon_r = Rect::from_min_size(r.min + vec2(16., 15.), vec2(32., 31.));
    p.rect_filled(
        icon_r,
        8.,
        if dark {
            color.gamma_multiply(0.4)
        } else {
            color
        },
    );
    nav_icon(
        p,
        icon_r.center(),
        icon,
        8.,
        if dark { ACCENT } else { INK },
    );
    p.text(
        r.min + vec2(16., 62.),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(15.),
        ui.visuals().text_color(),
    );
    p.text(
        r.min + vec2(16., 88.),
        Align2::LEFT_TOP,
        body,
        FontId::proportional(11.),
        MUTED,
    );
    p.text(
        r.right_top() + vec2(-17., 20.),
        Align2::RIGHT_TOP,
        ">",
        FontId::proportional(15.),
        MUTED,
    );
    if res.hovered() {
        p.rect_stroke(
            r,
            11.,
            Stroke::new(1.0_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    res.clicked()
}
fn color_control(ui: &mut egui::Ui, hex: &mut String) {
    let mut c = scenery::color(hex);
    if ui.color_edit_button_srgba(&mut c).changed() {
        *hex = scenery::hex(c);
    }
    ui.label(RichText::new(hex.as_str()).size(11.).color(MUTED));
}
fn nav_icon(p: &egui::Painter, c: egui::Pos2, page: Page, s: f32, color: Color32) {
    let stroke = Stroke::new(1.3_f32, color);
    match page {
        Page::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-s * 0.6, -s),
                    c + vec2(s, 0.),
                    c + vec2(-s * 0.6, s),
                ],
                Color32::TRANSPARENT,
                stroke,
            ));
        }
        Page::Profiles => {
            for x in [-1., 1.] {
                for y in [-1., 1.] {
                    p.rect_stroke(
                        Rect::from_center_size(
                            c + vec2(x * s * 0.55, y * s * 0.55),
                            Vec2::splat(s * 0.8),
                        ),
                        1.,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        Page::Mods => {
            p.add(egui::Shape::closed_line(
                vec![
                    c + vec2(0., -s),
                    c + vec2(s, 0.),
                    c + vec2(0., s),
                    c + vec2(-s, 0.),
                ],
                stroke,
            ));
            p.line_segment([c + vec2(-s, 0.), c + vec2(0., s * 0.48)], stroke);
            p.line_segment([c + vec2(s, 0.), c + vec2(0., s * 0.48)], stroke);
        }
        Page::Hud => {
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 2.)),
                2.,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.line_segment([c + vec2(-s, 0.), c + vec2(s, 0.)], stroke);
            p.line_segment([c + vec2(0., -s), c + vec2(0., s)], stroke);
        }
        Page::Sky => scenery::star(p, c, s + 1., color),
        Page::Settings => {}
    }
}
fn drag_widget(
    ui: &mut egui::Ui,
    preview: Rect,
    id: &str,
    widget: &mut HudWidget,
    text: &str,
    size: Vec2,
    accent: Color32,
    opacity: f32,
) {
    if !widget.enabled {
        return;
    }
    let avail = (preview.size() - size).max(Vec2::splat(1.));
    let rect = Rect::from_min_size(
        preview.min + vec2(widget.x * avail.x, widget.y * avail.y),
        size,
    );
    let response = ui.interact(rect, ui.id().with(("hud-widget", id)), Sense::drag());
    if response.dragged() {
        let delta = ui.input(|i| i.pointer.delta());
        widget.x = (widget.x + delta.x / avail.x).clamp(0., 1.);
        widget.y = (widget.y + delta.y / avail.y).clamp(0., 1.);
    }
    let r = Rect::from_min_size(
        preview.min + vec2(widget.x * avail.x, widget.y * avail.y),
        size,
    );
    let p = ui.painter().with_clip_rect(preview);
    p.rect_filled(
        r,
        6.,
        Color32::from_rgba_unmultiplied(18, 28, 39, (opacity * 255.) as u8),
    );
    p.rect_stroke(
        r,
        6.,
        Stroke::new(
            1.0_f32,
            if response.hovered() || response.dragged() {
                accent
            } else {
                Color32::from_white_alpha(22)
            },
        ),
        egui::StrokeKind::Inside,
    );
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(12.),
        accent,
    );
    response.on_hover_cursor(egui::CursorIcon::Grab);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_drag_tracks_pointer_without_snapping_and_stays_in_bounds() {
        use egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
        let ctx = egui::Context::default();
        let mut widget = HudWidget {
            enabled: true,
            x: 0.1,
            y: 0.1,
        };
        let preview = Rect::from_min_size(Pos2::new(20., 20.), vec2(600., 300.));
        let start = Pos2::new(20. + 522. * 0.1 + 15., 20. + 276. * 0.1 + 12.);
        let mut frame = |events: Vec<Event>, time: f64| {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000., 600.))),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        drag_widget(
                            ui,
                            preview,
                            "test-widget",
                            &mut widget,
                            "144 FPS",
                            vec2(78., 24.),
                            ACCENT,
                            0.72,
                        );
                    });
                },
            );
        };
        frame(vec![], 0.);
        frame(
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            0.1,
        );
        frame(vec![Event::PointerMoved(start + vec2(100., 50.))], 0.2);
        frame(
            vec![Event::PointerButton {
                pos: start + vec2(100., 50.),
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            0.3,
        );
        assert!(
            (widget.x - (0.1 + 100. / 522.)).abs() < 0.001,
            "x was {}",
            widget.x
        );
        assert!(
            (widget.y - (0.1 + 50. / 276.)).abs() < 0.001,
            "y was {}",
            widget.y
        );
    }
}
