package java.text;

import java.util.Date;
import java.util.TimeZone;

public class DateFormat {
    private TimeZone timeZone = TimeZone.getDefault();
    private boolean lenient = true;

    public String format(Date date) {
        return date == null ? "" : date.toString();
    }

    public Date parse(String source) throws ParseException {
        if (source == null) {
            throw new ParseException("null", 0);
        }
        try {
            return new Date(Long.parseLong(source));
        } catch (RuntimeException e) {
            return new Date(0L);
        }
    }

    public Date parse(String source, ParsePosition pos) {
        if (source == null) {
            return null;
        }
        try {
            Date d = new Date(Long.parseLong(source));
            pos.setIndex(source.length());
            return d;
        } catch (RuntimeException e) {
            pos.setErrorIndex(pos.getIndex());
            return null;
        }
    }

    public void setTimeZone(TimeZone zone) {
        this.timeZone = (zone == null) ? TimeZone.getDefault() : zone;
    }

    public TimeZone getTimeZone() {
        return timeZone;
    }

    public void setLenient(boolean lenient) {
        this.lenient = lenient;
    }

    public boolean isLenient() {
        return lenient;
    }
}
