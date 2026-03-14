import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.WeakReference;

public class ReferenceQueueTest {
    public static String run() {
        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object referent = new Object();
        WeakReference<Object> ref = new WeakReference<>(referent, queue);

        if (queue.poll() != null) return "poll:not-empty";
        if (ref.get() != referent) return "get:initial";
        if (!ref.refersTo(referent)) return "refersTo:initial";
        if (ref.isEnqueued()) return "enqueued:initial";

        if (!ref.enqueue()) return "enqueue:false";
        if (!ref.isEnqueued()) return "enqueued:after-enqueue";

        Reference<? extends Object> polled = queue.poll();
        if (polled != ref) return "poll:identity";
        if (ref.isEnqueued()) return "enqueued:after-poll";
        if (queue.poll() != null) return "poll:not-drained";

        ref.clear();
        if (ref.get() != null) return "get:cleared";
        if (ref.refersTo(referent)) return "refersTo:old";
        if (!ref.refersTo(null)) return "refersTo:null";

        return "ok";
    }
}
