package java.lang;

public class ClassLoader {
    protected ClassLoader() {}

    public static native ClassLoader getSystemClassLoader();

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        // The 'resolve' flag (link resolution) is intentionally ignored — not applicable in 199xVM.
        Class<?> c = findLoadedClass(name);
        if (c == null) {
            c = findClass(name);
        }
        return c;
    }

    protected native Class<?> findLoadedClass(String name);

    protected native Class<?> findClass(String name) throws ClassNotFoundException;

    protected final Class<?> defineClass(String name, byte[] b, int off, int len) {
        return null;
    }
}
