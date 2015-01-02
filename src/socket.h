#pragma once
#include "spotifyd.h"

void sock_send_str(int sockfd, const char * const str);
int sock_readline(int sockfd, char *buf);
void *sock_connection_handler(void *sockfd_ptr);
void sock_send_track_with_trackn(int sockfd, sp_track *track, int trackn);
void sock_send_track(int sockfd, sp_track *track);
void *sock_accept_connections(void *sock);
int sock_create_un();
