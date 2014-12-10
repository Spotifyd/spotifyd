#pragma once
#include "spotifyd.h"

void sock_send_str(int sockfd, const char * const str);
int sock_readline(int sockfd, char *buf);
void *sock_connection_handler(void *sockfd_ptr);
void sock_send_track(int sockfd, sp_track *track, int trackn);
int sock_create_un();
