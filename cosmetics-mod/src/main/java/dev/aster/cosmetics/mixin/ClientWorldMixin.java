package dev.aster.cosmetics.mixin;

import dev.aster.cosmetics.AsterClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.math.Vec3d;
import net.minecraft.world.World;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

@Mixin(ClientWorld.class)
public abstract class ClientWorldMixin {
    @Inject(method="getSkyColor",at=@At("RETURN"),cancellable=true)
    private void aster$skyColor(Vec3d cameraPos,float tickDelta,CallbackInfoReturnable<Vec3d> cir) {
        ClientWorld world=(ClientWorld)(Object)this;
        if(!world.getRegistryKey().equals(World.OVERWORLD) || "vanilla".equals(AsterClient.CONFIG.sky.preset)) return;
        int color=AsterClient.CONFIG.skyColor();
        Vec3d custom=new Vec3d(((color>>16)&255)/255.0,((color>>8)&255)/255.0,(color&255)/255.0);
        // Respect the day/night cycle and weather. This changes appearance only, never world time or lighting.
        double night=Math.min(1,world.getStarBrightness(tickDelta)*2.5);
        double amount=.18+.70*night;
        cir.setReturnValue(custom);
    }
}
