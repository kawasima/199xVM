import java.util.PriorityQueue;

public class PriorityQueueTest {
    public static String run() {
        PriorityQueue<Integer> pq = new PriorityQueue<>();
        pq.offer(30);
        pq.offer(10);
        pq.offer(20);
        StringBuilder sb = new StringBuilder();
        while (!pq.isEmpty()) {
            if (sb.length() > 0) sb.append(",");
            sb.append(pq.poll());
        }
        return sb.toString();
    }
}
