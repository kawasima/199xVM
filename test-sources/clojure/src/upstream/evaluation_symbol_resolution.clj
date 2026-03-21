(ns upstream.evaluation-symbol-resolution
  (:require [clojure.test :refer :all]
            [clojure.test-helper :refer :all]
            [clojure.test-clojure.evaluation :as evaluation
             :refer [class-for-name in-test-ns test-that]])
  (:import (clojure.lang Compiler$CompilerException)))

(deftest QualifiedVars
  (test-that
    "If a symbol is namespace-qualified, the evaluated value is the value
     of the binding of the global var named by the symbol"
    (is (= (eval 'resolution-test/bar) 123)))

  (test-that
    "It is an error if there is no global var named by the symbol"
    (is (thrown-with-cause-msg? Compiler$CompilerException
          #"(?s).*Unable to resolve symbol: bar.*"
          (eval 'bar))))

  (test-that
    "It is an error if the symbol reference is to a non-public var in a
     different namespace"
    (is (thrown-with-cause-msg? Compiler$CompilerException
          #"(?s).*resolution-test/baz is not public.*"
          (eval 'resolution-test/baz)))))

(deftest QualifiedClasses
  (test-that
    "If a symbol is package-qualified, its value is the Java class named by the
     symbol"
    (is (= (eval 'java.lang.Math) (class-for-name "java.lang.Math"))))

  (test-that
    "If a symbol is package-qualified, it is an error if there is no Class named
     by the symbol"
    (is (thrown? Compiler$CompilerException (eval 'java.lang.FooBar)))))

(deftest LookupOrderSpecialFormsA
  (test-that
    "Special forms are recognized before other lookup steps"
    (doseq [form '(def if do let quote var)]
      (is (thrown? Compiler$CompilerException (eval form))))))

(deftest LookupOrderSpecialFormsB
  (test-that
    "Special forms remain errors when evaluated as symbols"
    (doseq [form '(fn loop recur throw try monitor-enter monitor-exit)]
      (is (thrown? Compiler$CompilerException (eval form))))))

(deftest LookupOrderPositiveClassMappings
  (test-that
    "Class mappings resolve before local bindings"
    (let [if "foo"]
      (is (= (eval 'Boolean) (class-for-name "java.lang.Boolean"))))
    (let [Boolean "foo"]
      (is (= (eval 'Boolean) (class-for-name "java.lang.Boolean"))))))

(deftest LookupOrderPositiveLocalBinding
  (test-that
    "Local bindings resolve after class mappings"
    (is (= (eval '(let [foo "bar"] foo)) "bar"))))

(deftest LookupOrderPositiveCurrentNamespaceVar
  (test-that
    "Current namespace vars resolve after class mappings and local bindings"
    (in-test-ns
      (is (= (eval 'foo) "abc")))))

(deftest LookupOrderNegative
  (test-that
    "Unresolved current-namespace symbols still fail after class and var lookup"
    (is (thrown? Compiler$CompilerException (eval 'bar)))
    (is (thrown? Compiler$CompilerException (eval 'foobar)))))
