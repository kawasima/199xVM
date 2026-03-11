package java.time.temporal;

public interface Temporal extends TemporalAccessor {
    default Temporal with(TemporalField field, long newValue) { return this; }
    default Temporal plus(long amountToAdd, TemporalUnit unit) { return this; }
    default long until(Temporal endExclusive, TemporalUnit unit) { return 0L; }
}
