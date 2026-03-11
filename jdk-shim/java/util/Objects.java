package java.util;

public final class Objects {
    private Objects() {}

    public static <T> T requireNonNull(T obj) {
        if (obj == null) throw new NullPointerException();
        return obj;
    }

    public static <T> T requireNonNull(T obj, String message) {
        if (obj == null) throw new NullPointerException(message);
        return obj;
    }

    public static int checkIndex(int index, int length) {
        if (index < 0 || index >= length) throw new IndexOutOfBoundsException();
        return index;
    }

    public static int checkFromToIndex(int fromIndex, int toIndex, int length) {
        if (fromIndex < 0 || toIndex < fromIndex || toIndex > length) throw new IndexOutOfBoundsException();
        return fromIndex;
    }

    public static int checkFromIndexSize(int fromIndex, int size, int length) {
        if (fromIndex < 0 || size < 0 || fromIndex + size > length) throw new IndexOutOfBoundsException();
        return fromIndex;
    }

    public static <T> T requireNonNullElse(T obj, T defaultObj) {
        return (obj != null) ? obj : requireNonNull(defaultObj, "defaultObj");
    }

    public static boolean equals(Object a, Object b) {
        return (a == b) || (a != null && a.equals(b));
    }

    public static int hashCode(Object o) {
        return o != null ? o.hashCode() : 0;
    }

    public static int hash(Object... values) {
        if (values == null) return 0;
        int result = 1;
        for (Object element : values) {
            result = 31 * result + (element == null ? 0 : element.hashCode());
        }
        return result;
    }

    public static String toString(Object o) {
        return String.valueOf(o);
    }

    public static String toString(Object o, String nullDefault) {
        return (o != null) ? o.toString() : nullDefault;
    }
}
