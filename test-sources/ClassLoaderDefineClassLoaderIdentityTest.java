public class ClassLoaderDefineClassLoaderIdentityTest {
    private static final byte[] DEFINE_PROBE_CLASS = new byte[] {
        (byte) 0xca, (byte) 0xfe, (byte) 0xba, (byte) 0xbe, 0x00, 0x00, 0x00, 0x34, 0x00, 0x0f,
        0x0a, 0x00, 0x02, 0x00, 0x03, 0x07, 0x00, 0x04, 0x0c, 0x00, 0x05, 0x00, 0x06, 0x01,
        0x00, 0x10, 0x6a, 0x61, 0x76, 0x61, 0x2f, 0x6c, 0x61, 0x6e, 0x67, 0x2f, 0x4f, 0x62,
        0x6a, 0x65, 0x63, 0x74, 0x01, 0x00, 0x06, 0x3c, 0x69, 0x6e, 0x69, 0x74, 0x3e, 0x01,
        0x00, 0x03, 0x28, 0x29, 0x56, 0x07, 0x00, 0x08, 0x01, 0x00, 0x0b, 0x44, 0x65, 0x66,
        0x69, 0x6e, 0x65, 0x50, 0x72, 0x6f, 0x62, 0x65, 0x01, 0x00, 0x04, 0x43, 0x6f, 0x64,
        0x65, 0x01, 0x00, 0x0f, 0x4c, 0x69, 0x6e, 0x65, 0x4e, 0x75, 0x6d, 0x62, 0x65, 0x72,
        0x54, 0x61, 0x62, 0x6c, 0x65, 0x01, 0x00, 0x05, 0x76, 0x61, 0x6c, 0x75, 0x65, 0x01,
        0x00, 0x03, 0x28, 0x29, 0x49, 0x01, 0x00, 0x0a, 0x53, 0x6f, 0x75, 0x72, 0x63, 0x65,
        0x46, 0x69, 0x6c, 0x65, 0x01, 0x00, 0x10, 0x44, 0x65, 0x66, 0x69, 0x6e, 0x65, 0x50,
        0x72, 0x6f, 0x62, 0x65, 0x2e, 0x6a, 0x61, 0x76, 0x61, 0x00, 0x21, 0x00, 0x07, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x05, 0x00, 0x06, 0x00,
        0x01, 0x00, 0x09, 0x00, 0x00, 0x00, 0x1d, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x05, 0x2a, (byte) 0xb7, 0x00, 0x01, (byte) 0xb1, 0x00, 0x00, 0x00, 0x01, 0x00, 0x0a,
        0x00, 0x00, 0x00, 0x06, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x09, 0x00, 0x0b,
        0x00, 0x0c, 0x00, 0x01, 0x00, 0x09, 0x00, 0x00, 0x00, 0x1b, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x03, 0x10, 0x2a, (byte) 0xac, 0x00, 0x00, 0x00, 0x01, 0x00, 0x0a,
        0x00, 0x00, 0x00, 0x06, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x0d,
        0x00, 0x00, 0x00, 0x02, 0x00, 0x0e
    };

    private static final byte[] STATIC_PROBE_CLASS = bytes(
        202, 254, 186, 190, 0, 0, 0, 52, 0, 21, 10, 0, 2, 0, 3, 7,
        0, 4, 12, 0, 5, 0, 6, 1, 0, 16, 106, 97, 118, 97, 47, 108,
        97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 1, 0, 6, 60, 105, 110,
        105, 116, 62, 1, 0, 3, 40, 41, 86, 9, 0, 8, 0, 9, 7, 0,
        10, 12, 0, 11, 0, 12, 1, 0, 11, 83, 116, 97, 116, 105, 99, 80,
        114, 111, 98, 101, 1, 0, 4, 114, 111, 111, 116, 1, 0, 18, 76, 106,
        97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98, 106, 101, 99, 116, 59,
        1, 0, 4, 67, 111, 100, 101, 1, 0, 15, 76, 105, 110, 101, 78, 117,
        109, 98, 101, 114, 84, 97, 98, 108, 101, 1, 0, 4, 114, 101, 97, 100,
        1, 0, 20, 40, 41, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47,
        79, 98, 106, 101, 99, 116, 59, 1, 0, 5, 119, 114, 105, 116, 101, 1,
        0, 21, 40, 76, 106, 97, 118, 97, 47, 108, 97, 110, 103, 47, 79, 98,
        106, 101, 99, 116, 59, 41, 86, 1, 0, 10, 83, 111, 117, 114, 99, 101,
        70, 105, 108, 101, 1, 0, 16, 83, 116, 97, 116, 105, 99, 80, 114, 111,
        98, 101, 46, 106, 97, 118, 97, 0, 33, 0, 8, 0, 2, 0, 0, 0,
        1, 0, 9, 0, 11, 0, 12, 0, 0, 0, 3, 0, 1, 0, 5, 0,
        6, 0, 1, 0, 13, 0, 0, 0, 29, 0, 1, 0, 1, 0, 0, 0,
        5, 42, 183, 0, 1, 177, 0, 0, 0, 1, 0, 14, 0, 0, 0, 6,
        0, 1, 0, 0, 0, 1, 0, 9, 0, 15, 0, 16, 0, 1, 0, 13,
        0, 0, 0, 28, 0, 1, 0, 0, 0, 0, 0, 4, 178, 0, 7, 176,
        0, 0, 0, 1, 0, 14, 0, 0, 0, 6, 0, 1, 0, 0, 0, 4,
        0, 9, 0, 17, 0, 18, 0, 1, 0, 13, 0, 0, 0, 33, 0, 1,
        0, 1, 0, 0, 0, 5, 42, 179, 0, 7, 177, 0, 0, 0, 1, 0,
        14, 0, 0, 0, 10, 0, 2, 0, 0, 0, 7, 0, 4, 0, 8, 0,
        1, 0, 19, 0, 0, 0, 2, 0, 20
    );

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < values.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    private static final class ExposedLoader extends ClassLoader {
        private Class<?> defined;
        private Class<?> staticProbe;

        Class<?> defineUnnamed(byte[] bytes) {
            defined = defineClass(null, bytes, 0, bytes.length);
            return defined;
        }

        Class<?> defineStaticProbe() {
            staticProbe = defineClass(null, STATIC_PROBE_CLASS, 0, STATIC_PROBE_CLASS.length);
            return staticProbe;
        }

        @Override
        protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
            if ("DefineProbe".equals(name) && defined != null) {
                return defined;
            }
            if ("StaticProbe".equals(name) && staticProbe != null) {
                return staticProbe;
            }
            return super.loadClass(name, resolve);
        }
    }

    public static String run() {
        ExposedLoader loader1 = new ExposedLoader();
        ExposedLoader loader2 = new ExposedLoader();

        Class<?> class1 = loader1.defineUnnamed(DEFINE_PROBE_CLASS);
        Class<?> class2 = loader2.defineUnnamed(DEFINE_PROBE_CLASS);

        if (class1 == class2) {
            return "same-class-object";
        }
        if (!class1.getName().equals(class2.getName())) {
            return "different-name";
        }
        if (class1.getClassLoader() != loader1 || class2.getClassLoader() != loader2) {
            return "wrong-defining-loader";
        }
        try {
            if (Class.forName("DefineProbe", false, loader1) != class1) {
                return "forName-wrong-loader-class";
            }
        } catch (ClassNotFoundException e) {
            return "forName-cnfe";
        }
        if (class1.isAssignableFrom(class2) || class2.isAssignableFrom(class1)) {
            return "assignable-across-loaders";
        }
        try {
            Class<?> staticClass1 = loader1.defineStaticProbe();
            Class<?> staticClass2 = loader2.defineStaticProbe();
            Object marker1 = loader1;
            Object marker2 = loader2;
            staticClass1.getDeclaredMethod("write", Object.class).invoke(null, marker1);
            staticClass2.getDeclaredMethod("write", Object.class).invoke(null, marker2);
            Object read1 = staticClass1.getDeclaredMethod("read").invoke(null);
            Object read2 = staticClass2.getDeclaredMethod("read").invoke(null);
            if (read1 != marker1 || read2 != marker2) {
                return "static-field-cross-loader-leak";
            }
        } catch (Exception e) {
            return "static-field-error";
        }
        return "ok";
    }
}
