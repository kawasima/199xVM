(ns upstream.runner
  (:require [clojure.test :as t])
  (:gen-class
    :name ClojureUpstreamTestEntry
    :main true
    :methods [^{:static true} [run [] String]]
    :prefix "entry-"))

(def default-test-namespaces
  '[clojure.test-clojure.atoms
    clojure.test-clojure.logic
    clojure.test-clojure.try-catch
    ])

(def shared-test-namespaces
  '[clojure.test-helper])

(defn- selected-test-namespaces [args]
  (if (seq args)
    (mapv symbol args)
    (vec default-test-namespaces)))

(defn- configure-upstream-compat! []
  ;; Clojure's Reflector uses the Java 8 branch when this property is 1.8.
  ;; Keep that compatibility override local to the validation harness rather
  ;; than advertising it as the VM's global identity.
  (System/setProperty "java.specification.version" "1.8")
  (System/setProperty "java.vm.specification.version" "1.8"))

(defn- timing-ms [started-at]
  (/ (- (System/nanoTime) started-at) 1000000.0))

(defn- log-timing! [label started-at]
  (binding [*out* *err*]
    (println (format "timing %s %.2fms" label (timing-ms started-at)))
    (flush)))

(defn- run-selected-tests [args]
  (let [namespaces (selected-test-namespaces args)]
    (configure-upstream-compat!)
    (doseq [namespace shared-test-namespaces]
      (let [started-at (System/nanoTime)]
        (require namespace)
        (log-timing! (str "require " namespace) started-at)))
    (doseq [namespace namespaces]
      (let [started-at (System/nanoTime)]
        (require namespace)
        (log-timing! (str "require " namespace) started-at)))
    (let [started-at (System/nanoTime)
          summary (apply t/run-tests namespaces)]
      (log-timing! "run-tests" started-at)
      {:namespaces namespaces
       :summary summary
       :successful? (t/successful? summary)})))

(defn- summary-line [{:keys [namespaces summary successful?]}]
  (format "%s namespaces=%d test-vars=%d pass=%d fail=%d error=%d"
          (if successful? "ok" "fail")
          (count namespaces)
          (long (or (:test summary) 0))
          (long (or (:pass summary) 0))
          (long (or (:fail summary) 0))
          (long (or (:error summary) 0))))

(defn entry-run []
  (summary-line (run-selected-tests [])))

(defn entry-main [& args]
  (let [result (run-selected-tests args)
        line (summary-line result)]
    (println line)
    (flush)
    (System/exit (if (:successful? result) 0 1))))
