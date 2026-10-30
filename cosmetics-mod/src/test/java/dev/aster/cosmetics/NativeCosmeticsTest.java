package dev.aster.cosmetics;

import org.junit.Test;
import static org.junit.Assert.*;

/** Exercises the packaged DLL and actual JNI signatures, not a mocked geometry function. */
public class NativeCosmeticsTest {
    @Test public void replacementSkyIncludesOpaqueDomeAndCustomObjects() {
        NativeCosmetics.load();
        float[] empty=NativeCosmetics.advancedSky(3,0,0,1,1,false,false,0x123456,0xffffff,0xffffff,new float[0]);
        assertEquals(6*4*7,empty.length);assertEquals(18,empty[3],0);assertEquals(255,empty[6],0);
        float[] object=NativeCosmetics.advancedSky(3,0,0,1,1,false,false,0x123456,0xffffff,0xffffff,new float[]{30,45,8,255,100,20});
        assertEquals(empty.length+28,object.length);
        float[] small=NativeCosmetics.advancedSky(.5,0,1,1,1,true,false,0,0,0xffffff,new float[0]);
        float[] big=NativeCosmetics.advancedSky(.5,0,1,3,1,true,false,0,0,0xffffff,new float[0]);
        assertTrue(small.length>empty.length);assertFalse(java.util.Arrays.equals(small,big));
    }
    @Test public void packagedRustEngineLoadsAndReturnsVertices() {
        NativeCosmetics.load();
        float[] vertices=NativeCosmetics.skyVertices(10,1,.5,0,1,false,false);
        assertEquals(360*4*7,vertices.length);
        for(float n:vertices) assertTrue(Float.isFinite(n));
        int[] bounds=NativeCosmetics.widgetBounds(1000,600,80,24,1,1,1);
        assertArrayEquals(new int[]{920,576,80,24},bounds);
    }
    @Test public void reducedMotionDisablesNativeMeteorGeometry() {
        NativeCosmetics.load();
        assertEquals(0,NativeCosmetics.skyVertices(.5,1,0,1,0,true,true).length);
        assertTrue(NativeCosmetics.skyVertices(.5,1,0,1,0,true,false).length>0);
    }
}
