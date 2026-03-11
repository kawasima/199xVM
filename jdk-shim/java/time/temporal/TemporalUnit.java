package java.time.temporal;

public interface TemporalUnit {
    default boolean isDateBased() { return false; }
    default boolean isTimeBased() { return false; }
}
