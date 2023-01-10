;; Boolean functions
(define (or x y) (if x #t y))
(define (and x y) (if x y #f))
(define (not x) (if x #f #t))
(define (unless x y) (if x #t y))
(define (when x y) (if x y #f))