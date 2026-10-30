package dev.aster.cosmetics;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.gui.screen.TitleScreen;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;
import org.lwjgl.glfw.GLFW;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public final class AsterClient implements ClientModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("aster-cosmetics");
    public static CosmeticConfig CONFIG = new CosmeticConfig();
    @Override public void onInitializeClient() {
        NativeCosmetics.load();
        CONFIG = CosmeticConfig.load();
        KeyBinding edit = KeyBindingHelper.registerKeyBinding(new KeyBinding("Aster: customize HUD", InputUtil.Type.KEYSYM, GLFW.GLFW_KEY_RIGHT_SHIFT, "Aster Cosmetics"));
        HudRenderCallback.EVENT.register((context, counter) -> AsterHud.render(context, false));
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            while (edit.wasPressed()) {
                if (!(client.currentScreen instanceof AsterSettingsScreen)) client.setScreen(new AsterSettingsScreen(client.currentScreen));
            }
            if (CONFIG.ui.customMenu && client.currentScreen instanceof TitleScreen) client.setScreen(new AsterMenu());
        });
        LOGGER.info("Aster Cosmetics ready: Right Shift opens the HUD editor. Procedural geometry; no shader packs.");
    }
}
