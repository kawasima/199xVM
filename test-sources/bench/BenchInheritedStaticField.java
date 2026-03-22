class BenchInheritedStaticFieldBase {
    static int counter = 0;
}

class BenchInheritedStaticField extends BenchInheritedStaticFieldBase {
    static int run() {
        for (int i = 0; i < 1000; i++) {
            BenchInheritedStaticField.counter += i;
        }
        return BenchInheritedStaticFieldBase.counter;
    }
}
