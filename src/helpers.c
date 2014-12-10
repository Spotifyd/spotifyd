#include <stdio.h>
#include <stdlib.h>
#include <libspotify/api.h>
#include <time.h>

#include "spotifyd.h"
#include "helpers.h"

void notify_main_thread()
{
	pthread_mutex_lock(&notify_mutex);
	notify_do = 1;
	pthread_cond_signal(&notify_cond);
	pthread_mutex_unlock(&notify_mutex);
}

void debug(char *debug_msg)
{
	if(DEBUG)
		printf("%s", debug_msg);
}

struct timespec rel_to_abstime(int msec)
{
	struct timespec ts;

	if(clock_gettime(CLOCK_REALTIME, &ts) != 0)
	{
		perror("clock_gettime");
	}

	ts.tv_sec += msec / 1000;
	ts.tv_nsec += (msec % 1000) * 1000000;

	/*
	 * If tv_nsec gets bigger than 999999999,
	 * add a second.
	 */
	long nsec = ts.tv_nsec;
	ts.tv_nsec = ts.tv_nsec%1000000000;
	ts.tv_sec += nsec/1000000000;
	return ts;
}

void play(sp_session *session, sp_track *track, bool flush)
{
	debug("play\n");

	if(flush)
	{
		audio_fifo_flush(&g_audiofifo);
	}

	if(track == NULL)
	{
		return;
	}
	
	sp_error error = sp_session_player_load(session, track);

	if(error != SP_ERROR_OK)
	{
		printf("Error: %s\n", sp_error_message(error));
		return;
	}

	sp_session_player_play(session, 1);
}
