class BenchStaticField {
    static int counter = 0;

    static int run() {
        for (int i = 0; i < 1000; i++) {
            counter += i;
        }
        return counter;
    }
}
