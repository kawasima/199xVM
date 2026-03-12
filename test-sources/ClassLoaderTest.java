public class ClassLoaderTest {
    public static String run() {
        StringBuilder sb = new StringBuilder();

        // (1) ClassLoader.getSystemClassLoader() must be non-null
        ClassLoader cl = ClassLoader.getSystemClassLoader();
        sb.append(cl != null ? "cl:ok" : "cl:null");
        sb.append("|");

        // (2) Class.forName for a known shim class must succeed
        try {
            Class<?> c = Class.forName("java.util.ArrayList");
            sb.append(c != null ? "forName:ok" : "forName:null");
        } catch (ClassNotFoundException e) {
            sb.append("forName:cnf");
        }
        sb.append("|");

        // (3) getSystemClassLoader().loadClass for a known shim class must succeed
        try {
            Class<?> c = ClassLoader.getSystemClassLoader().loadClass("java.util.ArrayList");
            sb.append(c != null ? "loadClass:ok" : "loadClass:null");
        } catch (ClassNotFoundException e) {
            sb.append("loadClass:cnf");
        }

        return sb.toString();
    }
}
