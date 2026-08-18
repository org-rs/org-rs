;;; bench.el --- Time org-element-parse-buffer for a file -*- lexical-binding: t; -*-
;;
;; Used by `corpus-check --benchmark' to measure how long Emacs'
;; `org-element-parse-buffer' takes for a given file, isolated from emacs
;; startup cost.  The file is visited once, then parsed ITERS times inside this
;; single process; the total wall time is printed as a lexpr-readable plist:
;;
;;     (:iters N :seconds F)
;;
;; Invoked as:  emacs --batch -Q --load bench.el <file> <iters>

(require 'org)
(require 'org-element)
(require 'org-inlinetask)

(let* ((file (nth 0 command-line-args-left))
       (iters (max 1 (string-to-number (or (nth 1 command-line-args-left) "10")))))
  (with-temp-buffer
    (insert-file-contents file)
    (org-mode)
    ;; Warm up (and discard) one parse so caches are populated before timing.
    (org-element-parse-buffer)
    (let ((start (current-time)))
      (dotimes (_ iters)
        (org-element-parse-buffer))
      (let ((elapsed (float-time (time-subtract (current-time) start))))
        (prin1 (list :iters iters :seconds elapsed))
        (terpri)))))
