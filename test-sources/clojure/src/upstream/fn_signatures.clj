(ns upstream.fn-signatures
  (:use clojure.test clojure.test-helper))

(deftest fn-error-checking-signatures
  (testing "checks each signature"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn
                   ([a] 1)
                   ("a" 2))))))

  (testing "correct name but invalid args"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn a "a")))))

  (testing "first sig looks multiarity, rest of sigs should be lists"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn a
                   ([a] 1)
                   [a b]))))))
