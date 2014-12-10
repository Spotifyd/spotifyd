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
		sock_send_track(command->sockfd, search_result[i], i);
		++i;
	}
	pthread_mutex_unlock(&search_result_lock);
}

/*
 * Send a list of the queue to the client.
 */
void command_listq(sp_session *session, const struct command * const command)
{
	int i = 0;
	pthread_mutex_lock(&queue_lock);
	while(queue[i] != NULL && i < NUM_SEARCH_RESULTS)
	{
		sock_send_track(command->sockfd, queue[i], i);
		++i;
	}
	pthread_mutex_unlock(&queue_lock);
}
