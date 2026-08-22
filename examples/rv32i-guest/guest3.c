// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/mman.h>
#include <unistd.h>

#define ASSERT(expr, fmt, ...)                                                 \
    do {                                                                       \
        if (!(expr)) {                                                         \
            fprintf(stderr, "%s:%d ASSERT: " fmt " (err=%d errstr=%s)\n",      \
                    __FILE__, __LINE__, ##__VA_ARGS__, errno,                  \
                    strerror(errno));                                          \
            abort();                                                           \
        }                                                                      \
    } while (0)

__thread int th_local_val = 55;

static inline void print_tlsp(const char *tag) {
    uintptr_t tls;
    asm volatile("mv %0, tp" : "=r"(tls));
    printf("%s: tlsp=%08x\n", tag, tls);
}

static void *thread2(void *arg) {
    print_tlsp("thread2");

    printf("thread2 arg=%s\n", (const char *)arg);

    ASSERT(th_local_val == 55,
           "thread2: wrong initial thread local variable val=%d", th_local_val);
    return (void *)0x1337;
}

static void *thread1(void *arg) {
    int rc;
    pthread_t th;

    print_tlsp("thread1");

    ASSERT(th_local_val == 55,
           "thread1: wrong initial thread local variable val=%d", th_local_val);
    th_local_val = 4242;
    printf("thread1: th_local_val=%d\n", th_local_val);

    rc = pthread_create(&th, NULL, thread2, "thread2 some arg");
    return (void *)th;
}

int main(int argc, char *argv[]) {
    int rc;
    pthread_t th1, th2;
    void *ret = NULL;

    print_tlsp("main");

    ASSERT(th_local_val == 55,
           "main: wrong initial thread local variable val=%d", th_local_val);
    th_local_val = 123;

    rc = pthread_create(&th1, NULL, thread1, NULL);
    ASSERT(rc == 0, "pthread_create failed");

    rc = pthread_join(th1, &ret);
    ASSERT(rc == 0, "pthread_join failed");

    th2 = (pthread_t)ret;
    rc = pthread_join(th2, &ret);
    ASSERT(rc == 0, "pthread_join failed");
    printf("main: pthread_join() -> %p\n", ret);

    ASSERT(th_local_val == 123,
           "main: wrong final thread local variable val=%d", th_local_val);

    printf("main: end of fn\n");
    return 0;
}
