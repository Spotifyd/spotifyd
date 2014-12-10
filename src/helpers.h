#pragma once
#include "spotifyd.h"
#include "time.h"

#define DEBUG 0

void notify_main_thread();
struct timespec rel_to_abstime(int msec);
void debug(char *debug_msg);
void play(sp_session *session, sp_track *track, bool flush);
