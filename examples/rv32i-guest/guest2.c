// SPDX-License-Identifier: MIT
//
// Copyright (c) 2026, Johannes Stoelp <dev@memzero.de>

#include <stdio.h>

int main(int argc, char *argv[]) {
	printf("hello C, argc=%d\n", argc);
	for (int i=0; i<argc; ++i) {
		printf("argv[%d]=%s\n", i, argv[i]);
	}
}
