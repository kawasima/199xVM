package java.util;

public class Arrays {
    private Arrays() {}

    @SafeVarargs
    public static <T> List<T> asList(T... a) {
        ArrayList<T> list = new ArrayList<>();
        for (T e : a) {
            list.add(e);
        }
        return list;
    }
}
