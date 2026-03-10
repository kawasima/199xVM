package java.util.stream;

public interface Collector<T, A, R> {
    A supplier();
    void accumulator(A container, T element);
    R finisher(A container);
}
