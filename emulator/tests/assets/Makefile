.PHONY: all clean

src = assert_false.c assert_true.c mremap_private_anon.c stdio.c fstatat.c mmap.c getrandom.c dup.c
targets = $(src:.c=)

all: $(targets)

clean:
	$(RM) $(targets) *.o
