#include <stdlib.h>
#include <stdio.h>
#include <assert.h>

int main(void)
{
	char line[BUFSIZ];
	size_t linelen = 0;
	size_t outlen;
	FILE *fp = fopen("/etc/passwd", "r");
	assert(fp);

	linelen = fread(line, sizeof(line), 1, fp);
	while (linelen > 0) {
		outlen = fwrite(line, 1, linelen, stdout);
		assert(outlen == linelen); //TODO write in loop
		linelen = fread(line, sizeof(line), 1, fp);
	}

	assert(feof(fp));

	fclose(fp);
	return 0;
}
