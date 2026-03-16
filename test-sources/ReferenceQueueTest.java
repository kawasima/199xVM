import java.lang.ref.Reference;
import java.lang.ref.ReferenceQueue;
import java.lang.ref.WeakReference;
import java.lang.ref.SoftReference;

public class ReferenceQueueTest {
    public static String run() {
        String r;
        r = testBasicLifecycle();     if (!r.equals("ok")) return "basic:" + r;
        r = testGetAfterEnqueue();    if (!r.equals("ok")) return "getAfterEnqueue:" + r;
        r = testDoubleEnqueue();      if (!r.equals("ok")) return "doubleEnqueue:" + r;
        r = testNoQueue();            if (!r.equals("ok")) return "noQueue:" + r;
        r = testSoftReference();      if (!r.equals("ok")) return "soft:" + r;
        r = testMultipleRefs();       if (!r.equals("ok")) return "multi:" + r;
        return "ok";
    }

    /** Original lifecycle test: enqueue → poll → clear. */
    static String testBasicLifecycle() {
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

    /** enqueue() must NOT clear the referent — get() should still work. */
    static String testGetAfterEnqueue() {
        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object referent = new Object();
        WeakReference<Object> ref = new WeakReference<>(referent, queue);

        ref.enqueue();
        // JDK contract: enqueue does not clear
        if (ref.get() != referent) return "get-null-after-enqueue";
        if (!ref.refersTo(referent)) return "refersTo-after-enqueue";

        // Only explicit clear should null the referent
        ref.clear();
        if (ref.get() != null) return "get-not-null-after-clear";
        return "ok";
    }

    /** Second enqueue() returns false; queue stays consistent. */
    static String testDoubleEnqueue() {
        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object referent = new Object();
        WeakReference<Object> ref = new WeakReference<>(referent, queue);

        if (!ref.enqueue()) return "first-enqueue-false";
        if (ref.enqueue()) return "second-enqueue-true";
        // Only one entry in the queue
        if (queue.poll() != ref) return "poll-identity";
        if (queue.poll() != null) return "poll-not-drained";
        return "ok";
    }

    /** WeakReference without a queue: enqueue() returns false. */
    static String testNoQueue() {
        Object referent = new Object();
        WeakReference<Object> ref = new WeakReference<>(referent);

        if (ref.get() != referent) return "get-initial";
        if (ref.enqueue()) return "enqueue-true-no-queue";
        ref.clear();
        if (ref.get() != null) return "get-not-null-after-clear";
        return "ok";
    }

    /** SoftReference basic behavior. */
    static String testSoftReference() {
        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object referent = new Object();
        SoftReference<Object> ref = new SoftReference<>(referent, queue);

        if (ref.get() != referent) return "get-initial";
        ref.enqueue();
        if (ref.get() != referent) return "get-null-after-enqueue";
        ref.clear();
        if (ref.get() != null) return "get-not-null-after-clear";
        return "ok";
    }

    /** Multiple references enqueued to the same queue. */
    static String testMultipleRefs() {
        ReferenceQueue<Object> queue = new ReferenceQueue<>();
        Object a = new Object();
        Object b = new Object();
        WeakReference<Object> refA = new WeakReference<>(a, queue);
        WeakReference<Object> refB = new WeakReference<>(b, queue);

        refA.enqueue();
        refB.enqueue();

        // Both should be pollable (LIFO order in current impl)
        Reference<? extends Object> first = queue.poll();
        Reference<? extends Object> second = queue.poll();
        if (first == null || second == null) return "poll-null";
        if (first == second) return "poll-same";
        if (queue.poll() != null) return "poll-not-drained";
        // Both referents should still be alive
        if (refA.get() != a) return "refA-get";
        if (refB.get() != b) return "refB-get";
        return "ok";
    }
}
