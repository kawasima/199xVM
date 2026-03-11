package java.time;

public class ZoneId {
    private final String id;

    protected ZoneId(String id) { this.id = id == null ? "UTC" : id; }

    public static ZoneId of(String zoneId) { return new ZoneId(zoneId); }
    public static ZoneId ofOffset(String prefix, ZoneOffset offset) {
        return new ZoneId((prefix == null ? "" : prefix) + (offset == null ? "" : offset.toString()));
    }

    public String getId() { return id; }
    public java.time.zone.ZoneRules getRules() { return java.time.zone.ZoneRules.of(); }

    @Override
    public String toString() { return id; }
}
