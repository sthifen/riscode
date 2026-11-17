# Hola: pide el nombre por la terminal y saluda.
# Muestra cómo leer texto (syscall 8) e imprimir (syscall 4).
.data
pregunta: .string "¿Cómo te llamas? "
saludo:   .string "¡Hola, "
nombre:   .space 64

.text
main:
	la a0, pregunta
	li a7, 4          # imprimir texto
	ecall

	la a0, nombre
	li a1, 64         # tamaño máximo
	li a7, 8          # leer texto
	ecall

	la a0, saludo
	li a7, 4
	ecall
	la a0, nombre
	li a7, 4
	ecall

	li a7, 10         # salir
	ecall
