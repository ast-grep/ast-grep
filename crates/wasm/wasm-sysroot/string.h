#pragma once

void *memcpy(void *dest, const void *src, unsigned long n);
void *memmove(void *dest, const void *src, unsigned long n);
void *memset(void *s, int c, unsigned long n);
int memcmp(const void *ptr1, const void *ptr2, unsigned long n);
int strncmp(const char *s1, const char *s2, unsigned long n);
int strlen(const char *s);
int strncpy(char *dest, const char *src, unsigned long n);
void *memchr(const void *s, int c, unsigned long n);
char *strchr(const char *s, int c);
int strcmp(const char *s1, const char *s2);
