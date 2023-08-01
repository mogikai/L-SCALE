#define _GNU_SOURCE 1
#include <stdio.h>
#include <sys/mman.h>
#include <linux/poll.h>
#include <sys/epoll.h>

#define MAPPING1 0x3000f1000
#define MAPPING2 0x3000f4000
#define MAPPING3_OLD 0x3000f3000
#define MAPPING3_NEW 0x3000f9000


int main() {
	char* mapping1 = mmap((void *) MAPPING1, 0x3000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
	if (mapping1 == (void*)MAP_FAILED) {
		puts("mmap(MAPPING1) returned MAP_FAILED");
		return 1;
	}
	if (mapping1 != (void*)MAPPING1) {
		printf("Mapping 1 wrong pointer: %p\n", mapping1);
		return 1;
	}
	for (int i = 0; i < 0x3000; ++i) mapping1[i] = 'A';
	char* mapping2 = mmap((void *) 0x3000f4000, 0x3000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
	if (mapping2 == MAP_FAILED) {
		puts("mmap(MAPPING2) returned MAP_FAILED");
		return 1;
	}
	if (mapping2 != (void*)MAPPING2) {
		printf("Mapping 2 wrong pointer: %p\n", mapping2);
		return 1;
	}
	for (int i = 0; i < 0x3000; ++i) mapping2[i] = 'B';
	char* mapping3 = mremap((void *) 0x3000f3000, 0x3000, 0x3000, MREMAP_MAYMOVE | MREMAP_FIXED, 0x3000f9000);
	if (mapping3 == MAP_FAILED) {
		puts("mmap(MAPPING3) returned MAP_FAILED");
		return 1;
	}
	if (mapping3 != (void*)MAPPING3_NEW) {
		printf("Mapping 3 wrong pointer: %p\n", mapping3);
		return 1;
	}
	if  (mapping3[0xfff] != 'A') {
		puts("remapping error");
		return 1;
	}
	if  (mapping3[0x1000] != 'B') {
		puts("remapping error");
		return 1;
	}
	for (int i = 0; i < 0x3000; ++i) mapping3[i] = 'C';
	if  (mapping3[0xfff] != 'C') {
		puts("remapping error");
		return 1;
	}
	if  (mapping3[0x1000] != 'C') {
		puts("remapping error");
		return 1;
	}
	return 0;
}
