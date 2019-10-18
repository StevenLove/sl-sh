;;; Macros to make working with the shell easier.

;; Create an alias, intended to be used with executables not lisp code (use defn for that).
(defmacro alias (name body)
	`(defn ,name (&rest args)
		(use-stdout (loose-symbols (eval (append (quote ,body) args))))))

;; Redirect stdout to file, append the output.
(defmacro out>> (file body)
	`(use-stdout (stdout-to ,file ,body)))

;; Redirect stdout to file, truncate the file first.
(defmacro out> (file body)
	`(progn (file-trunc ,file) (use-stdout (stdout-to ,file ,body))))

;; Redirect stderr to file, append the output.
(defmacro err>> (file body)
	`(use-stdout (stderr-to ,file ,body)))

;; Redirect stderr to file, truncate the file first.
(defmacro err> (file body)
	`(progn (file-trunc ,file) (use-stdout (stderr-to ,file ,body))))

;; Redirect both stdout and stderr to the same file, append the output.
(defmacro out-err>> (file body)
	`(let ((f nil)) (loose-symbols (setq f (open ,file :create :append))) (stdout-to f (stderr-to f ,body))))

;; Redirect both stdout and stderr to the same file, truncate the file first.
(defmacro out-err> (file body)
	`(let ((f nil)) (loose-symbols (setq f (open ,file :create :truncate))) (stdout-to f (stderr-to f ,body))))
	;`(progn (file-trunc ,file) (stdout-to ,file (stderr-to ,file ,body))))

;; Redirect stdout to null (/dev/null equivelent).
(defmacro out>null (body)
	`(out-null ,body))

;; Redirect stderr to null (/dev/null equivelent).
(defmacro err>null (body)
	`(err-null ,body))

;; Redirect both stdout and stderr to null (/dev/null equivelent).
(defmacro out-err>null (body)
	`(out-null (err-null ,body)))

;; Shorthand for pipe builtin.
(defmacro | (&rest body)
	`(pipe ,@body))

(defq pushd nil)
(defq popd nil)
(defq dirs nil)
(defq clear-dirs nil)
(defq set-dirs-max nil)
;; Scope to contain then pushd/popd/dirs functions.
(let ((dir_stack '()) (dir_stack_max 20))
	;; Push current directory on the directory stack and change to new directory.
	(setfn pushd (dir) (if (form (cd dir))
		(progn
			(push dir_stack $OLDPWD)
			(if (> (length dir_stack) dir_stack_max) (remove-nth 0 dir_stack))
			t)
		nil))
	;; Pop first directory from directory stack and change to it.
	(setfn popd () (if (> (length dir_stack) 0)
		(cd (pop dir_stack))
		(println "Dir stack is empty")))
	;; List the directory stack.
	(setfn dirs ()
		(for d dir_stack (println d)))
	;; Clears the directory stack.
	(setfn clear-dirs ()
		(clear dir_stack))
	;; Sets the max number of directories to save in the stack.
	(setfn set-dirs-max (max)
		(if (and (= (get-type max) "Int")(> max 1))
			(setq dir_stack_max max)
			(println "Error, max must be a positive Int greater then one"))))
