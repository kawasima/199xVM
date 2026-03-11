package java.lang;

import java.io.Serializable;

public class Throwable implements Serializable {
    private String detailMessage;
    private Throwable cause = this;

    public Throwable() {}
    public Throwable(String message) { this.detailMessage = message; }
    public Throwable(String message, Throwable cause) {
        this.detailMessage = message;
        this.cause = cause;
    }
    public Throwable(Throwable cause) {
        this.detailMessage = cause == null ? null : cause.toString();
        this.cause = cause;
    }
    protected Throwable(String message, Throwable cause, boolean enableSuppression, boolean writableStackTrace) {
        this.detailMessage = message;
        this.cause = cause;
    }

    public String getMessage() { return detailMessage; }
    public synchronized Throwable getCause() { return (cause == this ? null : cause); }
    public synchronized Throwable initCause(Throwable cause) {
        if (this.cause != this) throw new IllegalStateException("Can't overwrite cause");
        if (cause == this) throw new IllegalArgumentException("Self-causation not permitted");
        this.cause = cause;
        return this;
    }
    final void setCause(Throwable cause) {
        this.cause = cause;
    }
    public String toString() {
        String s = getClass().getName();
        String message = getMessage();
        return (message != null) ? (s + ": " + message) : s;
    }
}
