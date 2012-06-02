all: kvmsample test.bin

kvmsample: main.o
	gcc main.c -o kvmsample -lpthread

test.bin: test.o
	ld -m elf_i386 --oformat binary -N -e _start -Ttext 0x10000 -o test.bin test.o

test.o: test.S
	as -32 test.S -o test.o
	
