package dev.aster.cosmetics;

import org.junit.Test;
import static org.junit.Assert.*;

public class CosmeticConfigTest {
    @Test public void preventsOutOfBoundsAndNonFiniteConfigValues() {
        CosmeticConfig config=new CosmeticConfig();
        config.hud.scale=100; config.hud.opacity=Double.NaN;
        config.hud.fps.x=-8; config.hud.fps.y=Double.POSITIVE_INFINITY;
        config.hud.crosshair.size=-2; config.sky.starDensity=2;
        config.sanitize();
        assertEquals(2,config.hud.scale,0); assertEquals(.1,config.hud.opacity,0);
        assertEquals(0,config.hud.fps.x,0); assertEquals(0,config.hud.fps.y,0);
        assertEquals(1,config.hud.crosshair.size); assertEquals(1,config.sky.starDensity,0);
    }
    @Test public void repairsPartialConfigsAndInvalidColors() {
        CosmeticConfig config=new CosmeticConfig();
        config.hud.coordinates=null; config.ui=null; config.hud.crosshair=null;
        config.hud.accent="oops"; config.sky.color=null; config.sky.preset="unknown";
        config.sanitize();
        assertNotNull(config.hud.coordinates); assertNotNull(config.hud.crosshair); assertNotNull(config.ui);
        assertEquals("#B8C9FF",config.hud.accent); assertEquals("#192545",config.sky.color); assertEquals("midnight",config.sky.preset);
        assertEquals(0xFFB8C9FF,config.accent()); assertEquals(0x192545,config.skyColor());
    }
    @Test public void preservesSupportedCustomization() {
        CosmeticConfig config=new CosmeticConfig();
        config.sky.preset="vanilla"; config.hud.accent="#12ab34"; config.hud.keystrokes.x=.81;
        config.sanitize();
        assertEquals("vanilla",config.sky.preset); assertEquals(0xFF12AB34,config.accent()); assertEquals(.81,config.hud.keystrokes.x,0);
    }
}
