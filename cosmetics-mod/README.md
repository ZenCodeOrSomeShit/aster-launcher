# Aster Cosmetics

Original **Rust cosmetics engine with a required Fabric JVM bridge**, for **Minecraft Java 1.21.1 on Windows x64**, Java 21, Fabric Loader 0.16+, and Fabric API. The launcher installs the matching dependencies. This jar deliberately declares the exact supported Minecraft version; it must not be copied into 1.7, newer Minecraft releases, or Forge instances.

All procedural sky geometry (star sphere, aurora ribbon, shooting-star trajectories and trails) and normalized HUD placement run in `native/src/lib.rs`. Its compiled Rust DLL is embedded in the mod jar and loaded through JNI. Java supplies the Fabric event hooks, Minecraft UI wrappers, config decoding, and final rendering calls required by the JVM game. There is no Java geometry fallback. The bridge is not an Electron app or webview.

Features:

- FPS, coordinates, live W/A/S/D, space, mouse buttons, and custom crosshair.
- **Right Shift** opens the HUD editor. Drag widgets with the mouse; the grab offset is preserved. Escape or Done saves changes.
- A custom title screen connects to Minecraft's ordinary singleplayer, multiplayer, and options screens.
- Configurable sky tint, a procedural star sphere, aurora ribbons, and tapered shooting-star trails. Effects use world-space geometry in the sky pass, not a full-screen HUD overlay.
- Weather, dimension, fluid submersion, blindness/darkness, and the day/night cycle gate the sky effects. No game time, world lighting, or server state is changed.
- Reduced motion disables moving meteors and aurora drift. The night sky remains static.

No shader pack, OptiFine, Iris, texture pack, downloaded sky artwork, custom GLSL, or post-processing is used. The renderer generates ordinary colored vertices on the CPU and submits them through Minecraft's built-in unlit position/color graphics program, as required by Minecraft's rendering API.

## Configuration

The launcher's controls and the mod share `config/aster-cosmetics.json` inside the selected instance. Launcher changes apply when the game next starts. The in-game editor saves on close. Widget positions are normalized from `0` to `1` over the available space **after subtracting the widget size**, keeping the complete widget on screen. Scale is 0.5–2, opacity 0.1–1; density/frequency are 0–1. A meteor frequency of zero disables meteors.

```json
{
  "schemaVersion": 1,
  "hud": {
    "enabled": true,
    "scale": 1,
    "opacity": 0.72,
    "accent": "#B8C9FF",
    "fps": {"enabled": true, "x": 0.025, "y": 0.04},
    "coordinates": {"enabled": true, "x": 0.025, "y": 0.16},
    "keystrokes": {"enabled": true, "x": 0.025, "y": 0.72},
    "crosshair": {"enabled": true, "size": 5, "gap": 3, "thickness": 1}
  },
  "sky": {
    "preset": "midnight",
    "color": "#192545",
    "shootingStars": true,
    "starDensity": 0.7,
    "meteorFrequency": 0.6
  },
  "ui": {"customMenu": true, "reducedMotion": false}
}
```

Sky presets: `vanilla`, `midnight`, `aurora`. The vanilla preset keeps the original sky and can optionally retain meteors. Aurora adds a low-contrast ribbon above the horizon. Hex colors use `#RRGGBB`.

## Build

Run `build.ps1` from PowerShell with Rust/MSVC and JDK 21 installed, or use the project-local Java toolchain if present. The script tests and builds the Rust engine first, then builds the Fabric bridge. The generated Gradle wrapper uses Gradle 8.10.2. The production mod is `build/libs/aster-cosmetics-0.1.0.jar`; the `-sources.jar` is for development only. The DLL is embedded automatically by `processResources`.

```powershell
.\build.ps1
# Or manually with a configured JDK 21 and Rust:
cargo test --manifest-path native/Cargo.toml
cargo build --release --manifest-path native/Cargo.toml
.\gradlew.bat build
```

Fabric Loom remaps the production jar to intermediary names. Four Rust tests check geometry determinism, bounded inputs, reduced motion, and malformed input. JVM tests check safe config recovery and load the actual packaged Rust DLL through JNI to verify its signatures, returned geometry, and HUD bounds. Rendering compatibility with replacement renderers is not claimed without in-game testing.

References: [Fabric HUD events](https://maven.fabricmc.net/docs/fabric-api-0.100.2+1.21/net/fabricmc/fabric/api/client/rendering/v1/HudRenderCallback.html), [Yarn 1.21.1 rendering API](https://maven.fabricmc.net/docs/yarn-1.21.1+build.3/net/minecraft/client/render/WorldRenderer.html), [Fabric key bindings](https://wiki.fabricmc.net/tutorial:keybinds).
