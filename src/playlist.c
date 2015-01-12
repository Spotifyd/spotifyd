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
#include <libspotify/api.h>

#include "playlist.h"
#include "helpers.h"

static sp_playlistcontainer_callbacks pc_callbacks = {
	.container_loaded = &playlist_container_loaded,
};

sp_playlistcontainer *playlist_container = NULL;

void playlist_init(sp_session *session)
{
	sp_playlistcontainer_add_callbacks(
			sp_session_playlistcontainer(session),
			&pc_callbacks,
			NULL
		);
}

void playlist_container_loaded(sp_playlistcontainer *pc, void *userdata)
{
	debug("playlist_container_loaded\n");

	playlist_container = pc;
}

unsigned playlist_len()
{
	if(playlist_container == NULL)
	{
		return 0;
	}
	return sp_playlistcontainer_num_playlists(playlist_container);
}

const char *playlist_get_name(unsigned i)
{
	if(playlist_container == NULL)
	{
		return NULL;
	}
	return sp_playlist_name(sp_playlistcontainer_playlist(playlist_container, i));
}

bool playlist_new(const char * const name)
{
	return sp_playlistcontainer_add_new_playlist(playlist_container, name) != NULL;
}

bool playlist_add_track(unsigned playlist, sp_track * const track, sp_session *session)
{
	sp_playlist *pl = sp_playlistcontainer_playlist(playlist_container, playlist);
	return sp_playlist_add_tracks(pl, &track, 1, sp_playlist_num_tracks(pl), session) == SP_ERROR_OK;
}

bool playlist_del_track(unsigned playlist, int track)
{
	sp_playlist *pl = sp_playlistcontainer_playlist(playlist_container, playlist);
	return SP_ERROR_OK == sp_playlist_remove_tracks(pl, &track, 1);
}

bool playlist_remove(unsigned playlist)
{
	return SP_ERROR_OK == sp_playlistcontainer_remove_playlist(playlist_container, playlist);
}

bool playlist_for_each(unsigned playlistn, bool (*func_ptr)(sp_track *))
{
	if(playlist_container == NULL)
	{
		return 0;
	}

	if(playlistn >= sp_playlistcontainer_num_playlists(playlist_container))
	{
		return 0;
	}

	sp_playlist *pl = sp_playlistcontainer_playlist(playlist_container, playlistn);

	int i;
	for(i=0; i<sp_playlist_num_tracks(pl); ++i)
	{
		if(!func_ptr(sp_playlist_track(pl, i)))
		{
			return 0;
		}
	}
	
	return 1;
}
