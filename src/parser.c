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
#include <ctype.h>

#include "commandq.h"
#include "helpers.h"

struct commandq_entry *parse_input_line(struct commandq_entry *entry, char *line, int sockfd)
{
	struct commandq_entry *ret_val = NULL;
	struct command *command = malloc(sizeof(struct command));
	if(command == NULL)
	{
		LOG_PRINT("Can't allocate memory. Quitting.\n");
		exit(1);
	}
	command->handled = 0;
	command->done = 0;
	command->sockfd = sockfd;
	entry->val = command;
	if(!strncasecmp(line, "search ", strlen("search ")))
	{
		command->type = SEARCH;
		command->search_string = malloc(sizeof(char) * strlen(line + strlen("search ")) + 1);
		if(command->search_string == NULL)
		{
			LOG_PRINT("Can't allocate memory. Quitting.\n");
			exit(1);
		}
		strcpy(command->search_string, line + strlen("search "));

		ret_val = entry;
	}
	if(!strncasecmp(line, "link ", strlen("link ")))
	{
		command->type = LINK;
		command->search_string = malloc(sizeof(char) * strlen(line + strlen("link ")) + 1);
		if(command->search_string == NULL)
		{
			LOG_PRINT("Can't allocate memory. Quitting.\n");
			exit(1);
		}
		strcpy(command->search_string, line + strlen("link "));

		ret_val = entry;
	}
	if(!strncasecmp(line, "plcreate ", strlen("plcreate ")))
	{
		command->type = PLCREATE;
		command->name = malloc(sizeof(char) * strlen(line + strlen("plcreate ")) + 1);
		if(command->name == NULL)
		{
			LOG_PRINT("Can't allocate memory. Quitting.\n");
			exit(1);
		}
		strcpy(command->search_string, line + strlen("plcreate "));

		ret_val = entry;
	}
	else if(!strncasecmp(line, "qlist", strlen("qlist")))
	{
		command->type = QLIST;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "cur_playing", strlen("cur_playing")))
	{
		command->type = CUR_PLAYING;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "qprint", strlen("qprint")))
	{
		command->type = QPRINT;
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
	else if(!strncasecmp(line, "prev", strlen("prev")))
	{
		command->type = PREV;
		ret_val = entry;
	}
	else if(!strncasecmp(line, "next", strlen("next")))
	{
		command->type = NEXT;
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
	else if(!strncasecmp(line, "saddpl ", strlen("saddpl ")))
	{
		command->type = SADDPL;
		if(sscanf(line + strlen("saddpl "), "%d", &command->playlist) == 1)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "qaddpl ", strlen("qaddpl ")))
	{
		command->type = QADDPL;
		if(sscanf(line + strlen("qaddpl "), "%d", &command->playlist) == 1)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "pladd ", strlen("pladd ")))
	{
		command->type = PLADD;
		if(sscanf(line + strlen("pladd "), "%d %d", &command->track, &command->playlist) == 2)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "plrm ", strlen("plrm ")))
	{
		command->type = PLRM;
		if(sscanf(line + strlen("plrm "), "%d %d", &command->track, &command->playlist) == 2)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "pldelete ", strlen("pldelete ")))
	{
		command->type = PLDELETE;
		if(sscanf(line + strlen("pldelete "), "%d", &command->playlist) == 1)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "qadd ", strlen("qadd ")))
	{
		command->type = QADD;
		if(sscanf(line + strlen("qadd "), "%d", &command->track) == 1)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "qrm ", strlen("qrm ")))
	{
		command->type = QRM;
		if(sscanf(line + strlen("qrm "), "%d", &command->track) == 1)
		{
			ret_val = entry;
		}
	}
	else if(!strncasecmp(line, "play ", strlen("play ")))
	{
		command->type = PLAY;
		if(sscanf(line + strlen("play "), "%d", &command->track) == 1)
		{
			ret_val = entry;
		}
	} /* pl must be after play, otherwise play matches pl first. */
	else if(!strncasecmp(line, "pl", strlen("pl")) && (*(line + strlen("pl")) == '\0' || isspace(*(line + strlen("pl")))))
	{
		command->type = PL;
		ret_val = entry;
	}

	if(ret_val == NULL)
	{
		free(command);
	}
	return ret_val;
}
