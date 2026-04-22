import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

final class LoaderPhase0Fixtures {
    private LoaderPhase0Fixtures() {}

    static final class ExposedLoader extends ClassLoader {
        ExposedLoader() {
            super(null);
        }

        Class<?> define(String name, byte[] bytes) {
            return defineClass(name, bytes, 0, bytes.length);
        }

        Class<?> findLoaded(String name) {
            return findLoadedClass(name);
        }
    }

    static final class BytesLoader extends ClassLoader {
        private final String[] names;
        private final byte[][] bytes;

        BytesLoader(String[] names, byte[][] bytes) {
            super(null);
            this.names = names;
            this.bytes = bytes;
        }

        protected Class<?> findClass(String name) throws ClassNotFoundException {
            for (int i = 0; i < names.length; i++) {
                if (names[i].equals(name)) {
                    return defineClass(name, bytes[i], 0, bytes[i].length);
                }
            }
            throw new ClassNotFoundException(name);
        }

        Class<?> findLoaded(String name) {
            return findLoadedClass(name);
        }
    }

    static final class LinkageThrowingLoader extends ClassLoader {
        private final String failingName;
        private final String[] names;
        private final byte[][] bytes;

        LinkageThrowingLoader(String failingName, String[] names, byte[][] bytes) {
            super(null);
            this.failingName = failingName;
            this.names = names;
            this.bytes = bytes;
        }

        protected Class<?> findClass(String name) throws ClassNotFoundException {
            if (failingName.equals(name)) {
                throw new LinkageError("loader failure");
            }
            for (int i = 0; i < names.length; i++) {
                if (names[i].equals(name)) {
                    return defineClass(name, bytes[i], 0, bytes[i].length);
                }
            }
            throw new ClassNotFoundException(name);
        }
    }

    static String invokeString(Class<?> klass, String method) throws Exception {
        Method m = klass.getDeclaredMethod(method, new Class<?>[0]);
        Object value = m.invoke(null, new Object[0]);
        return (String) value;
    }

    static void invokeSetInt(Class<?> klass, int value) throws Exception {
        Method m = klass.getDeclaredMethod("set", new Class<?>[] { Integer.TYPE });
        m.invoke(null, new Object[] { Integer.valueOf(value) });
    }

    static String repeatedFailure(String callerName, String[] names, byte[][] bytes) throws Exception {
        BytesLoader loader = new BytesLoader(names, bytes);
        Class<?> caller = loader.loadClass(callerName);
        Method m = caller.getDeclaredMethod("run", new Class<?>[0]);
        String first = failureFamily(m);
        String second = failureFamily(m);
        return first.equals(second) ? first : new StringBuilder().append(first).append("!=").append(second).toString();
    }

    private static String failureFamily(Method method) {
        try {
            method.invoke(null, new Object[0]);
            return "none";
        } catch (InvocationTargetException e) {
            return simpleName(e.getCause());
        } catch (Throwable t) {
            return simpleName(t);
        }
    }

    static String simpleName(Throwable throwable) {
        if (throwable == null) {
            return "null";
        }
        String name = throwable.getClass().getName();
        int dot = name.lastIndexOf('.');
        return dot < 0 ? name : name.substring(dot + 1);
    }

    static byte[] bytes(String encoded) {
        int padding = 0;
        int length = encoded.length();
        if (length > 0 && encoded.charAt(length - 1) == '=') {
            padding++;
        }
        if (length > 1 && encoded.charAt(length - 2) == '=') {
            padding++;
        }

        byte[] out = new byte[(length * 3) / 4 - padding];
        int outIndex = 0;
        for (int i = 0; i < length; i += 4) {
            int block = (decode(encoded.charAt(i)) << 18)
                    | (decode(encoded.charAt(i + 1)) << 12)
                    | (decode(encoded.charAt(i + 2)) << 6)
                    | decode(encoded.charAt(i + 3));
            out[outIndex++] = (byte) (block >> 16);
            if (outIndex < out.length) {
                out[outIndex++] = (byte) (block >> 8);
            }
            if (outIndex < out.length) {
                out[outIndex++] = (byte) block;
            }
        }
        return out;
    }

