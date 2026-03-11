package java.time.chrono;

import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.time.temporal.TemporalAccessor;

public interface Chronology {
    default String getId() { return "ISO"; }
    default String getCalendarType() { return "iso8601"; }
    default ChronoLocalDate date(ChronoLocalDate temporal) { return temporal; }
    default List<Era> eras() { return new java.util.ArrayList<>(); }

    static Chronology from(TemporalAccessor temporal) { return IsoChronology.INSTANCE; }
    static Chronology ofLocale(Locale locale) { return IsoChronology.INSTANCE; }
    static Set<Chronology> getAvailableChronologies() {
        HashSet<Chronology> s = new HashSet<>();
        s.add(IsoChronology.INSTANCE);
        return s;
    }
}
