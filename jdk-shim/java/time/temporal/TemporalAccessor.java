package java.time.temporal;

public interface TemporalAccessor {
    default boolean isSupported(TemporalField field) { return false; }
    default long getLong(TemporalField field) { return 0L; }
    default int get(TemporalField field) { return (int) getLong(field); }
    default <R> R query(TemporalQuery<R> query) {
        return query == null ? null : query.queryFrom(this);
    }
}
