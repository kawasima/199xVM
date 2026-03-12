package java.lang;

public class ClassLoader {
    protected ClassLoader() {}

    public static native ClassLoader getSystemClassLoader();

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    // Simplified parent-delegation: checks the local registry via native stubs only.
    // No parent-loader chain and no link-resolution step (the 'resolve' flag is ignored)
    // — both are intentional simplifications for 199xVM's pre-bundled class model.
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
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
