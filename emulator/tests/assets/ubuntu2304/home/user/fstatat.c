#include <fcntl.h>
#include <sys/stat.h>
#include <assert.h>

int main() {
    // TODO: Extend tests to include more use cases of fstatat and also check the actual stat info
    int dirfd = open("/home/user", O_RDONLY);
    assert(dirfd >= 0);
    struct stat buf;
    assert(fstatat(dirfd, "fstatat.c", &buf, 0) == 0);
	return 0;
}
