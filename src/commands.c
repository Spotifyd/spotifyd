#include <libspotify/api.h>
#include <stdio.h>
#include <string.h>

#include "socket.h"
#include "queue.h"
#include "helpers.h"

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
	while(search_result[i] != NULL && i < NUM_SEARCH_RESULTS)
	{
		sock_send_track_with_trackn(command->sockfd, search_result[i], i);
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
		track_added = queue_add_track(search_result[command->track]);
		pthread_mutex_unlock(&search_result_lock);
	}
	if(track_added)
	{
		sock_send_str(command->sockfd, "Adding: ");
		sock_send_track(command->sockfd, search_result[command->track]);
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
