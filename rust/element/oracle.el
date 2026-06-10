(require 'org)
(require 'org-element)

(defun oracle-emit-children (children pos)
  "Emit CHILDREN as oracle S-expressions, threading POS for plain-text spans.
POS is the 1-based buffer position at which the first child begins.
Plain-text (Lisp strings) have no :begin/:end in Emacs 30; their spans
are recovered by advancing POS by the string length between sibling
objects whose :end positions anchor the counter."
  (let (result)
    (dolist (child children (delq nil (nreverse result)))
      (cond
       ((stringp child)
        (let* ((len (length child))
               (end (+ pos len)))
          (push `(plain-text (:begin ,pos :end ,end)) result)
          (setq pos end)))
       (t
        (push (oracle-emit child) result)
        (let ((child-end (org-element-property :end child)))
          (when child-end (setq pos child-end))))))))

(defun oracle-emit (node)
  "Emit a minimal, lexpr-readable S-expression for NODE.
Only :begin, :end, and :contents-begin are extracted via the stable
org-element-property API, avoiding the :standard-properties vector
introduced in Emacs 30 which contains non-readable buffer objects.

Container elements delegate to `oracle-emit-children' so that plain-text
Lisp strings receive correct :begin/:end positions tracked from their
surrounding siblings."
  (let ((type (org-element-type node)))
    (cond
     ((not (symbolp type)) nil)
     (t
      (let* ((begin (org-element-property :begin node))
             (end   (org-element-property :end   node))
             (plist (list :begin begin :end end))
             (contents-begin (org-element-property :contents-begin node))
             (raw-kids (org-element-contents node))
             (kids (when raw-kids
                     (oracle-emit-children raw-kids (or contents-begin begin)))))
        `(,type ,plist ,@kids))))))

(let ((file (car command-line-args-left)))
  (with-temp-buffer
    (insert-file-contents file)
    (org-mode)
    (prin1 (oracle-emit (org-element-parse-buffer)))
    (terpri)))
