(require 'org)
(require 'org-element)

(defun oracle-emit (node)
  (let ((type (org-element-type node)))
    (cond
     ((eq type 'plain-text) nil)
     ((not (symbolp type)) nil)
     (t
      (let* ((plist (copy-sequence (cadr node)))
             (_ (setq plist (org-plist-delete plist :parent)))
             (kids (delq nil (mapcar #'oracle-emit
                                     (org-element-contents node)))))
        `(,type ,plist ,@kids))))))

(let ((file (car command-line-args-left)))
  (with-temp-buffer
    (insert-file-contents file)
    (org-mode)
    (prin1 (oracle-emit (org-element-parse-buffer)))
    (terpri)))
