/* Run only through app_process in a disposable, network-isolated AOSP image. */
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.util.*;
import javax.crypto.*;

/** Exercises reviewed original DEX helpers with synthetic inputs. No application
 * lifecycle, Context, account, registration endpoint or replacement stub exists.
 * The host must pin the APK, reviewed selectors, runner and Android image.
 */
public final class AndroidCryptoProbe {
    static void require(boolean value, String message) {
        if (!value) throw new IllegalArgumentException(message);
    }
    static String hex(byte[] bytes) {
        StringBuilder s = new StringBuilder();
        for (byte b : bytes) s.append(String.format(Locale.ROOT, "%02x", b & 255));
        return s.toString();
    }
    static String hash(byte[] bytes) throws Exception {
        return hex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }
    static String fileHash(File file) throws Exception {
        require(file.isFile() && file.length() <= 64L * 1024 * 1024, "DEX size budget");
        MessageDigest hash = MessageDigest.getInstance("SHA-256");
        try (InputStream in = new FileInputStream(file)) {
            byte[] buffer = new byte[16384]; int count; long total = 0;
            while ((count = in.read(buffer)) != -1) {
                total += count; require(total <= 256L * 1024 * 1024, "DEX grew beyond budget");
                hash.update(buffer, 0, count);
            }
        }
        return hex(hash.digest());
    }
    static String quote(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"")
            .replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t") + "\"";
    }
    static String buildField(String name) throws Exception {
        return String.valueOf(Class.forName("android.os.Build").getField(name).get(null));
    }
    public static void main(String[] args) throws Exception {
        require(args.length == 3, "AndroidCryptoProbe <original-dex> <expected-sha256> <reviewed-class-name>");
        require(args[1].matches("[0-9a-f]{64}") && args[2].matches("[A-Za-z0-9_.$]+"), "pin/selector syntax");
        File originalDex = new File(args[0]);
        String dexHash = fileHash(originalDex); require(dexHash.equals(args[1]), "DEX hash mismatch");
        require(System.getProperty("java.vm.name", "").equals("Dalvik"), "requires Android runtime");
        Class<?> loaderType = Class.forName("dalvik.system.PathClassLoader");
        ClassLoader loader = (ClassLoader) loaderType.getConstructor(String.class, ClassLoader.class)
            .newInstance(originalDex.getAbsolutePath(), AndroidCryptoProbe.class.getClassLoader());
        // Original class initialization is intentionally retained. Review that
        // initializer and the reachable constructors before choosing this class.
        Class<?> target = Class.forName(args[2], true, loader);
        require(target.getClassLoader() == loader, "unexpected parent classloader");
        Method hmac = target.getDeclaredMethod("A0M", byte[].class, byte[][].class);
        Method pbkdf = target.getDeclaredMethod("A08", String.class, byte[].class, char[].class, int.class, int.class);
        require(Modifier.isStatic(hmac.getModifiers()) && Modifier.isStatic(pbkdf.getModifiers()), "not static helpers");
        byte[] key1 = new byte[20]; Arrays.fill(key1, (byte)0x0b);
        byte[] tokenKey = new byte[64], tokenMessage = new byte[29];
        for (int i = 0; i < 64; i++) tokenKey[i] = (byte)i;
        tokenMessage[0] = 0x30; tokenMessage[1] = 1; tokenMessage[2] = 7;
        for (int i = 0; i < 16; i++) tokenMessage[3+i] = (byte)i;
        Arrays.fill(tokenMessage, 19, 29, (byte)'0');
        byte[][] keys = {key1, "Jefe".getBytes(StandardCharsets.US_ASCII),
            new byte[]{(byte)0x80, (byte)0xff, 0, 1}, tokenKey};
        byte[][] messages = {"Hi There".getBytes(StandardCharsets.US_ASCII),
            "what do ya want for nothing?".getBytes(StandardCharsets.US_ASCII), new byte[0], tokenMessage};
        String[] known = {"b617318655057264e28bc0b6fb378c8ef146be00",
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79", null, "c617773e23b380eb66ce5a9fe4c9ce8c6e6d6c9d"};
        List<String> results = new ArrayList<>();
        for (int i = 0; i < keys.length; i++) {
            byte[] output = (byte[])hmac.invoke(null, keys[i], new byte[][]{messages[i]});
            if (known[i] != null) require(hex(output).equals(known[i]), "synthetic HMAC mismatch");
            results.add("{\"id\":\"synthetic-" + i + "\",\"key_sha256\":" + quote(hash(keys[i]))
                + ",\"message_sha256\":" + quote(hash(messages[i])) + ",\"output_hex\":" + quote(hex(output))
                + ",\"output_sha256\":" + quote(hash(output)) + ",\"output_base64\":"
                + quote(Base64.getEncoder().encodeToString(output)) + "}");
        }
        try { hmac.invoke(null, key1, new byte[0][]); throw new IllegalStateException("empty parts accepted"); }
        catch (InvocationTargetException e) {
            require(e.getCause() instanceof ArrayIndexOutOfBoundsException, "unexpected empty-parts error");
        }
        byte[] packageBytes = "com.whatsapp".getBytes(StandardCharsets.UTF_8);
        byte[] logo = {0, (byte)0xff, (byte)0x80, 1}, salt = new byte[16];
        byte[] password = Arrays.copyOf(packageBytes, packageBytes.length + logo.length);
        System.arraycopy(logo, 0, password, packageBytes.length, logo.length);
        char[] chars = new char[password.length];
        for (int i = 0; i < password.length; i++) chars[i] = (char)password[i];
        for (int i = 0; i < salt.length; i++) salt[i] = (byte)i;
        String algorithm = "PBKDF2WithHmacSHA1And8BIT";
        SecretKey derived = (SecretKey)pbkdf.invoke(null, algorithm, salt, chars, 128, 512);
        String sdk = String.valueOf(Class.forName("android.os.Build$VERSION").getField("SDK_INT").get(null));
        System.out.println("{\"schema_version\":1,\"execution\":\"android_art_original_dex\","
            + "\"application_lifecycle\":\"not_run\",\"registration\":\"not_run\",\"stubs\":[],"
            + "\"dynamic_import_trace\":\"not_recorded\",\"class_initialization\":\"original\","
            + "\"whatsapp_jni\":\"not_run\",\"clock\":\"unused_by_selected_helpers\",\"randomness\":\"unused_by_selected_helpers\","
            + "\"dex_sha256\":" + quote(dexHash) + ",\"class\":" + quote(args[2])
            + ",\"vm_name\":" + quote(System.getProperty("java.vm.name"))
            + ",\"vm_version\":" + quote(System.getProperty("java.vm.version"))
            + ",\"android_sdk\":" + quote(sdk) + ",\"build_fingerprint\":" + quote(buildField("FINGERPRINT"))
            + ",\"build_abis\":" + quote(Arrays.toString((String[])Class.forName("android.os.Build").getField("SUPPORTED_ABIS").get(null)))
            + ",\"hmac_provider\":" + quote(Mac.getInstance("HmacSHA1").getProvider().toString())
            + ",\"pbkdf_provider\":" + quote(SecretKeyFactory.getInstance(algorithm).getProvider().toString())
            + ",\"cases\":[" + String.join(",", results) + "],\"empty_parts\":\"array_bounds_error\","
            + "\"pbkdf\":{\"id\":\"synthetic-binary-password-8bit\",\"iterations\":128,\"bits\":512,"
            + "\"password_sha256\":" + quote(hash(password)) + ",\"salt_sha256\":" + quote(hash(salt))
            + ",\"output_hex\":" + quote(hex(derived.getEncoded())) + "}}");
    }
}
