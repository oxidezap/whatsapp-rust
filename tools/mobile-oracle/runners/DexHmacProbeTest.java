import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.file.*;

/** Run against D8 output of DexHmacFixture, never against proprietary bytes. */
public final class DexHmacProbeTest {
    static void reject(Path dex, String hash, String method, String reason) throws Exception {
        try {
            DexHmacProbe.main(new String[]{dex.toString(), hash, "Lfixtures/DexHmacFixture;", method});
        } catch (Exception e) {
            for (Throwable cause = e; cause != null; cause = cause.getCause()) {
                if (cause.getMessage() != null && cause.getMessage().contains(reason)) return;
            }
            throw new AssertionError("unexpected rejection for " + method, e);
        }
        throw new AssertionError("accepted " + method);
    }
    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("synthetic fixture DEX path required");
        Path dex = Path.of(args[0]);
        byte[] bytes = Files.readAllBytes(dex);
        String hash = DexHmacProbe.hash(bytes);
        PrintStream original = System.out;
        var capture = new ByteArrayOutputStream();
        try {
            System.setOut(new PrintStream(capture));
            DexHmacProbe.main(new String[]{dex.toString(), hash, "Lfixtures/DexHmacFixture;", "hmac"});
        } finally { System.setOut(original); }
        String report = capture.toString(java.nio.charset.StandardCharsets.UTF_8);
        DexHmacProbe.require(report.contains("\"qualified_android_execution\":false")
            && report.contains("\"id\":\"synthetic-3\"")
            && report.contains("c617773e23b380eb66ce5a9fe4c9ce8c6e6d6c9d"), "synthetic output mismatch");
        reject(dex, "0".repeat(64), "hmac", "hash mismatch");
        reject(dex, hash, "missing", "selector absent");
        reject(dex, hash, "forbidden", "unsupported call");
        reject(dex, hash, "field", "unsupported field access");
        reject(dex, hash, "jni", "unsupported method code");
        Path truncated = Files.createTempFile("synthetic-dex-truncated-", ".dex");
        try {
            byte[] shortBytes = java.util.Arrays.copyOf(bytes, 50);
            Files.write(truncated, shortBytes);
            reject(truncated, DexHmacProbe.hash(shortBytes), "hmac", "not standalone DEX");
        } finally { Files.deleteIfExists(truncated); }
        System.out.println("7 synthetic DEX probe checks passed; APK/ART/JNI execution not qualified");
    }
}
