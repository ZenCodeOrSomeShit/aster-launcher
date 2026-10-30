package dev.aster.cosmetics;

import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.multiplayer.MultiplayerScreen;
import net.minecraft.client.gui.screen.option.OptionsScreen;
import net.minecraft.client.gui.screen.world.SelectWorldScreen;
import net.minecraft.client.gui.widget.ButtonWidget;
import net.minecraft.text.Text;
import java.util.Random;

public final class AsterMenu extends Screen {
    public AsterMenu() { super(Text.literal("Aster")); }
    @Override protected void init() {
        int w=200, x=32, y=height/2-18;
        addDrawableChild(AsterButton.make(Text.literal("Singleplayer"), b -> client.setScreen(new SelectWorldScreen(this))).dimensions(x,y,w,24).build());
        addDrawableChild(AsterButton.make(Text.literal("Multiplayer"), b -> client.setScreen(new MultiplayerScreen(this))).dimensions(x,y+30,w,24).build());
        addDrawableChild(AsterButton.make(Text.literal("Personalize"), b -> client.setScreen(new AsterSettingsScreen(this))).dimensions(x,y+60,96,24).build());
        addDrawableChild(AsterButton.make(Text.literal("Options"), b -> client.setScreen(new OptionsScreen(this,client.options))).dimensions(x+104,y+60,96,24).build());
        addDrawableChild(AsterButton.make(Text.literal("Quit game"), b -> client.scheduleStop()).dimensions(x,y+90,w,24).build());
    }
    @Override public void render(DrawContext c,int mx,int my,float delta) {
        c.fillGradient(0,0,width,height,0xFF000000 | AsterClient.CONFIG.skyColor(),CosmeticConfig.rgb(AsterClient.CONFIG.ui.background));
        Random random=new Random(86197);
        for(int i=0;i<120;i++) { int sx=random.nextInt(Math.max(1,width)),sy=random.nextInt(Math.max(1,height)); c.fill(sx,sy,sx+1,sy+1,0x55D1DFFF); }
        var cfg=AsterClient.CONFIG;
        if(!cfg.ui.reducedMotion && cfg.sky.shootingStars && cfg.sky.meteorFrequency>0) {
            double time=System.nanoTime()/1e9;
            for(int j=0;j<4;j++){double period=3+(1-cfg.sky.meteorFrequency)*15;double age=(time+j*period/4)%period;if(age<1.6){int sx=(int)(width*(.2+j*.2)-age*width*.28),sy=(int)(height*(.08+j*.10)+age*height*.2);int size=Math.max(1,(int)cfg.sky.meteorSize);for(int k=0;k<48;k++)c.fill(sx+k*2,sy-k,sx+k*2+size+1,sy-k+size,((int)((1-k/48.0)*220)<<24)|(CosmeticConfig.rgb(cfg.sky.meteorColor)&0xFFFFFF));}}
        }
        for(int x=0;x<width;x+=8) { int peak=(int)(height*.76+Math.sin(x*.013)*18+Math.sin(x*.031)*10); c.fill(x,peak,x+8,height,0xFF141D31); }
        c.getMatrices().push(); c.getMatrices().translate(32,height/2f-76,0); c.getMatrices().scale(2.6f,2.6f,1);
        c.drawText(textRenderer,"ASTER",0,0,CosmeticConfig.rgb(cfg.ui.text),false); c.getMatrices().pop();
        c.drawText(textRenderer,"MINECRAFT JAVA / COSMETICS",32,height/2-36,cfg.accent(),false);
        
        super.render(c,mx,my,delta);
        c.drawText(textRenderer,"Aster Cosmetics 0.1.0 · Minecraft 1.21.1",8,height-13,0xFF8D9BB8,false);
    }
    @Override public boolean shouldCloseOnEsc() { return false; }
    @Override public void renderBackground(DrawContext context,int mx,int my,float delta) { }
}

