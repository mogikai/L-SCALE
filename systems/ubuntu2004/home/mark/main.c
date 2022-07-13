#include <stdio.h>

int main() {
    puts("hello world");
    char c = 0;
    while ((c = getchar()) != '\n') {
        puts("Got char");
    }
}
