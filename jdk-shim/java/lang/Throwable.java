package java.lang;

import java.io.Serializable;

public class Throwable implements Serializable {
    private String detailMessage;

    public Throwable() {}
    public Throwable(String message) { this.detailMessage = message; }
    public Throwable(String message, Throwable cause) { this.detailMessage = message; }

    public String getMessage() { return detailMessage; }
    public String toString() {
        String s = getClass().getName();
        String message = getMessage();
        return (message != null) ? (s + ": " + message) : s;
    }
}
