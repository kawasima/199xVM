package java.lang;

public class Runtime {
    private static final Runtime currentRuntime = new Runtime();

    private Runtime() {}

    public static Runtime getRuntime() {
        return currentRuntime;
    }

    public void gc() {}

    public long freeMemory() {
        return 0L;
    }

    public long totalMemory() {
        return 0L;
    }

    public long maxMemory() {
        return 9223372036854775807L;
    }

    public int availableProcessors() {
        return 1;
    }
}
