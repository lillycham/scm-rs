;; Boolean functions
(define (or x y) (if x #t y))
(define (and x y) (if x y #f))
(define (not x) (if x #f #t))
