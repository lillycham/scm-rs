;; Module for functional programming primitives, primarily composed of lambda forms.
(define apply (lambda (f x) (f x)))
(define compose (lambda (f g) (lambda (x) (f (g x)))))
(define identity (lambda (x) x))
(define constant (lambda (x) (lambda (y) x)))
(define twice (lambda (f) (compose f f)))
