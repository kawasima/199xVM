public class ClassMirrorIdentityTest {
    static class Base {
    }

    static class Sample extends Base implements java.io.Serializable {
        public String ping() {
            return "ok";
        }
    }

    public static String run() throws Exception {
        Class<?> fromName = Class.forName("ClassMirrorIdentityTest$Sample");
        Class<?> literal = Sample.class;
        Class<?> fromObject = new Sample().getClass();
        boolean sameMirror = fromName == literal && literal == fromObject;
        boolean assignable = Base.class.isAssignableFrom(fromObject);
        boolean sawPing = false;
        for (java.lang.reflect.Method method : fromObject.getDeclaredMethods()) {
            if ("ping".equals(method.getName())) {
                sawPing = true;
                break;
            }
        }
        return sameMirror
            + "|"
            + fromObject.getSuperclass().getName()
            + "|"
            + fromObject.getInterfaces().length
            + "|"
            + assignable
            + "|"
            + sawPing;
    }
}
