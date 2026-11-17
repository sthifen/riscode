.eqv CONST 0x1234
.eqv GRANDE, 0x7FFFF800
.data
b1: .byte 1, -2, 255
h1: .half 0x1234, -1
w1: .word 0xDEADBEEF, w1, b1
str1: .string "hola\n"
str2: .ascii "ab"
.align 2
sp1: .space 6
w2: .word 3:4
.text
main:
  add t0, t1, t2
  sub a0, a1, a2
  and s0, s1, s2
  or x5, x6, x7
  xor t3, t4, t5
  sll t0, t0, t1
  srl t0, t0, t1
  sra t0, t0, t1
  slt a0, a1, a2
  sltu a0, a1, a2
  mul t0, t1, t2
  mulh t0, t1, t2
  mulhsu t0, t1, t2
  mulhu t0, t1, t2
  div t0, t1, t2
  divu t0, t1, t2
  rem t0, t1, t2
  remu t0, t1, t2
  addi t0, t0, -2048
  addi t0, t0, 2047
  andi t0, t1, 0xff
  ori t0, t1, 7
  xori t0, t1, -1
  slti t0, t1, 5
  sltiu t0, t1, 5
  slli t0, t1, 31
  srli t0, t1, 3
  srai t0, t1, 3
  lb t0, -1(sp)
  lh t0, 2(sp)
  lw t0, (sp)
  lbu t0, 1(sp)
  lhu t0, 2(sp)
  sb t0, 0(sp)
  sh t0, 2(sp)
  sw t0, 4(sp)
  lui t0, 0x12345
  auipc t0, 1
  li t0, -1
  li t0, 2048
  li t0, -2049
  li t0, 0x80000000
  li t0, 0xFFFFFFFF
  li t0, CONST
  li t0, GRANDE
  li t0, 0x7FFFFFFF
  la t0, w1
  la t1, str1
  lw t0, w1
  lb t0, b1
  sw t0, w2, t1
  sb t0, b1, t2
  lw t0, 0xFFFF0004
  sw zero, 0xFFFF0000, t0
  lw t0, 100
  mv t0, t1
  not t0, t1
  neg t0, t1
  nop
  seqz t0, t1
  snez t0, t1
  sltz t0, t1
  sgtz t0, t1
atras:
  beq t0, t1, atras
  bne t0, t1, adelante
  blt t0, t1, atras
  bge t0, t1, adelante
  bltu t0, t1, atras
  bgeu t0, t1, adelante
  beqz t0, atras
  bnez t0, adelante
  blez t0, atras
  bgez t0, adelante
  bltz t0, atras
  bgtz t0, adelante
  bgt t0, t1, atras
  ble t0, t1, adelante
  bgtu t0, t1, atras
  bleu t0, t1, adelante
adelante:
  j atras
  jal atras
  jal t0, adelante
  jalr t0
  jalr ra, 4(t0)
  jalr ra, t0, 8
  jr t0
  ret
  call main
  tail main
  ecall
  ebreak
  csrrw t0, ustatus, t1
  csrrs t0, cycle, zero
  csrrwi t0, uie, 5
  csrr t0, time
  csrw t1, ustatus
  rdcycle t0
  rdinstret t1
  fence 1, 1
