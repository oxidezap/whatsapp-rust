/* Narrow JNI smoke test. Requires a disposable, network-isolated Android image. */
import java.io.File;
import java.lang.reflect.*;
import java.util.Arrays;

/** The original APK must be a pinned, read-only entry on app_process's CLASSPATH.
 * This loads only its separately hash-pinned Superpack JNI library. It tests
 * native registration and a synthetic byte-array marker, not decompression,
 * protocol cryptography, application startup or device/account registration.
 */
public final class AndroidSuperpackProbe {
    public static void main(String[] args) throws Exception {
        AndroidCryptoProbe.require(args.length == 2, "AndroidSuperpackProbe <library> <sha256>");
        AndroidCryptoProbe.require(System.getProperty("java.vm.name", "").equals("Dalvik"), "requires Android runtime");
        File library = new File(args[0]);
        AndroidCryptoProbe.require(library.length() <= 4 * 1024 * 1024, "JNI library budget");
        String hash = AndroidCryptoProbe.fileHash(library);
        AndroidCryptoProbe.require(hash.equals(args[1]), "JNI library hash mismatch");
        Class<?> target = Class.forName("com.facebook.superpack.AssetDecompressor", true,
            AndroidSuperpackProbe.class.getClassLoader());
        AndroidCryptoProbe.require(target.getClassLoader() == AndroidSuperpackProbe.class.getClassLoader(),
            "unexpected JNI classloader");
        System.load(library.getAbsolutePath());
        Method marker = target.getDeclaredMethod("testDecompressorLibraryUsable", byte[].class);
        Method architecture = target.getDeclaredMethod("get_architecture");
        AndroidCryptoProbe.require(Modifier.isNative(marker.getModifiers())
            && Modifier.isNative(architecture.getModifiers()), "not native methods");
        byte[] bytes = new byte[8];
        marker.invoke(null, (Object)bytes);
        AndroidCryptoProbe.require(Arrays.equals(bytes, new byte[]{0x47, 0x77, 0x53, 0, 0, 0, 0, 0}),
            "native byte-array marker mismatch");
        try {
            marker.invoke(null, (Object)new byte[2]);
            throw new IllegalStateException("undersized JNI array accepted");
        } catch (InvocationTargetException e) {
            AndroidCryptoProbe.require(e.getCause() instanceof ArrayIndexOutOfBoundsException,
                "unexpected native array error");
        }
        int arch = (Integer)architecture.invoke(null);
        System.out.println("{\"schema_version\":1,\"execution\":\"android_art_original_jni\","
            + "\"scope\":\"superpack_native_registration_and_synthetic_array_marker\","
            + "\"application_lifecycle\":\"not_run\",\"registration\":\"not_run\",\"decompression\":\"not_run\","
            + "\"protocol_crypto\":\"not_run\",\"stubs\":[],\"dynamic_import_trace\":\"not_recorded\","
            + "\"library_sha256\":" + AndroidCryptoProbe.quote(hash) + ",\"architecture_result\":" + arch
            + ",\"marker_hex\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.hex(bytes))
            + ",\"undersized_array\":\"array_bounds_error\",\"vm_version\":"
            + AndroidCryptoProbe.quote(System.getProperty("java.vm.version"))
            + ",\"build_fingerprint\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.buildField("FINGERPRINT")) + "}");
    }
}
