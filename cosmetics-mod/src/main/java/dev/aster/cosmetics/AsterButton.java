package dev.aster.cosmetics;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

/** Minecraft input/narration semantics with Aster's restrained material treatment. */
public final class AsterButton extends ButtonWidget {
    private AsterButton(int x,int y,int w,int h,Text label,PressAction action) { super(x,y,w,h,label,action,DEFAULT_NARRATION_SUPPLIER); }
    public static Factory make(Text label,PressAction action) { return new Factory(label,action); }
    public static final class Factory {
        private final Text label;private final PressAction action;
        private int x,y,w=140,h=20;
        private Factory(Text label,PressAction action) {this.label=label;this.action=action;}
        public Factory dimensions(int x,int y,int w,int h) {this.x=x;this.y=y;this.w=w;this.h=h;return this;}
        public AsterButton build() {return new AsterButton(x,y,w,h,label,action);}
    }
    @Override protected void renderWidget(DrawContext context,int mx,int my,float delta) {
        MinecraftClient client=MinecraftClient.getInstance();
        boolean pressed=isHovered() && GLFW.glfwGetMouseButton(client.getWindow().getHandle(),GLFW.GLFW_MOUSE_BUTTON_LEFT)==GLFW.GLFW_PRESS;
        int fill=pressed ? (0xF0000000 | AsterClient.CONFIG.accent() & 0xFFFFFF) : isHovered() || isFocused() ? (0xE4000000 | AsterClient.CONFIG.accent() & 0xFFFFFF) : (0xD5000000 | CosmeticConfig.rgb(AsterClient.CONFIG.ui.button)&0xFFFFFF);
        AsterHud.panel(context,getX(),getY(),getWidth(),getHeight(),fill);
        if(isFocused()) context.drawBorder(getX(),getY(),getWidth(),getHeight(),0xA6B8C9FF);
        context.drawCenteredTextWithShadow(client.textRenderer,getMessage(),getX()+getWidth()/2,getY()+(getHeight()-8)/2,active ? CosmeticConfig.rgb(AsterClient.CONFIG.ui.text) : 0xFF7E8BA4);
    }
}
