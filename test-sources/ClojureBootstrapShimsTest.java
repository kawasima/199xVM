import java.io.*;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.Properties;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.locks.ReentrantReadWriteLock;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class ClojureBootstrapShimsTest {
    public static String run() {
        StringBuilder sb = new StringBuilder();

        // 1. String.equalsIgnoreCase(null) must not NPE
        if ("hello".equalsIgnoreCase(null)) return "FAIL:equalsIgnoreCase-null-true";
        sb.append("eqIC");

        // 2. StringConcatFactory with char args
        char sep = '/';
        String path = "clojure" + sep + "core";
        if (!"clojure/core".equals(path)) return "FAIL:concat-char:" + path;
        sb.append("|concat");

        // 3. Regex capture groups (matches)
        Pattern p = Pattern.compile("(\\d+)\\.(\\d+)\\.(\\d+)");
        Matcher m = p.matcher("1.12.0");
        if (!m.matches()) return "FAIL:matches";
        if (m.groupCount() != 3) return "FAIL:groupCount:" + m.groupCount();
        if (!"1".equals(m.group(1))) return "FAIL:group1:" + m.group(1);
        if (!"12".equals(m.group(2))) return "FAIL:group2:" + m.group(2);
        if (!"0".equals(m.group(3))) return "FAIL:group3:" + m.group(3);
        sb.append("|regex");

        // 4. Regex with optional group (like Clojure version parsing)
        Pattern vp = Pattern.compile("(\\d+)\\.(\\d+)\\.(\\d+)(?:-([a-zA-Z0-9_]+))?(?:-(SNAPSHOT))?");
        Matcher vm = vp.matcher("1.12.0");
        if (!vm.matches()) return "FAIL:version-match";
        if (!"1".equals(vm.group(1))) return "FAIL:v-major:" + vm.group(1);
        if (!"12".equals(vm.group(2))) return "FAIL:v-minor:" + vm.group(2);
        if (vm.group(4) != null) return "FAIL:v-qualifier-not-null:" + vm.group(4);
        sb.append("|vparse");

        // 5. System.getProperty
        String enc = System.getProperty("file.encoding");
        if (enc == null) return "FAIL:getProperty-null";
        sb.append("|sysprop");

        // 6. ThreadLocal (without lambda — withInitial has checkcast issue)
        ThreadLocal<String> tl = new ThreadLocal<>();
        tl.set("hello");
        if (!"hello".equals(tl.get())) return "FAIL:threadlocal-set:" + tl.get();
        tl.remove();
        if (tl.get() != null) return "FAIL:threadlocal-remove:" + tl.get();
        sb.append("|tlocal");

        // 7. AtomicInteger
        AtomicInteger ai = new AtomicInteger(0);
        ai.incrementAndGet();
        ai.addAndGet(9);
        if (ai.get() != 10) return "FAIL:atomic:" + ai.get();
        if (!ai.compareAndSet(10, 20)) return "FAIL:cas-true";
        if (ai.compareAndSet(10, 30)) return "FAIL:cas-false";
        sb.append("|atomic");

        // 8. Properties load
        String propsText = "version=1.12.0\nmajor=1\n";
        Properties props = new Properties();
        try {
            props.load(new ByteArrayInputStream(propsText.getBytes()));
        } catch (Exception e) {
            return "FAIL:props-load:" + e;
        }
        if (!"1.12.0".equals(props.getProperty("version"))) return "FAIL:props-version:" + props.getProperty("version");
        if (!"1".equals(props.getProperty("major"))) return "FAIL:props-major";
        if (props.getProperty("missing") != null) return "FAIL:props-missing-not-null";
        sb.append("|props");

        // 9. Charset
        Charset utf8 = Charset.forName("UTF-8");
        if (!"UTF-8".equals(utf8.name())) return "FAIL:charset:" + utf8.name();
        if (StandardCharsets.UTF_8 == null) return "FAIL:stdcharset-null";
        sb.append("|charset");

        // 10. String.replace
        if (!"a.b.c".equals("a/b/c".replace('/', '.'))) return "FAIL:replace-char";
        if (!"hello world".equals("hello-world".replace("-", " "))) return "FAIL:replace-seq";
        sb.append("|replace");

        // 11. Double.toHexString / bit conversion
        long bits = Double.doubleToLongBits(1.0);
        double back = Double.longBitsToDouble(bits);
        if (back != 1.0) return "FAIL:double-bits";
        if (!Double.isFinite(1.0)) return "FAIL:isFinite";
        if (Double.isFinite(Double.POSITIVE_INFINITY)) return "FAIL:isFinite-inf";
        sb.append("|double");

        // 12. ReentrantReadWriteLock
        ReentrantReadWriteLock rwl = new ReentrantReadWriteLock();
        rwl.readLock().lock();
        rwl.readLock().unlock();
        rwl.writeLock().lock();
        rwl.writeLock().unlock();
        sb.append("|rwlock");

        // 13. File path basics
        File f = new File("/tmp/test.txt");
        if (!"/tmp/test.txt".equals(f.getPath())) return "FAIL:file-path:" + f.getPath();
        sb.append("|file");

        // 14. Boolean.toString static
        if (!"true".equals(Boolean.toString(true))) return "FAIL:bool-toString";
        if (!"false".equals(Boolean.toString(false))) return "FAIL:bool-toString-false";
        sb.append("|bool");

        return sb.toString();
    }
}
