# Pantalla bitmap + teclado: mueve un cuadro amarillo con W A S D.
# Pantalla: unidades de 8x8, 512x512, base en gp (64x64 unidades).
# En RISCode haz clic en la pantalla para que el teclado vaya al programa.
.eqv ANCHO, 64
.eqv LADO, 4
.eqv MAXIMO, 60                 # ANCHO - LADO
.eqv TECLA_LISTA, 0xFFFF0000
.eqv TECLA, 0xFFFF0004

.data
x: .word 30
y: .word 30
ayuda: .string "Usa W A S D para mover el cuadro.\n"

.text
main:
	la a0, ayuda
	li a7, 4
	ecall
bucle:
	lw t0, TECLA_LISTA        # ¿hay una tecla?
	beqz t0, dibujar
	lw t0, TECLA              # código ASCII (leerlo apaga el bit de "lista")
	lw t1, x
	lw t2, y
	li t3, 'w'
	bne t0, t3, no_w
	addi t2, t2, -1
no_w:
	li t3, 's'
	bne t0, t3, no_s
	addi t2, t2, 1
no_s:
	li t3, 'a'
	bne t0, t3, no_a
	addi t1, t1, -1
no_a:
	li t3, 'd'
	bne t0, t3, no_d
	addi t1, t1, 1
no_d:
	mv a0, t1
	jal limitar
	sw a0, x, t4
	mv a0, t2
	jal limitar
	sw a0, y, t4
dibujar:
	jal fondo
	lw a0, x
	lw a1, y
	li a2, 0x00FFCC00         # amarillo
	jal cuadro
	li a0, 16                 # esperar 16 ms (~60 cuadros por segundo)
	li a7, 32
	ecall
	j bucle

# limitar: deja a0 entre 0 y MAXIMO
limitar:
	bgez a0, lim_ok1
	li a0, 0
lim_ok1:
	li t5, MAXIMO
	ble a0, t5, lim_ok2
	mv a0, t5
lim_ok2:
	ret

# fondo: pinta un degradado en toda la pantalla
fondo:
	li t0, 0                  # y
	li t5, ANCHO
f_y:
	li t1, 0                  # x
f_x:
	slli t2, t1, 2            # rojo  = x * 4
	slli t2, t2, 16
	slli t3, t0, 2            # verde = y * 4
	slli t3, t3, 8
	or t2, t2, t3
	ori t2, t2, 0x60          # un poco de azul
	slli t4, t0, 6            # dirección = gp + (y*64 + x) * 4
	add t4, t4, t1
	slli t4, t4, 2
	add t4, t4, gp
	sw t2, 0(t4)
	addi t1, t1, 1
	blt t1, t5, f_x
	addi t0, t0, 1
	blt t0, t5, f_y
	ret

# cuadro: a0 = x, a1 = y, a2 = color
cuadro:
	li t0, 0
	li t3, LADO
c_y:
	li t1, 0
c_x:
	add t2, a1, t0
	slli t2, t2, 6
	add t4, a0, t1
	add t2, t2, t4
	slli t2, t2, 2
	add t2, t2, gp
	sw a2, 0(t2)
	addi t1, t1, 1
	blt t1, t3, c_x
	addi t0, t0, 1
	blt t0, t3, c_y
	ret
