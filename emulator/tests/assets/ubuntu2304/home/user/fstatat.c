#include <fcntl.h>
#include <sys/stat.h>
#include <assert.h>

int main() {
    // TODO: Extend tests to include more use cases of fstatat and also check the actual stat info
    int dirfd = open("/home/user", O_RDONLY);
    int fd = open("/home/user/fstatat", O_RDONLY);
    assert(dirfd >= 0);
    struct stat buf;
    assert(fstatat(dirfd, "fstatat.c", &buf, 0) == 0);
    // Fail, if path is relative but no valid dir supplied.
    assert(fstatat(-1, "fstatat.c", &buf, 0) == -1);
    // Fail, if path is relative but fd is not a directory.
    assert(fstatat(fd, "fstatat.c", &buf, 0) == -1);
    // fd is ignored if path is absolute
    assert(fstatat(fd, "/home/user/fstatat.c", &buf, 0) == 0);
    // Fail if buf is NULL
    assert(fstatat(fd, "/home/user/fstatat.c", 0, 0) == -1);
    // fd is invalid file descriptor
    assert(fstatat(20, "fstatat.c", &buf, 0) == -1);
	return 0;
}
