#include <stdlib.h>
#include <stdio.h>

int main(void)
{
	char line[BUFSIZ];
	size_t linelen = 0;
	size_t outlen;
	FILE *fp = fopen("/etc/passwd", "r");
	if (!fp)
		return 1;

	linelen = fread(line, sizeof(line), 1, fp);
	while (linelen > 0) {
		outlen = fwrite(line, 1, linelen, stdout);
		if (outlen != linelen)
			return 3;
		linelen = fread(line, sizeof(line), 1, fp);
	}

	printf("linelen = %li\n", linelen);

	if (!feof(fp))
		return 5;

	fclose(fp);
	return 0;
}