    static byte[] bytesWithInvalidSuperClass(String encoded) {
        byte[] out = bytes(encoded);
        int p = 8;
        int cpCount = u16(out, p);
        p += 2;
        for (int i = 1; i < cpCount; i++) {
            int tag = out[p++] & 0xff;
            switch (tag) {
                case 1:
                    p += 2 + u16(out, p);
                    break;
                case 3:
                case 4:
                case 9:
                case 10:
                case 11:
                case 12:
                case 17:
                case 18:
                    p += 4;
                    break;
                case 5:
                case 6:
                    p += 8;
                    i++;
                    break;
                case 7:
                case 8:
                case 16:
                case 19:
                case 20:
                    p += 2;
                    break;
                case 15:
                    p += 3;
                    break;
                default:
                    throw new IllegalArgumentException("unsupported cp tag");
            }
        }
        p += 4;
        out[p] = (byte) 0xff;
        out[p + 1] = (byte) 0xff;
        return out;
    }

    private static int u16(byte[] bytes, int offset) {
        return ((bytes[offset] & 0xff) << 8) | (bytes[offset + 1] & 0xff);
    }

    private static int decode(char ch) {
        if (ch >= 'A' && ch <= 'Z') {
            return ch - 'A';
        }
        if (ch >= 'a' && ch <= 'z') {
            return ch - 'a' + 26;
        }
        if (ch >= '0' && ch <= '9') {
            return ch - '0' + 52;
        }
        if (ch == '+') {
            return 62;
        }
        if (ch == '/') {
            return 63;
        }
        return 0;
    }

