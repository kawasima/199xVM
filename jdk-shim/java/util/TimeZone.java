package java.util;

import java.io.Serializable;

public class TimeZone implements Serializable, Cloneable {
    private String ID;

    public TimeZone() {
        this("UTC");
    }

    protected TimeZone(String id) {
        this.ID = (id == null) ? "UTC" : id;
    }

    public static TimeZone getTimeZone(String ID) {
        return new TimeZone(ID);
    }

    public static TimeZone getDefault() {
        return new TimeZone("UTC");
    }

    public String getID() {
        return ID;
    }

    public void setID(String ID) {
        this.ID = (ID == null) ? "UTC" : ID;
    }

    public int getRawOffset() {
        return 0;
    }

    public int getOffset(long date) {
        return 0;
    }

    public boolean useDaylightTime() {
        return false;
    }

    public boolean inDaylightTime(Date date) {
        return false;
    }

    @Override
    public Object clone() {
        return new TimeZone(ID);
    }
}
