(ns smoke.core
  (:gen-class
    :name ClojureSmokeEntry
    :methods [^{:static true} [run [] String]]
    :prefix "entry-"))

(defn entry-run []
  "ok")
