package java.time.temporal;

public enum ChronoUnit implements TemporalUnit {
    NANOS, MICROS, MILLIS, SECONDS, MINUTES, HOURS, HALF_DAYS, DAYS,
    WEEKS, MONTHS, YEARS, DECADES, CENTURIES, MILLENNIA, ERAS, FOREVER;

    @Override
    public boolean isDateBased() {
        return this.ordinal() >= DAYS.ordinal() && this != FOREVER;
    }

    @Override
    public boolean isTimeBased() {
        return this.ordinal() <= HALF_DAYS.ordinal();
    }
}
