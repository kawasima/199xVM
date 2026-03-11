package java.time.temporal;

import java.util.Locale;

public interface TemporalField {
    TemporalUnit getBaseUnit();
    TemporalUnit getRangeUnit();
    ValueRange range();
    boolean isDateBased();
    boolean isTimeBased();

    default ValueRange rangeRefinedBy(TemporalAccessor temporal) { return range(); }
    default long getFrom(TemporalAccessor temporal) { return temporal.getLong(this); }
    default <R extends Temporal> R adjustInto(R temporal, long newValue) { return temporal; }
    default String getDisplayName(Locale locale) { return toString(); }
    default boolean isSupportedBy(TemporalAccessor temporal) { return temporal != null && temporal.isSupported(this); }
}
