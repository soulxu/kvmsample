# A test code for kvmsample

.globl _start
    .code16
_start:
    xorw %ax, %ax
    
loop1:
    in $0x10, %ax
    jmp loop1

