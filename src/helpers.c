/*
 * spotifyd - A daemon playing music from spotify, in the spirit of MPD.
 * Copyright (C) 2015 Simon Persson
 * 
 * Spotifyd program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Spotifyd program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
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

bool play(sp_session *session, sp_track *track, bool flush)
{
	debug("play\n");

	if(flush)
	{
		audio_fifo_flush(&g_audiofifo);
	}

	if(track == NULL)
	{
		return 0;
	}

	sp_error error = sp_session_player_load(session, track);

	if(error != SP_ERROR_OK)
	{
		return 0;
	}

	sp_session_player_play(session, 1);
	is_playing = 1;

	return 1;
}
