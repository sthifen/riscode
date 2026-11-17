# Suma los números del 1 al 10 y la imprime.
# Prueba el paso a paso (F10) y mira cómo cambian t0 y a0 en REGISTROS.
.data
titulo: .string "La suma es: "

.text
main:
	li t0, 1          # contador
	li t1, 10         # límite
	li a0, 0          # acumulador
ciclo:
	add a0, a0, t0
	addi t0, t0, 1
	ble t0, t1, ciclo

	mv t2, a0
	la a0, titulo
	li a7, 4          # imprimir texto
	ecall
	mv a0, t2
	li a7, 1          # imprimir entero
	ecall

	li a7, 10         # salir
	ecall
