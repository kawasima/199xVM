public class FindLoadedClassStateTest {
    public static final class ProbeLoader extends ClassLoader {
        public ProbeLoader() {
            super(ClassLoader.getSystemClassLoader());
        }

        public Class<?> probe(String name) {
            return findLoadedClass(name);
        }
    }

    public static final class Target {
    }

    public static String run() throws Exception {
        ProbeLoader loader = new ProbeLoader();
        String name = "FindLoadedClassStateTest$Target";

        Class<?> before = loader.probe(name);
        Class<?> loaded = Class.forName(name, false, loader);
        Class<?> after = loader.probe(name);

        return (before == null) + "|" + (loaded != null) + "|" + (after == loaded);
    }
}
