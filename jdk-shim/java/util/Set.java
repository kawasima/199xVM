package java.util;

public interface Set<E> extends Collection<E> {
    @SafeVarargs
    static <E> Set<E> of(E... elements) {
        HashSet<E> s = new HashSet<>();
        if (elements != null) {
            for (int i = 0; i < elements.length; i++) {
                s.add(elements[i]);
            }
        }
        return s;
    }
}
