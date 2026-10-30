package dev.aster.cosmetics.mixin;

import dev.aster.cosmetics.AsterSky;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.WorldRenderer;
import org.joml.Matrix4f;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(WorldRenderer.class)
public abstract class WorldRendererMixin {
    @Inject(method="renderSky",at=@At("HEAD"),cancellable=true)
    private void aster$sky(Matrix4f view,Matrix4f projection,float delta,Camera camera,boolean thickFog,Runnable fogCallback,CallbackInfo ci) {
        if(!"vanilla".equals(dev.aster.cosmetics.AsterClient.CONFIG.sky.preset) && net.minecraft.client.MinecraftClient.getInstance().world != null && net.minecraft.client.MinecraftClient.getInstance().world.getRegistryKey().equals(net.minecraft.world.World.OVERWORLD)) { fogCallback.run(); AsterSky.render(view,delta,camera,thickFog); ci.cancel(); }
    }
}
