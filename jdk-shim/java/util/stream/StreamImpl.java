package java.util.stream;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import java.util.function.Predicate;

public class StreamImpl<T> implements Stream<T> {
    private final List<T> elements;

    public StreamImpl(List<T> elements) {
        this.elements = elements;
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> Stream<R> map(Function<? super T, ? extends R> mapper) {
        List<R> result = new ArrayList<>();
        for (T e : elements) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> filter(Predicate<? super T> predicate) {
        List<T> result = new ArrayList<>();
        for (T e : elements) {
            if (predicate.test(e)) result.add(e);
        }
        return new StreamImpl<>(result);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> R collect(Collector<? super T, ?, R> collector) {
        Collector<T, Object, R> c = (Collector<T, Object, R>) collector;
        Object container = c.supplier();
        for (T e : elements) {
            c.accumulator(container, e);
        }
        return c.finisher(container);
    }
}
