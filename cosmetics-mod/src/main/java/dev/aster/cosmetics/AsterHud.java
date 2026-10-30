package dev.aster.cosmetics;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.math.MatrixStack;
import java.util.List;

public final class AsterHud {
    public record WidgetBox(CosmeticConfig.Widget widget, int x, int y, int width, int height) {}
    private static final MinecraftClient MC = MinecraftClient.getInstance();
    public static void panel(DrawContext context, int x, int y, int w, int h, int color) {
        context.fill(x + 2, y, x + w - 2, y + h, color);
        context.fill(x, y + 2, x + w, y + h - 2, color);
        context.fill(x + 3, y, x + w - 3, y + 1, 0x18FFFFFF);
    }
    public static List<WidgetBox> boxes(int width, int height) {
        CosmeticConfig.Hud h = AsterClient.CONFIG.hud;
        return List.of(box(h.fps, width, height, 78, 24), box(h.coordinates, width, height, 164, 24), box(h.keystrokes, width, height, 84, 86));
    }
    private static WidgetBox box(CosmeticConfig.Widget widget, int w, int h, int baseW, int baseH) {
        int[] bounds=NativeCosmetics.widgetBounds(w,h,baseW,baseH,widget.x,widget.y,AsterClient.CONFIG.hud.scale);
        return new WidgetBox(widget,bounds[0],bounds[1],bounds[2],bounds[3]);
    }
    public static void render(DrawContext context, boolean editor) {
        if (MC.player == null && !editor) return;
        if (MC.currentScreen instanceof AsterSettingsScreen && !editor) return;
        CosmeticConfig c = AsterClient.CONFIG;
        if ((!c.hud.enabled || MC.options.hudHidden) && !editor) return;
        int width = MC.getWindow().getScaledWidth(), height = MC.getWindow().getScaledHeight();
        List<WidgetBox> boxes = boxes(width, height);
        for (int i = 0; i < boxes.size(); i++) {
            WidgetBox b = boxes.get(i);
            if (!b.widget.enabled && !editor) continue;
            MatrixStack matrices = context.getMatrices();
            matrices.push(); matrices.translate(b.x, b.y, 0); matrices.scale((float)c.hud.scale, (float)c.hud.scale, 1);
            int bg = ((int)(c.hud.opacity * 255) << 24) | (CosmeticConfig.rgb(c.ui.background)&0xFFFFFF);
            if (i == 0) {
                panel(context, 0, 0, 78, 24, bg);
                context.drawText(MC.textRenderer, "FPS", 9, 8, CosmeticConfig.rgb(c.ui.text), false);
                context.drawText(MC.textRenderer, String.valueOf(editor ? 144 : MC.getCurrentFps()), 36, 8, c.accent(), false);
            } else if (i == 1) {
                panel(context, 0, 0, 164, 24, bg);
                String coordinates = MC.player == null ? "X 128   Y 72   Z -256" : String.format("X %d   Y %d   Z %d", MC.player.getBlockX(), MC.player.getBlockY(), MC.player.getBlockZ());
                context.drawText(MC.textRenderer, coordinates, 9, 8, c.accent(), false);
            } else {
                key(context, 28, 0, 26, 26, "W", MC.options.forwardKey, editor, bg);
                key(context, 0, 28, 26, 26, "A", MC.options.leftKey, editor, bg);
                key(context, 28, 28, 26, 26, "S", MC.options.backKey, editor, bg);
                key(context, 56, 28, 26, 26, "D", MC.options.rightKey, editor, bg);
                key(context, 0, 56, 82, 14, "SPACE", MC.options.jumpKey, editor, bg);
                key(context, 0, 72, 40, 14, "LMB", MC.options.attackKey, editor, bg);
                key(context, 42, 72, 40, 14, "RMB", MC.options.useKey, editor, bg);
            }
            matrices.pop();
            if (editor) context.drawBorder(b.x - 2, b.y - 2, b.width + 4, b.height + 4, b.widget.enabled ? 0x667C9EFF : 0x337C9EFF);
        }
        if (c.hud.crosshair.enabled && (editor || MC.options.getPerspective().isFirstPerson()) && (MC.player == null || !MC.player.isSpectator())) {
            int x = width / 2, y = height / 2;
            int size = c.hud.crosshair.size, gap = c.hud.crosshair.gap, t = c.hud.crosshair.thickness;
            int color = c.accent();
            context.fill(x-gap-size, y, x-gap, y+t, color); context.fill(x+gap+t, y, x+gap+size+t, y+t, color);
            context.fill(x, y-gap-size, x+t, y-gap, color); context.fill(x, y+gap+t, x+t, y+gap+size+t, color);
        }
    }
    private static void key(DrawContext context, int x, int y, int w, int h, String text, KeyBinding key, boolean editor, int bg) {
        boolean down = !editor && key.isPressed();
        panel(context, x, y, w, h, down ? (0x99000000 | AsterClient.CONFIG.accent() & 0xFFFFFF) : bg);
        context.drawCenteredTextWithShadow(MC.textRenderer, text, x+w/2, y+(h-8)/2, down ? 0xFF101624 : CosmeticConfig.rgb(AsterClient.CONFIG.ui.text));
    }
}
