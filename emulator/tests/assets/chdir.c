#include <assert.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>

int main(int argc, char** argv) {
    char buf[100] = {};
    getcwd(buf, 100);
    // Assert that initial cwd is set correctly
    assert(strcmp(buf, "/home/user"));
    chdir("/home");
    getcwd(buf, 100);
    // Assert that changing the directory with chdir works
    assert(strcmp(buf, "/home"));
    int fd = openat(0, "/", O_DIRECTORY);
    fchdir(fd);
    getcwd(buf, 100);
    // Assert that changing the directory with fchdir works
    assert(strcmp(buf, "/home"));
    // Assert that getcwd fails if the buffer is too small
    assert(getcwd(buf, 0) == (char*)-1); 
    return 0;
}
