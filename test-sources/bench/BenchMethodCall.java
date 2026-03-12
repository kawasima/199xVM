class BenchMethodCall {
    static int add(int a, int b) {
        return a + b;
    }

    static int run() {
        int s = 0;
        for (int i = 0; i < 1000; i++) {
            s = add(s, i);
        }
        return s;
    }
}
