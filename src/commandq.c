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
		LOG_PRINT("commandq_pop: first elem is NULL!\n");
	}
	else if(commandq.tqh_first->val->done == 0)
	{
		LOG_PRINT("commandq_pop: first elem isn't handled.\n");
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
	if(e->val->type == SEARCH || e->val->type == LINK)
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
		LOG_PRINT("execute_command: command is null-ptr\n");
		exit(1);
	}
	else if(session == NULL)
	{
		LOG_PRINT("execute_command: session is null-ptr\n");
		exit(1);
	}

	debug("Entered commandq_execute_command with ");
	debug(COMMAND_STR[command->type]);
	debug(" on top of queue.\n");

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
		else if(command->type == LINK)
		{
			command_link(command);
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
		else if(command->type == CUR_PLAYING)
		{
			command_cur_playing(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PAUSE)
		{
			command_pause(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PL)
		{
			command_pl(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QCLEAR)
		{
			sock_send_str(command->sockfd, "Clearing queue.\n");
			command_qclear(session);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QRM)
		{
			command_qrm(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QADD)
		{
			command_qadd(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == SADDPL)
		{
			command_saddpl(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QADDPL)
		{
			command_qaddpl(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PLAY)
		{
			command_play(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PREV)
		{
			command_prev(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == NEXT)
		{
			command_next(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == QPRINT)
		{
			command_qprint(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PLCREATE)
		{
			command_plcreate(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PLDELETE)
		{
			command_pldelete(command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PLADD)
		{
			command_pladd(session, command);
			close(command->sockfd);
			command->done = 1;
		}
		else if(command->type == PLRM)
		{
			command_plrm(command);
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
