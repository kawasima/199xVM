(ns upstream.fn-bad-arglists
  (:use clojure.test clojure.test-helper))

(deftest fn-error-checking-bad-arglists
  (testing "bad arglist"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn "a" a)))))

  (testing "treat first param as args"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn "a" [])))))

  (testing "looks like listy signature, but malformed declaration"
    (is (fails-with-cause? clojure.lang.ExceptionInfo
          #"Call to clojure.core/fn did not conform to spec"
          (eval '(fn (1)))))))
