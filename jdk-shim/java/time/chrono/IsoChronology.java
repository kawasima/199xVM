package java.time.chrono;

import java.util.ArrayList;
import java.util.List;

public final class IsoChronology implements Chronology {
    public static final IsoChronology INSTANCE = new IsoChronology();

    private IsoChronology() {}

    @Override
    public String getId() { return "ISO"; }

    @Override
    public String getCalendarType() { return "iso8601"; }

    @Override
    public ChronoLocalDate date(ChronoLocalDate temporal) { return temporal; }

    @Override
    public List<Era> eras() { return new ArrayList<>(); }
}
