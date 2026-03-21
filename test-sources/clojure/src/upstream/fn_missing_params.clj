(ns upstream.fn-missing-params
  (:use clojure.test clojure.test-helper))

(deftest fn-error-checking-missing-params
  (testing "missing parameter declaration"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn a))))
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn))))))
