#include <math.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>

double evaluate_polynomial_derivative(double *coeff, size_t n_coeff, double x) {
        if (!coeff || n_coeff == 0) {
                return 0.0;
        }

        double result = 0.0;
        double term = 1.0;

        for (size_t i = 1; i < n_coeff; i++) {
                result += i * coeff[i] * term;
                term *= x;
        }

        return result;
}

double find_root(double x, int n_iterations) {
        if (x < 0) {
                return 0.0;
        }
        if (x == 0) {
                return 0.0;
        }

        double guess = x / 2.0;  // Initial guess

        for (int i = 0; i < n_iterations; i++) {
                // Newton's method: x_n+1 = x_n - f(x_n)/f'(x_n)
                // For square root, f(x) = x^2 - number, f'(x) = 2x
                guess = guess - (guess * guess - x) / (2.0 * guess);
        }

        return guess;
}

double custom_pow(double base, int exponent) {
        if (exponent == 0) return 1.0;
        if (exponent < 0) {
                base = 1.0 / base;
                exponent = -exponent;
        }

        double result = 1.0;
        while (exponent > 0) {
                if (exponent & 1) result *= base;
                base *= base;
                exponent >>= 1;
        }
        return result;
}

void calculate_digits_of_pi(int *digits, size_t n_digits) {
        if (!digits || n_digits == 0) {
                return;
        }

        // Using Bailey-Borwein-Plouffe formula
        double pi = 0.0;
        for (int k = 0; k < 10; k++) {
                pi += (1.0 / custom_pow(16, k)) * (
                                                   (4.0 / (8 * k + 1)) -
                                                   (2.0 / (8 * k + 4)) -
                                                   (1.0 / (8 * k + 5)) -
                                                   (1.0 / (8 * k + 6))
                                                  );
        }

        // Convert to decimal digits
        double decimal = pi;
        for (size_t i = 0; i < n_digits; i++) {
                decimal *= 10;
                digits[i] = (int)decimal;
                decimal -= digits[i];
        }
}

void multiply_matrix_vector(int *result, int *vec_in, int *mat_in, size_t rows, size_t cols) {
        if (!result || !vec_in || !mat_in || rows == 0 || cols == 0) {
                return;
        }

        for (size_t i = 0; i < rows; i++) {
                result[i] = 0;
                for (size_t j = 0; j < cols; j++) {
                        result[i] += mat_in[i * cols + j] * vec_in[j];
                }
        }
}

int median(int *numbers, size_t n) {
        if (!numbers || n == 0) {
                return 0;
        }

        // Create a copy of the array to avoid modifying original
        int temp[n];
        for (size_t i = 0; i < n; i++) {
                temp[i] = numbers[i];
        }

        // Simple bubble sort
        for (size_t i = 0; i < n - 1; i++) {
                for (size_t j = 0; j < n - i - 1; j++) {
                        if (temp[j] > temp[j + 1]) {
                                int swap = temp[j];
                                temp[j] = temp[j + 1];
                                temp[j + 1] = swap;
                        }
                }
        }

        // Return middle element for odd length, average of two middle elements for even
        if (n % 2 == 0) {
                return (temp[n/2 - 1] + temp[n/2]) / 2;
        } else {
                return temp[n/2];
        }
}

double variance(double *input, size_t n_input) {
        if (!input || n_input == 0) {
                return 0.0;
        }

        // Calculate mean
        double sum = 0.0;
        for (size_t i = 0; i < n_input; i++) {
                sum += input[i];
        }
        double mean = sum / n_input;

        // Calculate variance
        double var_sum = 0.0;
        for (size_t i = 0; i < n_input; i++) {
                double diff = input[i] - mean;
                var_sum += diff * diff;
        }

        return var_sum / n_input;
}

void find_prime_factors(int input, int *factors, size_t *n_factors) {
        if (!factors || !n_factors || input <= 0) {
                if (n_factors) {
                        *n_factors = 0;
                }
                return;
        }

        *n_factors = 0;
        int n = input;

        // Handle special cases
        if (input == 1) {
                factors[0] = 1;
                *n_factors = 1;
                return;
        }

        // Find factors
        for (int i = 2; i * i <= n && *n_factors < 64; i++) {
                while (n % i == 0 && *n_factors < 64) {
                        factors[*n_factors] = i;
                        (*n_factors)++;
                        n /= i;
                }
        }

        // If n is greater than 1, it is prime
        if (n > 1 && *n_factors < 64) {
                factors[*n_factors] = n;
                (*n_factors)++;
        }
}

uint64_t multiply(uint64_t a, uint64_t b) {
        return a * b;
}

void multiply_array(uint64_t *a, uint64_t *b, uint64_t n) {
        for (size_t i = 0; i < n; i++) {
                a[i] = a[i] * b[i];
        }
}

int main(int argc, char **argv) {
        int input = 2;
        int factors[64];
        size_t n_factors;
        find_prime_factors(input, factors, &n_factors);
        printf("%lu\n", n_factors);

        return 0;
}