    static final String SAME_NAME_B64 = "yv66vgAAADQAEQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQAEc2FtZQcACgEAD3BoYXNlMC9TYW1lTmFtZQEABENvZGUBAA9MaW5lTnVtYmVyVGFibGUBAAV0b2tlbgEAFCgpTGphdmEvbGFuZy9TdHJpbmc7AQAKU291cmNlRmlsZQEADVNhbWVOYW1lLmphdmEAIQAJAAIAAAAAAAIAAQAFAAYAAQALAAAAHQABAAEAAAAFKrcAAbEAAAABAAwAAAAGAAEAAAABAAkADQAOAAEACwAAABsAAQAAAAAAAxIHsAAAAAEADAAAAAYAAQAAAAEAAQAPAAAAAgAQ";
    static final String ISOLATED_B64 = "yv66vgAAADQAJgoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCQAIAAkHAAoMAAsADAEAD3BoYXNlMC9Jc29sYXRlZAEABXZhbHVlAQABSQcADgEAF2phdmEvbGFuZy9TdHJpbmdCdWlsZGVyCgANAAMKAA0AEQwAEgATAQAGYXBwZW5kAQAcKEkpTGphdmEvbGFuZy9TdHJpbmdCdWlsZGVyOwoADQAVDAASABYBABwoQylMamF2YS9sYW5nL1N0cmluZ0J1aWxkZXI7CQAIABgMABkADAEABWluaXRzCgANABsMABwAHQEACHRvU3RyaW5nAQAUKClMamF2YS9sYW5nL1N0cmluZzsBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQADc2V0AQAEKEkpVgEACHNuYXBzaG90AQAIPGNsaW5pdD4BAApTb3VyY2VGaWxlAQANSXNvbGF0ZWQuamF2YQAhAAgAAgAAAAIACgALAAwAAAAKABkADAAAAAQAAQAFAAYAAQAeAAAAHQABAAEAAAAFKrcAAbEAAAABAB8AAAAGAAEAAAABAAkAIAAhAAEAHgAAAB0AAQABAAAABRqzAAexAAAAAQAfAAAABgABAAAAAQAJACIAHQABAB4AAAA0AAIAAAAAABy7AA1ZtwAPsgAHtgAQEDq2ABSyABe2ABC2ABqwAAAAAQAfAAAABgABAAAAAQAIACMABgABAB4AAAAlAAIAAAAAAA0EswAHsgAXBGCzABexAAAAAQAfAAAABgABAAAAAQABACQAAAACACU=";
    static final String CALLER_B64 = "yv66vgAAADQAFAoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCgAIAAkHAAoMAAsADAEAD3BoYXNlMC9SZXNvbHZlZAEABXZhbHVlAQAUKClMamF2YS9sYW5nL1N0cmluZzsHAA4BAA1waGFzZTAvQ2FsbGVyAQAEQ29kZQEAD0xpbmVOdW1iZXJUYWJsZQEABGNhbGwBAApTb3VyY2VGaWxlAQALQ2FsbGVyLmphdmEAIQANAAIAAAAAAAIAAQAFAAYAAQAPAAAAHQABAAEAAAAFKrcAAbEAAAABABAAAAAGAAEAAAABAAkAEQAMAAEADwAAABwAAQAAAAAABLgAB7AAAAABABAAAAAGAAEAAAABAAEAEgAAAAIAEw==";
    static final String STRING_ARRAY_CALLER_B64 = "yv66vgAAADQAGgoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAQamF2YS9sYW5nL1N0cmluZwoAAgAKDAALAAwBAAhnZXRDbGFzcwEAEygpTGphdmEvbGFuZy9DbGFzczsKAA4ADwcAEAwAEQASAQAPamF2YS9sYW5nL0NsYXNzAQAHZ2V0TmFtZQEAFCgpTGphdmEvbGFuZy9TdHJpbmc7BwAUAQAYcGhhc2UwL1N0cmluZ0FycmF5Q2FsbGVyAQAEQ29kZQEAD0xpbmVOdW1iZXJUYWJsZQEAA3J1bgEAClNvdXJjZUZpbGUBABZTdHJpbmdBcnJheUNhbGxlci5qYXZhACEAEwACAAAAAAACAAEABQAGAAEAFQAAAB0AAQABAAAABSq3AAGxAAAAAQAWAAAABgABAAAAAgAJABcAEgABABUAAAApAAEAAQAAAA0EvQAHSyq2AAm2AA2wAAAAAQAWAAAACgACAAAABAAFAAUAAQAYAAAAAgAZ";
    static final String RESOLVED_LEFT_B64 = "yv66vgAAADQAEQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQAEbGVmdAcACgEAD3BoYXNlMC9SZXNvbHZlZAEABENvZGUBAA9MaW5lTnVtYmVyVGFibGUBAAV2YWx1ZQEAFCgpTGphdmEvbGFuZy9TdHJpbmc7AQAKU291cmNlRmlsZQEADVJlc29sdmVkLmphdmEAIQAJAAIAAAAAAAIAAQAFAAYAAQALAAAAHQABAAEAAAAFKrcAAbEAAAABAAwAAAAGAAEAAAABAAkADQAOAAEACwAAABsAAQAAAAAAAxIHsAAAAAEADAAAAAYAAQAAAAEAAQAPAAAAAgAQ";
    static final String RESOLVED_RIGHT_B64 = "yv66vgAAADQAEQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQAFcmlnaHQHAAoBAA9waGFzZTAvUmVzb2x2ZWQBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQAFdmFsdWUBABQoKUxqYXZhL2xhbmcvU3RyaW5nOwEAClNvdXJjZUZpbGUBAA1SZXNvbHZlZC5qYXZhACEACQACAAAAAAACAAEABQAGAAEACwAAAB0AAQABAAAABSq3AAGxAAAAAQAMAAAABgABAAAAAQAJAA0ADgABAAsAAAAbAAEAAAAAAAMSB7AAAAABAAwAAAAGAAEAAAABAAEADwAAAAIAEA==";
    static final String MEMBER_CALLER_B64 = "yv66vgAAADQAJQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAXamF2YS9sYW5nL1N0cmluZ0J1aWxkZXIKAAcAAwoACwAMBwANDAAOAA8BAAxwaGFzZTAvT3duZXIBAAV2YWx1ZQEAFCgpTGphdmEvbGFuZy9TdHJpbmc7CgAHABEMABIAEwEABmFwcGVuZAEALShMamF2YS9sYW5nL1N0cmluZzspTGphdmEvbGFuZy9TdHJpbmdCdWlsZGVyOwoABwAVDAASABYBABwoQylMamF2YS9sYW5nL1N0cmluZ0J1aWxkZXI7CQALABgMABkAGgEABVZBTFVFAQASTGphdmEvbGFuZy9TdHJpbmc7CgAHABwMAB0ADwEACHRvU3RyaW5nBwAfAQATcGhhc2UwL01lbWJlckNhbGxlcgEABENvZGUBAA9MaW5lTnVtYmVyVGFibGUBAARyZWFkAQAKU291cmNlRmlsZQEAEU1lbWJlckNhbGxlci5qYXZhACEAHgACAAAAAAACAAEABQAGAAEAIAAAAB0AAQABAAAABSq3AAGxAAAAAQAhAAAABgABAAAAAQAJACIADwABACAAAAA0AAIAAAAAABy7AAdZtwAJuAAKtgAQEHy2ABSyABe2ABC2ABuwAAAAAQAhAAAABgABAAAAAQABACMAAAACACQ=";
    static final String OWNER_LEFT_B64 = "yv66vgAAADQAGAoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQALbGVmdC1tZXRob2QIAAoBAApsZWZ0LWZpZWxkCQAMAA0HAA4MAA8AEAEADHBoYXNlMC9Pd25lcgEABVZBTFVFAQASTGphdmEvbGFuZy9TdHJpbmc7AQAEQ29kZQEAD0xpbmVOdW1iZXJUYWJsZQEABXZhbHVlAQAUKClMamF2YS9sYW5nL1N0cmluZzsBAAg8Y2xpbml0PgEAClNvdXJjZUZpbGUBAApPd25lci5qYXZhACEADAACAAAAAQAJAA8AEAAAAAMAAQAFAAYAAQARAAAAHQABAAEAAAAFKrcAAbEAAAABABIAAAAGAAEAAAABAAkAEwAUAAEAEQAAABsAAQAAAAAAAxIHsAAAAAEAEgAAAAYAAQAAAAEACAAVAAYAAQARAAAAHgABAAAAAAAGEgmzAAuxAAAAAQASAAAABgABAAAAAQABABYAAAACABc=";
    static final String OWNER_RIGHT_B64 = "yv66vgAAADQAGAoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQAMcmlnaHQtbWV0aG9kCAAKAQALcmlnaHQtZmllbGQJAAwADQcADgwADwAQAQAMcGhhc2UwL093bmVyAQAFVkFMVUUBABJMamF2YS9sYW5nL1N0cmluZzsBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQAFdmFsdWUBABQoKUxqYXZhL2xhbmcvU3RyaW5nOwEACDxjbGluaXQ+AQAKU291cmNlRmlsZQEACk93bmVyLmphdmEAIQAMAAIAAAABAAkADwAQAAAAAwABAAUABgABABEAAAAdAAEAAQAAAAUqtwABsQAAAAEAEgAAAAYAAQAAAAEACQATABQAAQARAAAAGwABAAAAAAADEgewAAAAAQASAAAABgABAAAAAQAIABUABgABABEAAAAeAAEAAAAAAAYSCbMAC7EAAAABABIAAAAGAAEAAAABAAEAFgAAAAIAFw==";
    static final String MISSING_CLASS_CALLER_B64 = "yv66vgAAADQAFQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAYcGhhc2UwL01pc3NpbmdEZXBlbmRlbmN5CgAHAAMKAAIACwwADAANAQAIdG9TdHJpbmcBABQoKUxqYXZhL2xhbmcvU3RyaW5nOwcADwEAGXBoYXNlMC9NaXNzaW5nQ2xhc3NDYWxsZXIBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQADcnVuAQAKU291cmNlRmlsZQEAF01pc3NpbmdDbGFzc0NhbGxlci5qYXZhACEADgACAAAAAAACAAEABQAGAAEAEAAAAB0AAQABAAAABSq3AAGxAAAAAQARAAAABgABAAAAAQAJABIADQABABAAAAAjAAIAAAAAAAu7AAdZtwAJtgAKsAAAAAEAEQAAAAYAAQAAAAEAAQATAAAAAgAU";
    static final String MISSING_FIELD_CALLER_B64 = "yv66vgAAADQAFQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCQAIAAkHAAoMAAsADAEAGHBoYXNlMC9NaXNzaW5nRmllbGRPd25lcgEABXZhbHVlAQABSQcADgEAGXBoYXNlMC9NaXNzaW5nRmllbGRDYWxsZXIBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQADcnVuAQADKClJAQAKU291cmNlRmlsZQEAF01pc3NpbmdGaWVsZENhbGxlci5qYXZhACEADQACAAAAAAACAAEABQAGAAEADwAAAB0AAQABAAAABSq3AAGxAAAAAQAQAAAABgABAAAAAQAJABEAEgABAA8AAAAcAAEAAAAAAASyAAesAAAAAQAQAAAABgABAAAAAQABABMAAAACABQ=";
    static final String MISSING_METHOD_CALLER_B64 = "yv66vgAAADQAFAoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCgAIAAkHAAoMAAsADAEAGXBoYXNlMC9NaXNzaW5nTWV0aG9kT3duZXIBAAV2YWx1ZQEAFCgpTGphdmEvbGFuZy9TdHJpbmc7BwAOAQAacGhhc2UwL01pc3NpbmdNZXRob2RDYWxsZXIBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQADcnVuAQAKU291cmNlRmlsZQEAGE1pc3NpbmdNZXRob2RDYWxsZXIuamF2YQAhAA0AAgAAAAAAAgABAAUABgABAA8AAAAdAAEAAQAAAAUqtwABsQAAAAEAEAAAAAYAAQAAAAEACQARAAwAAQAPAAAAHAABAAAAAAAEuAAHsAAAAAEAEAAAAAYAAQAAAAEAAQASAAAAAgAT";
    static final String MISSING_INTERFACE_CALLER_B64 = "yv66vgAAADQAFwoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAbcGhhc2UwL01pc3NpbmdJbnRlcmZhY2VJbXBsCgAHAAMLAAsADAcADQwADgAPAQAXcGhhc2UwL01pc3NpbmdJbnRlcmZhY2UBAAV2YWx1ZQEAFCgpTGphdmEvbGFuZy9TdHJpbmc7BwARAQAdcGhhc2UwL01pc3NpbmdJbnRlcmZhY2VDYWxsZXIBAARDb2RlAQAPTGluZU51bWJlclRhYmxlAQADcnVuAQAKU291cmNlRmlsZQEAG01pc3NpbmdJbnRlcmZhY2VDYWxsZXIuamF2YQAhABAAAgAAAAAAAgABAAUABgABABIAAAAdAAEAAQAAAAUqtwABsQAAAAEAEwAAAAYAAQAAAAEACQAUAA8AAQASAAAAJwACAAEAAAAPuwAHWbcACUsquQAKAQCwAAAAAQATAAAABgABAAAAAQABABUAAAACABY=";
    static final String MISSING_FIELD_OWNER_NO_FIELD_B64 = "yv66vgAAADQADQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAYcGhhc2UwL01pc3NpbmdGaWVsZE93bmVyAQAEQ29kZQEAD0xpbmVOdW1iZXJUYWJsZQEAClNvdXJjZUZpbGUBABZNaXNzaW5nRmllbGRPd25lci5qYXZhACEABwACAAAAAAABAAEABQAGAAEACQAAAB0AAQABAAAABSq3AAGxAAAAAQAKAAAABgABAAAAAQABAAsAAAACAAw=";
    static final String MISSING_METHOD_OWNER_NO_METHOD_B64 = "yv66vgAAADQADQoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWBwAIAQAZcGhhc2UwL01pc3NpbmdNZXRob2RPd25lcgEABENvZGUBAA9MaW5lTnVtYmVyVGFibGUBAApTb3VyY2VGaWxlAQAXTWlzc2luZ01ldGhvZE93bmVyLmphdmEAIQAHAAIAAAAAAAEAAQAFAAYAAQAJAAAAHQABAAEAAAAFKrcAAbEAAAABAAoAAAAGAAEAAAABAAEACwAAAAIADA==";
    static final String MISSING_INTERFACE_NO_METHOD_B64 = "yv66vgAAADQABwcAAgEAF3BoYXNlMC9NaXNzaW5nSW50ZXJmYWNlBwAEAQAQamF2YS9sYW5nL09iamVjdAEAClNvdXJjZUZpbGUBABVNaXNzaW5nSW50ZXJmYWNlLmphdmEGAQABAAMAAAAAAAAAAQAFAAAAAgAG";
    static final String MISSING_INTERFACE_IMPL_B64 = "yv66vgAAADQAEwoAAgADBwAEDAAFAAYBABBqYXZhL2xhbmcvT2JqZWN0AQAGPGluaXQ+AQADKClWCAAIAQAEaW1wbAcACgEAG3BoYXNlMC9NaXNzaW5nSW50ZXJmYWNlSW1wbAcADAEAF3BoYXNlMC9NaXNzaW5nSW50ZXJmYWNlAQAEQ29kZQEAD0xpbmVOdW1iZXJUYWJsZQEABXZhbHVlAQAUKClMamF2YS9sYW5nL1N0cmluZzsBAApTb3VyY2VGaWxlAQAZTWlzc2luZ0ludGVyZmFjZUltcGwuamF2YQAhAAkAAgABAAsAAAACAAEABQAGAAEADQAAAB0AAQABAAAABSq3AAGxAAAAAQAOAAAABgABAAAAAQABAA8AEAABAA0AAAAbAAEAAQAAAAMSB7AAAAABAA4AAAAGAAEAAAABAAEAEQAAAAIAEg==";
}
