(let ((count 0))
    (let ((count 0))
        (defn inc_count2 () (setq count (+ count 1)))
        (defn dec_count2 () (setq count (- count 1)))
        (defn get_count2 () (count))
        (defn get_count02 () (get_count0)))

    (defn inc_count () (setq count (+ count 1)))
    (defn dec_count () (setq count (- count 1)))
    (defn get_count () (count))
    (defn print_count () (println count)))

(defq count 0)

(defn o_count () (println count))
(defn get_count0 () (count))

(if (= (get_count) 0) (println "PASS") (println "FAIL"))
(if (= (get_count2) 0) (println "PASS") (println "FAIL"))
(if (= (get_count0) 0) (println "PASS") (println "FAIL"))
(if (= (get_count02) 0) (println "PASS") (println "FAIL"))
(if (= count 0) (println "PASS") (println "FAIL"))
(inc_count)
(if (= (get_count) 1) (println "PASS") (println "FAIL"))
(if (= count 0) (println "PASS") (println "FAIL"))
(inc_count)
(inc_count)
(inc_count)
(inc_count)
(if (= (get_count) 5) (println "PASS") (println "FAIL"))
(if (= count 0) (println "PASS") (println "FAIL"))
(dec_count)
(dec_count)
(if (= (get_count) 3) (println "PASS") (println "FAIL"))
(if (= count 0) (println "PASS") (println "FAIL"))
(setq count 10)
(if (= (get_count) 3) (println "PASS") (println "FAIL"))
(if (= count 10) (println "PASS") (println "FAIL"))


(if (= (get_count) 3) (println "PASS") (println "FAIL"))
(if (= count 10) (println "PASS") (println "FAIL"))
(if (= (get_count2) 0) (println "PASS") (println "FAIL"))
(inc_count2)
(if (= (get_count) 3) (println "PASS") (println "FAIL"))
(if (= count 10) (println "PASS") (println "FAIL"))
(if (= (get_count2) 1) (println "PASS") (println "FAIL"))
(inc_count2)
(inc_count2)
(inc_count2)
(inc_count2)
(if (= (get_count) 3) (println "PASS") (println "FAIL"))
(if (= count 10) (println "PASS") (println "FAIL"))
(if (= (get_count0) 10) (println "PASS") (println "FAIL"))
(if (= (get_count02) 10) (println "PASS") (println "FAIL"))
(if (= (get_count2) 5) (println "PASS") (println "FAIL"))
(dec_count2)
(inc_count)
(setq count 11)
(if (= (get_count) 4) (println "PASS") (println "FAIL"))
(if (= (get_count0) 11) (println "PASS") (println "FAIL"))
(if (= (get_count02) 11) (println "PASS") (println "FAIL"))
(if (= count 11) (println "PASS") (println "FAIL"))
(if (= (get_count2) 4) (println "PASS") (println "FAIL"))

