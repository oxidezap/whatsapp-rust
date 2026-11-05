package fixtures;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;

/** Entirely synthetic input for D8 and the translated-method probe. */
public final class DexHmacFixture {
    static {
        if (System.nanoTime() != 0) throw new AssertionError("initializer must be excluded");
    }
    public static byte[] hmac(byte[] key, byte[][] parts) {
        try {
            Mac mac = Mac.getInstance("HmacSHA1");
            mac.init(new SecretKeySpec(key, "HmacSHA1"));
            mac.update(parts[0]);
            return mac.doFinal();
        } catch (java.security.GeneralSecurityException e) {
            throw new RuntimeException(e);
        }
    }
    public static byte[] forbidden(byte[] key, byte[][] parts) {
        System.getProperty("java.home");
        return key;
    }
    public static byte[] field(byte[] key, byte[][] parts) {
        System.out.println("must never execute");
        return key;
    }
    public static native byte[] jni(byte[] key, byte[][] parts);
}
