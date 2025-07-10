#include <assert.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>

int main(int argc, char** argv) {
    char buf[100] = {};
    getcwd(buf, 100);
    // Assert that initial cwd is set correctly
    assert(strcmp(buf, "/home/user") == 0);
    assert(chdir("/home") == 0);
    getcwd(buf, 100);
    // Assert that changing the directory with chdir works
    assert(strcmp(buf, "/home") == 0);
    int fd = openat(0, "/lib", O_DIRECTORY);
    assert(fchdir(fd) == 0);
    getcwd(buf, 100);
    // Assert that changing the directory with fchdir works
    assert(strcmp(buf, "/lib") == 0);
    // Assert that getcwd fails if the buffer is too small
    assert(getcwd(buf, 0) == 0);
    return 0;
}
