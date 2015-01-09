#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <libspotify/api.h>
#include <pthread.h>
#include <unistd.h>

#include "queue.h"
#include "session.h"
#include "spotifyd.h"
#include "socket.h"
#include "helpers.h"
#include "commands.h"

void commandq_pop()
{
	if(commandq.tqh_first == NULL)
	{
		printf("commandq_pop: first elem is NULL!\n");
	}
	else if(commandq.tqh_first->val->done == 0)
	{
		printf("commandq_pop: first elem isn't handled.\n");
	}
	else
	{
		struct commandq_entry *tmp = commandq.tqh_first;
		TAILQ_REMOVE(&commandq, commandq.tqh_first, entries);
		commandq_free_entry(tmp);
	}
}

int commandq_init()
{
	if(pthread_mutex_init(&commandq_lock, NULL) != 0)
	{
		return 1;
	}
	TAILQ_INIT(&commandq);
	return 0;
}

void commandq_execute_front(sp_session *session)
{
	if(commandq.tqh_first != NULL)
	{
		commandq_execute_command(session, commandq.tqh_first->val);
	}
}

void commandq_insert(struct commandq_entry *entry)
{
	TAILQ_INSERT_TAIL(&commandq, entry, entries);
}

void commandq_free_entry(struct commandq_entry *e)
{
	if(e->val->type == SEARCH)
	{
		free(e->val->search_string);
	}
	free(e->val);
	free(e);
}

void commandq_execute_command(sp_session *session, struct command *command)
{
	if(command == NULL)
	{
		printf("execute_command: command is null-ptr\n");
		exit(1);
	}
	else if(session == NULL)
	{
		printf("execute_command: session is null-ptr\n");
		exit(1);
	}

	/*
	 * Unless the command is already handled, handle it here.
	 * Commands get the 'done' property set to true once they
	 * are done, before this a response is sent to the client
	 * and the socket is closed.
	 */
	if(command->handled == 0)
	{
		if(command->type == SEARCH)
		{
			/*
			 * this command isn't finished until
			 * it reaches the on_search_finished callback.
			 */
			command->done = 0;
			command_search(session, command);
		}
		else if(command->type == QLIST)
		{
			command_listq(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QRAND)
		{
			command_qrand(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == SLIST)
		{
			command_lists(session, command);	
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == HELP)
		{
			sock_send_str(command->sockfd, help_str);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QADD)
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
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QPLAY)
		{
			if(command->track < queue_get_len())
			{
				queue_set_current(command->track);
				play(session, queue_get(command->track), 1);
			}
			sock_send_str(command->sockfd, "Playing: ");
			sock_send_track(command->sockfd, queue_get(command->track));
			close(command->sockfd);
			command->done = 1;
		}
		command->handled = 1;
	}

	if(command->done == 1)
	{
		commandq_pop();
	}
}
