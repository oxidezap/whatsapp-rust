/* Host-only translated-DEX experiment. Never load an APK as an application. */
import com.googlecode.d2j.Method;
import com.googlecode.d2j.dex.Dex2jar;
import com.googlecode.d2j.dex.DexExceptionHandler;
import com.googlecode.d2j.node.DexMethodNode;
import com.googlecode.d2j.reader.BaseDexFileReader;
import com.googlecode.d2j.reader.DexFileReader;
import com.googlecode.d2j.visitors.*;
import java.lang.reflect.InvocationTargetException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import java.util.jar.JarInputStream;
import org.objectweb.asm.*;

/** Executes one reviewed static ([B[[B)[B helper using only allowlisted JDK calls.
 * Class initialization and all other APK methods are deliberately excluded.
 * The JVM must additionally be launched with an OS deadline and no network.
 */
public final class DexHmacProbe {
    static final String DESC = "([B[[B)[B";
    static final Set<String> CALLS = Set.of(
        "javax/crypto/Mac.getInstance(Ljava/lang/String;)Ljavax/crypto/Mac;",
        "javax/crypto/Mac.init(Ljava/security/Key;)V",
        "javax/crypto/Mac.update([B)V", "javax/crypto/Mac.doFinal()[B",
        "javax/crypto/spec/SecretKeySpec.<init>([BLjava/lang/String;)V",
        "java/lang/RuntimeException.<init>(Ljava/lang/Throwable;)V");
    static void require(boolean ok, String reason) {
        if (!ok) throw new IllegalArgumentException(reason);
    }
    static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }
    static byte[] bounded(Path path, int limit) throws Exception {
        require(!Files.isSymbolicLink(path) && Files.size(path) <= limit, "input size/symlink");
        try (var in = Files.newInputStream(path)) {
            byte[] bytes = in.readNBytes(limit + 1);
            require(bytes.length <= limit, "input grew beyond budget");
            return bytes;
        }
    }
    static final class Selected implements BaseDexFileReader {
        final DexFileReader reader;
        final String owner, name;
        int matches;
        Selected(byte[] bytes, String owner, String name) {
            reader = new DexFileReader(bytes); this.owner = owner; this.name = name;
        }
        public int getDexVersion() { return reader.getDexVersion(); }
        public List<String> getClassNames() { return List.of(owner); }
        public void accept(DexFileVisitor visitor) { accept(visitor, 0); }
        public void accept(DexFileVisitor visitor, int flags, int index) {
            require(index == 0, "unexpected selected class index"); accept(visitor, flags);
        }
        public void accept(DexFileVisitor target, int flags) {
            matches = 0;
            reader.accept(new DexFileVisitor() {
                public void visitDexFileVersion(int v) { target.visitDexFileVersion(v); }
                public DexClassVisitor visit(int access, String cls, String parent, String[] interfaces) {
                    if (!cls.equals(owner)) return null;
                    require("Ljava/lang/Object;".equals(parent)
                        && (interfaces == null || interfaces.length == 0), "unsupported inheritance");
                    DexClassVisitor out = target.visit(access, cls, parent, interfaces);
                    return new DexClassVisitor() {
                        public DexMethodVisitor visitMethod(int access, Method method) {
                            if (!method.getName().equals(name) || !method.getDesc().equals(DESC)) return null;
                            require((access & 8) != 0 && (access & 0x500) == 0, "method must be static managed code");
                            matches++;
                            return out.visitMethod(access, method);
                        }
                        public void visitEnd() { out.visitEnd(); }
                    };
                }
                public void visitEnd() { target.visitEnd(); }
            }, flags | DexFileReader.SKIP_DEBUG | DexFileReader.SKIP_ANNOTATION);
            require(matches == 1, "missing or ambiguous method selector");
        }
    }
    // Independently locate the method's original instruction bytes. The entire
    // DEX hash also binds exception tables, pools and metadata outside this range.
    static final class DexRange {
        final byte[] bytes; final ByteBuffer b;
        DexRange(byte[] bytes) {
            this.bytes = bytes; b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
            require(bytes.length >= 112 && bytes[0] == 'd' && bytes[1] == 'e'
                && bytes[2] == 'x' && bytes[3] == '\n' && bytes[7] == 0, "not standalone DEX");
            require(u32(32) == bytes.length && u32(36) == 112 && u32(40) == 0x12345678,
                "unsupported DEX header/endian/container");
        }
        int u32(int p) { require(p >= 0 && p <= bytes.length - 4, "DEX offset");
            int n = b.getInt(p); require(n >= 0, "DEX integer budget"); return n; }
        int u16(int p) { require(p >= 0 && p <= bytes.length - 2, "DEX offset"); return b.getShort(p) & 65535; }
        int item(int table, int count, int index, int width) {
            require(index >= 0 && index < count, "DEX index");
            int p = Math.addExact(table, Math.multiplyExact(index, width));
            require(p <= bytes.length - width, "DEX table bounds"); return p;
        }
        int leb(int[] p) {
            int n = 0;
            for (int shift = 0; shift < 35; shift += 7) {
                require(p[0] >= 0 && p[0] < bytes.length, "truncated ULEB");
                int v = bytes[p[0]++] & 255;
                require(shift < 28 || (v & 0xf8) == 0, "ULEB overflow");
                n |= (v & 127) << shift;
                if ((v & 128) == 0) return n;
            }
            throw new IllegalArgumentException("ULEB overflow");
        }
        String string(int index) {
            int[] p = {u32(item(u32(60), u32(56), index, 4))}; leb(p);
            int start = p[0];
            while (p[0] < bytes.length && bytes[p[0]] != 0 && p[0] - start < 1024) {
                require(bytes[p[0]] > 0, "selector must be ASCII"); p[0]++;
            }
            require(p[0] < bytes.length && bytes[p[0]] == 0, "DEX string budget");
            return new String(bytes, start, p[0] - start, StandardCharsets.US_ASCII);
        }
        String type(int index) { return string(u32(item(u32(68), u32(64), index, 4))); }
        String descriptor(int proto) {
            int p = item(u32(76), u32(72), proto, 12), params = u32(p + 8);
            StringBuilder s = new StringBuilder("(");
            if (params != 0) {
                int count = u32(params); require(count <= 32, "parameter budget");
                for (int i = 0; i < count; i++) s.append(type(u16(item(params + 4, count, i, 2))));
            }
            return s.append(')').append(type(u32(p + 4))).toString();
        }
        int[] find(String owner, String name) {
            int target = -1;
            require(u32(88) <= 65536 && u32(96) <= 65536, "DEX method/class budget");
            for (int i = 0; i < u32(88); i++) {
                int p = item(u32(92), u32(88), i, 8);
                if (type(u16(p)).equals(owner) && string(u32(p + 4)).equals(name)
                    && descriptor(u16(p + 2)).equals(DESC)) {
                    require(target == -1, "duplicate method ID"); target = i;
                }
            }
            require(target >= 0, "selector absent");
            for (int i = 0; i < u32(96); i++) {
                int cls = item(u32(100), u32(96), i, 32);
                if (!type(u32(cls)).equals(owner)) continue;
                int[] p = {u32(cls + 24)}; require(p[0] != 0, "missing class data");
                int sf = leb(p), inf = leb(p), dm = leb(p), vm = leb(p);
                require((long)sf + inf + dm + vm <= 262144, "class data budget");
                for (int j = 0; j < sf + inf; j++) { leb(p); leb(p); }
                for (int group : new int[]{dm, vm}) {
                    int index = 0;
                    for (int j = 0; j < group; j++) {
                        index = Math.addExact(index, leb(p)); int access = leb(p), code = leb(p);
                        if (index != target) continue;
                        require((access & 8) != 0 && code != 0 && code % 4 == 0, "unsupported method code");
                        int start = Math.addExact(code, 16), size = Math.multiplyExact(u32(code + 12), 2);
                        require(size > 0 && size <= 65536 && start <= bytes.length - size, "instruction bounds");
                        return new int[]{start, size};
                    }
                }
            }
            throw new IllegalArgumentException("method has no code");
        }
    }
    static void checkClass(byte[] bytes, String owner, String name, Set<String> calls) {
        new ClassReader(bytes).accept(new ClassVisitor(Opcodes.ASM9) {
            int methods;
            public void visit(int v, int a, String n, String sig, String parent, String[] interfaces) {
                require(n.equals(owner.substring(1, owner.length()-1)) && "java/lang/Object".equals(parent)
                    && (interfaces == null || interfaces.length == 0), "translated class identity");
            }
            public FieldVisitor visitField(int a, String n, String d, String s, Object v) {
                throw new IllegalArgumentException("unexpected field");
            }
            public MethodVisitor visitMethod(int a, String n, String d, String s, String[] e) {
                require(n.equals(name) && d.equals(DESC) && (a & 8) != 0 && ++methods == 1,
                    "unexpected translated method or initializer");
                return new MethodVisitor(Opcodes.ASM9) {
                    public void visitMethodInsn(int op, String o, String n, String d, boolean iface) {
                        String call = o + "." + n + d;
                        require(CALLS.contains(call), "unsupported call: " + call); calls.add(call);
                    }
                    public void visitFieldInsn(int op, String o, String n, String d) {
                        throw new IllegalArgumentException("unsupported field access");
                    }
                    public void visitInvokeDynamicInsn(String n, String d, Handle h, Object... a) {
                        throw new IllegalArgumentException("unsupported invokedynamic");
                    }
                    public void visitLdcInsn(Object value) {
                        require(value instanceof String || value instanceof Number, "unsupported constant");
                    }
                    public void visitTypeInsn(int op, String t) {
                        require(t.equals("javax/crypto/spec/SecretKeySpec")
                            || t.equals("java/lang/RuntimeException"), "unsupported type: " + t);
                    }
                };
            }
            public void visitEnd() { require(methods == 1, "missing translated method"); }
        }, 0);
    }
    static final class OnlyClass extends ClassLoader {
        OnlyClass() { super(ClassLoader.getPlatformClassLoader()); }
        Class<?> load(byte[] bytes) { return defineClass(null, bytes, 0, bytes.length); }
    }
    static String quote(String s) { return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"")
        .replace("\n", "\\n").replace("\r", "\\r") + "\""; }
    public static void main(String[] args) throws Exception {
        require(args.length == 4, "usage: DexHmacProbe <dex> <sha256> <class-descriptor> <method>");
        byte[] dex = bounded(Path.of(args[0]), 64 * 1024 * 1024);
        require(hash(dex).equals(args[1]), "DEX hash mismatch");
        String owner = args[2], name = args[3];
        require(owner.matches("L[A-Za-z0-9_/$]+;") && name.matches("[A-Za-z0-9_$]+"), "selector syntax");
        int[] range = new DexRange(dex).find(owner, name);
        Path jar = Files.createTempFile("mobile-dex-probe-", ".jar");
        try {
            Files.delete(jar);
            Dex2jar.from(new Selected(dex, owner, name)).withExceptionHandler(new DexExceptionHandler() {
                public void handleFileException(Exception e) { throw new IllegalArgumentException("translation failed", e); }
                public void handleMethodTranslateException(Method m, DexMethodNode n, MethodVisitor v, Exception e) {
                    throw new IllegalArgumentException("method translation failed", e);
                }
            }).to(jar);
            byte[] translated = null;
            try (var in = new JarInputStream(Files.newInputStream(jar))) {
                for (var entry = in.getNextJarEntry(); entry != null; entry = in.getNextJarEntry()) {
                    if (entry.isDirectory()) continue;
                    require(entry.getName().equals(owner.substring(1, owner.length()-1) + ".class")
                        && translated == null, "unexpected translated artifact");
                    translated = in.readNBytes(1024 * 1024 + 1);
                    require(translated.length <= 1024 * 1024, "translation size budget");
                }
            }
            require(translated != null, "no translated class");
            Set<String> calls = new TreeSet<>(); checkClass(translated, owner, name, calls);
            var method = new OnlyClass().load(translated).getDeclaredMethod(name, byte[].class, byte[][].class);
            byte[] key1 = new byte[20]; Arrays.fill(key1, (byte)0x0b);
            byte[] tokenKey = new byte[64], tokenMessage = new byte[29];
            for (int i = 0; i < tokenKey.length; i++) tokenKey[i] = (byte)i;
            tokenMessage[0] = 0x30; tokenMessage[1] = 1; tokenMessage[2] = 7;
            for (int i = 0; i < 16; i++) tokenMessage[3+i] = (byte)i;
            Arrays.fill(tokenMessage, 19, 29, (byte)'0');
            byte[][] keys = {key1, "Jefe".getBytes(StandardCharsets.US_ASCII), new byte[]{(byte)0x80, (byte)0xff, 0, 1}, tokenKey};
            byte[][] inputs = {"Hi There".getBytes(StandardCharsets.US_ASCII),
                "what do ya want for nothing?".getBytes(StandardCharsets.US_ASCII), new byte[0], tokenMessage};
            String[] expected = {"b617318655057264e28bc0b6fb378c8ef146be00", "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79", null, null};
            List<String> results = new ArrayList<>();
            for (int i = 0; i < keys.length; i++) {
                byte[] result = (byte[]) method.invoke(null, keys[i], new byte[][]{inputs[i]});
                if (expected[i] != null) require(HexFormat.of().formatHex(result).equals(expected[i]), "RFC2202 mismatch");
                results.add("{\"id\":\"synthetic-" + i + "\",\"output_hex\":" + quote(HexFormat.of().formatHex(result)) + ",\"output_base64\":"
                    + quote(Base64.getEncoder().encodeToString(result))
                    + ",\"key_sha256\":" + quote(hash(keys[i])) + ",\"message_sha256\":" + quote(hash(inputs[i]))
                    + ",\"output_sha256\":" + quote(hash(result)) + "}");
            }
            try { method.invoke(null, key1, new byte[0][]); throw new IllegalArgumentException("empty parts accepted"); }
            catch (InvocationTargetException e) { require(e.getCause() instanceof ArrayIndexOutOfBoundsException, "unexpected empty-parts failure"); }
            System.out.println("{\"schema_version\":1,\"execution\":\"translated_dex_host_jvm\",\"qualified_android_execution\":false,"
                + "\"class_initialization\":\"excluded\",\"stubs\":[],\"android_runtime\":\"not_run\",\"jni\":\"not_run\",\"dynamic_import_trace\":\"not_recorded\",\"dex_sha256\":" + quote(hash(dex))
                + ",\"selector\":" + quote(owner + "->" + name + DESC) + ",\"offset_bytes\":" + range[0]
                + ",\"size_bytes\":" + range[1] + ",\"selector_sha256\":" + quote(hash(Arrays.copyOfRange(dex, range[0], range[0]+range[1])))
                + ",\"translated_class_sha256\":" + quote(hash(translated)) + ",\"java_runtime\":" + quote(System.getProperty("java.runtime.version"))
                + ",\"heap_limit_bytes\":" + Runtime.getRuntime().maxMemory()
                + ",\"clock\":\"unused\",\"randomness\":\"unused\",\"deadline\":\"external_required\""
                + ",\"crypto_provider\":" + quote(javax.crypto.Mac.getInstance("HmacSHA1").getProvider().toString())
                + ",\"allowed_call_sites\":[" + String.join(",", calls.stream().map(DexHmacProbe::quote).toList())
                + "],\"cases\":[" + String.join(",", results) + "],\"empty_parts\":\"array_bounds_error\"}");
        } finally { Files.deleteIfExists(jar); }
    }
}
