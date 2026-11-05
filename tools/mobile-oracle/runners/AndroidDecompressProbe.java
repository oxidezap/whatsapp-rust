/* Runs only in a disposable, network-isolated Android image with pinned inputs. */
import java.io.File;
import java.lang.reflect.Method;
import java.util.*;

/** Extract a reviewed bounded range using the APK's actual Superpack JNI.
 * Original APK must be on the runner classpath. No application is installed.
 * Reports and extracted proprietary files must remain private.
 */
public final class AndroidDecompressProbe {
    public static void main(String[] args) throws Exception {
        AndroidCryptoProbe.require(args.length == 5,
            "AndroidDecompressProbe <loader> <loader-sha256> <container> <container-sha256> <fresh-output>");
        AndroidCryptoProbe.require(System.getProperty("java.vm.name", "").equals("Dalvik"), "requires Android");
        File library = new File(args[0]), container = new File(args[2]), output = new File(args[4]);
        AndroidCryptoProbe.require(AndroidCryptoProbe.fileHash(library).equals(args[1]), "loader pin mismatch");
        AndroidCryptoProbe.require(AndroidCryptoProbe.fileHash(container).equals(args[3]), "container pin mismatch");
        String root = "/data/local/tmp/mobile-oracle/";
        AndroidCryptoProbe.require(output.getCanonicalPath().startsWith(root) && !output.exists(), "fresh private output required");
        AndroidCryptoProbe.require(output.mkdirs(), "cannot create private output");
        Class<?> target = Class.forName("com.facebook.superpack.AssetDecompressor", true,
            AndroidDecompressProbe.class.getClassLoader());
        AndroidCryptoProbe.require(target.getClassLoader() == AndroidDecompressProbe.class.getClassLoader(), "unexpected classloader");
        System.load(library.getAbsolutePath());
        Method method = target.getDeclaredMethod("decompress_range_from_so", String.class, String.class,
            String.class, int.class, int.class);
        // This profile deliberately exercises the first 18 primary libraries,
        // matching the separately reviewed original caller for this snapshot.
        String[] names = (String[])method.invoke(null, container.getAbsolutePath(), "spo", output.getAbsolutePath(), 0, 18);
        AndroidCryptoProbe.require(names != null && names.length == 18, "result count budget");
        String[] files = output.list();
        AndroidCryptoProbe.require(files != null && files.length == 18, "output inventory budget");
        Arrays.sort(files);
        List<String> artifacts = new ArrayList<>(); long total = 0;
        for (String name : files) {
            File file = new File(output, name);
            AndroidCryptoProbe.require(file.isFile() && file.getCanonicalFile().getParentFile().equals(output.getCanonicalFile()), "unsafe output");
            total += file.length(); AndroidCryptoProbe.require(total <= 512L * 1024 * 1024, "aggregate output budget");
            artifacts.add("{\"name\":" + AndroidCryptoProbe.quote(file.getName()) + ",\"size_bytes\":"
                + file.length() + ",\"sha256\":" + AndroidCryptoProbe.quote(AndroidCryptoProbe.fileHash(file)) + "}");
        }
        System.out.println("{\"schema_version\":1,\"execution\":\"android_original_superpack_jni\","
            + "\"application_lifecycle\":\"not_run\",\"registration\":\"not_run\",\"stubs\":[],"
            + "\"range_start\":0,\"range_count\":18,\"native_result_count\":" + names.length
            + ",\"artifacts\":[" + String.join(",", artifacts) + "],\"build_fingerprint\":"
            + AndroidCryptoProbe.quote(AndroidCryptoProbe.buildField("FINGERPRINT")) + "}");
    }
}
