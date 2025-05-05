#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

uint64_t is_prime(uint64_t a)
{
        int is_prime = 1;

        for (uint64_t i = 2; i < a; i++) {
                if (a % i == 0) {
                        is_prime = 0;
                        break;
                }
        }

        return is_prime;
}

uint64_t nth_fibonacci(size_t n)
{
        if (n < 2) {
                return 1;
        }

        uint64_t a = 1;
        uint64_t b = 1;
        for (int i = 0; i < n; i++) {
                uint64_t sum = a + b;
                a = b;
                b = sum;
        }

        return b;
}

uint64_t max_byte(uint8_t *bytes, size_t n) {
        uint8_t max = 0;
        for (size_t i = 0; i < n; i++) {
                if (bytes[i] > max) {
                        max = bytes[i];
                }
        }

        return max;
}

int leading_zeroes(uint64_t a) {
        for (int i = 0; i < 64; i++) {
                if ((a & (1 << (63 - i))) > 0) {
                        return i;
                }
        }

        return 63;
}

void swap(int* a, int* b) {
        int temp = *a;
        *a = *b;
        *b = temp;
}

int partition(int *arr, int low, int high) {
        int pivot = arr[high];  // Choosing the last element as pivot
        int i = low - 1;

        for (int j = low; j < high; j++) {
                if (arr[j] <= pivot) {
                        i++;
                        swap(&arr[i], &arr[j]);
                }
        }

        swap(&arr[i + 1], &arr[high]);
        return i + 1;
}

void quicksort(int *arr, int low, int high)
{
        if (low < high) {
                int pi = partition(arr, low, high);

                quicksort(arr, low, pi - 1);
                quicksort(arr, pi + 1, high);
        }
}

int main(int argc, char **argv)
{
        return 0;
}
