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

(defn- parse-test-selector [arg]
  (let [arg (str arg)
        slash (.lastIndexOf arg "/")]
    (if (neg? slash)
      {:namespace (symbol arg)}
      (let [namespace (subs arg 0 slash)
            test-var (subs arg (inc slash))]
        (when (or (empty? namespace) (empty? test-var))
          (throw (IllegalArgumentException.
                   (str "Invalid upstream test selector: " arg))))
        {:namespace (symbol namespace)
         :test-var (symbol test-var)}))))

(defn- selected-test-targets [args]
  (if (seq args)
    (mapv parse-test-selector args)
    (mapv (fn [namespace] {:namespace namespace}) default-test-namespaces)))

(defn- target-namespaces [targets]
  (->> targets
       (map :namespace)
       distinct
       vec))

(defn- selected-target-mode [targets]
  (cond
    (every? (comp nil? :test-var) targets) :namespaces
    (every? :test-var targets) :vars
    :else
    (throw (IllegalArgumentException.
             "Mixed namespace and test-var selectors are not supported"))))

(defn- configure-upstream-compat! []
  ;; Clojure's Reflector uses the Java 8 branch when this property is 1.8.
  ;; Keep that compatibility override local to the validation harness rather
  ;; than advertising it as the VM's global identity.
  (System/setProperty "java.specification.version" "1.8")
  (System/setProperty "java.vm.specification.version" "1.8"))

(defn- timing-ms [started-at]
  (/ (- (System/nanoTime) started-at) 1000000.0))

(defn- timing-enabled? []
  (contains? #{"1" "true" "TRUE" "yes" "YES"}
             (or (System/getenv "UPSTREAM_TIMING") "")))

(defn- log-timing! [label started-at]
  (when (timing-enabled?)
    (binding [*out* *err*]
      (println (format "timing %s %.2fms" label (timing-ms started-at)))
      (flush))))

(defn- require-target-namespaces! [targets]
  (doseq [namespace (target-namespaces targets)]
    (let [started-at (System/nanoTime)]
      (require namespace)
      (log-timing! (str "require " namespace) started-at))))

(defn- resolve-test-var [{:keys [namespace test-var]}]
  (let [qualified (symbol (str namespace) (str test-var))
        resolved (find-var qualified)]
    (when-not resolved
      (throw (IllegalArgumentException.
               (str "Unable to resolve upstream test var: " qualified))))
    (when-not (:test (meta resolved))
      (throw (IllegalArgumentException.
               (str qualified " is not a clojure.test deftest var"))))
    resolved))

(defn- run-selected-test-vars [targets]
  (let [test-vars (mapv resolve-test-var targets)
        counters (for [[ns-obj vars] (group-by (comp :ns meta) test-vars)]
                   (binding [t/*report-counters* (ref t/*initial-report-counters*)]
                     (t/do-report {:type :begin-test-ns :ns ns-obj})
                     (t/test-vars vars)
                     (t/do-report {:type :end-test-ns :ns ns-obj})
                     @t/*report-counters*))
        summary (assoc (apply merge-with + counters) :type :summary)]
    (t/do-report summary)
    summary))

(defn- run-selected-tests [args]
  (let [targets (selected-test-targets args)
        namespaces (target-namespaces targets)
        mode (selected-target-mode targets)]
    (configure-upstream-compat!)
    (doseq [namespace shared-test-namespaces]
      (let [started-at (System/nanoTime)]
        (require namespace)
        (log-timing! (str "require " namespace) started-at)))
    (require-target-namespaces! targets)
    (let [started-at (System/nanoTime)
          summary (case mode
                    :namespaces (apply t/run-tests namespaces)
                    :vars (run-selected-test-vars targets))]
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
