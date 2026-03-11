package java.lang;

public final class Void {
    @SuppressWarnings("unchecked")
    public static final Class<Void> TYPE = (Class<Void>) primitiveType("void");
    private static Class<?> primitiveType(String name) { try { return Class.forName(name); } catch (ClassNotFoundException e) { return null; } }
    private Void() {}
}
