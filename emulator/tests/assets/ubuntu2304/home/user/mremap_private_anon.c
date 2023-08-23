#define _GNU_SOURCE 1
#include <assert.h>
#include <sys/mman.h>

#define MAPPING1 0x3000f1000
#define MAPPING2 0x3000f4000
#define MAPPING3_OLD 0x3000f3000
#define MAPPING3_NEW 0x3000f9000


int main() {
	char* mapping1 = mmap((void *) MAPPING1, 0x3000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    assert(mapping1 != MAP_FAILED);
    assert(mapping1 == (void*)MAPPING1);
	for (int i = 0; i < 0x3000; ++i) mapping1[i] = 'A';
	char* mapping2 = mmap((void *) 0x3000f4000, 0x3000, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED, -1, 0);
    assert(mapping2 != MAP_FAILED);
    assert(mapping2 == (void*)MAPPING2);
	for (int i = 0; i < 0x3000; ++i) mapping2[i] = 'B';
	char* mapping3 = mremap((void *) 0x3000f3000, 0x3000, 0x3000, MREMAP_MAYMOVE | MREMAP_FIXED, 0x3000f9000);
    assert(mapping3 != MAP_FAILED);
	assert(mapping3 == (void*)MAPPING3_NEW);
    assert(mapping3[0xfff] == 'A');
    assert(mapping3[0x1000] == 'B');
	for (int i = 0; i < 0x3000; ++i) mapping3[i] = 'C';
	assert(mapping3[0xfff] == 'C');
    assert(mapping3[0x1000] == 'C');
    assert(0);
	return 0;
}
