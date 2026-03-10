package java.util.stream;

import java.util.function.Function;
import java.util.function.Predicate;

public interface Stream<T> {
    <R> Stream<R> map(Function<? super T, ? extends R> mapper);
    Stream<T> filter(Predicate<? super T> predicate);
    <R> R collect(Collector<? super T, ?, R> collector);
}
