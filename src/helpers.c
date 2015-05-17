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
#include "commandq.h"

void num_pre(char *buf, size_t len, int trackn, void (*f)(char *, size_t, void *), void *p)
{
	snprintf(buf, len, "%d | ", trackn);
	f(buf + strlen(buf), len - strlen(buf), p);
}
void track_to_str(char *buf, size_t len, void *v)
{
	sp_track *track = (sp_track *) v;
	if(track != NULL && sp_track_error(track) == SP_ERROR_OK)
	{
		sp_artist *artist = sp_track_artist(track, 0);
		sp_link *l = sp_link_create_from_track(track, 0);
		snprintf(buf, len, "%s | %s | ", sp_track_name(track), sp_artist_name(artist));
		sp_link_as_string(l, buf + strlen(buf), len - strlen(buf));
		sp_link_release(l);
		strncat(buf+strlen(buf), " | TRACK", len - strlen(buf));
	}
	else if(track != NULL && sp_track_error(track) == SP_ERROR_IS_LOADING)
	{
		snprintf(buf, len, "Track is loading, wait a second.");
	}
	else if(track == NULL)
	{
		snprintf(buf, len, "Track is NULL-ptr, this is a bug.");
	}
	else
	{
		snprintf(buf, len, "An unknown error occured. Try again.");
	}
}

void album_to_str(char *buf, size_t len, void *v)
{
	sp_album *album = (sp_album *) v;
	if(sp_album_is_loaded(album))
	{
		sp_link *l = sp_link_create_from_album(album);
		sp_artist *artist = sp_album_artist(album);
		const char *name = sp_album_name(album);
		snprintf(buf, len, "%s | %s | ", name, sp_artist_name(artist));
		sp_link_as_string(l, buf + strlen(buf), len - strlen(buf));
		sp_link_release(l);
		strncat(buf+strlen(buf), " | ALBUM", len - strlen(buf));
	}
	else
	{
		strncat(buf, "Album is not loaded yet...", len);
	}
}

void playlist_to_str(char *buf, size_t len, void *v)
{
	sp_playlist *playlist = (sp_playlist *)v;
	if(sp_playlist_is_loaded(playlist))
	{
		sp_link *l = sp_link_create_from_playlist(playlist);
		const char *name = sp_playlist_name(playlist);
		snprintf(buf, len, "%s | ", name);
		sp_link_as_string(l, buf + strlen(buf), len - strlen(buf));
		sp_link_release(l);
		strncat(buf+strlen(buf), " | PLAYLIST", len - strlen(buf));
	}
	else
	{
		strncat(buf, "Playlist is not loaded yet...", len);
	}
}

void notify_main_thread()
{
	pthread_mutex_lock(&notify_mutex);
	notify_do = 1;
	pthread_cond_signal(&notify_cond);
	pthread_mutex_unlock(&notify_mutex);
}

void debug(const char *debug_msg)
{
	if(DEBUG)
		LOG_PRINT("%s", debug_msg);
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
