import java.time.LocalDateTime;

public class LocalDateTimeNowTest {
    public static String run() {
        LocalDateTime now = LocalDateTime.now();
        // Just verify it returns a non-null value with a reasonable year
        return now != null && now.getYear() >= 2024 ? "ok" : "fail";
    }
}
