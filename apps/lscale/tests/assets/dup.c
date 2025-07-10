#define _GNU_SOURCE

#include <fcntl.h>
#include <unistd.h>
#include <assert.h>

int main(int argc, char *argv[])
{
        int in = 0;
        int out = 1;
        assert(dup(in) == 3); // first available file descriptor (3)
        assert(dup(out) == 4); // next one (4)
        assert(dup2(out, 5) == 5);
        assert(dup2(4, 4) == 4); // dup2 doesn't do anything if new == old
        assert(dup3(out, out, 0) == -1); // dup3 returns an error if new == old
        assert(dup3(in, 123, O_CLOEXEC) == 123);
        assert(dup(5) == 6); // next one

        return 0;
}
