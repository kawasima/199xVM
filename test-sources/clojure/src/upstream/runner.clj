(ns upstream.runner
  (:require [clojure.test :as t])
  (:gen-class
    :name ClojureUpstreamTestEntry
    :main true
    :methods [^{:static true} [run [] String]]
    :prefix "entry-"))

(def default-test-namespaces
  '[clojure.test-clojure.atoms
    clojure.test-clojure.evaluation
    clojure.test-clojure.fn
    clojure.test-clojure.keywords
    clojure.test-clojure.logic
    clojure.test-clojure.macros
    clojure.test-clojure.other-functions
    clojure.test-clojure.special
    clojure.test-clojure.string])

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

(defn- run-selected-tests [args]
  (let [namespaces (selected-test-namespaces args)]
    (configure-upstream-compat!)
    (doseq [namespace namespaces]
      (require namespace))
    (let [summary (apply t/run-tests namespaces)]
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
