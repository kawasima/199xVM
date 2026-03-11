package java.lang.annotation;

public interface Annotation {
    default Class<? extends Annotation> annotationType() {
        @SuppressWarnings("unchecked")
        Class<? extends Annotation> t = (Class<? extends Annotation>) this.getClass();
        return t;
    }
}
