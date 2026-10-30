# Aster Launcher

## Cosmetics update

Right Shift opens a movable studio panel with HUD, Sky, Colors, and Objects tabs. Drag its header to move it, drag HUD widgets directly, scroll the panel on smaller screens, and press Escape to save. Sliders control density, meteor amount, meteor size, HUD scale, and opacity. Hex fields edit panel, text, button, accent, sky, star, and meteor colors.

Custom sky presets now replace the Overworld sky renderer instead of blending with it. Vanilla restores the normal renderer. Add moons or suns under Objects, cycle the selected object by clicking its name, adjust its azimuth, elevation, size, and color, or remove it. Objects are procedural colored square celestial bodies, not imported models or textures. Custom skies intentionally stay visible during daytime; they do not change world time or lighting.

The main menu has an animated meteor background and custom drawn controls. In launcher Settings, Cosmetic palette uses the current profile's colors. Expanded colors and objects are also editable in the launcher's HUD/Sky studios. Existing profiles migrate automatically. Restart Minecraft and reinstall Aster Cosmetics from the updated launcher to update an existing instance. Current target remains Fabric 1.21.1 / Windows x64.

A native Rust Windows launcher for Minecraft Java, with isolated mod profiles and an original Aster cosmetic mod. The interface follows the provided apple-design skill: considered typography, layered surfaces, immediate press feedback, direct HUD manipulation, and reduced-motion support. It uses eframe/egui; it does not use Electron, a webview, or a JavaScript frontend.

## Portable build

Open `Aster-Launcher.exe` from the supplied Windows x64 bundle (or `release/Aster-Launcher.exe` in this workspace). Rust and build tools are only needed to build from source. The complete cosmetic mod is embedded in the executable; the separate JAR is supplied for manual installation too.

## Build from source

Install the stable Rust toolchain and the Microsoft C++ build tools required by the Windows MSVC target, then run from this directory:

```powershell
cargo run
```

For a production build:

```powershell
cargo build --release
.\target\release\Aster-Launcher.exe
```

When building from source, build the cosmetic JAR first; see [the mod project](cosmetics-mod/README.md). The complete cosmetic JAR, including its Rust native library, is embedded in the launcher executable. The launcher does not bundle Minecraft game files or a Java runtime.

The launcher itself is Rust. The cosmetic mod's star sphere, aurora ribbons, meteor trajectories, tapered geometry, and normalized HUD positioning also run in a Rust native library. Fabric loads JVM bytecode, so a Java bridge connects Minecraft events, configuration, menus, and rendering submissions to that library through JNI. The mod is therefore a Rust core with required JVM integration, rather than a pure Rust Fabric JAR. Its native library currently targets Windows x64.

## First launch

1. Leave Java selection automatic, or choose an installed Java executable in Settings. On Windows x64, Aster discovers matching runtimes and can provision the required Eclipse Temurin runtime into its own data directory. Downloads are checked against the official SHA-256 checksum. Different profiles can use different Java paths.
2. The publisher must configure Aster's Microsoft application registration (see OAUTH-SETUP.md). Once configured, click **Sign in with Microsoft**. This build does not include an approved application ID.
3. Choose or create a profile, select a Minecraft release and loader, then launch. Aster downloads game files on demand into its own data directory.

The Microsoft app must support personal Microsoft accounts and browser authorization code flow with PKCE, using a desktop `http://localhost` redirect. No client secret belongs in this desktop application. The login requests `XboxLive.signin` and `offline_access`; Minecraft Services may also require approval. App registration alone does not guarantee that access. See [Microsoft's OAuth documentation](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow). Aster never fabricates account credentials or falls back to an offline account.

Account credentials are encrypted with Windows DPAPI and kept out of profile configuration and launch logs. Signing out removes the saved account. Never paste account tokens or a Microsoft client secret into this launcher.

## Versions and mods

The version list comes from Mojang's official manifest and includes published release versions in the requested 1.7–26.3 range. This is a manifest-driven launcher, not a claim that every version/loader/mod combination has been tested. This build installs vanilla Minecraft and Fabric; Forge installation is not implemented. Fabric installation depends on loader support for the chosen game version. Imported mod JARs must match both the Minecraft version and loader; Aster does not resolve arbitrary third-party mod dependencies.

The required Java major version is read from each release's metadata. Legacy metadata falls back to Java 8 for 1.7–1.16, Java 16 for 1.17, Java 17 for 1.18–1.20.4, and Java 21 starting at 1.20.5. Newer numbering uses published metadata rather than assuming a runtime version. Aster reports a missing or incompatible runtime before attempting to launch.

The original **Aster Cosmetics 0.1.0** mod currently targets **Fabric on Minecraft 1.21.1 only**. Launcher support for another Minecraft release does not make this JAR compatible with that release. Its HUD, menu, and sky settings are saved per profile to `config/aster-cosmetics.json`. The custom sky and shooting stars use original geometry rendered through Minecraft's rendering API, without a shader pack. The normal Minecraft rendering pipeline still runs, as it does for every game scene.

## Data and verification

The launcher shows its data directory in Settings. Profiles have separate instance directories for saves, mods, configuration, and logs. Disabling a mod changes its filename to `.jar.disabled`. Removing a profile from the launcher preserves its files.

```powershell
cargo test
cargo build --release
```

The Fabric bridge and Rust cosmetic core have their own builds and tests; see the mod README. Launcher integration tests cover instance path isolation, metadata rules, Java selection, argument handling, and persisted cosmetic settings. Temporary verification output belongs in `test-results/`. The opt-in provisioning check downloads a real Java 25 runtime into this directory:

```powershell
cargo test --test backend provision_managed_java_25 -- --ignored --nocapture
cargo test --test backend install_embedded_cosmetics -- --ignored --nocapture
```

Verified in this workspace: native profile creation and persistence; isolated profile/config round trips; path and archive containment; DPAPI credential encryption round trips; native HUD pointer tracking with preserved grab offset; a real Java 25.0.5 download, checksum verification, extraction, and execution; and installation of the embedded cosmetic JAR plus a matching Fabric API release. The installed JAR was checked for its embedded Rust DLL and byte-for-byte agreement with the built mod.

Minecraft's Fabric startup, mixins, Rust DLL loading, and custom title screen were also reached in a development launch. In-world sky rendering and the in-game HUD editor have not completed visual verification. Actual Microsoft login and gameplay still require the user's account and an approved client ID. Automated UI and backend checks do not replace an authenticated gameplay test.

Minecraft is a trademark of Mojang/Microsoft. Aster is an independent project.
