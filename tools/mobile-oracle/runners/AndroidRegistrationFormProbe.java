/* Offline original request builder/field serializer, never its HTTP caller. */
import java.lang.reflect.*;
import java.io.InputStream;
import java.security.MessageDigest;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Original APK on app_process classpath must have separately verified pins.
 * Uses synthetic bytes/text only. No bridge/HTTP client is constructed or called.
 */
public final class AndroidRegistrationFormProbe {
    static String referenceDex(ClassLoader loader) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream in = loader.getResourceAsStream("classes10.dex")) {
            AndroidCryptoProbe.require(in != null, "reviewed DEX resource absent");
            byte[] buffer = new byte[16384]; int count; long total = 0;
            while ((count = in.read(buffer)) != -1) {
                total += count; AndroidCryptoProbe.require(total <= 64L * 1024 * 1024, "DEX resource budget");
                digest.update(buffer, 0, count);
            }
        }
        return AndroidCryptoProbe.hex(digest.digest());
    }
    static Object builder(Class<?> type) throws Exception { return type.getConstructor().newInstance(); }
    static String serialize(Object builder, Class<?> lambda) throws Exception {
        Object formatter = lambda.getConstructor(Object.class, int.class).newInstance(builder, 46);
        Method invoke = lambda.getDeclaredMethod("invoke", Object.class);
        Map<?,?> fields = (Map<?,?>)builder.getClass().getField("A00").get(builder);
        StringBuilder body = new StringBuilder();
        for (Map.Entry<?,?> entry : fields.entrySet()) {
            if (body.length() != 0) body.append('&');
            body.append((String)invoke.invoke(formatter, entry));
        }
        return body.toString();
    }
    static String observation(String id, String body) throws Exception {
        return "{\"id\":" + AndroidCryptoProbe.quote(id) + ",\"body\":" + AndroidCryptoProbe.quote(body)
            + ",\"body_sha256\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.hash(body.getBytes(StandardCharsets.UTF_8))) + "}";
    }
    public static void main(String[] args) throws Exception {
        AndroidCryptoProbe.require(args.length == 0, "this profile has no live inputs");
        AndroidCryptoProbe.require(System.getProperty("java.vm.name", "").equals("Dalvik"), "requires Android");
        ClassLoader loader = AndroidRegistrationFormProbe.class.getClassLoader();
        String dexHash = referenceDex(loader);
        Class<?> type = Class.forName("X.KCq", true, loader), lambda = Class.forName("X.KrQ", true, loader);
        AndroidCryptoProbe.require(type.getClassLoader() == loader && lambda.getClassLoader() == loader, "unexpected classloader");
        Method raw = type.getDeclaredMethod("A06", String.class, byte[].class);
        Method b64 = type.getDeclaredMethod("A04", String.class, byte[].class);
        Method optionalBytes = type.getDeclaredMethod("A05", String.class, byte[].class);
        Method optionalString = type.getDeclaredMethod("A02", String.class, String.class);
        Method text = type.getDeclaredMethod("A01", String.class, String.class);
        Method bool = type.getDeclaredMethod("A00", String.class, int.class);
        Method uuid = type.getDeclaredMethod("A03", String.class, String.class);
        byte[] all = new byte[256]; for (int i = 0; i < all.length; i++) all[i] = (byte)i;
        Object first = builder(type); raw.invoke(first, "probe_blob", all);
        String rawBody = serialize(first, lambda);
        Set<?> flags = (Set<?>)type.getField("A01").get(first);
        AndroidCryptoProbe.require(flags.contains("probe_blob"), "encoded-field flag missing");
        Object second = builder(type); b64.invoke(second, "probe_blob", all);
        String b64Body = serialize(second, lambda);
        Object third = builder(type);
        text.invoke(third, "label", "synthetic space +%&/\u00e9~*");
        optionalString.invoke(third, "omitted_text", null);
        optionalBytes.invoke(third, "omitted_bytes", null);
        bool.invoke(third, "yes", 1); bool.invoke(third, "no", 0); bool.invoke(third, "omitted_bool", 2);
        uuid.invoke(third, "probe_uuid", "00010203-0405-0607-0809-0a0b0c0d0e0f");
        String mixedBody = serialize(third, lambda);
        Map<?,?> map = (Map<?,?>)type.getField("A00").get(third);
        AndroidCryptoProbe.require(!map.containsKey("omitted_text") && !map.containsKey("omitted_bytes")
            && !map.containsKey("omitted_bool"), "optional omission changed");
        System.out.println("{\"schema_version\":1,\"execution\":\"android_original_registration_form\","
            + "\"http\":\"not_called\",\"registration\":\"not_run\",\"application_lifecycle\":\"not_run\",\"stubs\":[],"
            + "\"reference_dex_sha256\":" + AndroidCryptoProbe.quote(dexHash) + ",\"corpus\":\"all_256_bytes_and_synthetic_text\",\"formatter_profile\":46,\"encoded_flag_retained\":true,"
            + "\"cases\":[" + observation("raw-all-bytes", rawBody) + "," + observation("base64-all-bytes", b64Body)
            + "," + observation("optional-text-uuid", mixedBody) + "],\"build_fingerprint\":"
            + AndroidCryptoProbe.quote(AndroidCryptoProbe.buildField("FINGERPRINT")) + "}");
    }
}
