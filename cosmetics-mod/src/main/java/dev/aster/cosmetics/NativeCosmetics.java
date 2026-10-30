package dev.aster.cosmetics;

import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;

/** Minimal JVM to Rust JNI boundary. There is deliberately no Java geometry fallback. */
public final class NativeCosmetics {
    private static boolean loaded;
    public static synchronized void load() {
        if(loaded) return;
        String os=System.getProperty("os.name").toLowerCase();
        String arch=System.getProperty("os.arch").toLowerCase();
        if(!os.contains("win") || !(arch.equals("amd64") || arch.equals("x86_64"))) throw new UnsupportedOperationException("Aster Cosmetics 0.1.0 native engine requires Windows x64.");
        try(InputStream source=NativeCosmetics.class.getResourceAsStream("/native/windows-x86_64/aster_cosmetics_native.dll")) {
            if(source==null) throw new IllegalStateException("Aster native engine is missing from this jar. Build native/Cargo.toml before assembling the Fabric bridge.");
            byte[] bytes=source.readAllBytes();
            String digest=HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
            Path directory=Path.of(System.getProperty("java.io.tmpdir"),"aster-cosmetics-native",digest);
            Files.createDirectories(directory);
            Path library=directory.resolve("aster_cosmetics_native.dll");
            if(!Files.exists(library)) Files.write(library,bytes);
            System.load(library.toAbsolutePath().toString());
            loaded=true;
        } catch(Exception e) { throw new IllegalStateException("Aster could not load its Rust cosmetics engine",e); }
    }
    public static native float[] advancedSky(double time,double density,double frequency,double size,int preset,boolean meteors,boolean reduced,int skyColor,int starColor,int meteorColor,float[] bodies);
    public static native float[] skyVertices(double time,float night,double density,double frequency,int preset,boolean meteors,boolean reducedMotion);
    public static native int[] widgetBounds(int width,int height,int baseWidth,int baseHeight,double x,double y,double scale);
}
