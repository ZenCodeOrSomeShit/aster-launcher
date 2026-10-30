package dev.aster.cosmetics.mixin;

import dev.aster.cosmetics.AsterClient;
import net.minecraft.client.gui.hud.InGameHud;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.render.RenderTickCounter;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(InGameHud.class)
public abstract class InGameHudMixin {
    @Inject(method="renderCrosshair",at=@At("HEAD"),cancellable=true)
    private void aster$crosshair(DrawContext context, RenderTickCounter counter, CallbackInfo ci) {
        if(AsterClient.CONFIG.hud.enabled && AsterClient.CONFIG.hud.crosshair.enabled) ci.cancel();
    }
}
