#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <libspotify/api.h>
#include <pthread.h>

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
			command->done = 1;
		}
		else if(command->type == QRAND)
		{
			command_qrand(session, command);
			command->done = 1;
		}
		else if(command->type == SLIST)
		{
			command_lists(session, command);	
			command->done = 1;
		}
		else if(command->type == HELP)
		{
			sock_send_str(command->sockfd, help_str);
			command->done = 1;
		}
		else if(command->type == QADD)
		{
			if(command->track < NUM_SEARCH_RESULTS)
			{
				pthread_mutex_lock(&search_result_lock);
				queue_add_track(search_result[command->track]);
				pthread_mutex_unlock(&search_result_lock);
			}
			command->done = 1;
		}
		else if(command->type == QPLAY)
		{
			printf("QPLAY.");
			if(command->track < queue_len)
			{
				queue_position = command->track;
				play(session, queue[command->track], 1);
			}
			command->done = 1;
		}
		command->handled = 1;
	}

	if(command->done == 1)
	{
		commandq_pop();
	}
}
