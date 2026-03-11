package java.time.temporal;

public final class TemporalQueries {
    private TemporalQueries() {}

    private static final TemporalQuery<java.time.ZoneId> ZONE_ID = temporal -> null;
    private static final TemporalQuery<java.time.ZoneId> ZONE = temporal -> null;
    private static final TemporalQuery<java.time.chrono.Chronology> CHRONOLOGY = temporal -> null;

    public static TemporalQuery<java.time.ZoneId> zoneId() { return ZONE_ID; }
    public static TemporalQuery<java.time.ZoneId> zone() { return ZONE; }
    public static TemporalQuery<java.time.chrono.Chronology> chronology() { return CHRONOLOGY; }
}
