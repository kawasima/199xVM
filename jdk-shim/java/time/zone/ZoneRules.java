package java.time.zone;

public class ZoneRules {
    private static final ZoneRules UTC = new ZoneRules();

    public static ZoneRules of() { return UTC; }

    public boolean isDaylightSavings(java.time.Instant instant) { return false; }

    public ZoneOffsetTransition getTransition(java.time.LocalDateTime ldt) { return null; }
}
