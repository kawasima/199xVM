package java.util.stream;

import java.util.function.Function;
import java.util.function.Predicate;
import java.util.function.Consumer;
import java.util.function.BinaryOperator;
import java.util.Comparator;
import java.util.Optional;

public interface Stream<T> {
    <R> Stream<R> map(Function<? super T, ? extends R> mapper);
    Stream<T> filter(Predicate<? super T> predicate);
    <R> R collect(Collector<? super T, ?, R> collector);

    default java.util.List<T> toList() {
        return collect(Collectors.toList());
    }

    default void forEach(Consumer<? super T> action) {
        java.util.List<T> list = toList();
        for (int i = 0; i < list.size(); i++) {
            action.accept(list.get(i));
        }
    }

    default T reduce(T identity, BinaryOperator<T> op) {
        T acc = identity;
        java.util.List<T> list = toList();
        for (int i = 0; i < list.size(); i++) {
            acc = op.apply(acc, list.get(i));
        }
        return acc;
    }

    default Optional<T> reduce(BinaryOperator<T> op) {
        java.util.List<T> list = toList();
        if (list.isEmpty()) return Optional.empty();
        T acc = list.get(0);
        for (int i = 1; i < list.size(); i++) {
            acc = op.apply(acc, list.get(i));
        }
        return Optional.ofNullable(acc);
    }

    default Optional<T> findAny() {
        java.util.List<T> list = toList();
        return list.isEmpty() ? Optional.empty() : Optional.ofNullable(list.get(0));
    }

    default Optional<T> min(Comparator<? super T> comparator) {
        java.util.List<T> list = toList();
        if (list.isEmpty()) return Optional.empty();
        T min = list.get(0);
        for (int i = 1; i < list.size(); i++) {
            T v = list.get(i);
            if (comparator.compare(v, min) < 0) min = v;
        }
        return Optional.ofNullable(min);
    }
}
