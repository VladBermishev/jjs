#include "proto.h"
#include <cstdlib>
#include <cstring>
#include <cstddef>
#include <cstdint>

char* get_env(const char* var_name) {
    char* res = getenv(var_name);
    if (res == nullptr) {
        fprintf(stderr, "ERROR: var %s not present\n", var_name);
        exit(1);
    }
    return res;
}

int get_env_int(const char* var_name) {
    char* res = get_env(var_name);
    int ans;
    if (sscanf(res, "%d", &ans) == 0) {
        fprintf(stderr, "ERROR: var `%s` has value `%s`, which is not integer\n", var_name, res);
        exit(1);
    }
    return ans;
}

FILE* get_env_file(const char* var_name, const char* mode) {
    int fd = get_env_int(var_name);
    FILE* file = fdopen(fd, mode);
    if (file == nullptr) {
        fprintf(stderr, "ERROR: var `%s` contains fd `%d`, which is not file of mode %s", var_name, fd, mode);
        exit(1);
    }
    return file;
}

const uint8_t CHAR_BAD = 255;

uint8_t decode_hex_char(char x) {
    if ('0' <= x && x <= '9') return x - '0';
    if ('a' <= x && x <= 'f') return x - 'a' + 10;
    //printf("CHAR_BAD: '%c'\n", x);
    return CHAR_BAD;
}

BinString decode_hex(char* data) {
    size_t n = strlen(data);
    if (n % 2 != 0) return {};
    auto out = new uint8_t[n / 2];
    for (int i = 0; i < n / 2; ++i) {
        auto a = decode_hex_char(data[2 * i]);
        auto b = decode_hex_char(data[2 * i + 1]);
        if (a == CHAR_BAD || b == CHAR_BAD) {
            delete[] out;
            return {};
        }
        out[i] = a * 16 + b;
    }
    BinString bs;
    bs.len = n / 2;
    bs.head = out;
    return bs;
}

void BinString::dealloc() {
    delete[] head;
}

BinString get_env_hex(const char* var_name) {
    char* value = get_env(var_name);

    auto res = decode_hex(value);
    if (!res.head) {
        fprintf(stderr, "ERROR: var `%s` contains '%s', which is not hex\n", var_name, value);
        exit(1);
    }
    return res;
}
