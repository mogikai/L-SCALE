#include <stdio.h>

int fizz_buzz(int a) {
    return ((int)((a % 5) == 0)) + ((int)((a % 3) == 0));
}

int main() {
    printf("%d\n", fizz_buzz(15));
    return 0;
}