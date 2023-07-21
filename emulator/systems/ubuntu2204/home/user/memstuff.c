//
// Created by mark on 04.04.23.
//

#include <sys/mman.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int
main(int argc, char **argv) {
    void *ptr1 = mmap(NULL, 0x20000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    void *ptr2 = mmap(NULL, 0x20000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    void *ptr3 = mmap(NULL, 0x20000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);

    munmap(ptr1 + 0x10000, 0x5000);
    munmap(ptr1 + 0x00000, 0x5000);

    char *my_mem = malloc(2000);
    for(int i = 0; i < 2000; ++i) {
        my_mem[i] = 0x41 + i % 26;
    }

    printf("Test: %.2000s\n", my_mem);
    free(my_mem);
    execv("main", argv);
}