package java.util;

import java.io.Serializable;

public class Date implements Serializable, Cloneable, Comparable<Date> {
    private long fastTime;

    public Date() {
        this(System.currentTimeMillis());
    }

    public Date(long date) {
        this.fastTime = date;
    }

    public long getTime() {
        return fastTime;
    }

    public void setTime(long time) {
        this.fastTime = time;
    }

    public boolean before(Date when) {
        return this.fastTime < when.fastTime;
    }

    public boolean after(Date when) {
        return this.fastTime > when.fastTime;
    }

    @Override
    public int compareTo(Date anotherDate) {
        long thisTime = this.fastTime;
        long anotherTime = anotherDate.fastTime;
        return (thisTime < anotherTime ? -1 : (thisTime == anotherTime ? 0 : 1));
    }

    @Override
    public Object clone() {
        return new Date(fastTime);
    }

    @Override
    public String toString() {
        return Long.toString(fastTime);
    }
}
