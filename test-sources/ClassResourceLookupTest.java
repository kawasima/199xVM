import java.io.InputStream;

public class ClassResourceLookupTest {
    public static String run() throws Exception {
        var loader = ClassLoader.getSystemClassLoader();
        long t1 = loader.getResource("MatcherTest.class").openConnection().getLastModified();
        long t2 = loader.getResource("MatcherTest.class").openConnection().getLastModified();
        InputStream in = loader.getResourceAsStream("MatcherTest.class");
        if (in == null) {
            return "missing";
        }
        byte[] bytes = in.readAllBytes();
        if (bytes.length < 4) {
            return "short";
        }
        return (bytes[0] & 0xff) + "|"
                + (bytes[1] & 0xff) + "|"
                + (bytes[2] & 0xff) + "|"
                + (bytes[3] & 0xff) + "|"
                + (t1 == t2 && t1 > 0L);
    }
}
