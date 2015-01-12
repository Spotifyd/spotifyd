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
#include <stdio.h>
#include <string.h>

#include "socket.h"
#include "queue.h"
#include "helpers.h"
#include "playlist.h"
#include "search.h"

void command_search(sp_session *session, const struct command * const command)
{
	if(command->search_string == NULL)
	{
		printf("execute_command: search_string is null-ptr.\n");
	}
	else
	{
		sp_search_create(session, 
			command->search_string,
			0,
			100,
			0,
			0,
			0,
			0,
			0,
			0,
			SP_SEARCH_STANDARD,
			&on_search_complete,
			session
		);
	}
}

void command_qrand(sp_session *session, const struct command * const command)
{
	sock_send_str(command->sockfd, "Turned queue ranomness ");
	bool queue_is_random = queue_toggle_random();
	if(queue_is_random)
	{
		sock_send_str(command->sockfd, "on.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "off.\n");
	}
}

/*
 * Send a list of the search result to the client.
 */
void command_lists(sp_session *session, const struct command * const command)
{
	int i = 0;
	pthread_mutex_lock(&search_result_lock);
	while(search_get(i) != NULL)
	{
		sock_send_track_with_trackn(command->sockfd, search_get(i), i);
		sock_send_str(command->sockfd, "\n");
		++i;
	}
	pthread_mutex_unlock(&search_result_lock);
}

/*
 * Send a list of the queue to the client.
 */
void command_listq(sp_session *session, const struct command * const command)
{
	unsigned i = 0;
	pthread_mutex_lock(&queue_lock);
	while(queue_get(i) != NULL && i < NUM_SEARCH_RESULTS)
	{
		sock_send_track_with_trackn(command->sockfd, queue_get(i), i);
		sock_send_str(command->sockfd, "\n");
		++i;
	}
	pthread_mutex_unlock(&queue_lock);
}

void command_qadd(sp_session *session, const struct command * const command)
{
	bool track_added = 0;
	if(command->track < NUM_SEARCH_RESULTS)
	{
		pthread_mutex_lock(&search_result_lock);
		track_added = queue_add_track(search_get(command->track));
		pthread_mutex_unlock(&search_result_lock);
	}
	if(track_added)
	{
		sock_send_str(command->sockfd, "Adding: ");
		sock_send_track(command->sockfd, search_get(command->track));
		sock_send_str(command->sockfd, "\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Not a valid track number!\n");
	}
}

void command_play(sp_session *session, const struct command * const command)
{
	if(command->track < queue_get_len())
	{
		queue_set_current(command->track);
		play(session, queue_get(command->track), 1);
	}
	sock_send_str(command->sockfd, "Playing: ");
	sock_send_track(command->sockfd, queue_get(command->track));
	sock_send_str(command->sockfd, "\n");
}

void command_pause(sp_session *session, const struct command * const command)
{
	sp_session_player_play(session, is_playing =! is_playing);
	if(is_playing)
	{
		sock_send_str(command->sockfd, "Started playback.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Paused playback.\n");
	}
}

void command_pl(const struct command * const command)
{
	unsigned i = 0;
	for(i = 0; i<playlist_len(); ++i)
	{
		const char *playlist_name = playlist_get_name(i);
		if(playlist_name == NULL)
		{
			break;
		}
		char name_str[API_MESSAGE_LEN];
		snprintf(name_str, API_MESSAGE_LEN, "%d | %s\n", i, playlist_name);
		sock_send_str(command->sockfd, name_str);
	}
}

void command_pladd(sp_session *session, const struct command * const command)
{
	if(playlist_add_track(command->playlist, queue_get(command->track), session))
	{
		sock_send_str(command->sockfd, "Added track ");
		sock_send_track(command->sockfd, queue_get(command->track));
		sock_send_str(command->sockfd, " to playlist ");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, ".\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Couldn't add track.\n");
	}

}

void command_plrm(const struct command * const command)
{
	if(playlist_del_track(command->playlist, command->track))
	{
		sock_send_str(command->sockfd, "Removed track ");
		sock_send_track(command->sockfd, queue_get(command->track));
		sock_send_str(command->sockfd, " from playlist ");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, ".\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Couldn't remove track.\n");
	}

}

void command_plcreate(const struct command * const command)
{
	if(playlist_new(command->name))
	{
		sock_send_str(command->sockfd, "Created new playlist.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Couldn't create new playlist.\n");
	}
}

void command_pldelete(const struct command * const command)
{
	if(playlist_remove(command->playlist))
	{
		sock_send_str(command->sockfd, "Removed playlist.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Couldn't remove playlist.\n");
	}
}

void command_qaddpl(const struct command * const command)
{	
	while(queue_get_len() != 0) queue_del_track(0);
	if(playlist_for_each(command->playlist, &queue_add_track))
	{
		sock_send_str(command->sockfd, "Added playlist \"");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, "\" to queue.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Tried to add playlist \"");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, "\" to queue but something went wrong.\n");
	}
}

void command_saddpl(const struct command * const command)
{
	pthread_mutex_lock(&search_result_lock);
	search_clear();
	if(playlist_for_each(command->playlist, &search_add_track))
	{
		sock_send_str(command->sockfd, "Added playlist \"");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, "\" to search list.\n");
	}
	else
	{
		sock_send_str(command->sockfd, "Tried to add playlist \"");
		sock_send_str(command->sockfd, playlist_get_name(command->playlist));
		sock_send_str(command->sockfd, "\" to search list but something went wrong.\n");
	}
	pthread_mutex_unlock(&search_result_lock);
}
