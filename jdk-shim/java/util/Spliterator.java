package java.util;

import java.util.function.Consumer;
import java.util.function.IntConsumer;

public interface Spliterator<T> {
    int ORDERED = 0x00000010;
    int DISTINCT = 0x00000001;
    int SORTED = 0x00000004;
    int SIZED = 0x00000040;
    int SUBSIZED = 0x00004000;

    boolean tryAdvance(Consumer<? super T> action);
    Spliterator<T> trySplit();
    long estimateSize();
    int characteristics();

    default void forEachRemaining(Consumer<? super T> action) {
        while (tryAdvance(action)) {}
    }

    default Comparator<? super T> getComparator() {
        throw new IllegalStateException();
    }

    interface OfInt extends Spliterator<Integer> {
        boolean tryAdvance(IntConsumer action);
        default void forEachRemaining(IntConsumer action) {
            while (tryAdvance(action)) {}
        }
    }
}
