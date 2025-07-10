#define _GNU_SOURCE 1
#include <assert.h>
#include <sys/mman.h>

#define MAPPING1 0x3000f1000
#define MAPPING2 0x3000f4000
#define MAPPING3_OLD 0x3000f3000
#define MAPPING3_NEW 0x3000f9000


int main() {
    // TODO: Extend mmap tests.
	char* mapping1 = mmap((void *) MAPPING1, 0x3000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
	assert(mapping1 != MAP_FAILED);
	assert(mapping1 == (void*)MAPPING1);
	return 0;
}
