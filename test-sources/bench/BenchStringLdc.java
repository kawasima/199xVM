class BenchStringLdc {
    static String run() {
        String s = "";
        for (int i = 0; i < 1000; i++) {
            s = "hello";
        }
        return s;
    }
}
