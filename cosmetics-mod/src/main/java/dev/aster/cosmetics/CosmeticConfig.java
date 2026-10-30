package dev.aster.cosmetics;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import net.fabricmc.loader.api.FabricLoader;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/** Shared with the launcher's config writer. Unknown keys are preserved by the launcher. */
public final class CosmeticConfig {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    public int schemaVersion = 1;
    public Hud hud = new Hud();
    public Sky sky = new Sky();
    public Ui ui = new Ui();
    public static final class Widget {
        public boolean enabled = true;
        public double x, y;
        public Widget(double x, double y) { this.x = x; this.y = y; }
        public void sanitize() { x = clamp(x, 0, 1); y = clamp(y, 0, 1); }
    }
    public static final class Crosshair {
        public boolean enabled = true;
        public int size = 5, gap = 3, thickness = 1;
    }
    public static final class Hud {
        public boolean enabled = true;
        public double scale = 1, opacity = 0.72;
        public String accent = "#B8C9FF";
        public Widget fps = new Widget(.025, .04);
        public Widget coordinates = new Widget(.025, .16);
        public Widget keystrokes = new Widget(.025, .72);
        public Crosshair crosshair = new Crosshair();
    }
    public static final class Sky {
        public String preset = "midnight", color = "#192545";
        public boolean shootingStars = true;
        public double meteorSize = 1;
        public String starColor = "#C1D3FF", meteorColor = "#D7E6FF";
        public java.util.List<Body> bodies = new java.util.ArrayList<>(java.util.List.of(new Body()));
        public double starDensity = .7, meteorFrequency = .6;
    }
    public static final class Body { public String name="Moon", color="#FFF1CF"; public double azimuth=30,elevation=45,size=8; public boolean enabled=true; }
    public static final class Ui { public String background="#111521", text="#E4E8F2", button="#2B374E"; public boolean customMenu = true, reducedMotion = false; }

    private static Path path() { return FabricLoader.getInstance().getConfigDir().resolve("aster-cosmetics.json"); }
    public static CosmeticConfig load() {
        try {
            if (Files.exists(path())) {
                CosmeticConfig c = GSON.fromJson(Files.readString(path()), CosmeticConfig.class);
                if (c != null) { c.sanitize(); return c; }
            }
        } catch (Exception e) { AsterClient.LOGGER.warn("Could not load Aster config; keeping a safe default", e); }
        return new CosmeticConfig();
    }
    public void save() {
        sanitize();
        try {
            Files.createDirectories(path().getParent());
            Path tmp = path().resolveSibling("aster-cosmetics.json.tmp");
            Files.writeString(tmp, GSON.toJson(this));
            Files.move(tmp, path(), StandardCopyOption.REPLACE_EXISTING);
        } catch (IOException e) { AsterClient.LOGGER.error("Could not save Aster config", e); }
    }
    public void sanitize() {
        if (hud == null) hud = new Hud(); if (sky == null) sky = new Sky(); if (ui == null) ui = new Ui();
        if (hud.fps == null) hud.fps = new Widget(.025, .04);
        if (hud.coordinates == null) hud.coordinates = new Widget(.025, .16);
        if (hud.keystrokes == null) hud.keystrokes = new Widget(.025, .72);
        if (hud.crosshair == null) hud.crosshair = new Crosshair();
        hud.fps.sanitize(); hud.coordinates.sanitize(); hud.keystrokes.sanitize();
        hud.scale = clamp(hud.scale, .5, 2); hud.opacity = clamp(hud.opacity, .1, 1);
        hud.crosshair.size = (int)clamp(hud.crosshair.size, 1, 20);
        hud.crosshair.gap = (int)clamp(hud.crosshair.gap, 0, 15);
        hud.crosshair.thickness = (int)clamp(hud.crosshair.thickness, 1, 4);
        hud.accent = validColor(hud.accent, "#B8C9FF"); sky.color = validColor(sky.color, "#192545");
        if (!"vanilla".equals(sky.preset) && !"aurora".equals(sky.preset)) sky.preset = "midnight";
        sky.bodies = sky.bodies == null ? new java.util.ArrayList<>() : sky.bodies;
        sky.bodies.removeIf(java.util.Objects::isNull);
        while(sky.bodies.size()>64)sky.bodies.remove(sky.bodies.size()-1);
        for(var b:sky.bodies){b.color=validColor(b.color,"#FFF1CF");b.size=clamp(b.size,.1,30);b.azimuth=clamp(b.azimuth,-180,180);b.elevation=clamp(b.elevation,0,90);}
        sky.meteorSize=clamp(sky.meteorSize,.2,5);
        if(sky.bodies==null) sky.bodies=new java.util.ArrayList<>();
        sky.starColor=validColor(sky.starColor,"#C1D3FF"); sky.meteorColor=validColor(sky.meteorColor,"#D7E6FF");
        ui.background=validColor(ui.background,"#111521"); ui.text=validColor(ui.text,"#E4E8F2"); ui.button=validColor(ui.button,"#2B374E");
        sky.starDensity = clamp(sky.starDensity, 0, 1); sky.meteorFrequency = clamp(sky.meteorFrequency, 0, 1);
    }
    public static double clamp(double n, double low, double high) { return Double.isFinite(n) ? Math.max(low, Math.min(high, n)) : low; }
    private static String validColor(String color, String fallback) { return color != null && color.matches("#[0-9a-fA-F]{6}") ? color : fallback; }
    public static int rgb(String s) { return 0xFF000000 | Integer.parseInt(s.substring(1),16); }
    public int accent() { return 0xFF000000 | Integer.parseInt(hud.accent.substring(1), 16); }
    public int skyColor() { return Integer.parseInt(sky.color.substring(1), 16); }
}
