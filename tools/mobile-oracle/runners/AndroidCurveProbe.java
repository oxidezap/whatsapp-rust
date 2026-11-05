/* Only for synthetic, offline execution in a disposable Android runtime. */
import java.io.File;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Calls original Curve25519 JNI from a separately pinned Superpack extraction.
 * Original APK must be on app_process's classpath. Never use this deterministic
 * test material for accounts, sessions, attestation or registration.
 */
public final class AndroidCurveProbe {
    static byte[] decode(String hex) {
        AndroidCryptoProbe.require(hex.length() % 2 == 0, "hex length");
        byte[] out = new byte[hex.length()/2];
        for (int i = 0; i < out.length; i++) out[i] = (byte)Integer.parseInt(hex.substring(i*2,i*2+2),16);
        return out;
    }
    static String output(String id, byte[] value) throws Exception {
        return "{\"id\":" + AndroidCryptoProbe.quote(id) + ",\"output_hex\":"
            + AndroidCryptoProbe.quote(AndroidCryptoProbe.hex(value)) + ",\"output_sha256\":"
            + AndroidCryptoProbe.quote(AndroidCryptoProbe.hash(value)) + "}";
    }
    public static void main(String[] args) throws Exception {
        AndroidCryptoProbe.require(args.length == 2, "AndroidCurveProbe <curve-library> <expected-sha256>");
        AndroidCryptoProbe.require(System.getProperty("java.vm.name", "").equals("Dalvik"), "requires Android");
        File library = new File(args[0]);
        AndroidCryptoProbe.require(library.length() <= 4 * 1024 * 1024, "library budget");
        String hash = AndroidCryptoProbe.fileHash(library);
        AndroidCryptoProbe.require(hash.equals(args[1]), "library pin mismatch");
        Class<?> type = Class.forName("org.whispersystems.curve25519.NativeCurve25519Provider", true,
            AndroidCurveProbe.class.getClassLoader());
        AndroidCryptoProbe.require(type.getClassLoader() == AndroidCurveProbe.class.getClassLoader(), "unexpected classloader");
        System.load(library.getAbsolutePath());
        Object provider = type.getConstructor().newInstance();
        Method privateKey = type.getDeclaredMethod("generatePrivateKey", byte[].class);
        Method publicKey = type.getDeclaredMethod("generatePublicKey", byte[].class);
        Method agreement = type.getDeclaredMethod("calculateAgreement", byte[].class, byte[].class);
        Method sign = type.getDeclaredMethod("calculateSignature", byte[].class, byte[].class, byte[].class);
        Method verify = type.getDeclaredMethod("verifySignature", byte[].class, byte[].class, byte[].class);
        Method torsion = type.getDeclaredMethod("isTorsionFree", byte[].class);
        for (Method m : new Method[]{privateKey, publicKey, agreement, sign, verify, torsion})
            AndroidCryptoProbe.require(Modifier.isNative(m.getModifiers()), "method not native");
        // Public RFC 7748 section 6.1 vectors, never real credentials.
        byte[] aliceSeed = decode("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        byte[] bobSeed = decode("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        byte[] alice = (byte[])privateKey.invoke(provider, (Object)aliceSeed.clone());
        byte[] bob = (byte[])privateKey.invoke(provider, (Object)bobSeed.clone());
        byte[] alicePublic = (byte[])publicKey.invoke(provider, (Object)alice);
        byte[] bobPublic = (byte[])publicKey.invoke(provider, (Object)bob);
        AndroidCryptoProbe.require(AndroidCryptoProbe.hex(alicePublic).equals("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"), "RFC Alice mismatch");
        AndroidCryptoProbe.require(AndroidCryptoProbe.hex(bobPublic).equals("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"), "RFC Bob mismatch");
        byte[] shared = (byte[])agreement.invoke(provider, alice, bobPublic);
        AndroidCryptoProbe.require(AndroidCryptoProbe.hex(shared).equals("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"), "RFC shared mismatch");
        AndroidCryptoProbe.require(Arrays.equals(shared, (byte[])agreement.invoke(provider, bob, alicePublic)), "DH symmetry mismatch");
        byte[] message = "mobile oracle synthetic".getBytes(StandardCharsets.US_ASCII);
        byte[] signature = (byte[])sign.invoke(provider, new byte[64], alice, message);
        AndroidCryptoProbe.require(signature.length == 64 && (Boolean)verify.invoke(provider, alicePublic, message, signature), "signature self verification failed");
        byte[] changed = message.clone(); changed[0] ^= 1;
        AndroidCryptoProbe.require(!(Boolean)verify.invoke(provider, alicePublic, changed, signature), "tampered signature accepted");
        boolean torsionFree = (Boolean)torsion.invoke(provider, (Object)alicePublic);
        String lowOrder;
        try {
            byte[] raw = (byte[])agreement.invoke(provider, alice, new byte[32]);
            lowOrder = "{\"status\":\"returned\",\"output_hex\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.hex(raw)) + "}";
        } catch (InvocationTargetException e) {
            lowOrder = "{\"status\":\"threw\",\"exception\":" + AndroidCryptoProbe.quote(e.getCause().getClass().getName()) + "}";
        }
        System.out.println("{\"schema_version\":1,\"execution\":\"android_original_curve_jni\","
            + "\"application_lifecycle\":\"not_run\",\"registration\":\"not_run\",\"stubs\":[],"
            + "\"randomness\":\"explicit_synthetic_zero_64_bytes\",\"dynamic_import_trace\":\"not_recorded\","
            + "\"library_sha256\":" + AndroidCryptoProbe.quote(hash)
            + ",\"cases\":[" + output("alice-public", alicePublic) + "," + output("bob-public", bobPublic)
            + "," + output("shared", shared) + "," + output("signature", signature) + "]"
            + ",\"message_sha256\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.hash(message))
            + ",\"signature_valid\":true,\"tampered_signature_valid\":false,\"alice_torsion_free\":" + torsionFree
            + ",\"low_order_agreement\":" + lowOrder + ",\"build_fingerprint\":"
            + AndroidCryptoProbe.quote(AndroidCryptoProbe.buildField("FINGERPRINT")) + "}");
    }
}
