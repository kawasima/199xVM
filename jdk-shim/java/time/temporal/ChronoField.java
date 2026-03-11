package java.time.temporal;

public enum ChronoField implements TemporalField {
    NANO_OF_SECOND, NANO_OF_DAY,
    MICRO_OF_SECOND, MICRO_OF_DAY,
    MILLI_OF_SECOND, MILLI_OF_DAY,
    SECOND_OF_MINUTE, SECOND_OF_DAY,
    MINUTE_OF_HOUR, MINUTE_OF_DAY,
    HOUR_OF_AMPM, CLOCK_HOUR_OF_AMPM,
    HOUR_OF_DAY, CLOCK_HOUR_OF_DAY,
    AMPM_OF_DAY,
    DAY_OF_WEEK, ALIGNED_DAY_OF_WEEK_IN_MONTH, ALIGNED_DAY_OF_WEEK_IN_YEAR,
    DAY_OF_MONTH, DAY_OF_YEAR, EPOCH_DAY,
    ALIGNED_WEEK_OF_MONTH, ALIGNED_WEEK_OF_YEAR,
    MONTH_OF_YEAR, PROLEPTIC_MONTH,
    YEAR_OF_ERA, YEAR, ERA,
    INSTANT_SECONDS, OFFSET_SECONDS;

    @Override
    public TemporalUnit getBaseUnit() {
        return ChronoUnit.DAYS;
    }

    @Override
    public TemporalUnit getRangeUnit() {
        return ChronoUnit.FOREVER;
    }

    @Override
    public ValueRange range() {
        switch (this) {
            case NANO_OF_SECOND: return ValueRange.of(0, 999_999_999);
            case SECOND_OF_MINUTE: return ValueRange.of(0, 59);
            case MINUTE_OF_HOUR: return ValueRange.of(0, 59);
            case HOUR_OF_DAY: return ValueRange.of(0, 23);
            case MONTH_OF_YEAR: return ValueRange.of(1, 12);
            case DAY_OF_MONTH: return ValueRange.of(1, 31);
            case DAY_OF_YEAR: return ValueRange.of(1, 366);
            case YEAR: return ValueRange.of(-999_999_999, 999_999_999);
            default: return ValueRange.of(-999_999_999L, 999_999_999L);
        }
    }

    @Override
    public boolean isDateBased() {
        return ordinal() >= DAY_OF_WEEK.ordinal() && this != INSTANT_SECONDS && this != OFFSET_SECONDS;
    }

    @Override
    public boolean isTimeBased() {
        return ordinal() <= AMPM_OF_DAY.ordinal();
    }

    public int checkValidIntValue(long value) {
        return range().checkValidIntValue(value, this);
    }
}
