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
#include <libspotify/api.h>
#include <pthread.h>
#include <stdlib.h>

#include "search.h"
#include "config.h"
#include "helpers.h"

sp_search *search;
/*
 * clear search list. Caller must take search_result_lock before calling.
 */
void search_clear()
{
	if(search != NULL)
	{
		sp_search_release(search);
		search = NULL;
	}
}

void search_init()
{
	search = NULL;
}

void search_new_search(sp_search *s)
{
	search_clear();
	sp_search_add_ref(s);
	search = s;
}

char *search_str_list()
{
	size_t tracks = sp_search_num_tracks(search);
	size_t albums = sp_search_num_albums(search);
	size_t playlists = sp_search_num_playlists(search);
	size_t size = API_MESSAGE_LEN*(albums+tracks);
	char *ret = malloc(size);
	ret[0] = '\0';
	int i;
	for(i=0; i<tracks; ++i)
	{
		snprintf(ret + strlen(ret), size - strlen(ret), "%d | ", i);
		track_to_str(ret + strlen(ret), size - strlen(ret), sp_search_track(search, i));
		strncat(ret + strlen(ret), "\n", size - strlen(ret));
	}
	for(; i<(tracks+albums); ++i)
	{
		snprintf(ret + strlen(ret), size - strlen(ret), "%d | ", i);
		album_to_str(ret + strlen(ret), size - strlen(ret), sp_search_album(search, i-tracks));
		strncat(ret + strlen(ret), "\n", size - strlen(ret));
	}
	for(; i<(tracks+albums+playlists); ++i)
	{
		snprintf(ret + strlen(ret), size - strlen(ret), "%d | ", i);
		playlist_to_str(ret + strlen(ret), size - strlen(ret), sp_search_playlist(search, i-tracks-albums));
		strncat(ret + strlen(ret), "\n", size - strlen(ret));
	}

	return ret;
}

bool search_is_track(size_t i)
{
	if(i<sp_search_num_tracks(search))
	{
		return 1;
	}
	return 0;
}

bool search_is_album(size_t i)
{
	if(sp_search_num_tracks(search)<= i && i < (sp_search_num_tracks(search)+sp_search_num_albums(search)))
	{
		return 1;
	}
	return 0;
}

bool search_is_playlist(size_t i)
{
	if(sp_search_num_tracks(search)+sp_search_num_albums(search) <= i
			&& i < (sp_search_num_tracks(search)+sp_search_num_albums(search)+sp_search_num_playlists(search)))
	{
		return 1;
	}
	return 0;
}

void search_for_tracks_at(sp_session *session, char *buf, size_t len, size_t i, bool (*f)(sp_track *))
{
	if(search == NULL)
	{
		return;
	}
	buf[0] = '\0';
	if(search_is_track(i) && sp_track_is_loaded(sp_search_track(search, i)))
	{
		f(sp_search_track(search, i));
		if(buf != NULL)
		{
			track_to_str(buf, len, sp_search_track(search, i));
		}
	}
	if(search_is_album(i) && sp_album_is_loaded(sp_search_album(search, i-sp_search_num_tracks(search))))
	{
		sp_albumbrowse_create(session, sp_search_album(search, i-sp_search_num_tracks(search))
				,on_albumbrowse_complete, f);
		if(buf != NULL)
		{
			album_to_str(buf, len, sp_search_album(search, i-sp_search_num_tracks(search)));
		}
	}
	if(search_is_playlist(i))
	{
		sp_playlist *pl = sp_search_playlist(search, 
					i-sp_search_num_tracks(search)-sp_search_num_albums(search));
		if(sp_playlist_is_loaded(pl))
		{
			int j;
			for(j = 0; j < sp_playlist_num_tracks(pl); ++j)
			{
				f(sp_playlist_track(pl, j));
			}
			if(buf != NULL)
			{
				playlist_to_str(buf, len, pl);
			}
		}
	}
}
