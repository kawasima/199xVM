package java.time.temporal;

public interface TemporalQuery<R> {
    R queryFrom(TemporalAccessor temporal);
}
