class BenchVirtualCall {
    static int run() {
        Adder adder = new Adder() {
            public int add(int a, int b) {
                return a + b;
            }
        };
        int s = 0;
        for (int i = 0; i < 1000; i++) {
            s = adder.add(s, i);
        }
        return s;
    }
}
