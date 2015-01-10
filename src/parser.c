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

#include "commandq.h"

struct commandq_entry *parse_input_line(struct commandq_entry *entry, const char * const line, int sockfd)
{
	struct commandq_entry *ret_val = NULL;
	struct command *command = malloc(sizeof(struct command));
	command->handled = 0;
	command->done = 0;
	command->sockfd = sockfd;
	entry->val = command;
	if(!strncasecmp(line, "search ", strlen("search ")))
	{
		command->type = SEARCH;
		command->search_string = malloc(sizeof(char) * strlen(line + strlen("search ")) + 1);
		strcpy(command->search_string, line + strlen("search "));

		ret_val = entry;
	}
	else if(!strncasecmp(line, "qlist", strlen("qlist")))
	{
		command->type = QLIST;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "slist", strlen("slist")))
	{
		command->type = SLIST;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qrand", strlen("qrand")))
	{
		command->type = QRAND;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "pause", strlen("pause")))
	{
		command->type = PAUSE;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "help", strlen("help")))
	{
		command->type = HELP;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qclear", strlen("qclear")))
	{
		command->type = QCLEAR;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qadd ", strlen("qadd ")))
	{
		command->type = QADD;
		char *tmp;
		command->track = strtol(line + strlen("qadd "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "qrm ", strlen("qrm ")))
	{
		command->type = QRM;
		char *tmp;
		command->track = strtol(line + strlen("qrm "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "play ", strlen("play ")))
	{
		command->type = PLAY;
		char *tmp;
		command->track = strtol(line + strlen("play "), &tmp, 10);
		if(*tmp == '\0')
		{
			ret_val = entry;
		}
	}

	if(ret_val == NULL)
	{
		free(command);
	}
	return ret_val;
}
