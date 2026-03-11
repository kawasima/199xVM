package java.lang;

public class ClassLoader {
    protected ClassLoader() {}

    public static ClassLoader getSystemClassLoader() {
        return null;
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return Class.forName(name);
    }
}
