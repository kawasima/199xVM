package java.lang.reflect;

public class UndeclaredThrowableException extends RuntimeException {
    @java.io.Serial
    static final long serialVersionUID = 330127114055056639L;

    public UndeclaredThrowableException(Throwable undeclaredThrowable) {
        super(null, undeclaredThrowable);
    }

    public UndeclaredThrowableException(Throwable undeclaredThrowable, String s) {
        super(s, undeclaredThrowable);
    }

    public Throwable getUndeclaredThrowable() {
        return super.getCause();
    }
}
