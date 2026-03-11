package java.time;

public enum Month {
    JANUARY(1),
    FEBRUARY(2),
    MARCH(3),
    APRIL(4),
    MAY(5),
    JUNE(6),
    JULY(7),
    AUGUST(8),
    SEPTEMBER(9),
    OCTOBER(10),
    NOVEMBER(11),
    DECEMBER(12);

    private final int value;

    Month(int value) {
        this.value = value;
    }

    public int getValue() {
        return value;
    }

    public static Month of(int month) {
        if (month < 1 || month > 12) {
            throw new RuntimeException("Invalid month: " + month);
        }
        return values()[month - 1];
    }
}
